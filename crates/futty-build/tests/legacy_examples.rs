use std::{fs, path::Path};

use futty_core::Camera;
use futty_runtime::{Scene, project::Project};

#[test]
fn legacy_3d_scenes_keep_ids_and_references_after_save() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let temp = tempfile::tempdir().unwrap();
    for relative in [
        "agent-escape/Beacon Bot Escape.futty.scene.ron",
        "animation-playground/Beacon Bot.futty.scene.ron",
        "prefab-playground/Prefab Workshop.futty.scene.ron",
        "pulse-range/Pulse Range.futty.scene.ron",
        "pulse-range/Range Lobby.futty.scene.ron",
    ] {
        let original = Scene::load(&examples.join(relative)).unwrap_or_else(|error| {
            panic!("{relative}: {error}");
        });
        let saved = temp.path().join("Roundtrip.futty.scene.ron");
        original.save(&saved).unwrap();
        assert_eq!(Scene::load(&saved).unwrap(), original, "{relative}");
    }
}

#[test]
fn legacy_pulse_range_package_has_both_scenes_and_unchanged_asset_bytes() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = repo.join("examples/pulse-range/Pulse Range.futty.project.ron");
    let source = Project::load(&manifest).unwrap();
    let exe = repo.join("dist/futty-editor.exe");
    futty_build::validate_windows_template(&exe).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("Portable");
    futty_build::export(&manifest, &output, Camera::default(), &exe).unwrap();
    futty_build::validate_windows_template(&output.join("Game.exe")).unwrap();
    let launch: futty_build::Launch =
        ron::from_str(&fs::read_to_string(output.join(futty_build::MANIFEST)).unwrap()).unwrap();
    let packed = launch.project.unwrap();
    let root = output.canonicalize().unwrap();
    assert_eq!(packed.start, source.list().start);
    assert_eq!(packed.scenes.len(), 2);
    let mut copied_assets = 0;
    for id in source.list().scenes.keys() {
        let before = source.resolve(id).unwrap();
        let after = packed.resolve(&root, id).unwrap();
        let original = Scene::load(&before).unwrap();
        let packaged = Scene::load(&after).unwrap();
        assert_eq!(packaged.scene_uuid, original.scene_uuid);
        assert_eq!(packaged.entities, original.entities);
        for asset in &packaged.assets {
            if packaged.script_sources.contains_key(&asset.id) {
                continue;
            }
            assert!(original.assets.contains(asset));
            assert_eq!(
                futty_assets::read_saved(&before, asset).unwrap(),
                futty_assets::read_saved(&after, asset).unwrap(),
                "{}",
                asset.name
            );
            copied_assets += 1;
        }
    }
    assert!(copied_assets > 0);
}
