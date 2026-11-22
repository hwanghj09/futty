use rmcp::{ErrorData, model::CallToolResult, schemars};
use serde::Serialize;
use serde_json::{Value, json};

use crate::play::Status;

const SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    SceneValidation,
    ProjectValidation,
    Playtest,
    CameraCapture,
    WindowsBuild,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Code {
    ValidationFailed,
    UnexpectedStatus,
    FailedChecks,
    ExecutionFailed,
}

#[derive(Serialize, schemars::JsonSchema)]
pub(crate) struct Context {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay_path: Option<String>,
}

impl Context {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.into(),
            output_path: None,
            tick: None,
            seed: None,
            replay_path: None,
        }
    }

    pub fn output(path: &str, output_path: &str) -> Self {
        Self {
            output_path: Some(output_path.into()),
            ..Self::new(path)
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Evidence {
    Validation {
        issues: Vec<Value>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        invalid_scenes: Vec<Value>,
    },
    Playtest {
        expected: Status,
        actual: Status,
        failed_checks: Vec<Value>,
    },
}

#[derive(Serialize, schemars::JsonSchema)]
struct FailureReport {
    #[schemars(schema_with = "version_schema")]
    schema_version: u8,
    stage: Stage,
    code: Code,
    message: String,
    context: Context,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<Evidence>,
}

fn version_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type":"integer","const":SCHEMA_VERSION})
}

pub(crate) fn schema() -> CallToolResult {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<FailureReport>();
    CallToolResult::structured(json!(schema))
}

pub(crate) fn failure(
    stage: Stage,
    code: Code,
    message: impl Into<String>,
    context: Context,
    evidence: Option<Evidence>,
) -> Value {
    json!(FailureReport {
        schema_version: SCHEMA_VERSION,
        stage,
        code,
        message: message.into(),
        context,
        evidence,
    })
}

pub(crate) fn validation(
    mut result: Value,
    stage: Stage,
    path: &str,
    issues: Vec<Value>,
    invalid_scenes: Vec<Value>,
) -> CallToolResult {
    if !issues.is_empty() || !invalid_scenes.is_empty() {
        result["failure_report"] = failure(
            stage,
            Code::ValidationFailed,
            "Validation found issues",
            Context::new(path),
            Some(Evidence::Validation {
                issues,
                invalid_scenes,
            }),
        );
    }
    CallToolResult::structured(result)
}

pub(crate) fn execution(mut error: ErrorData, stage: Stage, context: Context) -> ErrorData {
    let mut data = match error.data.take() {
        Some(Value::Object(fields)) => fields,
        Some(value) => [("original_data".into(), value)].into_iter().collect(),
        None => Default::default(),
    };
    data.entry("failure_report").or_insert_with(|| {
        failure(
            stage,
            Code::ExecutionFailed,
            error.message.to_string(),
            context,
            None,
        )
    });
    error.data = Some(Value::Object(data));
    error
}
