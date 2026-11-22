use std::{fs::File, io::Read, path::Path};

use rmcp::{ErrorData, schemars};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    authoring::invalid,
    io_error,
    play::{Controls, Status},
};

pub(crate) const MAX_TICKS: u32 = 36_000;
const FORMAT: u32 = 1;
const MAX_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveReplay {
    #[schemars(with = "String")]
    pub session_id: Uuid,
    /// New workspace-relative .json file. Existing files are preserved.
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayFile {
    /// Existing workspace-relative replay .json file.
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlayTest {
    /// Workspace-relative scene .ron file.
    pub path: String,
    #[serde(default)]
    pub seed: u32,
    /// Tick limit from 1 to 36000. Defaults to 3600.
    pub max_ticks: Option<u32>,
    pub steps: Vec<Controls>,
    /// Expected final status. Defaults to success.
    pub expect: Option<Status>,
    /// Expected final values of named HUD bindings.
    #[serde(default)]
    pub expect_hud: std::collections::BTreeMap<String, serde_json::Value>,
    /// Required collision or trigger starts, in either object order.
    #[serde(default)]
    pub expect_collisions: Vec<ExpectedCollision>,
    /// Expected requested destination scene ID.
    pub expect_scene: Option<String>,
    /// Optional new workspace-relative replay .json file.
    pub replay_path: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpectedCollision {
    #[schemars(with = "String")]
    pub first: Uuid,
    #[schemars(with = "String")]
    pub second: Uuid,
    /// True for trigger entry; false for solid collision. Omit to accept either.
    pub sensor: Option<bool>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Replay {
    format: u32,
    engine_version: String,
    pub scene: String,
    pub source_digest: String,
    pub seed: u32,
    pub max_ticks: u32,
    pub steps: Vec<Controls>,
    /// Index 0 is the initial observation and every following index is its tick.
    pub marks: Vec<Mark>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mark {
    pub digest: String,
    pub player_position: [f32; 3],
    pub status: Status,
}

impl Replay {
    pub fn new(scene: String, source_digest: String, seed: u32, max_ticks: u32) -> Self {
        Self {
            format: FORMAT,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            scene,
            source_digest,
            seed,
            max_ticks,
            steps: Vec::new(),
            marks: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, ErrorData> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(io_error)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid("Replay exceeds 32 MiB"));
        }
        let replay: Self = serde_json::from_slice(&bytes).map_err(invalid)?;
        if replay.format != FORMAT
            || replay.engine_version != env!("CARGO_PKG_VERSION")
            || !(1..=MAX_TICKS).contains(&replay.max_ticks)
            || !(1..=replay.max_ticks as usize + 1).contains(&replay.marks.len())
            || replay.steps.len() >= replay.marks.len()
        {
            return Err(invalid("Incompatible or invalid replay"));
        }
        for step in &replay.steps {
            step.validate()?;
        }
        Ok(replay)
    }

    pub fn save(&self, path: &Path) -> Result<(), ErrorData> {
        let folder = path
            .parent()
            .ok_or_else(|| invalid("Replay path requires a folder"))?;
        let mut temp = tempfile::NamedTempFile::new_in(folder).map_err(io_error)?;
        serde_json::to_writer(temp.as_file_mut(), self).map_err(io_error)?;
        temp.as_file().sync_all().map_err(io_error)?;
        temp.persist_noclobber(path).map_err(io_error)?;
        Ok(())
    }
}

pub(crate) struct Fingerprint(u64);

impl Fingerprint {
    pub fn new() -> Self {
        Self(0xcbf29ce484222325)
    }

    pub fn add(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }

    pub fn finish(self) -> String {
        format!("{:016x}", self.0)
    }
}
