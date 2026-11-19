use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioStart {
    OnPlay,
    OnTriggerEnter,
    Manual,
}

impl AudioStart {
    pub fn label(self) -> &'static str {
        match self {
            Self::OnPlay => "On Play",
            Self::OnTriggerEnter => "On trigger enter",
            Self::Manual => "Manual",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSource {
    pub clip: Uuid,
    pub volume: f32,
    pub looping: bool,
    pub start: AudioStart,
}

impl AudioSource {
    pub fn new(clip: Uuid) -> Self {
        Self {
            clip,
            volume: 0.65,
            looping: false,
            start: AudioStart::OnPlay,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioListener {
    pub volume: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioCue {
    pub object: Uuid,
    pub source: AudioSource,
}
