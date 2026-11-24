---
name: futty-game-ui
description: Design and verify in-game Futty interfaces such as title screens, lobbies, pause menus, score and health HUDs, and results screens. Use when a request changes what players see or do through Futty game UI.
---

# Futty Game UI

1. Identify the current game state, what the player needs to know, and the next action they can take. Inspect existing HUD widgets with `hud_list` and preserve any bindings that still match the rules. For a new screen, choose a type hierarchy, neutral text color and an accent for the primary action that fit the game's art.
2. Put persistent values such as score or health at the edges. Reserve the center for menus, results, or a cue that needs immediate attention. Keep aim and interaction targets clear. Give each interactive screen one visually dominant action, use short action labels, and pair color changes with text or a value for important states.
3. Use Futty's `hud_create` and `hud_update` to set `anchor`, `offset`, `size`, `color` and `text_size`. Corners measure offsets inward and Center uses signed offsets from the middle. Use Text for labels or values, Gauge for bounded numeric values, Button for actions and Image for registered PNGs. A Text widget with a binding and a label puts the label above the value, so leave enough height for both.
4. Connect widgets with `hud_bind`. Set text and gauge values from Rhai via `set_hud`, and handle buttons via `hud_pressed`. Check the scene input mapping before showing key hints. In scenes that capture the mouse, test entering and leaving menus with cursor capture enabled.
5. Run `scene_validate` or `project_validate`. Use Play and a standalone build as appropriate to check narrow and wide windows, contrast over a bright scene, clipped labels, overlaps, and the primary action. Exercise any live bound state or scene transition that changed. A headless test can verify available HUD values and actions. `scene_capture` shows the camera view without the game HUD.

[Futty HUD guide](../../../docs/guide.md#게임-hud) · [Pulse Range lobby example](../../../examples/pulse-range/README.md)
