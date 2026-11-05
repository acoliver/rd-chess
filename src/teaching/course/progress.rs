use super::{Result, require, validation::identifier};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LessonProgress {
    /// Submitted moves, including wrong answers; opening a task is not an attempt.
    pub attempts: u64,
    /// Count revealed hints once each per attempt, not repeated requests at the cap.
    pub hints_used: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BandProgress {
    pub attempts: u64,
    pub solved: u64,
}

/// Save payload only. File paths and transactional I/O belong to the engine.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    version: u32,
    pub completed_lessons: BTreeSet<String>,
    pub lessons: BTreeMap<String, LessonProgress>,
    pub bands: BTreeMap<String, BandProgress>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            version: 1,
            completed_lessons: BTreeSet::new(),
            lessons: BTreeMap::new(),
            bands: BTreeMap::new(),
        }
    }
}

impl Progress {
    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(data: &str) -> Result<Self> {
        require(data.len() <= 262_144, "progress exceeds 256 KiB")?;
        let progress: Self = serde_json::from_str(data)?;
        progress.validate()?;
        Ok(progress)
    }

    fn validate(&self) -> Result<()> {
        require(self.version == 1, "unsupported progress version")?;
        require(
            self.lessons.len() <= 1024 && self.completed_lessons.len() <= 1024,
            "progress lesson count",
        )?;
        for id in self.lessons.keys().chain(&self.completed_lessons) {
            identifier(id)?;
        }
        for (band, progress) in &self.bands {
            require(
                ["under-1200", "1200-1799", "1800-plus"].contains(&band.as_str()),
                "unknown rating band",
            )?;
            require(
                progress.solved <= progress.attempts,
                "solved exceeds attempts",
            )?;
        }
        Ok(())
    }
}
