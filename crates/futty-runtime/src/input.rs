use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::InputAction;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputProfile {
    pub bindings: BTreeMap<String, String>,
    pub sensitivity: Option<f32>,
}

impl InputProfile {
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.sensitivity.is_none()
    }

    pub fn validate(&self) -> Result<(), String> {
        for (action, binding) in &self.bindings {
            if !InputAction::ALL.iter().any(|entry| entry.name() == action) {
                return Err(format!("Unknown input action {action:?}"));
            }
            if !valid_binding(binding) {
                return Err(format!("Invalid binding {binding:?} for {action}"));
            }
        }
        if self
            .sensitivity
            .is_some_and(|value| !(0.1..=4.0).contains(&value))
        {
            return Err("Mouse sensitivity must be between 0.1 and 4.0".into());
        }
        Ok(())
    }
}

fn valid_binding(binding: &str) -> bool {
    matches!(binding.as_bytes(), [b'A'..=b'Z' | b'0'..=b'9'])
        || matches!(
            binding,
            "ArrowUp"
                | "ArrowDown"
                | "ArrowLeft"
                | "ArrowRight"
                | "Space"
                | "Enter"
                | "Tab"
                | "Backspace"
                | "Mouse1"
                | "Mouse2"
                | "Mouse3"
                | "Mouse4"
                | "Mouse5"
        )
}

impl InputAction {
    pub fn default_binding(self) -> &'static str {
        match self {
            Self::MoveForward => "W",
            Self::MoveBackward => "S",
            Self::MoveLeft => "A",
            Self::MoveRight => "D",
            Self::Jump => "Space",
            Self::Fire => "Mouse1",
            Self::Reload => "R",
            Self::Interact => "E",
            Self::Retry => "Enter",
        }
    }
}
