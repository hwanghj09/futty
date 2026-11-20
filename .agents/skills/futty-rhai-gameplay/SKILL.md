---
name: futty-rhai-gameplay
description: Implement and test Futty Rhai game rules such as switches, doors, collectibles, HUD state, prefab spawning, and scene transitions.
---

# Futty Rhai Gameplay

- Use `on_start(entity)` for per-instance state and `on_update(entity, dt)` for input. Use `input_pressed("Interact")` for one-shot interactions.
- Use a unique tag with `find_tag`. Check for `()` before calling `send_event(target, name)`. Handle the event on the destination in `on_message(entity, sender, name)`; delivery occurs on a later tick.
- Transform and physics operations act on the current script entity. Use a kinematic body for a scripted solid door, and verify that its closed collider blocks the player.
- Use `set_hud(key, value)` with a matching HUD binding. Keep one-shot rules idempotent so holding or repeating input cannot duplicate a reward or toggle.
- Attach source with MCP `set_script` inside `scene_apply` or use the script panel in the Futty editor. Verify the expected success path, an invalid interaction, and a fresh Play session.

Use the [Rhai guide](../../../docs/guide.md#rhai-스크립트) for callbacks and supported functions. Review the [Beacon Bot Escape scene](../../../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron) for a switch-to-door rule with HUD feedback.
