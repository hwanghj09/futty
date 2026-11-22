use std::{cell::RefCell, collections::HashMap, rc::Rc};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self { max: 100.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HealthState {
    pub current: f32,
    pub max: f32,
}

impl HealthState {
    pub fn alive(self) -> bool {
        self.current > 0.0
    }
}

impl From<Health> for HealthState {
    fn from(config: Health) -> Self {
        Self {
            current: config.max,
            max: config.max,
        }
    }
}

pub(crate) type HealthStates = Rc<RefCell<HashMap<Uuid, HealthState>>>;
