---
name: futty-ai-player
description: Play a saved Futty scene through MCP by reading player observations, choosing movement or interaction actions, and checking the game result.
---

# Futty AI Player

- Start with `play_start` using the saved scene's workspace-relative path. Keep its `session_id` and current `tick`.
- Act on the latest observation. Use the player pose, visible objects, and HUD values to choose the next move. Re-aim before moving when the target is offscreen. Treat object visibility as a center-point test against physics colliders.
- Call `play_step` with the actions to hold, a tick count from 1 to 120, and the last observed tick as `expected_tick`. Use shorter steps around walls, switches, and doors. Use an empty action list to wait or release a held action before pressing it again.
- Read the next observation after each step. Stop when the session status changes from `running` or when the user's objective is met. Record the result and elapsed ticks, then close the session with `play_end`.

The [AI play guide](../../../docs/guide.md#ai-플레이-세션) lists the observation and action fields. Use [Beacon Bot Escape](../../../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron) to exercise approaching a switch, opening a door, and receiving a success result.
