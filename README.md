# futty

**Rust로 만든 Windows용 3D 씬 에디터와 게임 런타임.** 장면을 편집하고 바로 플레이한 뒤, 에셋과 함께 Windows 게임으로 내보낼 수 있습니다.

[![Windows CI](https://github.com/hwanghj09/futty/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hwanghj09/futty/actions/workflows/ci.yml)

![Futty Editor에서 Pulse Range를 연 화면. 왼쪽에 오브젝트 계층과 1인칭 미리보기, 오른쪽에 3D 뷰포트와 카메라 속성이 보입니다.](docs/images/futty-editor.jpg)

## 빠르게 시작하기

Windows x64에서 Git, Rustup, Visual Studio C++ Build Tools의 **MSVC**와 **Windows SDK**가 필요합니다. Rust 버전은 `rust-toolchain.toml`에 고정되어 있습니다.

```powershell
git clone https://github.com/hwanghj09/futty.git
cd futty
cargo run --locked -p futty-editor
```

### Pulse Range로 시작하기

45초 동안 움직이는 드론을 맞히는 FPS 연습장입니다. 편집용 씬과 직접 모델링한 연습장·블래스터·드론, 효과음이 함께 들어 있습니다.

```powershell
cargo run --locked -p futty-editor -- --edit "examples/pulse-range/Pulse Range.futty.scene.ron"
```

에디터의 **Play**를 누른 뒤 게임 화면을 클릭합니다. **WASD**로 이동, **마우스**로 조준, **왼쪽 클릭**으로 발사, **R**로 재장전합니다. **Esc**로 일시정지하고 라운드가 끝나면 **Enter**로 다시 시작할 수 있습니다. [씬과 모델링 원본](examples/pulse-range/README.md)

## 주요 기능

| 기능 | 지원 범위 |
| --- | --- |
| 씬 편집 | 계층, 다중 선택, 이동·회전·크기 조절, 스냅, Undo/Redo, 연결형 프리팹 |
| 카메라와 렌더링 | 원근·정사영 카메라, Scene/Game 뷰, Sky 배경, wgpu 렌더링, 조명과 그림자 |
| 에셋 | GLB 메시·스켈레톤 애니메이션·UUID 유지 재가져오기, PNG 텍스처, 공유 머티리얼, Sprite3D, WAV·Ogg 효과음 |
| 게임 실행 | ECS 런타임, Rapier 물리, Rhai 스크립트, Windows 게임 내보내기 |
| 프로젝트 | 시작 씬 지정, 런타임 씬 전환, 여러 씬을 묶은 Windows 빌드 |
| 자동화 | Codex에서 로컬 MCP로 씬 생성·조회, Sky 전환, 카메라 추가·선택 |

## 예제

| 예제 | 열거나 실행하는 방법 |
| --- | --- |
| [Pulse Range](examples/pulse-range/) | 연습장 씬을 바로 열거나 프로젝트를 열어 로비에서 시작 |
| [Beacon Bot](examples/animation-playground/Beacon%20Bot.futty.scene.ron) | 씬을 열고 **Play**를 눌러 Idle·Walk·Signal 전환 |
| [Prefab Workshop](examples/prefab-playground/Prefab%20Workshop.futty.scene.ron) | 원본을 편집해 인스턴스 갱신과 오버라이드 확인 |
| [Workshop GLB와 텍스처](examples/asset-preview/) | **Project → Import…**에서 GLB와 PNG 가져오기 |
| [Rhai 이동](examples/script-playground/) | **File → Script Playground** |
| [Sprite3D](examples/sprite-playground/) | **File → Sprite Playground** |
| [오디오 트리거](examples/audio-playground/) | **File → Audio Playground** |
| 물리 | **File → Physics Playground** |

## 문서

- [Futty AI 게임 엔진 로드맵](docs/ai-roadmap.md)
- [에디터 사용 가이드](docs/guide.md)
- [Codex에서 Futty MCP 연결](docs/guide.md#codex에서-futty-mcp-사용하기)
- [Windows 게임 내보내기](docs/guide.md#windows-게임-내보내기)

## 저장소 구조

| 경로 | 역할 |
| --- | --- |
| [`crates/futty-editor`](crates/futty-editor/) | 에디터 UI, 씬 플레이어, Windows 내보내기 |
| [`crates/futty-runtime`](crates/futty-runtime/) | 씬 포맷, ECS, 물리, 스크립트 |
| [`crates/futty-render`](crates/futty-render/) · [`shaders`](shaders/) | wgpu 렌더러와 WGSL 셰이더 |
| [`crates/futty-core`](crates/futty-core/) · [`crates/futty-assets`](crates/futty-assets/) · [`crates/futty-audio`](crates/futty-audio/) | 공용 타입, 에셋 로딩, 오디오 |
| [`crates/futty-mcp`](crates/futty-mcp/) | 로컬 stdio MCP 서버 |
