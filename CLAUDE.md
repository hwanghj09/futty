# Futty with Claude Code

Read the [canonical Futty product vision and implementation roadmap](docs/FUTTY_FINAL_GOAL.md) before planning 2D/3D, networking, one-shot art/animation/audio generation, or changing product priorities. Keep implemented MCP tools distinct from proposed APIs.

Before brainstorming or planning a game, or implementing an underspecified request such as "make an FPS game", read and use [futty-idea-interview](.agents/skills/futty-idea-interview/SKILL.md). Ask adaptive choice-based and imagination-oriented follow-up questions, then hand confirmed user intent to the game-design and one-shot skills. Respect detailed build-now requests or the user's wish to skip questions.

When asked to connect Futty MCP on Windows, run this from the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeCode
```

If an executable for this checkout is already built under WSL, pass its path with `-WslBinary` and add `-Distribution` if needed. Check `claude mcp get futty` and use `scene_inspect` with `examples/agent-escape/Beacon Bot Escape.futty.scene.ron` when the MCP tools are available. See the [connection guide](docs/guide.md#claude에서-futty-mcp-사용하기) and [game skill catalog](docs/game-skills.md).
