---
name: futty-one-shot-game
description: Use Futty MCP plus local procedural or connected asset providers to produce cohesive game art, 2D sprites, 3D GLB models, animation clips, WAV/OGG sound, scenes and playable Windows builds. Distinguish prototype placeholder quality from production assets and validate each stage.
---

# Futty One-Shot Game Production

## When to use

Use when asked to create a full Futty game from one brief, including design, modeling, animation and sound. Futty's primary user is the AI agent; avoid mandatory manual GUI steps.

Read:
- [Futty 통합 최종 목표 / 2D·3D·멀티플레이 / 원스톱 제작](../../../docs/FUTTY_FINAL_GOAL.md)
- [Futty MCP 도구 안내](../../../docs/guide.md)
- [에셋 제작 참고 스킬](../../../docs/game-skills.md)

## Mandatory workflow

1. **Check actual capabilities first.** Read current crate/API definitions and MCP tool names. Never present a planned API as available. Currently the engine can import PNG, GLB, WAV and OGG; external generative image/3D/audio providers are not built into Futty.
2. **Write a brief.** Determine 2D/3D, single-player/multiplayer, Windows target, core loop, win/loss conditions, art style, target asset count and performance budget. If critical choices are unspecified, use small documented defaults.
3. **Create art-direction rules** before generating assets: palette, mood, scale, silhouette, coordinate axis, UI, animation names, acoustic style, licensing rules.
4. **Make an asset checklist** for scene background, characters, enemies, props, UI, effects, animation clips, SFX, ambience and music. Identify dependencies and accepted quality per entry.
5. **Select a generation method**:
   - Prototype fallback: from repository root run the command below and read asset-manifest.json; it produces procedurally generated placeholders, not model-generated production art.
   - Connected external asset generators: use only providers actually connected/available, with user-approved costs and required rights. Normalize to Futty-supported formats. Do not say an external provider was used unless its output was actually returned.
6. **Validate every file** before import: PNG channels/pixel dimensions; GLB embedded mesh/material, expected axes and animation clip index; WAV/OGG decode, non-empty duration and sane peaks; source/license metadata; predictable file paths.
7. **Import and bind** with existing Futty MCP tools: asset_import / asset_reimport, scene_apply, scene_inspect, script_create / script_attach, input_set, hud_create / hud_bind. Use returned UUIDs. Save and close a scene in the editor before changing it through MCP.
8. **Build playable scenes** with a camera, objects, collider/controller, interaction logic and event-to-animation/audio bindings. Avoid a demo of unrelated assets: each required asset should serve a gameplay or UI purpose.
9. **Test** using scene_validate, project_validate, play_test, play_replay and scene_capture when graphics are available. Verify animation events, pickups/attacks, audio play events and whether dependencies enter the final package.
10. **Package** via project_build for currently supported Windows games. Dedicated multiplayer server and 2D-specific builds are planned, not available until implemented.
11. **Report** implemented versus placeholder vs missing assets, test status, build paths, licenses and follow-up blockers. Never claim a complete game if the project fails validation or is missing required assets.

## First working prototype asset generator

~~~bash
python scripts/generate-prototype-assets.py --output generated/my-game --name "My Game" --theme fantasy --seed 42
~~~

Allowed themes: fantasy, horror, scifi. It creates PNG sprites/UI, one simple animated GLB with Idle and Action clips, three synthesized WAV clips, art-direction.json and asset-manifest.json. The output folder must be empty. Use a workspace-relative output so Futty MCP can access the sources.

Test the generator:

~~~bash
python -m unittest discover -s scripts -p test_prototype_asset_pack.py
~~~

The generated material is **prototype quality**; replacing with high-quality art requires connected content generation tools or handcrafted assets. This command alone does not create a complete game or automatically import assets into a Futty scene.

## Quality gates

| Gate | Pass criteria |
| --- | --- |
| Brief | Every requested feature has a concrete asset/gameplay task |
| Visual | Consistent palette, size, axes, layer order and readable HUD |
| Model | GLB decodes, mesh non-empty, proper scale, compatible rig/clips |
| Animation | Expected clips load, Idle/Action transitions work, no missing references |
| Audio | WAV/OGG decodes, cues attached to actual events, no missing/empty files |
| Runtime | Scene/project validation + executable gameplay test pass |
| Production | No unacknowledged placeholders, source/license review, playable build |

## Design principles

- Game functionality and AI tool contracts are higher priority than GUI polish.
- Use explicit asset-manifest hashes, versioned source paths and stable scene UUIDs.
- Reimport whenever possible instead of deleting a referenced asset.
- Never assume a generated artifact is high-quality, legal for reuse, or supported by the importer merely because the file exists.
- Server authority should drive gameplay events when multiplayer support is eventually implemented. Cosmetic animation/audio is client-side.
- Do not automatically purchase generation credits or expose credentials in logs or repository commits.
