# Futty

**[Futty 최종 목표 · 통합 개발 로드맵](docs/FUTTY_FINAL_GOAL.md)** — AI가 기획부터 2D/3D, 멀티플레이, 디자인·모델링·애니메이션·사운드, 테스트와 배포까지 담당하는 엔진의 제품 기준 문서.

Futty는 **AI 에이전트가 MCP로 3D 게임을 만들고 검증할 수 있도록 설계된 Rust 게임 엔진**입니다. 사람은 Windows 에디터에서 씬을 직접 편집하고, 예제 게임을 플레이하고, 실행 파일을 내보낼 수 있습니다.

[![Windows CI](https://github.com/hwanghj09/futty/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hwanghj09/futty/actions/workflows/ci.yml)

**처음 시작하나요?** [사람용 시작 안내](#사람용-시작-안내) · [AI 에이전트용 작업 절차](#ai-에이전트용-작업-절차) · [ZIP 다운로드](https://github.com/hwanghj09/futty/archive/refs/heads/main.zip)

**[Windows x64 에디터 EXE 다운로드](https://github.com/hwanghj09/futty/raw/refs/heads/main/dist/futty-editor.exe)** · 다운로드한 `futty-editor.exe`를 더블클릭하면 에디터가 열립니다.

![Futty Editor에서 Pulse Range를 연 화면. 왼쪽에 오브젝트 계층과 1인칭 미리보기, 오른쪽에 3D 뷰포트와 카메라 속성이 보입니다.](docs/images/futty-editor.jpg)

## 게임 아이디어부터 시작하기

**[futty-idea-interview 스킬](.agents/skills/futty-idea-interview/SKILL.md)**은 "FPS 게임 만들어줘"처럼 아이디어가 아직 모호할 때 사용자의 머릿속 게임을 단계별 질문으로 구체화합니다. 참조 게임의 플레이 감각, 첫 장면, 핵심 재미, 전투·맵·비주얼·사운드 등에서 선택지와 후속 질문을 제공하며, 확정된 답은 [게임 기획 스킬](.agents/skills/futty-game-design/SKILL.md) 및 [원스톱 제작 스킬](.agents/skills/futty-one-shot-game/SKILL.md)로 연결합니다. 질문은 사용자가 충분히 답하거나 바로 제작해 달라고 할 때 중지합니다.

## 사람용 시작 안내

| 하고 싶은 일 | 시작 방법 |
| --- | --- |
| 게임 예제를 직접 실행하고 싶어요 | [Windows에서 에디터 실행](#windows에서-에디터-실행) |
| 새 게임을 만들고 싶어요 | **File → Project**에서 **New 2D Project…** 또는 **New 3D Project…**를 고르고 빈 폴더를 선택하세요. [프로젝트 사용법](docs/guide.md#프로젝트와-씬-전환) |
| Codex에게 설치와 연결을 맡기고 싶어요 | [다음 프롬프트를 복사](#codex로-futty-설치-및-mcp-연결) |
| Claude에서 게임을 만들고 싶어요 | [Claude Code와 Claude Desktop에 연결](#claude-code와-claude-desktop에-연결) |
| 만든 게임을 다른 사람에게 보내고 싶어요 | [Windows 게임 내보내기](docs/guide.md#windows-게임-내보내기) |

### Windows에서 에디터 실행

[Windows x64 실행 파일](https://github.com/hwanghj09/futty/raw/refs/heads/main/dist/futty-editor.exe)을 내려받아 실행하세요. 예제도 열어 보려면 [저장소 ZIP](https://github.com/hwanghj09/futty/archive/refs/heads/main.zip)을 풀고, 에디터에서 **File → Open Scene…**으로 `examples` 폴더의 씬 파일을 엽니다.

소스에서 빌드할 때는 Git, Rustup, Visual Studio C++ Build Tools의 **MSVC**와 **Windows SDK**가 필요합니다. Rust 버전은 `rust-toolchain.toml`에 고정되어 있습니다.

```powershell
git clone https://github.com/hwanghj09/futty.git
cd futty
cargo run --locked -p futty-editor
```

### 첫 예제 플레이하기

45초 동안 움직이는 드론을 맞히는 FPS 연습장입니다. 편집용 씬과 직접 모델링한 연습장·블래스터·드론, 효과음이 함께 들어 있습니다.

```powershell
cargo run --locked -p futty-editor -- --edit "examples/pulse-range/Pulse Range.futty.scene.ron"
```

에디터의 **Play**를 누른 뒤 게임 화면을 클릭합니다. **WASD**로 이동, **마우스**로 조준, **왼쪽 클릭**으로 발사, **R**로 재장전합니다. **Esc**로 일시정지하고 라운드가 끝나면 **Enter**로 다시 시작할 수 있습니다. [씬과 모델링 원본](examples/pulse-range/README.md)

### Codex로 Futty 설치 및 MCP 연결

아래 프롬프트를 새 Codex 채팅에 붙여넣으세요. 이미 Futty 폴더를 Codex에서 열어 둔 경우에는 **“Futty MCP 연결해줘”**라고 요청해도 됩니다.

```text
내 Windows PC에 Futty를 설치하고 여기에 Futty MCP를 연결해줘.
명령을 설명하는 데 그치지 말고 직접 실행해서 등록과 연결 확인까지 진행해줘.

1. https://github.com/hwanghj09/futty.git 를 새 futty 폴더에 클론해줘.
   이미 이 저장소를 받은 폴더가 있으면 그 사본을 사용하고, 기존 파일은 덮어쓰지 마.
2. Futty 폴더의 AGENTS.md와 rust-toolchain.toml을 읽어줘. Codex CLI, Rustup,
   Visual Studio C++ Build Tools(MSVC 및 Windows SDK)를 확인하고, 빠진 도구가
   있으면 설치 가능한 범위에서 설치해줘. 관리자 승인이나 재시작이 필요한 단계만 알려줘.
3. Futty 저장소 루트에서 아래 명령을 실행해서 서버를 빌드하고, 현재 Futty 폴더를
   작업 경로로 지정해 futty라는 이름으로 MCP에 등록해줘.
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
4. codex mcp get futty로 등록된 실행 파일과 작업 경로를 확인해줘.
   MCP 도구가 보이면 scene_inspect로 examples/agent-escape/Beacon Bot Escape.futty.scene.ron
   파일을 조회해 실제 응답을 확인해줘. 현재 채팅에 등록이 반영되지 않으면
   등록 결과를 보고하고 새 채팅이나 Codex 재시작이 필요한지 알려줘.
5. 마지막에 설치 경로와 등록 여부, 예제 씬 조회 결과를 알려줘.
```

### Claude Code와 Claude Desktop에 연결

Windows에서 Futty 폴더를 열고 사용하는 클라이언트에 맞는 명령을 실행하세요. 서버를 처음 빌드할 때는 Rustup과 Visual Studio C++ Build Tools가 필요합니다.

```powershell
# Claude Code
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeCode

# Claude Desktop
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeDesktop
```

- **Claude Code**에서는 같은 폴더에서 새 세션을 열고 `/mcp`로 `futty` 연결을 확인하세요. 이 폴더를 이미 Claude Code에서 열었다면 **“Futty MCP 연결해줘”**라고 요청해도 됩니다.
- **Claude Desktop**에서는 앱을 한 번 실행한 뒤 완전히 종료하고 등록 명령을 실행하세요. 앱을 다시 열면 Futty가 로컬 도구로 표시됩니다.

연결된 Claude에 **“Futty MCP로 Beacon Bot Escape 예제 씬을 조회해줘”**라고 요청해 보세요. [연결 상세 가이드](docs/guide.md#claude에서-futty-mcp-사용하기)

### 더 살펴볼 예제

| 예제 | 열거나 실행하는 방법 |
| --- | --- |
| [Pulse Range](examples/pulse-range/) | 연습장 씬을 바로 열거나 프로젝트를 열어 로비에서 시작 |
| [Beacon Bot](examples/animation-playground/Beacon%20Bot.futty.scene.ron) | 씬을 열고 **Play**를 눌러 Idle·Walk·Signal 전환 |
| [Beacon Bot Escape](examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron) | 에디터와 MCP에서 스위치를 켜 출구를 통과하거나 [플레이테스트 입력](examples/agent-escape/playtest.json)으로 성공 판정 확인 |
| [Prefab Workshop](examples/prefab-playground/Prefab%20Workshop.futty.scene.ron) | 원본을 편집해 인스턴스 갱신과 오버라이드 확인 |
| [Workshop GLB와 텍스처](examples/asset-preview/) | **Project → Import…**에서 GLB와 PNG 가져오기 |
| [Rhai 이동](examples/script-playground/) | **File → Script Playground** |
| [Sprite3D](examples/sprite-playground/) | **File → Sprite Playground** |
| [오디오 트리거](examples/audio-playground/) | **File → Audio Playground** |
| 물리 | **File → Physics Playground** |

## AI 에이전트용 작업 절차

Futty 작업을 시작할 때 Codex는 [AGENTS.md](AGENTS.md), Claude Code는 [CLAUDE.md](CLAUDE.md)를 읽고 현재 사본이 MCP에 등록됐는지 확인합니다. [Claude 연결 명령](docs/guide.md#claude에서-futty-mcp-사용하기)

| 단계 | 실행할 일 |
| --- | --- |
| 연결 | Windows의 저장소 루트에서 아래 스크립트를 실행하고 `codex mcp get futty`로 바이너리와 작업 폴더를 확인합니다. |
| 조회 | `scene_inspect`로 씬을 확인하고 `scene_objects`에서 필요한 UUID나 태그를 찾습니다. 도구의 씬 경로는 MCP 작업 폴더를 기준으로 합니다. |
| 제작 | `scene_create`로 시작하고 `scene_apply`로 관련 변경을 함께 저장합니다. 에디터에서 열린 씬은 저장하고 닫은 뒤 MCP로 편집합니다. |
| 검증 | `scene_validate`의 `issues`를 확인합니다. 동작은 `play_test`의 `passed`, `status`, `tick`으로 확인하고, 재현이 필요하면 기록을 저장한 뒤 `play_replay`를 실행합니다. |
| 프로젝트 | `project_create`로 씬을 묶고, `project_set_start_scene`으로 시작 씬을 지정한 뒤 `project_inspect`로 확인합니다. 프로젝트의 씬 경로는 매니페스트 폴더를 기준으로 합니다. |

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
```

**첫 MCP 확인**은 `scene_inspect`에 아래 입력을 넘깁니다.

```json
{"path":"examples/agent-escape/Beacon Bot Escape.futty.scene.ron"}
```

이후 [Beacon Bot Escape 플레이테스트 입력](examples/agent-escape/playtest.json)을 그대로 `play_test`에 전달하면 스위치를 켜고 출구를 통과하는 경로를 검증할 수 있습니다.

[도구별 입력 예제](docs/guide.md#codex에서-futty-mcp-사용하기) · [저장소에 포함된 에이전트 스킬](docs/game-skills.md) · **[Futty 최종 목표](docs/FUTTY_FINAL_GOAL.md)**

## 주요 기능

| 기능 | 지원 범위 |
| --- | --- |
| 씬 편집 | 계층, 다중 선택, 이동·회전·크기 조절, 스냅, Undo/Redo, 연결형 프리팹 |
| 카메라와 렌더링 | 원근·정사영 카메라, Scene/Game 뷰, Sky 배경, wgpu 렌더링, 조명과 그림자 |
| 에셋 | GLB 메시·스켈레톤 애니메이션·UUID 유지 재가져오기, PNG 텍스처, 공유 머티리얼, Sprite3D, WAV·Ogg 효과음 |
| 게임 실행 | ECS 런타임, Rapier 물리, Rhai 스크립트, Windows 게임 내보내기 |
| 프로젝트 | 시작 씬 지정, 런타임 씬 전환, 여러 씬을 묶은 Windows 빌드 |
| 자동화 | 로컬 MCP 씬 조회·편집·검증, 프로젝트 관리, 헤드리스 플레이테스트와 리플레이 |

## 더 알아보기

- [에디터와 MCP 사용 가이드](docs/guide.md)
- [게임 제작 및 검증 스킬](docs/game-skills.md)
- **[Futty 최종 목표 및 통합 개발 명세](docs/FUTTY_FINAL_GOAL.md)** — AI-first · 2D/3D · 멀티플레이 · 디자인/모델링/애니메이션/사운드 · 개발 순서
- [프로토타입 에셋 생성기](scripts/generate-prototype-assets.py)

## 저장소 구조

| 경로 | 역할 |
| --- | --- |
| [`crates/futty-editor`](crates/futty-editor/) | 에디터 UI, 씬 플레이어, Windows 내보내기 |
| [`crates/futty-runtime`](crates/futty-runtime/) | 씬 포맷, ECS, 물리, 스크립트 |
| [`crates/futty-render`](crates/futty-render/) · [`shaders`](shaders/) | wgpu 렌더러와 WGSL 셰이더 |
| [`crates/futty-core`](crates/futty-core/) · [`crates/futty-assets`](crates/futty-assets/) · [`crates/futty-audio`](crates/futty-audio/) | 공용 타입, 에셋 로딩, 오디오 |
| [`crates/futty-mcp`](crates/futty-mcp/) | 로컬 stdio MCP 서버 |
| [`crates/futty-build`](crates/futty-build/) | 에디터와 MCP가 공유하는 Windows 게임 패키징 |
