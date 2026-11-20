---
name: futty-replay-debug
description: Replay a recorded Futty play session and diagnose the first divergent tick using the saved actions, checkpoint, and source version.
---

# Futty Replay Debug

- Call `play_replay` with the workspace-relative replay JSON path. A matching replay returns its final status and tick.
- If `source_changed` is reported, recover the recorded scene and scripts or make a new recording for the current scene.
- If `state_changed` is reported, inspect `first_different_tick` and the expected and actual positions and statuses. Locate the recorded action batch that includes that tick. Start with the preceding state and check which input or Rhai event should change it.
- Use a fresh `play_test` or live MCP session to narrow the reproduction and check the repaired scenario.

The [playtest guide](../../../docs/guide.md#자동-플레이테스트와-리플레이) lists the replay format and expected results.
