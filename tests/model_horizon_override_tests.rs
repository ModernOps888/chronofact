//! CHRONOFACT_MODEL_HORIZON runtime override of the embedded model horizon data.
//!
//! Kept in its own test binary (single test function) because it mutates a process-wide
//! environment variable that `ModelRegistry::new()` reads.

use chronofact::temporal::registry::{
    HorizonDataSource, ModelHorizonData, EMBEDDED_MODEL_HORIZON_JSON, MODEL_HORIZON_ENV_VAR,
};
use chronofact::ModelRegistry;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_model_horizon_env_override_valid_and_invalid_fallback() {
    // Embedded data parses and carries the full seed set.
    let embedded = ModelHorizonData::parse(EMBEDDED_MODEL_HORIZON_JSON).expect("embedded data valid");
    assert_eq!(embedded.models.len(), 26);

    let dir = tempfile::tempdir().unwrap();

    // 1. Valid override file replaces the embedded seed data.
    let valid_path = dir.path().join("horizon_valid.json");
    std::fs::write(
        &valid_path,
        r#"{
          "schema_version": 1,
          "models": [{
            "model_id": "acme-test-model-9",
            "display_name": "Acme Test Model 9",
            "vendor": "Acme",
            "public_release_date": "2026-06-01",
            "estimated_training_freeze": "2025-12-01",
            "official_knowledge_cutoff": "2026-01-01",
            "is_frontier": true,
            "status": "Active",
            "notes": "override fixture",
            "aliases": ["acme-9"]
          }]
        }"#,
    )
    .unwrap();

    let (data, source) = ModelHorizonData::load_with_override(valid_path.to_str());
    assert_eq!(source, HorizonDataSource::Override(valid_path.to_str().unwrap().to_string()));
    assert_eq!(data.models.len(), 1);

    std::env::set_var(MODEL_HORIZON_ENV_VAR, &valid_path);
    let reg = ModelRegistry::new();
    let acme = reg.lookup("acme-9");
    assert_eq!(acme.model_id, "acme-test-model-9");
    assert_eq!(acme.vendor, "Acme");
    assert_eq!(acme.official_knowledge_cutoff.to_string(), "2026-01-01");
    // Embedded models are replaced, not merged: the flagship id no longer resolves to OpenAI data.
    assert_eq!(reg.lookup("gpt-6-astra").vendor, "Unknown/Generic");

    // 2. Invalid override file: no panic, warning logged, embedded data used.
    let invalid_path = dir.path().join("horizon_invalid.json");
    std::fs::write(&invalid_path, "{ this is not valid json").unwrap();

    let buf = SharedBuf::default();
    let writer = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();

    std::env::set_var(MODEL_HORIZON_ENV_VAR, &invalid_path);
    let (reg, source) = tracing::subscriber::with_default(subscriber, || {
        let (_, source) = ModelHorizonData::load_with_override(invalid_path.to_str());
        (ModelRegistry::new(), source)
    });
    assert_eq!(source, HorizonDataSource::Embedded);
    let astra = reg.lookup("gpt-6-astra");
    assert_eq!(astra.vendor, "OpenAI");
    assert_eq!(astra.official_knowledge_cutoff.to_string(), "2026-04-01");

    let logs = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("WARN"), "expected a WARN log line, got: {}", logs);
    assert!(
        logs.contains(MODEL_HORIZON_ENV_VAR) && logs.contains("falling back to embedded"),
        "warning must name the env var and the fallback, got: {}",
        logs
    );

    // 3. Missing file behaves like invalid (fallback, no panic).
    let (_, source) = ModelHorizonData::load_with_override(Some("Z:/definitely/missing/horizon.json"));
    assert_eq!(source, HorizonDataSource::Embedded);

    std::env::remove_var(MODEL_HORIZON_ENV_VAR);
}
