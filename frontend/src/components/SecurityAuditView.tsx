import React from 'react';
import { SecurityAuditReport } from '../types';
import { ShieldCheck, Lock, CheckCircle2 } from 'lucide-react';

interface Props {
  audit: SecurityAuditReport | null;
}

export const SecurityAuditView: React.FC<Props> = ({ audit }) => {
  return (
    <div className="space-y-6">
      <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl">
        <div className="flex items-center gap-2 text-gold-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
          <Lock className="w-4 h-4" /> Defensive Security & Injection Hardening
        </div>
        <h2 className="text-2xl font-bold text-white tracking-tight font-mono">
          System Security Posture & Threat Assessment
        </h2>
        <p className="text-emerald-400/80 text-sm mt-1">
          Zero-trust input sanitization, SSRF protection, strict SQL parameterization, and isolated indirect prompt injection barriers.
        </p>

        {audit && (
          <div className="mt-6 flex items-center gap-3 bg-emerald-950/70 border border-emerald-500/40 px-4 py-3 rounded-xl shadow-inner">
            <ShieldCheck className="w-6 h-6 text-emerald-400" />
            <div>
              <div className="text-sm font-bold text-emerald-300 font-mono">SECURITY AUDIT VERDICT: {audit.status}</div>
              <div className="text-xs text-emerald-400/80 font-mono">
                {audit.total_audited_vectors} Attack vectors verified across Rust modules. Last verified: {audit.timestamp}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Defensive Pillars Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* SSRF Outbound Firewall */}
        <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-base font-bold text-white flex items-center gap-2 font-mono">
              <CheckCircle2 className="w-5 h-5 text-emerald-400" />
              SSRF Outbound Firewall
            </h3>
            <span className={`text-xs font-mono px-2 py-0.5 rounded font-bold border ${
              audit?.ssrf_firewall_enabled
                ? 'bg-emerald-950 text-emerald-300 border-emerald-500/40'
                : 'bg-rose-950 text-rose-300 border-rose-500/40'
            }`}>
              {audit?.ssrf_firewall_enabled ? 'ENFORCED' : 'OFFLINE'}
            </span>
          </div>
          <p className="text-xs text-emerald-400/80">
            Guarantees that autonomous web search and external scrapers cannot pivot into private networks, cloud metadata, or internal services.
          </p>
          <div className="bg-[#050a07] p-3 rounded-xl border border-emerald-900/60 space-y-1 font-mono text-[11px] text-emerald-300 shadow-inner">
            <div>• <span className="text-rose-400 font-bold">BLOCKED</span>: 127.0.0.0/8 & localhost</div>
            <div>• <span className="text-rose-400 font-bold">BLOCKED</span>: 169.254.169.254 (Cloud metadata)</div>
            <div>• <span className="text-rose-400 font-bold">BLOCKED</span>: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16</div>
            <div>• <span className="text-gold-400 font-bold">ENFORCED</span>: Scheme restricted to HTTPS/HTTP</div>
          </div>
        </div>

        {/* Prompt Injection Shield */}
        <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-base font-bold text-white flex items-center gap-2 font-mono">
              <CheckCircle2 className="w-5 h-5 text-emerald-400" />
              Prompt Injection Defense
            </h3>
            <span className={`text-xs font-mono px-2 py-0.5 rounded font-bold border ${
              audit?.prompt_injection_shield_enabled
                ? 'bg-emerald-950 text-emerald-300 border-emerald-500/40'
                : 'bg-rose-950 text-rose-300 border-rose-500/40'
            }`}>
              {audit?.prompt_injection_shield_enabled ? 'ENFORCED' : 'OFFLINE'}
            </span>
          </div>
          <p className="text-xs text-emerald-400/80">
            Quarantines retrieved external web pages inside immutable XML containment boundaries with explicit non-executable data instructions.
          </p>
          <div className="bg-[#050a07] p-3 rounded-xl border border-emerald-900/60 space-y-1 font-mono text-[11px] text-emerald-300 shadow-inner">
            <div>• <span className="text-gold-400 font-bold">DEFANGED</span>: "Ignore previous instructions", "SYSTEM OVERRIDE"</div>
            <div>• <span className="text-gold-400 font-bold">STRIPPED</span>: Special delimiters &lt;|im_start|&gt;, &lt;|system|&gt;</div>
            <div>• <span className="text-gold-400 font-bold">ISOLATED</span>: Strict &lt;untrusted_external_evidence&gt; tags</div>
            <div>• <span className="text-emerald-400 font-bold">HASHED</span>: SHA-256 cryptographic provenance tracking</div>
          </div>
        </div>

        {/* SQL Parameterization */}
        <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-base font-bold text-white flex items-center gap-2 font-mono">
              <CheckCircle2 className="w-5 h-5 text-emerald-400" />
              SQL Parameterization & Memory Security
            </h3>
            <span className={`text-xs font-mono px-2 py-0.5 rounded font-bold border ${
              audit?.sql_parameterization_enforced
                ? 'bg-emerald-950 text-emerald-300 border-emerald-500/40'
                : 'bg-rose-950 text-rose-300 border-rose-500/40'
            }`}>
              {audit?.sql_parameterization_enforced ? 'ENFORCED' : 'OFFLINE'}
            </span>
          </div>
          <p className="text-xs text-emerald-400/80">
            Memory tables in SQLite use 100% prepared statements with parameterized parameter slots. Zero raw SQL string concatenation.
          </p>
          <div className="bg-[#050a07] p-3 rounded-xl border border-emerald-900/60 space-y-1 font-mono text-[11px] text-emerald-300 shadow-inner">
            <div>• <span className="text-emerald-400 font-bold">DEFENSE</span>: rusqlite params![...] prepared queries</div>
            <div>• <span className="text-emerald-400 font-bold">TESTED</span>: Malicious payload '; DROP TABLE ... neutralized</div>
            <div>• <span className="text-gold-400 font-bold">ISOLATION</span>: Thread-safe Arc&lt;Mutex&lt;Connection&gt;&gt;</div>
          </div>
        </div>

        {/* Path Traversal & Rate Limiter */}
        <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-base font-bold text-white flex items-center gap-2 font-mono">
              <CheckCircle2 className="w-5 h-5 text-emerald-400" />
              Filesystem Sandbox & Rate Limiting
            </h3>
            <span className={`text-xs font-mono px-2 py-0.5 rounded font-bold border ${
              audit?.memory_isolation_active
                ? 'bg-emerald-950 text-emerald-300 border-emerald-500/40'
                : 'bg-rose-950 text-rose-300 border-rose-500/40'
            }`}>
              {audit?.memory_isolation_active ? 'ENFORCED' : 'OFFLINE'}
            </span>
          </div>
          <p className="text-xs text-emerald-400/80">
            Enforces root canonicalization to block ../ directory escape attacks, and token-bucket rate limiting against DDoS or recursive loops.
          </p>
          <div className="bg-[#050a07] p-3 rounded-xl border border-emerald-900/60 space-y-1 font-mono text-[11px] text-emerald-300 shadow-inner">
            <div>• <span className="text-emerald-400 font-bold">SANDBOX</span>: Canonical root prefix validation</div>
            <div>• <span className="text-gold-400 font-bold">LIMITER</span>: 60 req/min with 15 burst tokens</div>
            <div>• <span className="text-emerald-400 font-bold">SAFETY</span>: Zero unvalidated system process execution</div>
          </div>
        </div>
      </div>
    </div>
  );
};
