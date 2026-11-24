# Futty

**MCP로 만들고, Windows 에디터에서 플레이하는 2D·3D 게임 엔진**

[![Windows CI](https://github.com/hwanghj09/futty/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hwanghj09/futty/actions/workflows/ci.yml)

[Windows 릴리스](https://github.com/hwanghj09/futty/releases) · [예제 포함 ZIP](https://github.com/hwanghj09/futty/archive/refs/heads/main.zip) · [사용 가이드](docs/guide.md) · [개발 로드맵](docs/FUTTY_FINAL_GOAL.md)

씬·에셋·게임 규칙을 에디터에서 편집하거나 Codex와 Claude에 맡길 수 있습니다. 완성한 프로젝트는 MCP로 플레이테스트하고 Windows 게임으로 내보낼 수 있습니다.

[사람용 시작 안내](#사람용-시작-안내) · [AI 에이전트용 작업 절차](#ai-에이전트용-작업-절차)

![Futty Windows 에디터에서 Pulse Range 예제를 열어 3D 씬과 카메라를 편집하는 화면](docs/images/futty-editor.jpg)

## 사람용 시작 안내

1. [Windows 릴리스](https://github.com/hwanghj09/futty/releases)에 서명된 배포판이 있으면 `futty-editor.exe`를 내려받습니다. 첫 서명 배포 전에는 [소스에서 빌드하기](#소스에서-빌드하기)를 이용할 수 있습니다.
2. 예제를 사용하려면 [저장소 ZIP](https://github.com/hwanghj09/futty/archive/refs/heads/main.zip)을 풀고, **File → Project → Open Project…**에서 `.futty.project.ron` 파일을 엽니다. 단일 씬은 **File → Open Scene…**에서 열 수 있습니다.
3. **Play**를 눌러 조작해 봅니다. 내 게임을 시작할 때는 **File → Project → New 2D Project…** 또는 **New 3D Project…**에서 빈 폴더를 고릅니다. 씬·이미지·스크립트 등 게임에 쓰는 파일을 프로젝트 폴더에서 관리할 수 있습니다. [프로젝트와 씬 전환](docs/guide.md#프로젝트와-씬-전환)

### 바로 열어볼 예제

| 예제 | 플레이 방법 |
| --- | --- |
| [Moving Platform Lab](examples/moving-platform/Moving%20Platform%20Lab.futty.project.ron) | 파란 발판을 타고 이동하다가 초록 착지대 근처에서 **D + Space**로 건너가는 2D 예제 |
| [Pulse Range](examples/pulse-range/Pulse%20Range.futty.project.ron) | 로비에서 연습장으로 들어가는 3D FPS. **WASD**, 마우스, 왼쪽 클릭, **R**로 이동·조준·발사·재장전 |
| [Beacon Bot Escape](examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron) | 스위치를 켜고 출구를 통과하는 퍼즐. [MCP 테스트 입력](examples/agent-escape/playtest.json) 제공 |
| [Planar Collision](examples/planar-collision/Planar%20Collision.futty.project.ron) | 충돌과 트리거를 관찰하는 2D 물리 예제 |

[전체 예제 폴더](examples/) · [Windows 게임 내보내기](docs/guide.md#windows-게임-내보내기)

### Windows 보안 경고가 표시될 때

저장소의 [개발용 Windows EXE](https://github.com/hwanghj09/futty/raw/refs/heads/main/dist/futty-editor.exe)는 코드 서명이 없어 다운로드한 파일에서 SmartScreen 경고가 표시될 수 있습니다. Windows에서 실제 발행자와 서명 상태를 확인하려면 다음 명령을 실행합니다. [Microsoft SmartScreen 안내](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)

```powershell
Get-AuthenticodeSignature -LiteralPath .\futty-editor.exe |
    Format-List Status, StatusMessage, SignerCertificate
```

[서명된 Windows 릴리스 준비](docs/guide.md#서명된-windows-릴리스-준비)

### EXE에서 AI 연결하기

`futty-editor.exe`에는 **Futty MCP 서버와 게임 제작 스킬**이 들어 있습니다. 에디터 상단의 **AI → Codex / Claude Code / Claude Desktop**에서 사용할 앱을 선택하고 작업할 게임 프로젝트 폴더를 고릅니다. 새 세션에서 해당 폴더를 열면 MCP로 게임을 제작할 수 있습니다.

- Codex는 Codex CLI가 설치되어 있어야 합니다. Claude Code에서는 새 세션에서 프로젝트의 MCP 연결을 승인합니다.
- Claude Desktop은 앱을 다시 열어 MCP를 연결합니다. 스킬은 프로젝트의 `.futty/claude-skills`에 생성된 ZIP을 **Customize → Skills**에서 가져옵니다.
- 프로젝트에 이미 있는 스킬 파일은 보존됩니다. [EXE 연결 상세 가이드](docs/guide.md#exe에-포함된-mcp와-스킬)

### Codex에 설치부터 맡기기

아래 프롬프트를 Windows의 Codex에 붙여넣으세요. Futty 폴더를 이미 열었다면 **“Futty MCP 연결해줘”**로 시작할 수 있습니다.

```text
내 Windows PC에 Futty를 설치하고 Futty MCP를 연결해줘.
명령을 직접 실행하고 실제로 연결됐는지 확인해줘.

1. https://github.com/hwanghj09/futty.git 을 새 futty 폴더에 클론해줘.
   이미 받은 사본이 있으면 그 폴더를 쓰고 기존 파일을 보존해줘.
2. 저장소의 AGENTS.md와 rust-toolchain.toml을 읽고 Codex CLI, Rustup,
   Visual Studio C++ Build Tools(MSVC 및 Windows SDK)를 확인해줘.
   빠진 도구는 설치 가능한 범위에서 설치하고, 승인이 필요한 단계만 알려줘.
3. 저장소 루트에서 다음 명령으로 서버를 빌드하고 futty라는 이름으로 등록해줘.
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
4. codex mcp get futty로 실행 파일과 작업 폴더가 이 사본을 가리키는지 확인해줘.
5. scene_inspect로 examples/agent-escape/Beacon Bot Escape.futty.scene.ron을 조회해줘.
   현재 채팅에 도구가 아직 표시되지 않으면 등록 결과와 새 채팅 또는 재시작 필요 여부를 알려줘.
6. 설치 경로, MCP 등록 상태, 예제 씬 조회 결과를 알려줘.
```

### Claude에서 연결하기

Futty 저장소 루트에서 사용하는 클라이언트에 맞는 명령을 실행합니다. 처음 빌드할 때는 Rustup과 Visual Studio C++ Build Tools가 필요합니다.

```powershell
# Claude Code
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeCode

# Claude Desktop
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeDesktop
```

- **Claude Code**는 같은 폴더에서 새 세션을 열고 `/mcp`로 연결을 확인합니다.
- **Claude Desktop**은 앱을 한 번 실행한 뒤 완전히 종료하고 등록 명령을 실행합니다. 다시 열면 로컬 도구에서 Futty를 확인할 수 있습니다.

[Claude 연결 상세 가이드](docs/guide.md#claude에서-futty-mcp-사용하기)

## AI 에이전트용 작업 절차

먼저 Codex는 [AGENTS.md](AGENTS.md), Claude Code는 [CLAUDE.md](CLAUDE.md)를 읽습니다. MCP의 씬 경로는 등록된 작업 폴더 기준이며, 프로젝트 파일에 기록되는 씬 경로는 매니페스트 폴더 기준입니다. 에디터에서 열린 씬은 MCP로 쓰기 전에 저장하고 닫습니다.

| 단계 | MCP 도구와 확인할 값 |
| --- | --- |
| 연결 확인 | `scene_inspect`에 `{"path":"examples/agent-escape/Beacon Bot Escape.futty.scene.ron"}` 전달 |
| 템플릿으로 시작 | `project_template_list`로 템플릿 조회 → `game_create_from_template`에 `{"template":"coin_runner_2d","output_path":"My-Coin-Runner"}` 전달. 새 폴더가 만들어지고 `project_path`, `scene_path`, `playtest`가 반환됨 |
| 씬 수정 | `scene_objects`로 대상 UUID 확인 → `scene_preview_ops`로 변경과 검증 이슈 확인 → `scene_apply`로 저장 |
| 동작 검증 | `play_test`에 템플릿의 `playtest`를 넘기고 `passed`, `status`, `tick` 확인. 재현은 `play_replay` 사용 |
| 패키징 | `project_validate`로 씬·에셋 검증 → `project_build`로 Windows 빌드. 플레이테스트·PNG·빌드를 함께 검증하려면 `game_review` 사용 |

- 아이디어가 아직 모호하면 [아이디어 인터뷰](.agents/skills/futty-idea-interview/SKILL.md)로 플레이 방식과 성공 조건을 정합니다.
- 게임 제작부터 리플레이·밸런스 검증까지 [저장소에 포함된 게임 스킬](docs/game-skills.md)을 사용합니다.
- 검증이나 빌드가 실패하면 응답의 `failure_report`에서 실패 단계와 코드를 확인합니다. [실패 보고서 형식](docs/failure-report.md)

## 현재 지원하는 기능

| 영역 | 지원 범위 |
| --- | --- |
| 2D | Sprite2D, 레이어·아틀라스 애니메이션, 픽셀 퍼펙트 직교 카메라, MCP 타일·미로 배치 |
| 3D | GLB 메시와 스켈레톤 애니메이션, 재질·조명·그림자·Sky, 원근 카메라 |
| 물리와 게임 규칙 | Rapier 2D·3D, 2D 박스·원·캡슐 충돌, 센서, 레이캐스트, 이동 발판, Rhai, HUD, 오디오 |
| 에디터와 프로젝트 | 씬 계층, 스냅·Undo/Redo, 프리팹, 씬 전환, 로컬 2인 협동, Windows 내보내기 |
| AI 제작과 검증 | 씬 수정 미리보기, 헤드리스 플레이테스트·리플레이, [Futty Ghost](docs/guide.md#futty-ghost-자동-플레이테스트), 카메라 PNG 캡처, 프로젝트 통합 검증 |

구현 범위와 이후 개발 순서는 [Futty 통합 로드맵](docs/FUTTY_FINAL_GOAL.md)에서 확인할 수 있습니다.

## 소스에서 빌드하기

Windows에 Git, Rustup, Visual Studio C++ Build Tools의 **MSVC**와 **Windows SDK**를 설치합니다. Rust 버전은 [rust-toolchain.toml](rust-toolchain.toml)에 고정되어 있습니다.

```powershell
git clone https://github.com/hwanghj09/futty.git
cd futty
cargo run --locked -p futty-editor
```

| 경로 | 역할 |
| --- | --- |
| [futty-editor](crates/futty-editor/) · [futty-build](crates/futty-build/) | Windows 에디터, 씬 실행, 게임 패키징 |
| [futty-runtime](crates/futty-runtime/) | 씬 포맷, ECS, 물리, Rhai 스크립트 |
| [futty-render](crates/futty-render/) · [shaders](shaders/) | wgpu 렌더러와 WGSL 셰이더 |
| [futty-core](crates/futty-core/) · [futty-assets](crates/futty-assets/) · [futty-audio](crates/futty-audio/) | 공용 타입, 에셋, 오디오 |
| [futty-mcp](crates/futty-mcp/) | Codex·Claude용 로컬 MCP 서버 |
