# Futty

제품 비전, 구현 우선순위, 실제 구현과 계획의 구분은 [최종 목표·통합 로드맵](docs/FUTTY_FINAL_GOAL.md)을 기준으로 합니다. AI-first 제작 시 GUI는 필수 단계가 아닙니다.

## 게임 기획/아이디어 인터뷰 (AI-first)

사용자가 게임 아이디어 구상·컨셉·기획을 요청하거나 "FPS 게임 만들어"처럼 원하는 게임이 불분명하면 **[futty-idea-interview](.agents/skills/futty-idea-interview/SKILL.md)를 우선 적용**합니다. 먼저 사용자가 상상한 장면/감정/플레이 방식에 관한 선택지와 심화 질문을 여러 차례 진행하고, 합의한 Game Brief를 [게임 기획 스킬](.agents/skills/futty-game-design/SKILL.md)과 [원스톱 제작 스킬](.agents/skills/futty-one-shot-game/SKILL.md)로 전달합니다. 이미 구체적이거나 바로 구현을 명시한 요청에 억지로 질문을 늘리지 않습니다. 사용자가 "더 물어봐"라고 하면 대화를 이어갑니다.

## MCP 연결

사용자가 “MCP 연결해줘” 또는 “Futty MCP 연결해줘”라고 하면 이 저장소 루트에서 아래 명령을 직접 실행합니다. Git 클론과 GitHub ZIP에서 똑같이 실행할 수 있습니다.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
```

Windows용 서버를 빌드하고 Codex에 `futty`라는 이름으로 등록합니다. 필요한 도구가 없으면 스크립트의 오류 메시지에 나온 항목을 확인하고 설치 가능한 환경에서는 설치 후 다시 실행합니다. 기존에 WSL에서 이 사본의 서버를 빌드한 경우 `-WslBinary`와 해당 바이너리의 경로로 기존 빌드를 등록할 수 있습니다.

`codex mcp get futty`로 현재 사본이 등록됐는지 확인합니다. 새 도구 목록이 보이면 `scene_inspect`에 `examples/agent-escape/Beacon Bot Escape.futty.scene.ron`을 넘겨 연결을 확인합니다. 이미 열린 Codex 세션에서 등록이 반영되지 않으면 새 채팅 또는 앱 재시작이 필요하다고 알립니다.
