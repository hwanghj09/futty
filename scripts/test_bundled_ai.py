"""Smoke-test the Windows editor as a standalone AI bundle."""

import filecmp
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import zipfile
from pathlib import Path


def setup(executable, workspace, client, env=None):
    result = subprocess.run(
        [executable, "--setup-ai", workspace, client],
        cwd=workspace,
        env=env,
        capture_output=True,
        timeout=60,
    )
    assert result.returncode == 0, (result.stdout, result.stderr)


def verify_mcp(executable, workspace):
    with subprocess.Popen(
        [executable, "--mcp", workspace],
        cwd=workspace,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    ) as server:
        lines = queue.Queue()
        def read_output():
            for line in server.stdout:
                lines.put(line)

        threading.Thread(target=read_output, daemon=True).start()
        counter = 0

        def request(method, params, notify=False):
            nonlocal counter
            message = {"jsonrpc": "2.0", "method": method, "params": params}
            if not notify:
                counter += 1
                message["id"] = counter
            server.stdin.write(json.dumps(message) + "\n")
            server.stdin.flush()
            if not notify:
                response = json.loads(lines.get(timeout=30))
                assert response.get("id") == counter and "result" in response, response
                return response["result"]

        def invoke(name, arguments):
            result = request("tools/call", {"name": name, "arguments": arguments})
            assert not result.get("isError"), result
            return result["structuredContent"]

        try:
            request("initialize", {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "bundle-smoke", "version": "1"},
            })
            request("notifications/initialized", {}, notify=True)
            fixture = "examples/agent-escape"
            test = json.loads((workspace / fixture / "playtest.json").read_text())
            assert invoke("play_test", test)["passed"]
            scene = test["path"]
            door, = invoke("scene_objects", {"path": scene, "tag": "exit-door"})["objects"]
            preview = invoke("surgery_preview", {
                "path": scene, "object_id": door["id"], "color": [1, 0, 0],
            })
            assert preview["affected_object_ids"] == [door["id"]]
            applied = invoke("surgery_apply", {
                "path": scene, "preview_token": preview["preview_token"],
            })
            assert invoke("play_test", test)["passed"]
            assert invoke("surgery_rollback", {
                "path": scene, "undo_token": applied["undo_token"],
            })["undone"]
            run = invoke("play_start", {"path": scene})
            session = {"session_id": run["session_id"]}
            tint = invoke("surgery_hot_reload", {
                **session, "expected_tick": run["tick"],
                "tint": {"object_id": door["id"], "color": [1, 0, 0]},
            })
            assert invoke("play_step", session)["tick"] == tint["applies_at_tick"]
            invoke("play_save_replay", {**session, "path": "Hot reload.json"})
            assert invoke("play_replay", {"path": "Hot reload.json"})["matched"]
            invoke("play_end", session)
            project = f"{fixture}/Escape.futty.project.ron"
            invoke("project_create", {
                "path": project, "start": "main",
                "scenes": {"main": "Beacon Bot Escape.futty.scene.ron"},
            })
            result = invoke("project_build", {"path": project, "output_path": "Windows Game"})
            assert filecmp.cmp(executable, workspace / result["executable"], shallow=False)
        finally:
            server.stdin.close()
            if server.poll() is None:
                server.kill()


def main(executable):
    with tempfile.TemporaryDirectory(prefix="Futty bundle ") as temporary:
        root = Path(temporary)
        standalone = root / "futty-editor.exe"
        shutil.copyfile(executable, standalone)
        workspace = root / "Game Project"
        workspace.mkdir()
        config = workspace / ".mcp.json"
        config.write_text(json.dumps({"mcpServers": {"other": {"command": "other"}}}))
        setup(standalone, workspace, "claude-code")
        servers = json.loads(config.read_text())["mcpServers"]
        assert servers["other"]["command"] == "other"
        assert Path(servers["futty"]["command"]) == standalone
        assert servers["futty"]["args"] == ["--mcp", str(workspace)]
        skills = workspace / ".claude/skills"
        assert (skills / "futty-idea-interview/references/question-bank.md").is_file()
        custom = skills / "futty-ai-player/SKILL.md"
        custom.write_text("My customized skill")
        unchanged_time = 1_700_000_000
        os.utime(config, (unchanged_time, unchanged_time))
        setup(standalone, workspace, "claude-code")
        assert custom.read_text() == "My customized skill"
        assert config.stat().st_mtime == unchanged_time

        appdata = root / "appdata"
        (appdata / "Claude").mkdir(parents=True)
        desktop_env = dict(os.environ, APPDATA=str(appdata))
        setup(standalone, workspace, "claude-desktop", desktop_env)
        desktop = appdata / "Claude/claude_desktop_config.json"
        assert json.loads(desktop.read_text())["mcpServers"]["futty"]["args"] == ["--mcp", str(workspace)]
        archives = list((workspace / ".futty/claude-skills").glob("*.zip"))
        assert len(archives) == len(list(skills.glob("*/SKILL.md")))
        idea_skill = workspace / ".futty/claude-skills/futty-idea-interview.zip"
        kept_skill = workspace / ".futty/claude-skills/ponytail.zip"
        os.utime(desktop, (unchanged_time, unchanged_time))
        os.utime(kept_skill, (unchanged_time, unchanged_time))
        idea_skill.unlink()
        setup(standalone, workspace, "claude-desktop", desktop_env)
        assert desktop.stat().st_mtime == unchanged_time
        assert kept_skill.stat().st_mtime == unchanged_time
        with zipfile.ZipFile(idea_skill) as archive:
            assert "futty-idea-interview/references/question-bank.md" in archive.namelist()
        setup(standalone, workspace, "claude-desktop", desktop_env)
        assert desktop.stat().st_mtime == unchanged_time
        verify_mcp(standalone, workspace)
        print(f"Standalone editor installed {len(archives)} skills and exported Game.exe through MCP.")


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve(strict=True))
