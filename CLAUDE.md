# Futty with Claude Code

When asked to connect Futty MCP on Windows, run this from the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeCode
```

If an executable for this checkout is already built under WSL, pass its path with `-WslBinary` and add `-Distribution` if needed. Check `claude mcp get futty` and use `scene_inspect` with `examples/agent-escape/Beacon Bot Escape.futty.scene.ron` when the MCP tools are available. See the [connection guide](docs/guide.md#claude에서-futty-mcp-사용하기) and [game skill catalog](docs/game-skills.md).
