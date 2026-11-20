---
name: futty-playtest
description: Run reproducible headless Futty playtests with MCP, checking a game result against the expected outcome and saving a replay when useful.
---

# Futty Playtest

- Choose a saved scene, seed, tick limit, and success or failure condition. Use a Rhai `finish_game()` result for a gameplay assertion and `tick_limit` for a timeout assertion.
- Build a short sequence of action batches and send it to `play_test`. An action stays held across consecutive batches that contain it. Insert a step without the action before pressing jump or interact again.
- Read `passed`, `status`, `tick`, and `last_observation`. A test can return successfully while its game result differs from `expect`.
- To keep a reproduction, set `replay_path` to a new workspace-relative JSON path in an existing directory. To record a live session, call `play_save_replay` before `play_end`.

Use [the Beacon Bot Escape test input](../../../examples/agent-escape/playtest.json) for the request shape and [the playtest guide](../../../docs/guide.md#자동-플레이테스트와-리플레이) for results.
