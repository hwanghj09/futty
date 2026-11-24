# futty Editor 사용 가이드

Windows용 Rust 3D 엔진 에디터입니다. 자체 wgpu/WGSL 렌더러가 GLB 메시와 텍스처를 깊이 버퍼가 있는 오프스크린 텍스처에 그립니다. 에디터는 eframe의 winit 창과 egui-wgpu UI를 사용합니다.

## 실행

Windows x64에서 Rustup과 Visual Studio C++ Build Tools의 MSVC 및 Windows SDK를 설치한 뒤 실행합니다. 저장소의 `rust-toolchain.toml`이 Rust 1.99.0을 선택합니다.

```powershell
cargo run --locked -p futty-editor
```

## 작업 공간

- 기본 배치는 왼쪽 **Hierarchy**, 중앙 **Scene / Game**, 오른쪽 **Inspector**, 아래 **Project**입니다. **Console**, **Physics**, **Script Editor**는 탭이나 Scene을 우클릭한 뒤 **Panels**에서 엽니다. Console과 Physics는 마지막 Play 기록을 Stop 이후에도 표시합니다.
- 탭을 다른 탭 옆에 놓으면 같은 그룹에 들어가고, 패널 가장자리에 놓으면 새 영역이 생깁니다. 구분선을 드래그해 크기를 조절합니다. 탭을 오른쪽 클릭해 **Eject**로 앱 안에 띄우고 **Dock in Main Window**로 다시 붙일 수 있습니다.
- 닫은 탭은 **Window** 메뉴에서 다시 열고, **Layout → Reset to Default**로 기본 배치로 돌아갑니다. 마지막 배치는 다음 실행에 복원됩니다.

## 편집

- **Hierarchy → + Add**에서 큐브, 구체, 원기둥, 원뿔, 평면을 추가합니다. 새 도형은 바로 선택되며 카메라가 그 도형에 초점을 맞춥니다. **Inspector**에서 이름, 위치, 회전, 크기, 색상을 수정합니다.
- **Hierarchy → Search objects**에 이름을 입력하면 목록이 즉시 좁혀집니다. Hierarchy와 뷰포트에서 **Ctrl+클릭**으로 선택을 추가하거나 해제합니다. Hierarchy에서는 **Shift+클릭**으로 보이는 행의 범위를, **Ctrl+A**로 현재 필터에 맞는 전체를 선택합니다.
- **Inspector → Parent**로 부모를 지정하거나 Hierarchy의 행을 다른 오브젝트 위로 드래그합니다. 화면상의 위치·회전·크기를 유지하며 부모가 바뀝니다. 씬 이름이나 Hierarchy 빈 공간에 놓으면 루트로 이동합니다. 복제와 삭제는 선택한 오브젝트의 하위 계층에도 적용됩니다.
- **Move** 모드에서 X/Y/Z 화살표를 끌면 해당 축으로 이동합니다. **Select** 모드에서는 이동 도구를 숨깁니다.
- 편집 모드에서 선택한 오브젝트에 **G**는 이동, **S**는 크기, **R**은 회전을 시작합니다. 마우스를 움직이거나 수치를 입력하고 **Enter** 또는 왼쪽 클릭으로 적용합니다. **Esc** 또는 오른쪽 클릭으로 취소합니다. **X/Y/Z**는 축을 고릅니다. 크기는 오브젝트의 로컬 축으로 조절합니다.
- **Scene 우클릭 → Transform Settings**에서 이동·회전의 **World / Local** 축, **Selection Center / Active Object / Individual Origins** 피벗을 고릅니다. Local의 기준은 마지막으로 선택한 오브젝트입니다. 이동 기즈모도 같은 축과 피벗을 사용합니다.
- 같은 메뉴의 **Snap**과 Move·Rotate·Scale 간격으로 마우스 변형을 일정 단위에 맞춥니다. **Ctrl**을 누르는 동안 임시 스냅도 가능합니다. 키보드로 직접 입력한 수치는 그대로 적용됩니다. 설정은 다음 실행에도 복원됩니다.
- 정확한 값을 적용하려면 **G → X → 2 → Enter**로 X 방향 2만큼 이동하거나, **R → Y → 90 → Enter**로 Y축 90도 회전합니다. **S → 2 → Enter**는 크기를 두 배로 만듭니다. 한 번의 적용은 Undo 한 번으로 되돌릴 수 있습니다.
- 단축키가 입력되면 뷰포트 상단에 키, 변형 모드, 축과 실시간 값이 표시됩니다. 축을 고르면 해당 축의 색상으로 가이드가 나타나고, 적용·취소 결과도 잠시 표시됩니다.
- Scene이나 Hierarchy에서 오브젝트를 우클릭하면 초점 맞추기, 복제, 삭제, 오브젝트 추가 메뉴가 열립니다. Scene 우클릭 메뉴에는 선택·이동 도구, 그리드, 콜라이더 표시 옵션도 있습니다. 우클릭 드래그로 카메라를 돌리고, 우클릭 중 **WASD**로 이동합니다. 가운데 버튼 드래그는 팬, 휠은 줌, **F**는 선택 오브젝트에 초점을 맞춥니다.
- **Ctrl+D**로 복제하고 **Delete**로 삭제합니다. **Ctrl+Z / Ctrl+Y**로 변경을 되돌리거나 다시 적용합니다.
- 여러 오브젝트에 **G/S/R**, 이동 기즈모, 복제·삭제를 한 번에 적용할 수 있습니다. 회전·크기 조절은 묶음의 중심을 기준으로 합니다. 부모와 자식을 함께 선택하면 자식은 부모를 따라갑니다. Inspector는 마지막으로 선택한 오브젝트의 속성을 표시합니다.
- **F**는 선택한 묶음 전체에 초점을 맞춥니다. Hierarchy에서 한 행을 더블클릭하면 해당 오브젝트로 이동합니다.
- **File → Save Scene**으로 씬을 저장하고 **Open Scene**으로 다시 엽니다. RON 파일에는 포맷 버전과 오브젝트 UUID가 기록됩니다.
- **Play**를 누르면 복사된 ECS 월드에서 회전이 활성화된 큐브가 움직입니다. **Stop**을 누르면 원래 씬과 카메라로 돌아갑니다.

## 모델과 텍스처

1. 하단 **Project → Import…**에서 [Workshop Crate.glb](../examples/asset-preview/Workshop%20Crate.glb)를 엽니다. 모델이 씬에 배치됩니다. 에셋을 우클릭해 **Place**를 누르거나 더블클릭으로 같은 모델을 추가합니다.
2. [Workshop Grid.png](../examples/asset-preview/Workshop%20Grid.png)를 가져옵니다. 큐브나 UV가 있는 모델을 선택하고 Project에서 PNG를 우클릭해 **Apply to Selection**을 누릅니다. Inspector에서 **Default material**로 되돌릴 수 있습니다.
3. 씬을 저장하면 에셋 원본을 씬과 같은 폴더의 `Assets/Futty/`에 UUID 이름으로 복사합니다. 다른 위치로 옮길 때는 씬 RON과 해당 폴더를 함께 옮깁니다. **Save Scene As…**는 새 위치에 필요한 에셋을 복사합니다.

씬 포맷 v19부터 프리팹 링크와 인스턴스 오버라이드를 저장합니다. v1~v18 씬도 열 수 있습니다. 누락된 모델은 뷰포트에서 회색 대체 큐브로, Project 목록에서 오류 문구로 표시합니다.

### GLB 재가져오기

Blender에서 변경한 모델을 GLB로 내보낸 뒤 **Project → 기존 GLB 우클릭 → Reimport GLB…**로 교체 파일을 고릅니다. 모델의 UUID와 표시 이름, 배치된 오브젝트와 프리팹의 연결을 유지합니다. 같은 에셋 폴더를 쓰는 씬은 다시 열 때 새 모델을 읽습니다.

저장된 GLB는 교체 즉시 디스크에 반영됩니다. 씬의 Undo는 오브젝트 편집에 적용되며 이전 GLB 파일로 돌아가려면 그 파일을 다시 가져옵니다. 시작 클립에 사용하는 번호는 교체한 GLB에도 있어야 합니다. Rhai에서 클립 번호를 사용한다면 내보내기 전에 클립 순서도 확인합니다.

## GLB 애니메이션

1. 스켈레톤과 클립이 들어 있는 GLB를 **Project → Import…**로 가져옵니다.
2. 모델을 선택하고 **Inspector → Mesh Renderer → Animation**에서 **Start clip**을 고릅니다. **Bind pose**를 선택하면 기본 자세로 시작합니다.
3. **Preview** 또는 시간 슬라이더를 써서 Scene과 Game 뷰에서 자세를 확인합니다. **Loop**와 **Speed**는 씬에 저장되며 Play와 Windows 게임에 반영됩니다.

게임 중에는 오브젝트에 붙인 Rhai 스크립트의 이벤트 콜백에서 `play_animation(entity, 2, false)`로 클립을 처음부터 재생할 수 있습니다. 인스펙터에 표시된 GLB 클립 번호를 사용합니다. 반복을 끄면 마지막 프레임을 유지합니다.

[Beacon Bot 예제](../examples/animation-playground/Beacon%20Bot.futty.scene.ron)의 **Idle**, **Walk**, **Signal** 버튼으로 클립을 바꿔볼 수 있습니다. 이 예제의 Walk는 제자리에서 재생되며, 게임 안에서의 이동은 캐릭터 컨트롤러나 스크립트로 조종합니다.

GLB의 이동·회전·스케일 채널과 Linear·Step·Cubic Spline 보간을 지원합니다. 스킨은 최대 256개 관절과 정점당 4개 가중치를 사용합니다. 모프 타깃과 클립 간 블렌딩은 후속 범위입니다. [glTF 2.0 스킨과 애니메이션 규격](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#animations)

## 공유 머티리얼

1. **Project 빈 공간 우클릭 → New Material**로 재질을 만듭니다. Project의 **MAT** 항목을 클릭하면 기존 Inspector에서 이름, 기본색, PNG 텍스처, **Roughness**, 발광색과 **Emission** 강도를 편집합니다.
2. 도형이나 GLB 오브젝트를 선택하고 MAT 항목을 우클릭해 **Apply to Selection**으로 적용합니다. 여러 오브젝트를 선택하면 한 번에 적용됩니다. 오브젝트의 **Inspector → Material**에서도 재질을 선택할 수 있습니다.
3. 같은 재질을 쓰는 오브젝트는 수정이 즉시 함께 반영됩니다. **Tint**로 오브젝트별 색을 곱할 수 있습니다. 새로운 재질을 적용하면 Tint는 흰색으로 초기화됩니다. **Edit Shared Material**로 재질 편집을 열고 **Back to Object**로 돌아옵니다.
4. **Per-object material**을 선택하면 기존 개별 색상·텍스처 설정을 사용합니다. PNG의 **Apply to Selection**은 해당 오브젝트를 개별 재질로 전환합니다. 사용 중인 재질과 재질에 연결된 PNG는 Project에서 삭제할 수 없습니다.

머티리얼은 씬과 함께 저장되며 Undo/Redo, Play와 Windows 내보내기에 반영됩니다. Roughness는 광택의 넓이와 강도를 조절하는 간단한 Blinn–Phong 모델입니다. 발광은 표면 자체의 밝기에 반영되며 주변을 비추거나 블룸을 만들지는 않습니다. 공유 재질은 불투명 3D 메시용이고 Sprite3D는 기존 Tint와 투명도를 사용합니다.

## 연결형 프리팹

1. Hierarchy에서 루트로 삼을 오브젝트 하나를 선택하고 우클릭 → **Create Prefab**을 누릅니다. 자식 계층과 기본 도형·모델, Sprite3D, 카메라·조명, 물리·스크립트·오디오, 공유 머티리얼 연결을 함께 저장합니다. Project 빈 공간의 **Create Prefab from Selection**에서도 만들 수 있습니다.
2. Project의 **PREFAB** 항목을 클릭하면 기존 Inspector에서 이름과 저장된 계층을 확인합니다. **Place in Scene**, 항목 더블클릭, 우클릭 → **Place in Scene**으로 반복 배치합니다. 새 계층은 기존 오브젝트 옆의 바닥에 배치되며 루트가 선택됩니다. **G / R / S**로 전체 계층을 조절합니다.
3. 프리팹의 **Edit Source**로 원본을 선택합니다. 원본을 편집하면 같은 씬의 연결된 인스턴스가 갱신됩니다. 변경과 전파는 Undo 한 번으로 되돌릴 수 있습니다.
4. 인스턴스에서 위치, 회전, 크기, 머티리얼, 텍스처 또는 색을 바꾸면 해당 항목이 오버라이드로 저장됩니다. **Reset Overrides**는 선택한 오브젝트의 값을 원본과 다시 연결합니다. 인스턴스 루트의 배치 위치와 부모는 그대로 사용합니다.

원본에 추가한 자식은 인스턴스에도 추가됩니다. 원본에서 자식을 제거하면 해당 복제본도 정리됩니다. 그 복제본에 개별 수정이나 따로 붙인 오브젝트가 있으면 씬에 보존됩니다. 인스턴스의 자식을 직접 삭제하거나 다른 부모로 옮기면 그 인스턴스를 연결 해제합니다. **Unpack**으로 수동으로 연결을 해제할 수도 있습니다.

**Remove Prefab**은 저장된 프리팹을 지우고 배치된 오브젝트를 일반 오브젝트로 유지합니다. 원본이 사라진 프리팹은 마지막으로 저장한 내용으로 배치할 수 있습니다. v18까지 만들어 둔 독립 복제본은 기존 상태로 열립니다. Windows 빌드는 적용된 인스턴스와 에셋을 포함합니다.

[Prefab Workshop 예제](../examples/prefab-playground/Prefab%20Workshop.futty.scene.ron)에서 왼쪽 **Signal Tower → Beacon**의 **Position Y**를 바꿔 보세요. 가운데 탑은 새 위치를 따르고, 오른쪽 탑은 높이와 주황색 재질 오버라이드를 유지합니다.

## 게임 카메라

1. **Hierarchy → + Add → Camera**에서 카메라를 추가합니다. 첫 카메라는 **Main Camera**가 되며, 추가할 때 현재 Scene 화면의 위치와 방향을 가져옵니다.
2. **Inspector → Object → Camera**에서 **Perspective**, **Orthographic**, 시야각 또는 정사영 크기, Near/Far 클리핑을 조절합니다. 다른 구도로 맞추려면 Scene에서 구도를 잡고 Hierarchy에서 카메라를 우클릭해 **Align to Scene View**를 누릅니다.
3. 카메라 우클릭 메뉴의 **Preview in Game**으로 구도를 비교하고 **Make Main Camera**로 실행할 카메라를 지정합니다. 선택된 카메라의 시야 범위는 **Scene**에 표시됩니다. **Play**와 Windows 게임은 Main Camera를 사용합니다. 카메라의 Transform은 부모와 연결하거나 Rhai로 움직일 수 있습니다.
4. 카메라가 없는 씬은 Scene 카메라로 미리 보고 실행할 수 있습니다.

## Scene 카메라와 배경

- **Alt + 왼쪽 드래그**로 화면 중심을 기준으로 회전합니다. **F** 또는 **Camera 메뉴의 Frame Selection**으로 선택한 오브젝트를, **Frame All**로 씬 전체를 화면에 맞춥니다.
- **Camera 메뉴의 Reset View**로 Scene 카메라를 초기 위치로 되돌립니다. **Fly Speed**는 우클릭 상태에서 WASD·Q/E로 이동하는 속도를 조절합니다.
- **Background 메뉴**에서 **Sky**와 **Solid**를 전환합니다. 설정은 씬에 저장되고 Game 미리보기, Play, 내보낸 Windows 게임에도 반영됩니다. 새 씬은 Sky로 시작하고 기존 씬은 Solid로 열립니다.

## EXE에 포함된 MCP와 스킬

Windows 에디터의 **AI** 메뉴에서 **Codex**, **Claude Code**, **Claude Desktop** 중 사용하는 앱을 선택한 뒤 Futty에게 맡길 프로젝트 폴더를 고릅니다. 그 폴더가 MCP 작업 경로가 됩니다. 설치에는 저장소나 Rust가 필요하지 않습니다.

| 앱 | 연결과 스킬 |
| --- | --- |
| Codex | 설치된 Codex CLI로 Futty를 등록합니다. 프로젝트를 새 채팅에서 열면 `.agents/skills`의 스킬을 사용할 수 있습니다. |
| Claude Code | 프로젝트의 `.mcp.json`을 갱신하고 `.claude/skills`에 스킬을 둡니다. 프로젝트에서 새 세션을 열고 Futty MCP를 승인합니다. |
| Claude Desktop | 앱을 한 번 실행한 뒤 **AI → Claude Desktop**을 사용하고 앱을 완전히 종료했다가 다시 엽니다. 스킬은 `.futty/claude-skills`의 ZIP을 **Customize → Skills**에서 가져옵니다. [Claude 스킬 가져오기](https://support.claude.com/en/articles/12512180-use-skills-in-claude) |

등록된 클라이언트는 `futty-editor.exe --mcp <작업 폴더>`로 같은 EXE의 서버 모드를 실행합니다. EXE를 옮겼다면 같은 메뉴에서 연결을 갱신합니다. `scene_inspect`에 `examples/agent-escape/Beacon Bot Escape.futty.scene.ron`을 넘겨 연결을 확인할 수 있습니다.

## Codex에서 Futty MCP 사용하기

GitHub에서 저장소를 클론하거나 **Code → Download ZIP**으로 받아 압축을 풉니다. Codex에서 Futty 폴더를 열고 **“Futty MCP 연결해줘”**라고 입력하면 저장소의 `AGENTS.md`에 따라 설치 스크립트를 실행합니다.

Windows에서 직접 연결하려면 저장소 루트의 PowerShell에서 같은 스크립트를 실행합니다. Codex CLI, rustup, Visual Studio Build Tools의 **Desktop development with C++** 워크로드가 필요합니다. 프로젝트의 Rust 버전은 `rust-toolchain.toml`에 지정되어 있습니다.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
```

스크립트는 서버를 빌드하고 현재 폴더를 작업 경로로 지정해 Codex의 `futty` 등록을 갱신합니다. 열려 있는 세션에서 도구가 보이지 않으면 새 채팅을 시작하거나 Codex를 다시 실행합니다.

- `scene_create`와 `scene_inspect`로 씬을 만들고 차원, 배경, 오브젝트 수, 카메라 UUID를 확인합니다.
- `scene_set_background`로 `sky` 또는 `solid`를 저장합니다. `scene_add_camera`로 위치와 바라볼 점을 지정하고 `scene_set_main_camera`에 카메라 UUID를 넘겨 게임 카메라를 선택합니다.
- `scene_objects`는 오브젝트 UUID, 태그, 부모, 로컬 Transform, 컴포넌트와 에셋 참조를 반환합니다. `tag`, `object_id`, `offset`, `limit`으로 범위를 좁힐 수 있습니다.
- `scene_apply`는 한 씬에 최대 64개의 편집을 순서대로 적용해 한 번에 저장합니다. 배치에서 만든 오브젝트의 `key`는 뒤에 나오는 작업의 `object`나 `parent`에서 사용할 수 있습니다. 작업이 실패하면 이전 씬 파일이 유지됩니다.
- `scene_undo`에는 `path`와 `scene_apply`가 반환한 `undo_token`을 사용합니다. 이 MCP 세션의 가장 최근 배치를 되돌릴 수 있으며, 이후 씬 내용이 바뀐 경우 오류를 반환합니다.
- 경로는 등록한 Futty 폴더 기준의 상대 `.ron` 경로를 사용합니다. 에디터에서 열린 씬은 MCP 편집 전에 저장하고 닫은 뒤, 편집이 끝나면 다시 엽니다.

| `scene_apply` 작업 | 하는 일 |
| --- | --- |
| `add_object`, `edit_object` | 도형을 추가하고 이름, 태그, 위치, XYZ 회전 각도, 크기, 색, 콜라이더를 설정합니다. |
| `add_camera`, `add_point_light` | 바라볼 점을 지정한 카메라와 점광원을 추가합니다. `character: true`인 카메라는 이동 입력을 받습니다. |
| `set_parent`, `remove_leaf` | 로컬 Transform을 유지하며 부모를 바꾸거나, 자식과 프리팹 링크가 없는 오브젝트를 삭제합니다. |
| `place_prefab`, `attach_asset` | 등록된 프리팹을 배치하거나 모델·텍스처·스크립트 UUID를 연결합니다. |
| `set_script`, `add_hud_text` | Rhai 소스를 검사해 연결하거나 텍스트 HUD와 값 바인딩을 추가합니다. |
| `set_component`, `remove_component` | 조명(`light`), 충돌체·강체(`body`), 캐릭터 컨트롤러(`character`)를 설정하거나 제거합니다. |

컴포넌트 하나를 조회하려면 `component_get`에 씬 `path`, 오브젝트 `object_id`, `component` 종류를 넘깁니다. 없는 컴포넌트는 `value: null`로 표시됩니다.

`scene_apply`에서 아래 편집은 `floor` 키로 등록된 오브젝트의 충돌체를 설정합니다. 기존 오브젝트는 키 자리에 UUID를 씁니다. 빠진 값은 기존 값을 유지하고, 새 컴포넌트라면 기본값을 사용합니다.

```json
{"op":"set_component","object":"floor","component":{"kind":"body","motion":"Fixed","size":[8,1,8],"sensor":false,"collision_layer":2}}
```

제거는 `{"op":"remove_component","object":"floor","component":"body"}`로 요청합니다. `light_type`은 `Point` 또는 `Directional`, `motion`은 `Fixed`, `Dynamic`, `Kinematic`을 사용합니다. 강체와 캐릭터 컨트롤러는 같은 루트 오브젝트에 동시에 둘 수 없습니다. 연결된 프리팹 인스턴스의 컴포넌트는 원본 오브젝트에서 바꿉니다.

바닥을 만들고 태그를 지정하는 `scene_apply` 입력 예시입니다.

```json
{
  "path": "examples/room.futty.scene.ron",
  "edits": [
    {"op": "add_object", "key": "floor", "name": "Floor", "properties": {
      "position": [0, -0.1, 0], "scale": [6, 0.2, 10], "collider": "fixed"
    }},
    {"op": "edit_object", "object": "floor", "properties": {"tags": ["room-floor"]}}
  ]
}
```

[Beacon Bot Escape](../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron)에서 조작할 수 있는 카메라, 고정 충돌체, Rhai 스위치와 문, 목표 HUD를 확인할 수 있습니다. **Play**를 누르고 **W**로 전진해 주황색 스위치 옆에서 **E**를 누른 뒤, 열린 문을 지나면 됩니다.

## Claude에서 Futty MCP 사용하기

Windows에서 Futty 폴더를 열고 사용할 클라이언트의 명령을 실행합니다. 기본 연결은 서버를 먼저 빌드하므로 Rustup과 Visual Studio C++ Build Tools가 필요합니다. 이 사본의 WSL 빌드가 이미 있다면 `-WslBinary`에 실행 파일 경로를 지정할 수 있습니다.

```powershell
# Claude Code
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeCode

# Claude Desktop
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1 -Client ClaudeDesktop
```

- **Claude Code**는 현재 작업 폴더에만 `futty`를 등록합니다. 이 폴더에서 Claude Code를 열고 `/mcp`로 연결을 확인합니다. [Claude Code MCP 문서](https://code.claude.com/docs/en/mcp)
- **Claude Desktop**은 한 번 실행해 설정 폴더를 만든 뒤 완전히 종료하고 등록 명령을 실행합니다. 스크립트가 `%APPDATA%\Claude\claude_desktop_config.json`의 `futty` 항목을 작성합니다. 앱을 다시 열어 로컬 도구가 나타나는지 확인합니다. [MCP 호스트 연결 문서](https://py.sdk.modelcontextprotocol.io/get-started/real-host/)

연결을 시험하려면 **“Futty MCP로 `examples/agent-escape/Beacon Bot Escape.futty.scene.ron` 씬을 조회해줘”**라고 요청합니다.

### 씬 편집 미리보기와 비교

저장하기 전에 `scene_apply`와 같은 `path`·`edits`를 `scene_preview_ops`에 전달합니다. 올바른 배치에는 `revision`과 `changes`가 반환됩니다. 올바르지 않은 배치는 `valid: false`와 검사 오류를 반환합니다. 편집 작업의 오류에는 1부터 시작하는 `edit` 번호가 포함됩니다.

```json
{
  "path":"Main.futty.scene.ron",
  "edits":[{"op":"add_object","key":"marker","name":"Start Marker"}]
}
```

확인한 배치를 적용할 때는 같은 요청에 `"expected_revision":"<미리보기의 revision>"`을 추가해 `scene_apply`를 호출합니다. 씬 파일 내용이 달라졌으면 새 미리보기를 만들어 적용해야 합니다. `preview_objects`의 UUID는 미리보기 안에서만 쓰며, 확정 UUID는 적용 응답의 `objects`에서 확인합니다.

저장된 파일 두 개는 `scene_diff`에 `{"before_path":"Before.ron","after_path":"After.ron"}`을 넘겨 비교합니다. 두 도구는 UUID를 기준으로 오브젝트·에셋·HUD·재질·프리팹·내장 스크립트 차이를 분류하고, 각 종류의 추가·삭제·수정 개수와 최대 64개 항목의 세부 정보를 반환합니다. 바뀐 오브젝트 속성에는 이전 값과 새 값이 표시됩니다. 둘 다 씬 파일이나 Undo 기록을 쓰지 않습니다.

## MCP 에셋과 재질

| 도구 | 사용 방법 |
| --- | --- |
| `asset_import` | 저장된 씬의 `path`와 작업 폴더 안의 파일 `source`를 넘깁니다. GLB, PNG, WAV, Ogg를 검사한 뒤 씬 옆의 `Assets/Futty/`에 복사하고 등록된 UUID를 반환합니다. |
| `asset_import_batch` | 한 씬에 새 에셋 등록과 UUID 유지 교체를 섞어서 처리하고, 오브젝트·스프라이트·공유 재질·HUD 이미지에 연결합니다. |
| `asset_list` | `path`의 등록된 에셋과 공유 재질을 조회합니다. |
| `asset_reimport` | `path`, `asset_id`, 같은 종류의 새 `source`를 지정합니다. 재질 및 오브젝트 참조에 쓰이는 UUID는 그대로 유지됩니다. GLB는 지정한 씬과 그 프리팹에서 참조하는 애니메이션 클립을 확인합니다. |
| `material_create` / `material_update` | `path`와 `settings`를 넘기고 갱신 시에는 `material_id`도 지정합니다. 설정은 `name`, `color`, `texture`, `roughness`, `emission`, `emission_strength`입니다. 생략한 필드는 유지되며 `texture: null`은 텍스처 연결을 해제합니다. |

`scene_apply`의 `attach_asset`으로 모델, 텍스처, 오디오를 오브젝트에 붙입니다. 공유 재질은 `{"op":"set_material","object":"floor","material_id":"<uuid>"}`로 할당합니다. `object`에는 기존 오브젝트 UUID 또는 현재 배치의 키를 쓸 수 있으며, `material_id: null`로 할당을 해제합니다.

별도 도구로 등록하거나 교체한 에셋 파일과 재질 설정은 `scene_undo`의 범위 밖입니다. `scene_apply`에서 한 할당은 배치와 함께 되돌릴 수 있습니다.

### 일괄 가져오기와 바인딩

```json
{
  "path": "Scenes/Main.futty.scene.ron",
  "assets": [
    {"key":"hero","source":"Generated/sprites/hero_walk.png","asset_id":"<기존 에셋 UUID>",
     "bind":[{"target":"sprite","id":"<캐릭터 오브젝트 UUID>","size":[1,1],"billboard":"Fixed"}]},
    {"key":"pickup","source":"Generated/audio/pickup.wav",
     "bind":[{"target":"object","id":"<오브젝트 UUID>"}]}
  ]
}
```

`asset_id`를 생략하면 새 UUID로 등록하고, 지정하면 같은 형식의 파일로 교체하면서 기존 UUID와 참조를 유지합니다. 응답의 `assets`는 각 `key`를 실제 UUID에 매핑합니다. 프리팹에 연결된 오브젝트는 원본에 바인딩합니다.

- `object`는 에셋 종류에 따라 모델, 텍스처, 오디오 소스를 연결합니다. `sprite`는 PNG를 Sprite3D로 설정합니다. `material`과 `hud_image`는 기존 재질이나 HUD 이미지 위젯에 PNG를 연결합니다.
- 파일·바인딩을 모두 검사한 뒤 반영합니다. 쓰기 오류가 발생하면 배치에서 이미 쓴 에셋 파일을 복구합니다. 호출당 에셋 32개, 바인딩 64개, 교체 백업 포함 128 MiB까지 처리합니다.

## MCP에서 Rhai 스크립트 작업하기

| 도구 | 입력과 결과 |
| --- | --- |
| `script_validate` | Rhai 코드인 `source` 또는 작업 폴더 기준 `.rhai` 파일 `path`를 지정합니다. `valid`, 유효한 `fields()`의 기본값 또는 오류 메시지·줄·열이 반환됩니다. |
| `script_create` | 씬 `path`, `name`, `source`를 넘기면 검사를 통과한 소스를 씬에 등록하고 `script_id`를 반환합니다. |
| `script_get` | 씬 `path`와 `script_id`로 등록된 스크립트의 이름과 원문을 읽습니다. |
| `script_update` | 씬 `path`, `script_id`, 새 `source`를 지정합니다. UUID와 오브젝트 연결이 유지됩니다. |
| `script_attach` | 씬 `path`, `object_id`, `script_id`를 넘깁니다. `script_id: null`로 연결을 해제하며, 변경은 `scene_undo`로 되돌릴 수 있습니다. |

여러 오브젝트에 붙이려면 `scene_apply`에서 `{"op":"attach_script","object":"switch","script_id":"<uuid>"}`를 사용합니다. `object`에는 UUID나 배치 내 오브젝트 키가 들어갑니다. 연결된 프리팹 인스턴스의 스크립트를 바꾸려면 원본을 편집합니다.

생성·수정은 검사를 통과한 소스만 씬 안에 저장합니다. 파일로 가져온 스크립트를 수정해도 원래 `.rhai` 파일은 보존되고, 그 씬은 새 내장 소스를 사용합니다. 줄과 열은 Rhai가 위치를 제공하는 오류에서 표시됩니다. 실행 결과는 별도로 `play_test`에서 확인할 수 있습니다.

## MCP에서 게임 입력과 HUD 설정하기

- `input_get`에 씬 `path`를 넘겨 저장된 입력 설정과 기본 바인딩을 확인합니다.
- `input_set`은 해당 씬의 입력 설정 전체를 교체합니다. 지정한 항목은 에디터 Play와 단독 게임에 적용되고, 나머지는 플레이어의 **F10 → Game Controls** 설정을 사용합니다. 프로젝트에서 씬을 전환하면 새 씬의 입력 설정이 적용됩니다.
- `capture_mouse: true`를 지정하면 게임 화면을 클릭했을 때 마우스를 잠그고, 버튼을 누르지 않아도 마우스 이동으로 시점을 돌립니다. `Esc` 또는 창 포커스 해제로 마우스 잠금을 풉니다. 이 모드에서 `Ctrl`은 전진 중 달리기, `Shift`는 느리게 걷기입니다.

```json
{
  "path": "Main.futty.scene.ron",
  "bindings": {"MoveForward":"ArrowUp","Jump":"J","Interact":"F"},
  "sensitivity": 1.3
}
```

키는 대문자 A–Z, 숫자 0–9, 방향키(`ArrowUp` 등), `Space`, `Enter`, `Tab`, `Backspace`, `Mouse1`–`Mouse5`를 지원합니다. 헤드리스 `play_step`과 `play_test`는 기존처럼 물리 키 대신 행동을 받습니다.

| 도구 | 용도 |
| --- | --- |
| `hud_list` | 씬 `path`의 위젯을 그리는 순서대로 조회 |
| `hud_create` | `path`, `kind`(`Text`, `Image`, `Button`, `Gauge`), `settings`로 생성 |
| `hud_update` | `path`, `widget_id`, `settings`로 필요한 속성만 수정 |
| `hud_bind` | `path`, `widget_id`, `binding`으로 값이나 버튼 동작 연결 |

설정에는 `label`, `binding`, `anchor`(`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Center`), `offset`, `size`, `color`, `text_size`, `image`를 쓸 수 있습니다. Image의 `image`에는 등록된 PNG UUID를 지정합니다. Text와 Gauge는 Rhai의 `set_hud("score", 값)`과 같은 키에 연결하고, Button은 `hud_pressed("retry")`와 같은 이름에 연결합니다.

## 씬 검증과 프로젝트 관리

- `scene_validate`에 저장된 씬의 `path`를 넘기면 `valid`와 `issues`를 반환합니다. 씬 구조 검사에서 오류가 나면 첫 오류를 보고하고, 구조가 유효하면 저장된 에셋 파일을 검사합니다. 문제에 따라 `field`, `object_id`, `asset_id`가 제공됩니다.
- `project_create`로 저장된 씬들을 프로젝트에 묶습니다. 아래 예제의 `path`는 MCP 작업 폴더 기준이며 `scenes`에 들어가는 경로는 프로젝트 파일이 있는 폴더 기준입니다. 프로젝트 파일은 에디터의 **Project → Open Project**에서 열 수 있습니다.

```json
{
  "path": "examples/agent-escape/Beacon Bot Escape.futty.project.ron",
  "start": "escape",
  "scenes": {"escape": "Beacon Bot Escape.futty.scene.ron"}
}
```

- `project_inspect`는 `path`에 해당하는 프로젝트의 씬 목록과 시작 씬 ID를 반환합니다.
- `project_validate`는 프로젝트 파일의 `path`를 받아 시작 씬 설정과 등록된 씬을 검사합니다. 씬별로 구조 오류, 누락되거나 읽을 수 없는 에셋, Rhai 오류를 반환합니다. 스크립트 오류의 줄·열은 Rhai가 위치를 제공할 때 표시됩니다.
- `project_set_start_scene`에는 프로젝트의 `path`와 새 시작 씬 ID인 `start`를 넘깁니다. 이미 등록되어 있고 불러올 수 있는 씬을 선택해야 합니다.

## MCP 게임 템플릿

`project_template_list`로 사용 가능한 템플릿을 확인합니다. 첫 템플릿인 **Coin Collection**은 1인칭 카메라, 코인 세 개, 점수 HUD와 승리 조건이 있는 작은 게임입니다.

```json
{"template":"coin_collection","output_path":"My-Coin-Game"}
```

이 입력을 `game_create_from_template`에 전달하면 새 폴더에 씬과 프로젝트 파일을 생성합니다. 기존 폴더의 내용은 보존됩니다. 반환된 `project_path`는 에디터에서 열거나 `project_build`에 전달하고, `playtest`는 그대로 `play_test`에 전달해 세 코인을 모으는지 확인할 수 있습니다. 직접 플레이할 때는 게임 화면을 클릭하고 기본 이동 키인 **WASD**를 사용합니다.

### 2D Coin Runner

```json
{"template":"coin_runner_2d","output_path":"My-Coin-Runner"}
```

`game_create_from_template`가 Sprite2D 캐릭터와 시트 애니메이션, Rapier2D 발판 타일, 추적 카메라, 코인과 점수 HUD를 갖춘 횡스크롤 프로젝트를 만듭니다. `A`·`D`로 이동하고 `Space`로 틈을 건너 코인 세 개를 모으세요. 틈 아래로 떨어지면 실패합니다. 에디터에서 다시 시도할 때는 **Stop** 후 **Play**를 누릅니다.

반환된 `playtest`로 점프와 코인 세 개 수집을 검증할 수 있습니다. 점프를 빼고 오른쪽으로 계속 이동하는 대조 테스트는 코인 하나를 모은 뒤 추락합니다. 템플릿의 PNG는 저장소의 프로토타입 에셋 생성기로 만든 파일이며, 프로젝트 폴더에 함께 복사됩니다. [프로토타입 에셋 생성기](../scripts/generate-prototype-assets.py)

에디터를 열지 않고 전체 흐름을 검증하려면 반환된 `project_path`로 `project_validate`를 먼저 실행합니다. `playtest.steps`를 `[{"ticks":120}]`으로 바꾼 대조 테스트에서는 코인 0개와 실패한 판정을 확인할 수 있습니다. 원래 `playtest`로 코인 3개와 승리를 확인한 뒤, 같은 프로젝트에 `project_build`를 호출하면 새 폴더에 `Game.exe`가 생성됩니다.

### 2D Power Maze

```json
{"template":"power_maze_2d","output_path":"My-Power-Maze"}
```

`game_create_from_template`가 시드 기반 미로, 충돌 벽, 직교 카메라, 애니메이션 캐릭터와 목표 HUD를 갖춘 탑다운 프로젝트를 만듭니다. `WASD`로 이동해 전원 셀을 얻고 출구로 돌아오세요. 전원 없이 출구에 닿으면 잠금 안내가 표시됩니다. 전원 셀은 닿으면 자동으로 수집됩니다.

응답의 `playtest`로 전원을 모아 탈출하는 경로를 재현한 뒤, `project_path`를 `project_build`에 전달해 Windows 게임으로 내보낼 수 있습니다.

## MCP 프리팹과 씬 전환

- `prefab_create`에 씬 `path`와 원본 `object_id`를 넘기면 오브젝트와 자식으로 프리팹을 만듭니다.
- `prefab_instantiate`에 `path`, `prefab_id`, `position`을 넘기면 연결된 인스턴스를 배치하고 루트 UUID와 Undo 토큰을 반환합니다. 일괄 편집에서는 `place_prefab`을 쓸 수 있습니다.
- `prefab_update`에 `path`, `prefab_id`, 선택적인 `name`을 넘기면 프리팹과 연결된 인스턴스를 갱신합니다. 구성은 원본 오브젝트를 편집합니다.
- `scene_transition_set`은 프로젝트 파일 `path`, 출발 씬 ID `from`, 도착 씬 ID `to`, 출발 씬의 `object_id`를 받습니다. 기본 트리거는 Interact이며, `button`에는 기존 HUD 버튼의 바인딩을 지정할 수 있습니다. 이 도구는 전환용 Rhai 스크립트를 연결하며 기존 스크립트를 보존합니다.

## MCP 카메라 PNG 캡처

`scene_capture`에 저장된 씬 `path`와 새 PNG의 `output_path`를 넘기면 카메라 화면을 파일로 저장합니다. 두 경로는 MCP 작업 폴더 기준입니다. `camera_id`를 지정해 다른 카메라를 선택하거나 생략해서 메인 카메라를 사용합니다. 크기는 기본 1280×720이고 가로·세로 각각 1~2048픽셀로 지정할 수 있습니다.

```json
{"path":"examples/agent-escape/Beacon Bot Escape.futty.scene.ron","output_path":"escape.png","width":960,"height":540}
```

캡처에는 그래픽 어댑터가 필요합니다. 에디터 창을 열지 않고 저장된 씬의 3D 화면을 렌더링합니다. HUD와 실행 중 스크립트 상태는 포함되지 않습니다. 기존 PNG는 보존됩니다.

## 제작 실패 리포트

검증, 플레이테스트, 캡처, Windows 빌드는 단계와 오류 코드를 담은 `failure_report`를 반환합니다. JSON Schema는 MCP의 `game_failure_schema`에서 조회합니다. [필드와 응답 위치](failure-report.md)

### 프로젝트 통합 검증

`game_review`는 프로젝트의 모든 등록 씬을 검증한 뒤 지정한 씬의 플레이테스트를 실행합니다. 테스트가 통과하면 요청한 PNG를 저장하고 Windows 게임을 패키징합니다.

```json
{
  "path": "My-Power-Maze/2D Power Maze.futty.project.ron",
  "playtest": {
    "path": "My-Power-Maze/Power Maze.futty.scene.ron",
    "steps": [
      {"actions":["forward"],"ticks":82},
      {"actions":["right"],"ticks":82},
      {"actions":["backward"],"ticks":82},
      {"actions":["forward"],"ticks":82}
    ],
    "expect":"success",
    "expect_hud":{"power":1}
  },
  "capture": {"path":"My-Power-Maze/Power Maze.futty.scene.ron","output_path":"Maze.png"},
  "build": {"output_path":"Maze-Windows"}
}
```

`playtest`에는 템플릿 생성 응답의 값을 그대로 사용할 수 있습니다. `capture`와 `build`는 선택 사항이며, 테스트와 캡처 대상은 그 프로젝트에 등록된 씬이어야 합니다. 응답의 `steps`에는 실행한 단계와 통과 여부가 기록됩니다. 실패한 단계에서는 `failure_report`를 반환하고 이후 단계는 실행하지 않습니다. 이미 생성한 리플레이와 PNG는 보존됩니다.

## AI 플레이 세션

Futty MCP의 플레이 도구는 카메라와 캐릭터 컨트롤러가 있는 저장된 씬을 창 없이 실행합니다. 두 컴포넌트는 같은 오브젝트에 둘 수도 있고 나눌 수도 있습니다. 카메라에 컨트롤러가 없으면 씬 목록의 첫 캐릭터를 조작합니다. 다음은 [Beacon Bot Escape](../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron)의 시작 입력입니다.

```json
{"path":"examples/agent-escape/Beacon Bot Escape.futty.scene.ron","max_ticks":600}
```

| 도구 | 입력과 결과 |
| --- | --- |
| `play_start` | 씬 경로와 선택적인 `max_ticks`를 받습니다. 세션 ID와 0틱의 관찰을 반환합니다. |
| `play_observe` | `session_id`의 현재 틱, 플레이어 위치와 방향, HUD 값, 보이는 오브젝트, 상태를 읽습니다. |
| `play_step` | `session_id`, `actions`, 선택적인 `ticks`, `look`, `expected_tick`을 사용합니다. 실행 후 새 관찰을 반환합니다. |
| `play_end` | 세션을 닫고 종료 시점의 틱과 상태를 반환합니다. 씬 파일은 그대로입니다. |

`actions`는 `forward`, `backward`, `left`, `right`, `jump`, `interact`입니다. 빈 목록으로 대기합니다. `ticks`는 호출당 1~120이며 한 틱은 1/60초입니다. 행동이 새로 들어온 틱에만 누름으로 처리됩니다. 한 번 더 점프하거나 상호작용하려면 먼저 해당 행동을 뺀 스텝을 실행합니다. `look`의 첫 값은 좌우, 두 번째는 상하 회전 각도이며 양수는 오른쪽과 위쪽으로 돌립니다.

`visible_objects`는 카메라의 16:9 시야에 들어오고 중심점으로 향하는 레이가 콜라이더에 가려지지 않은 오브젝트를 가까운 순서로 최대 32개 반환합니다. HUD에는 씬에 등록한 텍스트·게이지 바인딩의 현재 값이 들어갑니다.

Rhai의 `finish_game("success")` 또는 `finish_game("failure")`로 플레이 결과를 지정합니다. 제한에 도달하면 `tick_limit`이 반환됩니다. 현재 세션은 단일 씬에서 실행하며, `load_scene()` 요청이 있으면 `scene_transition`과 대상 ID를 반환하고 진행을 멈춥니다. Pulse Range 전용 입력을 쓰는 씬은 에디터나 단독 게임에서 실행합니다.

## 자동 플레이테스트와 리플레이

`play_test`의 `expect`에는 기존처럼 최종 상태를 지정합니다. 여기에 아래 조건을 추가하면 모든 조건이 맞을 때만 `passed`가 참입니다. `checks`에서 조건별 결과를 확인할 수 있습니다.

- `expect_hud`에는 종료 시점의 점수나 체력 값을 지정합니다. 예를 들어 `{"score":3}`은 HUD에 바인딩된 `score` 값을 확인합니다.
- `expect_collisions`의 각 항목에는 두 오브젝트 UUID인 `first`와 `second`를 지정합니다. `sensor: false`는 물리 충돌, `sensor: true`는 트리거 진입을 확인합니다. 관찰된 첫 틱은 `first_tick`에 반환됩니다.
- `expect_scene`에는 예상 전환 대상 ID를 지정합니다. 예를 들어 `"expect":"scene_transition","expect_scene":"results"`는 정확한 전환 요청이 발생했는지 확인합니다. 헤드리스 테스트는 요청 시점에 멈춥니다.

| 도구 | 사용 방법 |
| --- | --- |
| `play_test` | `path`, `steps`, 선택적인 `seed`, `max_ticks`, `expect`를 넣습니다. 단계마다 `actions`, `ticks`, `look`을 사용할 수 있습니다. 기대 상태의 기본값은 `success`입니다. 결과는 `passed`, 실제 상태, 틱, 마지막 관찰입니다. |
| `play_save_replay` | 진행 중이거나 종료된 세션에서 `session_id`와 새 `.json` 경로를 지정합니다. 세션을 닫기 전에 저장합니다. |
| `play_replay` | 기록의 `.json` 경로를 넘기면 같은 입력을 재실행합니다. 일치 여부를 반환하고, 결과가 달라지면 첫 번째 틱과 두 시점의 플레이어 위치·상태를 반환합니다. |

[Beacon Bot Escape 플레이테스트 입력](../examples/agent-escape/playtest.json)을 `play_test`에 넘기면 스위치를 켜고 문을 통과하는 경로를 검사합니다. 테스트에서 바로 리플레이를 남기려면 `"replay_path":"target/escape.replay.json"`을 추가합니다. 저장할 폴더는 이미 있어야 하며 같은 이름의 파일은 보존됩니다.

리플레이는 포맷·엔진 버전, 씬 경로, 씬과 스크립트의 지문, 시드, 행동, 틱별 상태 검증값을 저장합니다. 시드는 플레이 중 생성되는 프리팹 ID에 적용되며, `play_start`에서도 지정할 수 있습니다. 같은 엔진과 실행 환경에서 비교합니다. 씬이나 스크립트가 바뀌면 `source_changed`가, 기록의 상태와 재실행이 처음 어긋나면 `state_changed`가 반환됩니다.

## Futty Ghost 자동 플레이테스트

`ghost_batch_run`은 동일한 저장 씬을 규칙 기반 플레이어와 시드 조합으로 실행합니다. 기본 대상은 `coin` 태그이며, 가까운 가시 대상을 따라가면서 기존 게임의 성공·실패 판정을 기록합니다.

| 플레이어 유형 | 행동 |
| --- | --- |
| `beginner` | 시드에 따라 잠깐 기다린 뒤 대상을 따라갑니다. |
| `explorer` | 시드에 따라 좌우를 먼저 탐색하고, 상호작용 대상이 보이면 시도합니다. |
| `goal_driven` | 바로 대상을 향해 움직입니다. 사이드스크롤 씬에서 앞쪽 바닥이 끊기면 점프를 시도합니다. |

먼저 `game_create_from_template`에 `{"template":"coin_runner_2d","output_path":"Coin-Runner"}`를 전달합니다. 이어서 `ghost_batch_run`을 호출합니다.

```json
{
  "path": "Coin-Runner/Coin Runner.futty.scene.ron",
  "output_path": "Coin-Runner/Ghost",
  "seeds": [11, 22, 33],
  "max_ticks": 360
}
```

결과에 유형별 성공 횟수, 실행별 종료 틱·플레이어 위치·HUD와 리플레이 경로가 포함됩니다. `first_failure_tick`은 게임이 실패를 판정했거나 틱 한도에 도달한 틱을 기록합니다. `first_stall_tick`은 30틱 연속 이동 입력이 들어왔는데 이동 평면에서 시작 위치와의 거리가 0.02 월드 단위에 이르지 못한 구간의 시작입니다. 개별 실행은 반환된 `replay_path`를 `play_replay`의 `path`로 전달해 재현할 수 있습니다.

각 실행은 최대 1,200틱, 한 배치는 최대 12,000틱입니다. 결과 폴더에 `report.json`과 실행별 리플레이를 저장합니다. 기본 이동은 화면에 보이는 태그 대상과 근거리 바닥 검사에 기반하며, 복잡한 미로의 경로 탐색은 별도 정책이 필요합니다.

## Pulse Range

움직이는 드론을 상대로 45초 동안 점수를 쌓는 FPS 연습장입니다. [Pulse Range 씬](../examples/pulse-range/Pulse%20Range.futty.scene.ron)을 열고 **Play**를 누릅니다. 단독 실행과 모델링 원본은 [Pulse Range 문서](../examples/pulse-range/README.md)에 있습니다.

## 게임 HUD

**Window → Game HUD**를 열고 **+ Add widget**에서 Text, Image, Button, Gauge를 추가합니다. 게임 화면에서 배치를 보면서 앵커, 위치, 크기, 색상을 조절합니다. 오른쪽과 아래쪽 앵커에서 양수 오프셋은 화면 안쪽을 향합니다. 겹치는 위젯의 표시 순서는 **Earlier**와 **Later**로 바꿉니다.

- **Text**는 입력한 문구 또는 **Value binding**에 연결한 값을 표시합니다.
- **Image**는 Project에 임포트한 PNG를 사용하고 원본 비율을 유지합니다.
- **Gauge**는 연결된 숫자를 0~1 범위의 막대로 표시합니다.
- **Button**에 **Button action**을 지정하면 Play 중 해당 버튼을 클릭한 프레임에 Rhai의 `hud_pressed("charge")`로 입력을 읽을 수 있습니다. FPS 화면에서 마우스를 사용 중이면 **Esc**로 먼저 풀어 줍니다.

스크립트에서 `set_hud("energy", 0.5)`를 호출하고 Text 또는 Gauge의 **Value binding**을 `energy`로 지정합니다. Pulse Range의 내장 값은 `range.score`, `range.time`, `range.time_fraction`, `range.ammo`, `range.streak`입니다. 연속 명중 표시처럼 값이 있을 때만 나타나는 Text는 Label을 비워 둡니다.

HUD 배치는 씬 파일에 저장되며 Play와 Windows 내보내기에도 적용됩니다.

## 조명

- **Scene 우클릭 → Add Object → Light** 또는 **Hierarchy → + Add → Light**에서 조명을 추가합니다.
- **Directional Light**는 회전 방향으로 씬 전체를 비춥니다. **Point Light**는 배치한 위치에서 주위를 비추고 **Range**에 따라 거리가 멀수록 어두워집니다. Inspector에서 색과 밝기를 변경할 수 있습니다.
- Scene에서 조명을 선택하면 방향이나 범위가 표시됩니다. 메시 조명은 Game과 Windows 게임에도 반영됩니다. 조명을 추가하지 않은 씬은 기본 조명을 사용합니다. 첫 조명 8개가 렌더링에 반영됩니다.
- **Directional Light → Cast Shadows**로 메시 그림자를 켜고 **Shadow Distance**로 카메라에서 그림자가 적용되는 거리를 조절합니다. 기존 씬의 방향광은 토글을 켜서 사용합니다. 켜진 첫 방향광이 그림자를 만들고 다른 조명의 밝기는 그대로 합산됩니다.

## Sprite3D

### 2D 씬 구성하기

에디터에서 **File → New 2D Scene**을 선택하거나, MCP의 `scene_create`에 `{"path":"Main.futty.scene.ron","name":"Main","dimension":"TwoD"}`를 전달합니다. XY 평면을 바라보는 직교 카메라와 단색 배경으로 시작합니다. 기존 씬과 차원을 생략한 새 씬은 `ThreeD`로 열립니다. `scene_inspect`의 `dimension`으로 저장한 설정을 확인할 수 있습니다.

`TwoD` 씬은 Rapier2D로 XY 평면의 충돌을 계산합니다. 오브젝트의 Z는 화면 깊이로 유지되며, Z가 서로 달라도 XY에서 겹치면 충돌합니다. 2D 강체는 박스·원·캡슐을 지원하고, 캐릭터 컨트롤러는 캡슐을 사용합니다. 기존 `ThreeD` 씬은 Rapier3D를 사용합니다.

#### Sprite2D와 직교 카메라 캡처

2D 씬에 PNG를 가져온 후 **Place Sprite**로 배치합니다. **Inspector → Sprite2D**에서 크기, 레이어, 기준점, X/Y 뒤집기와 스프라이트 시트를 조정할 수 있습니다. 높은 레이어는 다른 스프라이트 위에 그려집니다. 기준점 `[0,0]`은 왼쪽 위, `[0.5,0.5]`는 중앙입니다.

```json
{"op":"set_sprite2d","object":"player","texture_id":"<PNG UUID>","size":[1.3,1.3],"layer":2,"pivot":[0.5,0.5],"flip":[true,false]}
```

위 작업을 `scene_apply`에 전달합니다. `scene_capture`는 씬의 직교 카메라에서 저장된 화면을 PNG로 출력합니다. [Planar Collision 예제](../examples/planar-collision/Planar%20Collision.futty.project.ron)를 열고 WASD로 벽을 돌아 초록색 목표에 도달하세요. 플레이어와 벽의 Z가 달라도 정면에서는 충돌합니다. [MCP 플레이테스트 입력](../examples/planar-collision/playtest.json)으로 우회 경로를 재현할 수 있습니다.

화면을 XY 평면으로 쓰려면 카메라를 +Z 방향에 두고 씬을 바라보게 합니다. 에디터에서는 카메라를 **Orthographic**으로 지정합니다. MCP에서는 `add_camera` 또는 `scene_add_camera`에 화면 세로 반높이인 `orthographic_size`를 넘깁니다.

```json
{"op":"add_camera","key":"view","name":"2D Camera","position":[0,2,10],"target":[0,2,0],"orthographic_size":4}
```

플레이어는 별도 오브젝트로 두고 **Physics → Character controller**의 이동 모드를 고릅니다. MCP의 `set_component`에서는 `{"kind":"character","mode":"side_scroller"}`처럼 설정합니다.

- **2D Side-scroller** (`side_scroller`)는 X축 이동과 Y축 중력·점프를 사용합니다.
- **2D Top-down** (`top_down_2d`)은 XY 평면을 이동하며 중력과 점프가 적용되지 않습니다.
- 두 모드에서 이동은 캐릭터의 Z 좌표를 유지합니다. 충돌은 기존 물리 엔진을 사용하므로 벽과 바닥의 충돌체가 캐릭터의 Z 평면과 겹쳐야 합니다.

MCP로 스프라이트를 붙이려면 PNG를 가져온 뒤 `scene_apply`의 `{"op":"set_sprite","object":"player","texture_id":"<png-uuid>","size":[1,1],"billboard":"Fixed"}`를 사용합니다. 저장한 씬은 `play_test`와 `scene_capture`에서 검사할 수 있습니다.

1. **Project → Import…**에서 PNG를 가져온 뒤 에셋을 우클릭해 **Place Sprite**를 누르거나 더블클릭합니다. 스프라이트는 이미지 비율과 투명도를 유지합니다.
2. **Inspector → Object → Sprite3D**에서 텍스처, 너비·높이, 빌보드 모드를 설정합니다. **Face camera**는 카메라 화면을 향하고, **Upright**는 Y축을 세운 채 카메라 쪽으로 돌며, **Fixed**는 오브젝트의 회전을 따릅니다.
3. **File → Sprite Playground**를 열고 우클릭 드래그로 카메라를 돌리면 세 가지 모드를 비교할 수 있습니다.

#### 스프라이트 시트와 카메라 추적

**Inspector → Sprite3D → Sprite sheet**에서 같은 크기의 프레임으로 나뉜 PNG의 열과 행, 시작 칸, 프레임 수, fps, 반복 여부를 지정합니다. 첫 칸은 0이고 왼쪽에서 오른쪽으로, 위에서 아래로 셉니다. **Loop**를 끄면 마지막 프레임에서 멈추고, 프레임을 1개로 지정하면 PNG의 한 칸을 계속 표시합니다.

MCP에서는 스프라이트와 직교 카메라를 만든 뒤 `scene_apply`에 아래 작업을 추가합니다. 카메라 추적은 씬에서 지정한 태그를 가진 첫 번째 대상의 월드 좌표를 사용합니다. 횡스크롤에서 `[true,false]`로 X축만 추적하거나, 탑다운에서 `[true,true]`로 두 축을 추적할 수 있습니다. 대상 태그가 없으면 카메라의 기존 위치를 사용합니다.

```json
[
  {"op":"set_sprite_sheet","object":"player","sheet":{"columns":4,"rows":2,"first_frame":0,"frames":4,"fps":8,"looping":true}},
  {"op":"set_camera_follow","object":"view","tag":"hero","offset":[0,1],"axes":[true,true]}
]
```

`sheet: null`은 전체 PNG를 표시하고, `tag: null`은 카메라 추적을 끕니다. `scene_capture`에는 설정한 시작 칸이 표시됩니다.

#### 상태에 따른 스프라이트 애니메이션

스프라이트 시트를 설정한 뒤 해당 오브젝트의 Rhai에서 `play_sprite(entity, first_frame, frames, fps, looping)`을 호출해 재생 구간을 전환합니다.

```rhai
fn on_update(entity, dt) {
    if input_down("MoveLeft") || input_down("MoveRight") {
        play_sprite(entity, 0, 4, 8, true);
    } else {
        play_sprite(entity, 0, 1, 8, false);
    }
}
```

같은 구간을 반복 요청하면 재생 위치를 유지하고, 구간이나 속도·반복 설정이 바뀌면 첫 칸부터 재생합니다. 반복을 끄면 마지막 칸을 유지합니다. 범위는 등록한 시트 안에 있어야 하며 속도는 1~60 fps입니다. 재생 상태는 Play 세션에 적용되며, MCP 관찰의 `visible_objects[].sprite_frame`에서 현재 칸 번호를 확인할 수 있습니다. Coin Runner 템플릿에서 대기와 이동 전환을 확인할 수 있습니다.

카메라가 레벨 끝에서 멈추게 하려면 `set_camera_follow`에 `"bounds":[[2,0],[18,8]]`처럼 카메라 중심의 최소·최대 XY 좌표를 추가합니다. 오프셋을 더한 뒤 추적 중인 축에만 적용됩니다. 화면 가장자리까지 맵을 채우려면 직교 카메라의 화면 반높이와 종횡비를 고려해 중심의 이동 범위를 정하세요.

#### 픽셀 퍼펙트 카메라

직교 카메라의 **Inspector → Camera → Pixel perfect**를 켜고 **Pixels per unit**을 설정합니다. MCP에서는 같은 설정을 `scene_apply`로 적용합니다.

```json
{"op":"set_camera_pixel_grid","object":"view","pixels_per_unit":32}
```

게임 화면과 `scene_capture`는 실제 출력 픽셀 크기에 맞춰 정수 배율을 선택합니다. 카메라와 Sprite2D의 표시 위치를 격자에 맞추고, Sprite2D 텍스처는 최근접 방식으로 그립니다. 32 px 너비의 스프라이트를 월드 1 단위로 배치할 때는 32를 지정합니다. 적용 중 확대 배율은 최소 1배이며, 창 크기가 변하면 표시 영역이 계단식으로 바뀔 수 있습니다. MCP에서는 `pixels_per_unit: null`로 끕니다.

#### 타일맵과 플랫폼 점프

격자 스프라이트 PNG를 `asset_import`로 등록한 뒤 `scene_apply`에서 `paint_tilemap2d`로 배치합니다. 아래 예제에서 아틀라스의 0번 칸은 바닥, 1번 칸은 장식입니다.

```json
{
  "op":"paint_tilemap2d",
  "map":{
    "texture_id":"<png-uuid>",
    "atlas":[2,1],
    "rows":[[null,1,null,null],[0,0,null,0]],
    "solid":[0],
    "origin":[0,0,1],
    "tile_size":1
  }
}
```

행은 위에서 아래로, 열은 왼쪽에서 오른쪽으로 읽습니다. `origin`은 격자의 왼쪽 위 칸 중심이고, `null`은 빈 칸입니다. 한 번에 최대 64×64 격자에서 1,024개 타일을 생성합니다. 타일은 각각 에디터에서 옮기고 수정할 수 있는 오브젝트로 저장됩니다. 여러 레이어는 Z가 다른 원점으로 배치할 수 있습니다.

**Inspector → Physics → Character controller → 2D Side-scroller**에서 **Ledge grace**는 발판을 벗어난 직후 점프를 허용하고, **Jump buffer**는 착지 직전 누른 점프를 착지 후 실행합니다. 두 값의 단위는 초이며, 0은 해당 보정을 끕니다. MCP에서는 `set_component`에 `{"kind":"character","mode":"side_scroller","coyote_time":0.10,"jump_buffer":0.12}`를 사용합니다.

#### 시드 기반 탑다운 미로

`scene_apply`의 `generate_maze2d`는 XY 평면에 방과 통로를 연결하고, 벽 충돌체와 탑다운 플레이어, 목표 지점을 배치합니다. 빈 구역에서 실행하세요. `scene_create`로 만든 씬의 기본 큐브는 `remove_leaf`로 지울 수 있습니다.

```json
[
  {"op":"generate_maze2d","start_key":"player","goal_key":"exit",
   "layout":{"seed":0,"rooms":[2,2],"cell_size":3,"origin":[0,0,1]}},
  {"op":"add_camera","key":"view","name":"Maze Camera",
   "position":[3,3,16],"target":[3,3,1],"orthographic_size":8},
  {"op":"set_script","object":"exit","name":"Reach exit",
   "source":"fn on_update(entity, dt) { if !find_nearby(entity, \"maze_player\", 0.5).is_empty() { finish_game(\"success\"); } }"}
]
```

- `rooms`는 X·Y 방향 각각 2~8개입니다. `cell_size`는 3~12 월드 단위이며 기본값은 3입니다. `origin`은 플레이어 시작점이며 기본값은 `[0,0,0]`입니다.
- 같은 시드와 설정은 같은 배치를 만듭니다. 생성한 벽과 마커는 에디터에서 수정할 수 있는 일반 오브젝트입니다. 플레이어와 목표의 UUID는 응답의 `objects`에서 각 키로 조회합니다.
- 예제에서는 목표에서 0.5 월드 단위 안으로 들어오면 승리합니다. `play_test`에서 `forward` 82틱과 `right` 82틱을 차례로 실행하고 `"expect":"success"`로 확인할 수 있습니다.

### 로컬 2인 협동

1. 씬에 캐릭터 컨트롤러를 가진 오브젝트를 두 개 이상 배치하고 주 카메라가 두 캐릭터를 담도록 구성합니다. 탑다운 게임에는 직교 카메라와 `top_down_2d` 이동 모드를 사용합니다.
2. 에디터의 **Window → Game Controls** 또는 **F10**에서 **Two-player local co-op**을 켜고 씬을 저장합니다. MCP에서는 `scene_apply`에 `{"op":"set_local_coop","enabled":true}`를 넣습니다.
3. **Play**를 누르거나 저장한 프로젝트를 Windows 게임으로 내보내 실행합니다. 두 플레이어는 주 카메라 화면을 공유합니다.

주 카메라에 캐릭터 컨트롤러가 붙어 있으면 그 캐릭터를 P1로 씁니다. 별도 카메라를 쓰면 씬 목록의 첫 캐릭터가 P1이고, P1을 제외한 첫 캐릭터가 P2입니다. `scene_inspect`의 `local_player_two`에서 P2 UUID를 확인할 수 있습니다.

| 동작 | P1 기본 키 | P2 키 |
| --- | --- | --- |
| 이동 | WASD | 방향키 |
| 점프 | Space | K |
| 발사 / 재장전 | 마우스 왼쪽 / R | L / O |
| 상호작용 | E | P |
| 재시도 | Enter | J |

P1은 **Game Controls**의 키 설정을 사용합니다. 같은 키가 두 플레이어에 배정되면 협동 중에는 P2가 사용합니다. 점프·발사·재시도 같은 동작은 씬의 이동 모드와 스크립트에 따라 작동합니다.

- 2D 캐릭터는 범위 안에서 장애물에 가리지 않은 가장 가까운 대상과 상호작용합니다. 안내에는 P1 또는 P2가 표시됩니다. 대상의 `on_interact(entity, actor)`에서 `actor == find_tag("player_two")` 같은 조건으로 플레이어별 규칙을 적용할 수 있습니다.
- 각 캐릭터 스크립트에서 `input_pressed()`를 사용해 해당 플레이어 입력을 읽습니다. 두 캐릭터에 체력을 추가하고 **Gauge**에 `player.health`와 `player2.health`를 지정하면 체력바를 따로 표시할 수 있습니다.
- MCP의 `play_step`과 `play_test`에서는 `players` 목록에 캐릭터 UUID와 동작을 각각 넘겨 협동 규칙을 검증합니다.

## 물리

1. **File → Physics Playground**를 선택하고 **Play**를 누릅니다. 큐브가 떨어져 바닥과 경사로에 부딪힙니다. 탭 우클릭 **→ Panels → Physics**에서 충돌과 트리거 이벤트를 확인할 수 있습니다.
2. 직접 구성할 때는 씬 루트에서 오브젝트를 선택하고 **Inspector → Physics → Rigid body**를 켭니다. 떨어지는 큐브에는 **Dynamic**, 바닥에는 **Fixed**를 지정합니다.
3. **Fit to mesh**로 상자 충돌체를 모델에 맞추거나 **Local size / Local center**를 조절합니다. **Trigger sensor**는 물체의 통과를 허용하면서 진입·이탈을 기록합니다.

물리는 Play 중 60 Hz로 실행됩니다. **Stop**을 누르면 편집 씬의 위치와 회전으로 돌아갑니다.

- 씬 루트의 카메라나 오브젝트에 **Physics → Character controller**를 켜면 캡슐로 벽과 바닥에 충돌하며 이동합니다. **WASD**로 이동하고 **Space**로 점프합니다. `Origin above feet`는 바닥에서 오브젝트 원점까지의 높이입니다. 같은 오브젝트에는 강체와 캐릭터 컨트롤러 중 하나를 사용합니다.
- 강체의 **Collision layer**는 1–32이며 **Collision mask**의 최하위 비트가 레이어 1입니다. `FFFFFFFF`는 모든 레이어를 선택합니다. 캐릭터는 마스크에 포함된 고체 충돌체를 통과할 수 없습니다. Pulse Range의 발사는 가장 가까운 고체에서 막힙니다.

### 2D 강체 충돌체 모양

2D 씬에서 오브젝트의 **Inspector → Physics → Rigid body**를 켜고 **Box**, **Circle**, **Capsule** 중 하나를 선택합니다. 씬 뷰에서 해당 오브젝트를 선택하면 실제 충돌 윤곽을 확인할 수 있습니다. 모양은 센서와 레이캐스트에도 적용됩니다.

```json
{"op":"set_component","object":"pickup","component":{"kind":"body","motion":"Fixed","shape":"capsule","size":[1,2,1],"sensor":true}}
```

원의 지름은 `size.x`와 오브젝트의 X 스케일로 정합니다. 캡슐은 로컬 Y축을 따라 서고, 스케일을 적용한 폭과 전체 높이 중 작은 값이 지름이 됩니다. 기존 파일과 `shape`를 생략한 새 강체는 박스로 저장·로드됩니다.

### 2D 이동 발판

1. 2D 씬의 발판에 **Kinematic** 강체를 추가하고 **Trigger sensor**와 **Rotate in Play**를 끕니다.
2. 발판 스크립트의 `on_fixed_update(entity, dt)`에서 `set_position`으로 발판의 X·Y 좌표를 이동시킵니다.
3. **Side-scroller** 캐릭터의 충돌 마스크에 발판 레이어를 포함합니다. 발판 위에서 쉴 때나 걸을 때는 발판의 위치 이동을 따라가며, 점프하거나 가장자리로 걸어 나가면 발판에서 벗어납니다.

[Moving Platform Lab](../examples/moving-platform/Moving%20Platform%20Lab.futty.project.ron)을 **Project → Open Project**에서 열고 **Play**를 누릅니다. 파란 발판이 초록 착지대에 가까워질 때까지 기다렸다가 **D + Space**로 건너갑니다.

MCP의 `play_test`로 같은 경로를 검증할 수 있습니다.

```json
{
  "path": "examples/moving-platform/Moving Platform Lab.futty.scene.ron",
  "steps": [{"ticks": 106}, {"actions": ["right", "jump"], "ticks": 36}, {"ticks": 38}],
  "expect": "success"
}
```

첫 스텝부터 `right`를 86틱 누르면 추락하며 `expect: "failure"`로 검증할 수 있습니다.

## Rhai 스크립트

### 체력과 피해

에디터의 **Physics → Health**에서 최대 체력을 설정하거나, MCP의 `scene_apply`에서 체력 컴포넌트를 추가합니다. 플레이를 시작하면 최대 체력으로 시작합니다.

```json
{"op":"set_component","object":"player","component":{"kind":"health","max":100}}
```

- `damage(target, amount)`와 `heal(target, amount)`는 양수로 호출합니다. 체력은 0과 최대 체력 사이에 머무릅니다.
- `get_health(target)`는 `current`, `max`, `fraction`, `alive`를 반환합니다. 체력 컴포넌트가 없으면 `()`입니다.
- 피격·회복 시 대상의 `on_damage(entity, source, amount)`, `on_heal(entity, source, amount)`가 호출됩니다. `amount`는 실제로 깎이거나 회복된 양입니다. 체력이 0이 되는 순간 `on_death(entity, source)`가 한 번 호출됩니다.
- 쓰러진 캐릭터는 이동과 상호작용을 멈춥니다. 연출은 `on_death`에서 제어하고, 부활은 살아 있을 때 저장한 체크포인트를 복원합니다. 일반 회복은 쓰러진 캐릭터에 적용되지 않습니다.

```rhai
fn on_update(entity, dt) {
    let health = get_health(entity);
    set_hud("player.health", health.fraction);
}
```

Gauge의 바인딩을 `player.health`로 지정하면 체력바를 그릴 수 있습니다. MCP 플레이 관찰의 `player.health`, `players`, `visible_objects`에서도 현재·최대 체력을 확인할 수 있습니다.

### 게임 상태와 체크포인트

`game_set("score", 7)`로 플레이 세션에 숫자·문자열·불리언을 저장하고, `game_get("score")`로 읽습니다. 없는 키는 `()`을 반환하고, `game_set("score", ())`는 키를 지웁니다. 에디터에서 Play 중이거나 단독 게임에서 프로젝트 씬을 전환하면 이 값들은 다음 씬의 `on_start`부터 읽을 수 있습니다. 값 설정과 `load_scene()`을 같은 콜백에서 호출할 경우 값부터 설정합니다.

캐릭터나 상호작용 스크립트에서 `save_checkpoint(actor)`를 호출하면 해당 캐릭터의 위치·회전·체력과 현재 게임 상태·HUD 값을 저장합니다. 같은 씬에서 `restore_checkpoint(actor)`를 호출하면 저장한 캐릭터 상태와 값이 돌아오고 낙하 속도가 초기화됩니다. 저장한 체크포인트가 없는 캐릭터는 그대로 유지됩니다.

게임 상태는 같은 플레이 세션에서 공유됩니다. 체크포인트를 복원하면 공유 값과 HUD가 저장 당시로 돌아갑니다. 체크포인트는 현재 씬에서만 유지되며 디스크 저장, 다른 오브젝트의 자세, 스크립트의 `this` 값과 타이머는 포함하지 않습니다. MCP 플레이 관찰에서는 `game_state`를 확인할 수 있습니다.

### 타이머와 쿨다운

타이머는 오브젝트별로 저장되고, 시뮬레이션이 진행될 때 고정 틱 기준으로 흐릅니다. 초당 60틱입니다.

| Rhai 함수 | 동작 |
| --- | --- |
| `set_timer("round", 180)` | 3초짜리 타이머를 시작하거나 다시 설정 |
| `timer_remaining("round")` | 남은 틱 수 반환. 타이머가 없거나 만료됐다면 0 |
| `cancel_timer("round")` | 타이머 취소 |
| `try_cooldown("dash", 30)` | 쿨다운이 준비됐다면 30틱을 설정하고 `true` 반환. 아직 진행 중이면 `false` |

시간 제한은 `on_fixed_update`에서 남은 틱을 확인하고, 입력에 반응하는 재사용 대기는 `if try_cooldown("dash", 30) { ... }`처럼 사용합니다. 씬을 다시 시작하면 타이머가 초기화됩니다.

### 오브젝트 상호작용

`scene_apply`에서 `interaction` 컴포넌트를 추가하고 대상에 `on_interact(entity, actor)`를 구현한 Rhai 스크립트를 붙입니다. `actor`는 입력을 보낸 캐릭터입니다.

```json
{"op":"set_component","object":"door","component":{"kind":"interaction","range":2.0,"prompt":"Open door"}}
```

거리 안에서 대상을 바라보면 안내 문구가 표시됩니다. 기본 상호작용 키는 **E**이며, 표준 캐릭터 씬에서는 **마우스 오른쪽 버튼을 누른 채** 시점을 돌릴 수 있습니다. 시야를 가리는 충돌체가 있으면 대상은 선택되지 않습니다.

### 여러 플레이어 입력 테스트

`play_observe`의 `players`에서 캐릭터 UUID와 위치, 시선, 상호작용 대상을 확인합니다. 같은 틱에 서로 다른 입력을 보내려면 `play_step` 또는 `play_test`의 스텝에 최대 네 명의 `players`를 지정합니다.

```json
{"ticks":3,"players":[{"player_id":"<player-a-uuid>","actions":["interact"]},{"player_id":"<player-b-uuid>","actions":["forward"],"look":[15,0]}]}
```

플레이어별 입력을 사용할 때는 스텝의 최상위 `actions`와 `look`을 생략합니다. 입력을 생략한 캐릭터는 대기합니다. 이 방식은 한 시뮬레이션 안에서 여러 캐릭터를 검증하는 기능입니다.

### 범위 감지와 활성화

`find_nearby(entity, "coin", 2.0)`은 기준 오브젝트에서 반경 2m 안의 활성 오브젝트를 가까운 순서로 최대 128개 반환합니다. 지정한 태그와 월드 좌표를 기준으로 검색하고, 기준 오브젝트는 결과에서 제외합니다. 위치는 프레임 시작 시점의 상태를 사용합니다.

`set_active(target, false)`는 다음 런타임 명령 처리에서 대상의 렌더링, 충돌, 스크립트 실행을 끕니다. `set_active(target, true)`는 보관한 대상 참조로 다시 활성화합니다. 비활성화 동안에도 UUID와 스크립트 상태는 유지됩니다. 대상 하나에만 적용되며 씬을 다시 열면 저장된 상태로 시작합니다.

1. 오브젝트를 선택하고 **Inspector → Script → New script**를 누르거나 Project 빈 공간을 우클릭해 **New Script**를 누릅니다.
2. Project의 스크립트를 더블클릭하거나 우클릭해 **Edit Code in VS Code**를 누르면 `.rhai` 파일이 열립니다. VS Code에서 **Ctrl+S**를 누르면 futty에 반영됩니다. 실행 중이었다면 **Stop → Play**로 새 코드를 실행합니다. VS Code가 없으면 내장 편집기 탭이 열립니다. 우클릭 메뉴의 **Edit in futty**로도 내장 편집기를 열 수 있습니다.
3. **File → Script Playground → Play**에서는 **W/A/S/D**로 이동하고 **Space**로 올라갑니다. 탭 우클릭 **→ Panels → Console**에서 로그와 오류를 확인합니다. [Move.rhai](../examples/script-playground/Move.rhai)

```rhai
fn fields() {
    #{ speed: 3.0 }
}

fn on_update(entity, dt) {
    if input_down("MoveRight") {
        translate(entity, param("speed") * dt, 0, 0);
    }
}
```

- `fields()`의 숫자·불리언·텍스트는 Inspector에서 오브젝트별로 조절합니다. `param("speed")`로 값을 읽고, `this.counter` 같은 상태는 오브젝트마다 이번 Play 동안 유지합니다.
- **Window → Game Controls** 또는 **F10**에서 키·마우스 버튼과 감도를 바꿀 수 있습니다. 단독 실행에서도 **F10**으로 열 수 있습니다. 설정은 에디터와 단독 실행 앱의 사용자 설정에 각각 저장됩니다.
- Rhai에서 `input_down("MoveForward")`, `input_pressed("Fire")`, `input_released("Fire")`로 입력을 읽습니다. `MoveForward`, `MoveBackward`, `MoveLeft`, `MoveRight`, `Jump`, `Fire`, `Reload`, `Interact`, `Retry`를 지원합니다. 직접 키를 읽는 `key_down("Space")`와 감도를 반영한 `mouse_delta_x()`·`mouse_delta_y()`도 사용할 수 있습니다.
- `key_pressed("3")`로 숫자 키를 짧게 누른 프레임을 읽고, `mouse_wheel_y()`로 해당 프레임의 휠 이동량을 읽을 수 있습니다.
- **Inspector → Tags**에 `door` 같은 태그를 입력합니다. `find_tag("door")`는 첫 번째 오브젝트를, `find_all_tagged("door")`는 해당 태그의 오브젝트 목록을 반환합니다. 대상이 없으면 각각 `()`와 빈 배열을 반환합니다.
- `send_event(target, "open")`은 대상 스크립트의 `on_message(entity, sender, name)`을 다음 틱에서 호출합니다. 예를 들어 `input_pressed("Interact")`일 때 스위치가 문에 `open` 이벤트를 보낼 수 있습니다.
- `spawn_prefab("Crate", 2, 0, 0)`은 다음 틱에서 프리팹 계층을 생성하며 루트 핸들을 반환합니다. `despawn(root)`으로 생성한 계층 전체를 제거합니다. Play 중 생성·제거는 저장한 편집 씬과 분리되어 실행됩니다.
- 좌표는 부모 기준입니다. `get_position`, `set_position`, `translate`, `translate_local`, `rotate`, `rotate_y`를 사용할 수 있습니다. 이동·회전은 강체 없는 오브젝트와 Kinematic 강체에서, 크기 변경 `set_scale`은 강체 없는 오브젝트에서 지원합니다. Dynamic 강체는 월드 기준 `set_velocity`와 `add_impulse`로 조종합니다. 회전 단위는 도입니다.
- 라이프사이클은 `on_start(entity)`, `on_update(entity, dt)`, `on_fixed_update(entity, dt)`, `on_collision_enter/exit(entity, other)`, `on_trigger_enter/exit(entity, other)`를 지원합니다. 스크립트는 자신이 붙은 오브젝트를 조종합니다. 오류가 난 인스턴스는 이번 Play에서 중지합니다.
- 편집기에서 적용한 코드는 **Save Scene**으로 씬 파일에 저장합니다. 기존 `.rhai`는 **Project → Import…**로 가져온 뒤 에셋을 우클릭해 **Attach to Selection**으로 연결합니다. 가져온 스크립트를 편집기로 수정하면 씬에 편집한 소스가 저장됩니다.

## 프로젝트와 씬 전환

1. **File → Project → New 2D Project…** 또는 **New 3D Project…**를 선택하고 빈 폴더를 지정합니다. Futty가 해당 차원의 시작 씬 `Main.futty.scene.ron`과 프로젝트 파일을 만듭니다. 이미 만든 씬으로 시작하려면 **New Project from Current Scene…**을 사용합니다.
2. **Add Scene…**으로 프로젝트 폴더 안의 다른 씬을 등록합니다. **Start Scene**에서 프로젝트를 열거나 단독으로 실행할 때 처음 보여 줄 씬을 고릅니다. **Edit Scene**에서 편집할 파일을 전환합니다.
3. 게임에서는 Rhai의 `load_scene("Pulse Range")`처럼 등록된 씬 ID를 넘겨 전환합니다. 에디터에서 등록할 때 ID는 씬 파일 이름의 확장자를 뺀 값입니다. 전환 대상의 카메라와 HUD, 스크립트를 새로 초기화합니다.

프로젝트가 열려 있으면 씬을 프로젝트 폴더 안에 저장합니다. **Import…**로 가져온 이미지, 모델, 오디오, Rhai 파일은 씬을 저장할 때 그 씬 옆의 `Assets/Futty/`에 복사됩니다. 에디터에서 생성한 Rhai 스크립트는 씬 파일에 함께 저장됩니다. 프로젝트를 옮기거나 공유할 때는 선택한 프로젝트 폴더 전체를 복사합니다.

에디터의 **Play**는 현재 편집 중인 씬에서 시작합니다. **Stop**을 누르면 편집 중이던 씬으로 돌아갑니다. **Remove Scene**은 프로젝트 목록에서 항목을 빼고 씬 파일은 보존합니다. 시작 씬을 제거하려면 먼저 다른 씬을 시작으로 지정합니다.

[Pulse Range 프로젝트](../examples/pulse-range/Pulse%20Range.futty.project.ron)를 **File → Project → Open Project…**에서 열고 **Play → Enter practice range**를 눌러 로비에서 연습장으로 전환할 수 있습니다.

## Futty Live Surgery

저장된 씬에서 선택한 오브젝트의 색상과 동작 스크립트를 한 번에 수정합니다. `surgery_preview`는 파일을 쓰기 전에 바뀔 필드와 영향받는 UUID를 반환합니다. 색상과 스크립트 중 하나만 보낼 수도 있습니다.

```json
{
  "path": "Main.futty.scene.ron",
  "object_id": "<문 오브젝트 UUID>",
  "color": [0.9, 0.1, 0.08],
  "script": {"name": "Exit door", "source": "fn on_start(entity) { this.opened = false; } fn on_message(entity, sender, name) { if name == \"open\" && !this.opened { this.opened = true; translate(entity, 0.0, 3.2, 0.0); } }"}
}
```

교체 스크립트는 Rhai 컴파일 검사를 거쳐 새 에셋으로 선택한 오브젝트에만 연결됩니다. 기존 스크립트와 다른 오브젝트의 바인딩은 유지됩니다. 교체할 오브젝트의 스크립트 필드값이 초기화되면 응답에 `clears_script_fields`가 표시됩니다. 프리팹까지 바뀌는 요청은 여러 오브젝트의 영향을 볼 수 있는 `scene_preview_ops`로 검토하세요.

같은 세션에서 `surgery_apply`에 `path`와 최신 `preview_token`을 보내 저장합니다. 미리보기 이후 저장 파일이 바뀌면 다시 미리보기해야 합니다. 다음 Play에서 동작을 확인하거나 `play_test`로 회귀 검증할 수 있습니다. `surgery_rollback`에 같은 경로와 응답의 `undo_token`을 보내면 현재 파일이 적용 당시와 일치할 때 마지막 변경을 되돌립니다.

### Play 중 색상 다시 불러오기

에디터에서 저장된 씬의 오브젝트를 선택하고 **Play**를 시작합니다. 다른 MCP 세션에서 `surgery_apply`로 해당 오브젝트 색상을 저장한 뒤, 에디터에서 **Window → Reload Selected Tint in Play**를 누르면 현재 플레이를 유지하면서 새 색상을 볼 수 있습니다. 편집 씬에도 디스크 변경을 읽어 오려면 Play를 종료하고 씬을 다시 엽니다.

MCP에서는 `play_start`로 헤드리스 세션을 열고 `surgery_hot_reload`에 마지막 관찰의 `session_id`, `expected_tick`과 `tint: {"object_id":"<UUID>","color":[0.9,0.1,0.08]}`를 보냅니다. 다음 `play_step`이 첫 틱 직전에 색을 바꿉니다. 그 사이 씬 파일과 현재 플레이 상태는 그대로이며, 적용된 색상은 저장한 리플레이에서 재현됩니다.

## Futty Feel 캐릭터 조절

에디터에서 캐릭터를 선택하고 **Window → Futty Feel**을 엽니다. 이동 속도와 점프 속도를 슬라이더나 숫자 입력으로 조절하거나 **Steady**, **Default**, **Agile** 프리셋에서 시작할 수 있습니다. 탑다운 캐릭터는 이동 속도를 조절합니다.

- **Preview in Play**는 초안 값을 사용해 Play를 시작합니다. **Stop Play**로 나오면 편집 씬은 그대로이며, 초안을 다시 조절할 수 있습니다.
- **Apply**로 편집 씬에 반영하고 **Save Scene**으로 디스크에 저장합니다. **Reset draft**로 초안의 원래 값을 복원하고, 반영한 변경은 **Undo**로 되돌릴 수 있습니다.
- MCP에서는 씬 `path`와 캐릭터 `object_id`로 `feel_get`을 호출합니다. `feel_preview`에 같은 대상과 `speed` 또는 `jump_speed`를 보내면 저장 없이 변경 내용을 확인할 수 있습니다. `feel_apply`에 확인한 `expected_revision`을 함께 보내 저장하고, 응답의 `undo_token`은 `scene_undo`에 사용합니다. 씬을 먼저 저장하고 MCP 변경 뒤 에디터에서 다시 여세요.

## 실행 중 자동 그래픽 품질

에디터의 **Play**와 Windows로 내보낸 게임에서 기본으로 작동합니다. 느린 프레임이 지속되면 게임 화면의 내부 렌더 해상도를 100%에서 85%, 70%, 55% 순으로 낮춥니다. 첫 단계에서는 방향광 그림자 거리를 줄이고, 더 낮은 단계에서는 그림자 렌더링을 생략합니다. 프레임에 여유가 수 초간 이어지면 한 단계씩 원래 품질로 돌아갑니다.

픽셀 퍼펙트 카메라는 픽셀 정렬을 유지하도록 원래 해상도를 사용합니다. HUD는 창의 해상도로 그리며, 물리와 게임 규칙은 기존 타이밍으로 실행합니다.

## Windows 게임 내보내기

**File → Export Windows Game…**에서 저장한 씬의 출력 위치를 선택합니다. 만들어진 `*-Windows` 폴더는 씬, 참조 에셋, 실행 설정과 `Game.exe`를 포함합니다. 폴더 전체를 옮긴 뒤 `Game.exe`를 실행하면 에디터 없이 게임이 열립니다. 씬에 카메라가 없으면 내보내기 당시의 Scene 카메라로 실행합니다.

프로젝트를 열어 둔 상태에서는 **Export Windows Project…**로 등록한 모든 씬과 참조 에셋을 묶습니다. `Game.exe`는 시작 씬에서 열립니다.

MCP에서는 `project_build`를 사용합니다. 에디터를 열지 않고 프로젝트에 등록된 씬과 그 참조 에셋을 새 폴더에 묶습니다. 결과는 `output_path`, `executable`, `manifest`를 반환합니다.

```json
{"path":"examples/pulse-range/Pulse Range.futty.project.ron","output_path":"Pulse-Range-Windows"}
```

입력과 출력은 MCP 작업 폴더 기준입니다. EXE에 포함된 MCP는 그 EXE를 기본 플레이어로 사용하고, 별도로 실행한 MCP 서버는 작업 폴더의 `dist/futty-editor.exe`를 사용합니다. 다른 Windows x64 Futty 빌드를 사용하려면 `template_path`를 넘깁니다. 출력은 아직 없는 폴더여야 합니다. 완성된 Windows 빌드는 폴더 전체를 옮긴 다음 `Game.exe`를 실행하세요.

Windows x64에서 CLI로도 실행할 수 있습니다. Futty는 새 콘솔 창을 열지 않습니다. PowerShell에서 `| Out-Host`를 붙이면 내보내기가 끝날 때까지 기다리고 결과를 현재 터미널에서 확인할 수 있습니다. 출력은 아직 없는 폴더를 지정합니다. 씬에 카메라가 없으면 CLI는 기본 카메라를 사용합니다.

```powershell
.\futty-editor.exe --export .\Main.futty.scene.ron .\Main-Windows | Out-Host
.\futty-editor.exe --export ".\examples\pulse-range\Pulse Range.futty.project.ron" .\Pulse-Range-Windows | Out-Host
```

### 서명된 Windows 릴리스 준비

공개 배포에는 공개적으로 신뢰되는 코드 서명 인증서와 개인 키에 접근할 수 있는 서명 도구가 필요합니다. Windows SDK의 SignTool이 사용할 수 있는 인증서가 있다면 릴리스 모드로 빌드한 뒤 아래 명령을 실행합니다. `ExpectedPublisher`에는 인증서에 표시될 게시자 이름을 입력합니다. [Microsoft 코드 서명 옵션](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options) · [SignTool 사용법](https://learn.microsoft.com/windows/win32/seccrypto/signtool)

```powershell
cargo build --locked --release -p futty-editor
.\scripts\prepare-windows-release.ps1 `
    -Executable .\target\release\futty-editor.exe `
    -OutputDirectory .\target\Futty-Windows-Release `
    -ExpectedPublisher '인증서에 표시된 게시자 이름' `
    -CertificateThumbprint '인증서 지문 40자리'
```

- `signtool.exe`가 PATH에 없으면 `-SignToolPath`로 Windows SDK의 실행 파일 경로를 지정합니다. 인증서 발급자가 별도 RFC 3161 타임스탬프 주소를 제공하면 `-TimestampUrl`로 지정합니다.
- 인증서 발급자의 도구나 클라우드 서비스로 이미 서명했다면 위 명령에서 `-CertificateThumbprint`를 생략하고 서명된 EXE를 넘깁니다.
- 스크립트는 서명된 복사본에 포함된 Authenticode 서명, 게시자 이름과 타임스탬프를 확인합니다. 검증을 통과한 `futty-editor.exe`와 `SHA256SUMS.txt`를 GitHub Releases의 같은 버전에 첨부합니다.
- 서명된 에디터에서 게임을 내보내면 같은 실행 파일이 `Game.exe`로 복사되어 서명이 유지됩니다. MCP로 내보낼 때는 서명된 EXE의 작업 폴더 상대 경로를 `template_path`에 지정합니다.

새로 서명된 파일도 게시자나 파일의 평판이 쌓일 때까지 SmartScreen 경고가 표시될 수 있습니다. [Microsoft SmartScreen 안내](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)

## 검증

```powershell
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

## 오디오

- **File → Audio Playground → Play**를 누르면 큐브가 트리거에 들어갈 때 차임이 한 번 재생됩니다.
- WAV 또는 OGG Vorbis 클립을 **Project → Import…**로 가져옵니다. 에셋을 우클릭해 **Preview Audio**로 미리 듣고 **Attach to Selection**으로 선택한 오브젝트에 연결합니다.
- **Inspector → Audio**에서 볼륨, 반복, 시작 조건을 편집합니다. **On Play**는 재생을 시작할 때, **On trigger enter**는 연결된 오브젝트가 트리거 진입 이벤트에 참여할 때 재생합니다. 씬에 오디오 리스너를 하나 추가해 전체 볼륨을 설정할 수 있습니다. **Stop**은 Play 중 재생한 사운드를 정지합니다.

GLB는 삼각형 메시와 내장 텍스처를 사용합니다. 재질은 불투명 기본색 기준으로 렌더링합니다. 오디오 거리 감쇠와 공간 팬닝은 후속 범위입니다.
