use std::collections::HashSet;

use rmcp::{ErrorData, schemars};
use serde::{Deserialize, Serialize};

use crate::authoring::invalid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Persona {
    Beginner,
    Explorer,
    GoalDriven,
}

impl Persona {
    pub fn label(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Explorer => "explorer",
            Self::GoalDriven => "goal_driven",
        }
    }
}

fn default_personas() -> Vec<Persona> {
    vec![Persona::Beginner, Persona::Explorer, Persona::GoalDriven]
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Batch {
    /// Saved workspace-relative scene.
    pub path: String,
    /// New workspace-relative directory for the report and replays.
    pub output_path: String,
    /// One to eight distinct simulation seeds.
    pub seeds: Vec<u32>,
    /// Defaults to beginner, explorer, and goal_driven.
    #[serde(default = "default_personas")]
    pub personas: Vec<Persona>,
    /// Target object tag. Defaults to coin.
    pub target_tag: Option<String>,
    /// From 1 to 1200. Defaults to 360. The entire batch is capped at 12000 ticks.
    pub max_ticks: Option<u32>,
}

impl Batch {
    pub fn validate(&self) -> Result<u32, ErrorData> {
        let ticks = self.max_ticks.unwrap_or(360);
        if !(1..=8).contains(&self.seeds.len())
            || self.seeds.iter().collect::<HashSet<_>>().len() != self.seeds.len()
            || !(1..=3).contains(&self.personas.len())
            || self.personas.iter().collect::<HashSet<_>>().len() != self.personas.len()
            || !(1..=1200).contains(&ticks)
            || self.seeds.len() * self.personas.len() * ticks as usize > 12000
            || self.target_tag.as_ref().is_some_and(|tag| {
                tag.is_empty() || tag.len() > 64 || tag.chars().any(char::is_control)
            })
        {
            return Err(invalid(
                "Use distinct seeds and personas, a valid target tag, and at most 12000 total ticks",
            ));
        }
        Ok(ticks)
    }
}
