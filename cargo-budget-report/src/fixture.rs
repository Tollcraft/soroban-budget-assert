use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub const FIXTURE_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FixtureFile {
    pub fixture_version: u32,
    pub entries: HashMap<String, serde_json::Value>,
}

impl FixtureFile {
    #[allow(dead_code)]
    pub fn new() -> Self {
        FixtureFile {
            fixture_version: FIXTURE_VERSION,
            entries: HashMap::new(),
        }
    }

    /// Loaded by `ReplayTransport`; only reachable from tests until a
    /// record/replay CLI flag lands.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read fixture file: {}", path.as_ref().display()))?;
        let fixture: FixtureFile = serde_json::from_str(&content).with_context(|| {
            format!("Failed to parse fixture file: {}", path.as_ref().display())
        })?;
        if fixture.fixture_version != FIXTURE_VERSION {
            anyhow::bail!(
                "Fixture version mismatch: expected {} got {}",
                FIXTURE_VERSION,
                fixture.fixture_version
            );
        }
        Ok(fixture)
    }

    /// Called by `RecordingTransport::into_fixture` consumers; only
    /// reachable from tests until a record/replay CLI flag lands.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize fixture file")?;
        std::fs::write(path.as_ref(), content).with_context(|| {
            format!("Failed to write fixture file: {}", path.as_ref().display())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Write `body` to `dir/fixture.json` and load it back, so the read,
    /// deserialize and version-check paths are all exercised from raw text.
    fn load_body(dir: &tempfile::TempDir, body: &str) -> Result<FixtureFile> {
        let path = dir.path().join("fixture.json");
        std::fs::write(&path, body).expect("failed to write test fixture");
        FixtureFile::load(&path)
    }

    /// One entry per JSON shape a recorded response can take, keyed the way
    /// the transport helpers build replay keys.
    fn sample_fixture() -> FixtureFile {
        FixtureFile {
            fixture_version: FIXTURE_VERSION,
            entries: HashMap::from([
                (
                    "invoke:abc".to_string(),
                    json!({ "status": "SUCCESS", "fuel_consumed": 42 }),
                ),
                ("simulate:def".to_string(), json!([1, 2, 3])),
                ("deploy:ghi".to_string(), json!("some-contract-id")),
                ("invoke:jkl".to_string(), json!(null)),
            ]),
        }
    }

    #[test]
    fn new_file_starts_empty_at_the_current_version() {
        let fixture = FixtureFile::new();
        assert_eq!(fixture.fixture_version, FIXTURE_VERSION);
        assert!(fixture.entries.is_empty());
    }

    #[test]
    fn on_disk_version_is_pinned() {
        // Bumping `FIXTURE_VERSION` invalidates every recorded fixture, so it
        // is a deliberate decision that should update this assertion too.
        assert_eq!(FIXTURE_VERSION, 1);
    }

    #[test]
    fn save_then_load_round_trips_every_entry_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.json");
        let original = sample_fixture();

        original.save(&path).unwrap();
        let loaded = FixtureFile::load(&path).unwrap();

        assert_eq!(loaded.fixture_version, original.fixture_version);
        assert_eq!(loaded.entries, original.entries);
        assert!(loaded.entries["invoke:jkl"].is_null());
    }

    #[test]
    fn save_writes_pretty_json_under_the_documented_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.json");
        sample_fixture().save(&path).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains('\n'), "expected pretty-printed JSON");
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["fixture_version"], FIXTURE_VERSION);
        assert_eq!(value["entries"]["simulate:def"], json!([1, 2, 3]));
        assert_eq!(
            value.as_object().unwrap().len(),
            2,
            "only the two documented fields are written"
        );
    }

    #[test]
    fn save_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.json");
        std::fs::write(&path, r#"{"fixture_version": 1, "entries": {}}"#).unwrap();

        let replacement = FixtureFile {
            fixture_version: FIXTURE_VERSION,
            entries: HashMap::from([("invoke:abc".to_string(), json!({ "status": "SUCCESS" }))]),
        };
        replacement.save(&path).unwrap();

        let loaded = FixtureFile::load(&path).unwrap();
        assert_eq!(loaded.entries, replacement.entries);
        assert_eq!(loaded.entries.len(), 1, "the previous contents are gone");
    }

    #[test]
    fn load_then_save_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.json");
        let second = dir.path().join("second.json");

        sample_fixture().save(&first).unwrap();
        FixtureFile::load(&first).unwrap().save(&second).unwrap();

        assert_eq!(
            FixtureFile::load(&first).unwrap().entries,
            FixtureFile::load(&second).unwrap().entries
        );
    }

    #[test]
    fn load_accepts_an_empty_entries_map() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = load_body(&dir, r#"{"fixture_version": 1, "entries": {}}"#).unwrap();
        assert!(fixture.entries.is_empty());
    }

    #[test]
    fn load_accepts_entries_of_any_json_shape() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = load_body(
            &dir,
            r#"{
                "fixture_version": 1,
                "entries": {
                    "invoke:abc": { "status": "SUCCESS" },
                    "deploy:def": [1, 2, 3],
                    "simulate:ghi": "0a0b0c",
                    "invoke:jkl": null
                }
            }"#,
        )
        .unwrap();

        assert_eq!(fixture.fixture_version, FIXTURE_VERSION);
        assert_eq!(fixture.entries.len(), 4);
        assert_eq!(fixture.entries["invoke:abc"]["status"], "SUCCESS");
        assert_eq!(fixture.entries["deploy:def"], json!([1, 2, 3]));
        assert_eq!(fixture.entries["simulate:ghi"], json!("0a0b0c"));
        assert!(fixture.entries["invoke:jkl"].is_null());
    }

    #[test]
    fn load_ignores_unknown_top_level_fields() {
        // Forward compatibility: a fixture written by a newer tool that adds
        // metadata must still replay rather than fail to parse.
        let dir = tempfile::tempdir().unwrap();
        let fixture = load_body(
            &dir,
            r#"{"fixture_version": 1, "entries": {}, "recorded_at": "2026-01-01"}"#,
        )
        .unwrap();
        assert!(fixture.entries.is_empty());
    }

    #[test]
    fn round_trip_preserves_unicode_keys_and_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.json");
        let original = FixtureFile {
            fixture_version: FIXTURE_VERSION,
            entries: HashMap::from([(
                "invoke:ünïcodé".to_string(),
                json!({ "memo": "héllo — wörld" }),
            )]),
        };

        original.save(&path).unwrap();
        let loaded = FixtureFile::load(&path).unwrap();
        assert_eq!(loaded.entries, original.entries);
    }

    // ── error paths ───────────────────────────────────────────────────

    #[test]
    fn load_reports_a_missing_file_with_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.json");
        let err = FixtureFile::load(&path).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Failed to read fixture file"),
            "got: {err:#}"
        );
        assert!(message.contains("absent.json"), "got: {message}");
    }

    #[test]
    fn load_rejects_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        // Reading a directory as a string is an I/O failure, not a parse one.
        let err = FixtureFile::load(dir.path()).unwrap_err();
        assert!(
            err.to_string().contains("Failed to read fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, "not json at all").unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, "").unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_a_json_array_root() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, "[]").unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_a_non_numeric_version() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, r#"{"fixture_version": "1", "entries": {}}"#).unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_entries_that_are_not_a_map() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, r#"{"fixture_version": 1, "entries": [1, 2]}"#).unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_a_missing_entries_field() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, r#"{"fixture_version": 1}"#).unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse fixture file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn load_rejects_a_future_fixture_version() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, r#"{"fixture_version": 2, "entries": {}}"#).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Fixture version mismatch"), "got: {err:#}");
        assert!(
            message.contains(&format!("expected {FIXTURE_VERSION} got 2")),
            "got: {message}"
        );
    }

    #[test]
    fn load_rejects_a_stale_fixture_version() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_body(&dir, r#"{"fixture_version": 0, "entries": {}}"#).unwrap_err();
        assert!(
            err.to_string().contains("Fixture version mismatch"),
            "got: {err:#}"
        );
    }

    #[test]
    fn save_reports_a_path_that_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        // The parent directory does not exist, so the write must fail with the
        // path named rather than panicking.
        let path = dir.path().join("missing-dir").join("fixture.json");
        let err = FixtureFile::new().save(&path).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Failed to write fixture file"),
            "got: {err:#}"
        );
        assert!(message.contains("fixture.json"), "got: {message}");
    }
}
