# Pulse Range

45초 동안 이동 표적 다섯 대를 맞혀 점수를 쌓는 1인칭 연습장입니다. 드론은 잠깐 사라진 뒤 다시 등장합니다. 연속 명중하면 보너스가 붙고 빗나가면 연속 기록이 끊깁니다.

Futty Editor의 **File → Open Scene…**에서 `Pulse Range.futty.scene.ron`을 엽니다. 프로젝트 루트에서 편집 화면으로 바로 열거나 단독 실행할 수도 있습니다.

```powershell
cargo run --locked -p futty-editor -- --edit "examples/pulse-range/Pulse Range.futty.scene.ron"
cargo run --locked -p futty-editor -- --play "examples/pulse-range/Pulse Range.futty.scene.ron"
```

에디터에서 실행하려면 **Play**를 누릅니다.

| 조작 | 키 |
| --- | --- |
| 시작 또는 이어 하기 | 게임 화면 클릭 |
| 이동 | WASD |
| 조준 | 마우스 |
| 발사 | 왼쪽 클릭 또는 누르기 |
| 재장전 | R |
| 일시정지 및 마우스 해제 | Esc |
| 라운드 종료 후 다시 시작 | Enter |

`Assets/Futty`에는 경기장, 드론, 블래스터 모델과 효과음이 들어 있습니다. `model.py`는 Blender에서 사용한 원본 모델링 및 효과음 생성 스크립트입니다. 새 씬을 조립하려면 Futty MCP로 씬과 FPS 카메라를 생성한 뒤, 다음 명령의 `.ron` 경로를 새 씬 경로로 지정합니다.

```powershell
blender --background --factory-startup --python-exit-code 1 --python examples/pulse-range/model.py -- target/pulse-range-models
cargo run --locked -p futty-editor --example build_pulse_range -- "path/to/new-scene.ron" target/pulse-range-models
```
