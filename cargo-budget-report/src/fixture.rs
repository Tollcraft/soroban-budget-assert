use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub const FIXTURE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
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

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize fixture file")?;
        std::fs::write(path.as_ref(), content).with_context(|| {
            format!("Failed to write fixture file: {}", path.as_ref().display())
        })?;
        Ok(())
    }
}
