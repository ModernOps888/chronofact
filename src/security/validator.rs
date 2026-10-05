use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("SSRF Blocked: URL resolves to private or loopback network ({0})")]
    SsrfBlocked(String),
    #[error("Disallowed scheme: {0}. Only http and https are allowed.")]
    DisallowedScheme(String),
    #[error("Path traversal detected: {0}")]
    PathTraversal(String),
    #[error("Path outside allowed project root: {0}")]
    PathOutsideRoot(String),
}

pub struct SecurityValidator;

impl SecurityValidator {
    /// Validates an outbound URL to prevent Server-Side Request Forgery (SSRF)
    pub fn validate_outbound_url(raw_url: &str) -> Result<Url, SecurityError> {
        let parsed = Url::parse(raw_url).map_err(|e| SecurityError::InvalidUrl(e.to_string()))?;

        // 1. Enforce HTTPS or HTTP only
        match parsed.scheme() {
            "http" | "https" => {}
            other => return Err(SecurityError::DisallowedScheme(other.to_string())),
        }

        // 2. Validate Host
        let host_str = parsed.host_str().ok_or_else(|| SecurityError::InvalidUrl("Missing host".to_string()))?;

        // Fast-path block for obvious localhost strings
        let lower_host = host_str.to_lowercase();
        if lower_host == "localhost" || lower_host.ends_with(".local") || lower_host.ends_with(".internal") {
            return Err(SecurityError::SsrfBlocked(host_str.to_string()));
        }

        // 3. Resolve DNS and inspect target IP addresses
        let port = parsed.port_or_known_default().unwrap_or(80);
        let socket_addrs = format!("{}:{}", host_str, port)
            .to_socket_addrs()
            .map_err(|e| SecurityError::InvalidUrl(format!("DNS resolution failed: {}", e)))?;

        for addr in socket_addrs {
            let ip = addr.ip();
            if Self::is_private_or_restricted_ip(ip) {
                return Err(SecurityError::SsrfBlocked(format!("{}: {}", host_str, ip)));
            }
        }

        Ok(parsed)
    }

    /// Checks if an IP is loopback, link-local, private, or multicast
    pub fn is_private_or_restricted_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                // 127.0.0.0/8 (Loopback)
                if octets[0] == 127 { return true; }
                // 10.0.0.0/8 (Private)
                if octets[0] == 10 { return true; }
                // 172.16.0.0/12 (Private)
                if octets[0] == 172 && (16..=31).contains(&octets[1]) { return true; }
                // 192.168.0.0/16 (Private)
                if octets[0] == 192 && octets[1] == 168 { return true; }
                // 169.254.0.0/16 (Link Local / Cloud Metadata 169.254.169.254)
                if octets[0] == 169 && octets[1] == 254 { return true; }
                // 0.0.0.0/8
                if octets[0] == 0 { return true; }
                // 224.0.0.0/4 (Multicast)
                if octets[0] >= 224 { return true; }
                false
            }
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified()
            }
        }
    }

    /// Enforces that a filesystem path stays safely inside a root workspace directory
    pub fn validate_safe_path(root: &Path, target: &Path) -> Result<PathBuf, SecurityError> {
        let canonical_root = root.canonicalize().map_err(|e| {
            SecurityError::PathTraversal(format!("Cannot canonicalize root: {}", e))
        })?;

        let candidate = if target.is_relative() {
            canonical_root.join(target)
        } else {
            target.to_path_buf()
        };

        // Canonicalize candidate if it exists, or check clean normalized path
        let canonical_candidate = match candidate.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                // If file doesn't exist yet, inspect normalized components
                let mut normalized = PathBuf::new();
                for comp in candidate.components() {
                    match comp {
                        std::path::Component::ParentDir => {
                            if !normalized.pop() {
                                return Err(SecurityError::PathTraversal("Path escapes root via ..".to_string()));
                            }
                        }
                        std::path::Component::Normal(c) => normalized.push(c),
                        std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                            normalized.push(comp);
                        }
                        _ => {}
                    }
                }
                normalized
            }
        };

        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(SecurityError::PathOutsideRoot(format!(
                "Candidate {:?} is outside root {:?}",
                canonical_candidate, canonical_root
            )));
        }

        Ok(canonical_candidate)
    }
}
