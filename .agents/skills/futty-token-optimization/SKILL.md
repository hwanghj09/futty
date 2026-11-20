---
name: futty-token-optimization
description: Keep Futty development context small when investigating Rust build logs, large RON scenes, or verbose MCP playtest responses, or when measuring command-output savings with RTK.
---

# Futty Token Optimization

## Shell and source

- Check for RTK when a command is likely to produce large output. If available, use `rtk git status` and `rtk cargo test -p futty-mcp` for compact output. Otherwise use Git's short status and target the relevant Cargo package or test.
- Locate symbols with `rg -n` and read the relevant ranges. Start from `crates/futty-mcp/src/main.rs` for tool routing and the relevant runtime module for scene behavior. Inspect examples directly when their saved data is the subject of the task.
- If a filtered command fails, recover its exact error with `rtk proxy` or the native command and narrow the read around that diagnostic. Keep exit status, file and line, and the first relevant error when summarizing.

## Futty MCP

- Use `scene_objects` with `object_id` or `tag` for targeted reads. Page with `offset` and `limit` when scanning a scene. Use `scene_validate` for saved-file diagnostics.
- For fixed input sequences, prefer one `play_test` call. For interactive sessions, advance with `play_step` and use the returned observation before making another query. Record status, tick, and relevant HUD values. Save a replay when the failure must be reproduced, then use `play_replay` to locate the first divergent tick.
- If the user requests a measurement, report `rtk gain` figures as estimated shell-output savings and state their scope. Use platform usage data for whole-session totals when available.

[Codex Token Optimizer](https://github.com/HelloWorld668/codex-token-optimizer/blob/dd2129d00fe4580041cd699ab87ac976ec277a4d/codex-token-optimizer/SKILL.md) · [RTK documentation](https://github.com/rtk-ai/rtk)
