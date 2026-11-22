use futty_runtime::project::Project;
use rmcp::{ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    BuildProject, Futty, ProjectFile,
    authoring::invalid,
    capture::Capture,
    io_error,
    replay::PlayTest,
    report::{self, Context, Stage},
};

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    /// Existing workspace-relative project manifest.
    path: String,
    /// Standard play_test input for a scene registered in the project.
    playtest: PlayTest,
    /// Optional scene_capture input for a scene registered in the project.
    capture: Option<Capture>,
    /// Optional Windows packaging destination.
    build: Option<Build>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Build {
    /// New workspace-relative output directory.
    output_path: String,
    /// Workspace-relative Windows x64 Futty player. Defaults to dist/futty-editor.exe.
    template_path: Option<String>,
}

#[derive(Serialize)]
struct Step {
    stage: Stage,
    passed: bool,
}

pub(crate) fn run(server: &Futty, input: Review) -> Result<CallToolResult, ErrorData> {
    let Review {
        path,
        playtest,
        capture,
        build,
    } = input;
    let _lock = server.lock_edits().map_err(|error| {
        report::execution(
            io_error(error),
            Stage::ProjectValidation,
            Context::new(&path),
        )
    })?;
    let mut steps = Vec::new();
    let mut summary = json!({"schema_version":1,"path":path});
    let result = (|| -> Result<(), Value> {
        record(
            &mut steps,
            Stage::ProjectValidation,
            Context::new(&path),
            server.project_validate(Parameters(ProjectFile { path: path.clone() })),
        )?;
        let mut playtest_context = Context::new(&playtest.path);
        playtest_context.seed = Some(playtest.seed);
        let project = server
            .project_path(&path, true)
            .and_then(|file| Project::load(&file).map_err(io_error));
        let played = project
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|project| require_scene(server, project, &playtest.path))
            .and_then(|()| server.play_test(Parameters(playtest)));
        let played = record(&mut steps, Stage::Playtest, playtest_context, played)?;
        summary["playtest"] = json!({
            "status":played["status"],"tick":played["tick"],"replay_path":played["replay_path"],
        });
        if let Some(capture) = capture {
            let context = Context::output(&capture.path, &capture.output_path);
            let image = project
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|project| require_scene(server, project, &capture.path))
                .and_then(|()| server.capture_scene(capture));
            summary["capture"] = record(&mut steps, Stage::CameraCapture, context, image)?;
        }
        if let Some(build) = build {
            let context = Context::output(&path, &build.output_path);
            // The review holds the same edit lock used by the standalone build tool.
            let package = server.package_project(BuildProject {
                path: path.clone(),
                output_path: build.output_path,
                template_path: build.template_path,
            });
            summary["build"] = record(&mut steps, Stage::WindowsBuild, context, package)?;
        }
        Ok(())
    })();
    summary["passed"] = json!(result.is_ok());
    summary["steps"] = json!(steps);
    if let Err(report) = result {
        summary["failure_report"] = report;
    }
    Ok(CallToolResult::structured(summary))
}

fn require_scene(server: &Futty, project: &Project, path: &str) -> Result<(), ErrorData> {
    let scene = server.scene_path(path, true)?;
    if project
        .list()
        .scenes
        .keys()
        .any(|id| project.resolve(id).is_ok_and(|path| path == scene))
    {
        Ok(())
    } else {
        Err(invalid("Choose a scene registered in the reviewed project"))
    }
}

fn record(
    steps: &mut Vec<Step>,
    stage: Stage,
    context: Context,
    result: Result<CallToolResult, ErrorData>,
) -> Result<Value, Value> {
    let payload = result.and_then(|result| {
        result
            .structured_content
            .ok_or_else(|| io_error("Review step returned no structured result"))
    });
    let result = match payload {
        Ok(payload) => match payload.get("failure_report") {
            Some(report) => Err(report.clone()),
            None => Ok(payload),
        },
        Err(error) => {
            let error = report::execution(error, stage, context);
            Err(error.data.unwrap()["failure_report"].clone())
        }
    };
    steps.push(Step {
        stage,
        passed: result.is_ok(),
    });
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use futty_runtime::Scene;

    use super::*;
    use crate::templates::{COIN_RUNNER, CreateGame};

    #[test]
    fn review_stops_on_failed_stages_and_only_packages_after_a_pass() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let created = server
            .game_create_from_template(Parameters(CreateGame {
                template: COIN_RUNNER.into(),
                output_path: "Runner".into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        let project = &created["project_path"];
        let scene = &created["scene_path"];
        let recipe = &created["playtest"];
        let mut exe = [0_u8; 70];
        exe[..2].copy_from_slice(b"MZ");
        exe[60..64].copy_from_slice(&64_u32.to_le_bytes());
        exe[64..].copy_from_slice(b"PE\0\0\x64\x86");
        fs::write(server.root.join("Template.exe"), exe).unwrap();
        let build = json!({"output_path":"Windows","template_path":"Template.exe"});
        let review = |request| {
            run(&server, serde_json::from_value(request).unwrap())
                .unwrap()
                .structured_content
                .unwrap()
        };

        let mut idle = recipe.clone();
        idle["steps"] = json!([{"ticks":10}]);
        let failed = review(json!({"path":project,"playtest":idle,
            "capture":{"path":scene,"output_path":"Preview.png"},"build":build}));
        assert_eq!(failed["passed"], false);
        assert_eq!(
            failed["steps"],
            json!([
                {"stage":"project_validation","passed":true},
                {"stage":"playtest","passed":false}
            ])
        );
        assert_eq!(failed["failure_report"]["code"], "unexpected_status");
        assert!(!server.root.join("Preview.png").exists());
        assert!(!server.root.join("Windows").exists());

        Scene::default()
            .save_new(&server.root.join("Unlisted.ron"))
            .unwrap();
        let mut foreign = recipe.clone();
        foreign["path"] = json!("Unlisted.ron");
        let rejected = review(json!({"path":project,"playtest":foreign}));
        assert_eq!(rejected["failure_report"]["stage"], "playtest");
        assert!(
            rejected["failure_report"]["message"]
                .as_str()
                .unwrap()
                .contains("registered")
        );

        let bad_capture = review(json!({"path":project,"playtest":recipe,
            "capture":{"path":scene,"output_path":"Preview.png","width":0},"build":build}));
        assert_eq!(bad_capture["failure_report"]["stage"], "camera_capture");
        assert_eq!(bad_capture["steps"].as_array().unwrap().len(), 3);
        assert!(!server.root.join("Windows").exists());

        let passed = review(json!({"path":project,"playtest":recipe,"build":build}));
        assert_eq!(passed["passed"], true, "{passed}");
        assert_eq!(passed["build"]["executable"], "Windows/Game.exe");
        assert_eq!(fs::read(server.root.join("Windows/Game.exe")).unwrap(), exe);
    }
}
