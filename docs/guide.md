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

## Codex에서 Futty MCP 사용하기

GitHub에서 저장소를 클론하거나 **Code → Download ZIP**으로 받아 압축을 풉니다. Codex에서 Futty 폴더를 열고 **“Futty MCP 연결해줘”**라고 입력하면 저장소의 `AGENTS.md`에 따라 설치 스크립트를 실행합니다.

Windows에서 직접 연결하려면 저장소 루트의 PowerShell에서 같은 스크립트를 실행합니다. Codex CLI, rustup, Visual Studio Build Tools의 **Desktop development with C++** 워크로드가 필요합니다. 프로젝트의 Rust 버전은 `rust-toolchain.toml`에 지정되어 있습니다.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/connect-mcp.ps1
```

스크립트는 서버를 빌드하고 현재 폴더를 작업 경로로 지정해 Codex의 `futty` 등록을 갱신합니다. 열려 있는 세션에서 도구가 보이지 않으면 새 채팅을 시작하거나 Codex를 다시 실행합니다.

- `scene_create`와 `scene_inspect`로 씬을 만들고 배경, 오브젝트 수, 카메라 UUID를 확인합니다.
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

## MCP 에셋과 재질

| 도구 | 사용 방법 |
| --- | --- |
| `asset_import` | 저장된 씬의 `path`와 작업 폴더 안의 파일 `source`를 넘깁니다. GLB, PNG, WAV, Ogg를 검사한 뒤 씬 옆의 `Assets/Futty/`에 복사하고 등록된 UUID를 반환합니다. |
| `asset_list` | `path`의 등록된 에셋과 공유 재질을 조회합니다. |
| `asset_reimport` | `path`, `asset_id`, 같은 종류의 새 `source`를 지정합니다. 재질 및 오브젝트 참조에 쓰이는 UUID는 그대로 유지됩니다. GLB는 지정한 씬과 그 프리팹에서 참조하는 애니메이션 클립을 확인합니다. |
| `material_create` / `material_update` | `path`와 `settings`를 넘기고 갱신 시에는 `material_id`도 지정합니다. 설정은 `name`, `color`, `texture`, `roughness`, `emission`, `emission_strength`입니다. 생략한 필드는 유지되며 `texture: null`은 텍스처 연결을 해제합니다. |

`scene_apply`의 `attach_asset`으로 모델, 텍스처, 오디오를 오브젝트에 붙입니다. 공유 재질은 `{"op":"set_material","object":"floor","material_id":"<uuid>"}`로 할당합니다. `object`에는 기존 오브젝트 UUID 또는 현재 배치의 키를 쓸 수 있으며, `material_id: null`로 할당을 해제합니다.

별도 도구로 등록하거나 교체한 에셋 파일과 재질 설정은 `scene_undo`의 범위 밖입니다. `scene_apply`에서 한 할당은 배치와 함께 되돌릴 수 있습니다.

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
- `project_set_start_scene`에는 프로젝트의 `path`와 새 시작 씬 ID인 `start`를 넘깁니다. 이미 등록되어 있고 불러올 수 있는 씬을 선택해야 합니다.

## AI 플레이 세션

Futty MCP의 플레이 도구는 저장된 씬에서 창 없이 게임을 실행합니다. 메인 카메라에 캐릭터 컨트롤러가 있어야 합니다. 다음은 [Beacon Bot Escape](../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron)의 시작 입력입니다.

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

| 도구 | 사용 방법 |
| --- | --- |
| `play_test` | `path`, `steps`, 선택적인 `seed`, `max_ticks`, `expect`를 넣습니다. 단계마다 `actions`, `ticks`, `look`을 사용할 수 있습니다. 기대 상태의 기본값은 `success`입니다. 결과는 `passed`, 실제 상태, 틱, 마지막 관찰입니다. |
| `play_save_replay` | 진행 중이거나 종료된 세션에서 `session_id`와 새 `.json` 경로를 지정합니다. 세션을 닫기 전에 저장합니다. |
| `play_replay` | 기록의 `.json` 경로를 넘기면 같은 입력을 재실행합니다. 일치 여부를 반환하고, 결과가 달라지면 첫 번째 틱과 두 시점의 플레이어 위치·상태를 반환합니다. |

[Beacon Bot Escape 플레이테스트 입력](../examples/agent-escape/playtest.json)을 `play_test`에 넘기면 스위치를 켜고 문을 통과하는 경로를 검사합니다. 테스트에서 바로 리플레이를 남기려면 `"replay_path":"target/escape.replay.json"`을 추가합니다. 저장할 폴더는 이미 있어야 하며 같은 이름의 파일은 보존됩니다.

리플레이는 포맷·엔진 버전, 씬 경로, 씬과 스크립트의 지문, 시드, 행동, 틱별 상태 검증값을 저장합니다. 시드는 플레이 중 생성되는 프리팹 ID에 적용되며, `play_start`에서도 지정할 수 있습니다. 같은 엔진과 실행 환경에서 비교합니다. 씬이나 스크립트가 바뀌면 `source_changed`가, 기록의 상태와 재실행이 처음 어긋나면 `state_changed`가 반환됩니다.

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

1. **Project → Import…**에서 PNG를 가져온 뒤 에셋을 우클릭해 **Place Sprite**를 누르거나 더블클릭합니다. 스프라이트는 이미지 비율과 투명도를 유지합니다.
2. **Inspector → Object → Sprite3D**에서 텍스처, 너비·높이, 빌보드 모드를 설정합니다. **Face camera**는 카메라 화면을 향하고, **Upright**는 Y축을 세운 채 카메라 쪽으로 돌며, **Fixed**는 오브젝트의 회전을 따릅니다.
3. **File → Sprite Playground**를 열고 우클릭 드래그로 카메라를 돌리면 세 가지 모드를 비교할 수 있습니다.

## 물리

1. **File → Physics Playground**를 선택하고 **Play**를 누릅니다. 큐브가 떨어져 바닥과 경사로에 부딪힙니다. 탭 우클릭 **→ Panels → Physics**에서 충돌과 트리거 이벤트를 확인할 수 있습니다.
2. 직접 구성할 때는 씬 루트에서 오브젝트를 선택하고 **Inspector → Physics → Rigid body**를 켭니다. 떨어지는 큐브에는 **Dynamic**, 바닥에는 **Fixed**를 지정합니다. **Kinematic**의 **Rotate in Play**는 회전하는 플랫폼처럼 다른 강체와 충돌하는 물체를 만듭니다.
3. **Fit to mesh**로 상자 충돌체를 모델에 맞추거나 **Local size / Local center**를 조절합니다. **Trigger sensor**는 물체의 통과를 허용하면서 진입·이탈을 기록합니다.

물리는 Play 중 60 Hz로 실행됩니다. **Stop**을 누르면 편집 씬의 위치와 회전으로 돌아갑니다. 현재 충돌체는 씬 루트에 있는 오브젝트의 상자 형태를 지원합니다.

- 씬 루트의 카메라나 오브젝트에 **Physics → Character controller**를 켜면 캡슐로 벽과 바닥에 충돌하며 이동합니다. **WASD**로 이동하고 **Space**로 점프합니다. `Origin above feet`는 바닥에서 오브젝트 원점까지의 높이입니다. 같은 오브젝트에는 강체와 캐릭터 컨트롤러 중 하나를 사용합니다.
- 강체의 **Collision layer**는 1–32이며 **Collision mask**의 최하위 비트가 레이어 1입니다. `FFFFFFFF`는 모든 레이어를 선택합니다. 캐릭터는 마스크에 포함된 고체 충돌체를 통과할 수 없습니다. Pulse Range의 발사는 가장 가까운 고체에서 막힙니다.

## Rhai 스크립트

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
- **Inspector → Tags**에 `door` 같은 태그를 입력합니다. `find_tag("door")`는 첫 번째 오브젝트를, `find_all_tagged("door")`는 해당 태그의 오브젝트 목록을 반환합니다. 대상이 없으면 각각 `()`와 빈 배열을 반환합니다.
- `send_event(target, "open")`은 대상 스크립트의 `on_message(entity, sender, name)`을 다음 틱에서 호출합니다. 예를 들어 `input_pressed("Interact")`일 때 스위치가 문에 `open` 이벤트를 보낼 수 있습니다.
- `spawn_prefab("Crate", 2, 0, 0)`은 다음 틱에서 프리팹 계층을 생성하며 루트 핸들을 반환합니다. `despawn(root)`으로 생성한 계층 전체를 제거합니다. Play 중 생성·제거는 저장한 편집 씬과 분리되어 실행됩니다.
- 좌표는 부모 기준입니다. `get_position`, `set_position`, `translate`, `translate_local`, `rotate`, `rotate_y`를 사용할 수 있습니다. 이동·회전은 강체 없는 오브젝트와 Kinematic 강체에서, 크기 변경 `set_scale`은 강체 없는 오브젝트에서 지원합니다. Dynamic 강체는 월드 기준 `set_velocity`와 `add_impulse`로 조종합니다. 회전 단위는 도입니다.
- 라이프사이클은 `on_start(entity)`, `on_update(entity, dt)`, `on_fixed_update(entity, dt)`, `on_collision_enter/exit(entity, other)`, `on_trigger_enter/exit(entity, other)`를 지원합니다. 스크립트는 자신이 붙은 오브젝트를 조종합니다. 오류가 난 인스턴스는 이번 Play에서 중지합니다.
- 편집기에서 적용한 코드는 **Save Scene**으로 씬 파일에 저장합니다. 기존 `.rhai`는 **Project → Import…**로 가져온 뒤 에셋을 우클릭해 **Attach to Selection**으로 연결합니다. 가져온 스크립트를 편집기로 수정하면 씬에 편집한 소스가 저장됩니다.

## 프로젝트와 씬 전환

1. **File → Project → New Project…**에서 빈 폴더를 선택합니다. Futty가 그 폴더에 프로젝트 파일과 `Main.futty.scene.ron`을 만들고 첫 씬을 엽니다. 이미 만든 씬으로 시작하려면 **New Project from Current Scene…**을 사용합니다.
2. **Add Scene…**으로 프로젝트 폴더 안의 다른 씬을 등록합니다. **Start Scene**에서 프로젝트를 열거나 단독으로 실행할 때 처음 보여 줄 씬을 고릅니다. **Edit Scene**에서 편집할 파일을 전환합니다.
3. 게임에서는 Rhai의 `load_scene("Pulse Range")`처럼 등록된 씬 ID를 넘겨 전환합니다. 에디터에서 등록할 때 ID는 씬 파일 이름의 확장자를 뺀 값입니다. 전환 대상의 카메라와 HUD, 스크립트를 새로 초기화합니다.

프로젝트가 열려 있으면 씬을 프로젝트 폴더 안에 저장합니다. **Import…**로 가져온 이미지, 모델, 오디오, Rhai 파일은 씬을 저장할 때 그 씬 옆의 `Assets/Futty/`에 복사됩니다. 에디터에서 생성한 Rhai 스크립트는 씬 파일에 함께 저장됩니다. 프로젝트를 옮기거나 공유할 때는 선택한 프로젝트 폴더 전체를 복사합니다.

에디터의 **Play**는 현재 편집 중인 씬에서 시작합니다. **Stop**을 누르면 편집 중이던 씬으로 돌아갑니다. **Remove Scene**은 프로젝트 목록에서 항목을 빼고 씬 파일은 보존합니다. 시작 씬을 제거하려면 먼저 다른 씬을 시작으로 지정합니다.

[Pulse Range 프로젝트](../examples/pulse-range/Pulse%20Range.futty.project.ron)를 **File → Project → Open Project…**에서 열고 **Play → Enter practice range**를 눌러 로비에서 연습장으로 전환할 수 있습니다.

## Windows 게임 내보내기

**File → Export Windows Game…**에서 저장한 씬의 출력 위치를 선택합니다. 만들어진 `*-Windows` 폴더는 씬, 참조 에셋, 실행 설정과 `Game.exe`를 포함합니다. 폴더 전체를 옮긴 뒤 `Game.exe`를 실행하면 에디터 없이 게임이 열립니다. 씬에 카메라가 없으면 내보내기 당시의 Scene 카메라로 실행합니다.

프로젝트를 열어 둔 상태에서는 **Export Windows Project…**로 등록한 모든 씬과 참조 에셋을 묶습니다. `Game.exe`는 시작 씬에서 열립니다.

Windows x64에서 CLI로도 실행할 수 있습니다. Futty는 새 콘솔 창을 열지 않습니다. PowerShell에서 `| Out-Host`를 붙이면 내보내기가 끝날 때까지 기다리고 결과를 현재 터미널에서 확인할 수 있습니다. 출력은 아직 없는 폴더를 지정합니다. 씬에 카메라가 없으면 CLI는 기본 카메라를 사용합니다.

```powershell
.\futty-editor.exe --export .\Main.futty.scene.ron .\Main-Windows | Out-Host
.\futty-editor.exe --export ".\examples\pulse-range\Pulse Range.futty.project.ron" .\Pulse-Range-Windows | Out-Host
```

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
