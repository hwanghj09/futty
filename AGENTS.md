# Futty

## MCP 연결

사용자가 “MCP 연결해줘” 또는 “Futty MCP 연결해줘”라고 하면 이 저장소 루트에서 아래 명령을 직접 실행합니다. Git 클론과 GitHub ZIP에서 똑같이 실행할 수 있습니다.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
```

Windows용 서버를 빌드하고 Codex에 `futty`라는 이름으로 등록합니다. 필요한 도구가 없으면 스크립트의 오류 메시지에 나온 항목을 확인하고 설치 가능한 환경에서는 설치 후 다시 실행합니다. 기존에 WSL에서 이 사본의 서버를 빌드한 경우 `-WslBinary`와 해당 바이너리의 경로로 기존 빌드를 등록할 수 있습니다.

`codex mcp get futty`로 현재 사본이 등록됐는지 확인합니다. 새 도구 목록이 보이면 `scene_inspect`에 `examples/agent-escape/Beacon Bot Escape.futty.scene.ron`을 넘겨 연결을 확인합니다. 이미 열린 Codex 세션에서 등록이 반영되지 않으면 새 채팅 또는 앱 재시작이 필요하다고 알립니다.
