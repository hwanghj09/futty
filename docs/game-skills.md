# Futty 게임 스킬 카탈로그

Futty에서 게임을 기획하고 제작하며 플레이로 검증할 때 쓸 AI 에이전트 스킬을 모았습니다. 공개 스킬의 원본과 적용 범위를 기록했고, 하단에 Futty에 기본 탑재할 후보와 필요한 엔진 기능을 정리했습니다. [AI 게임 엔진 로드맵](ai-roadmap.md)

## 공개 스킬

### 기획과 게임플레이

| 스킬 | Futty에서 활용할 작업 | 원본 |
| --- | --- | --- |
| `game-studio` | 게임의 목표와 조작을 잡고 기획, 에셋, UI, 테스트 작업으로 나눕니다. Futty용 작업 분배 흐름의 참고입니다. | [OpenAI Game Studio](https://github.com/openai/plugins/blob/main/plugins/game-studio/skills/game-studio/SKILL.md) |
| `game-designer` | 기획서, 게임 규칙, 레벨 흐름, 진행도와 테스트 시나리오를 작성합니다. | [Setcode](https://github.com/setcodes/gamedev-skills/blob/main/skills/game-designer/SKILL.md) |
| `exploring-game-design-space` | 같은 테마에서 다른 조작과 승리 조건을 가진 프로토타입 후보를 만듭니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/exploring-game-design-space/SKILL.md) |
| `maximizing-game-feel` | 이동, 착지, 버튼 작동, 아이템 획득의 시각·음향 피드백을 설계합니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/maximizing-game-feel/SKILL.md) |
| `godot-gamedev` | 씬과 입력, 게임 상태를 연결하는 구현·검증 절차를 참고합니다. 실행 절차는 Godot 프로젝트용입니다. | [Setcode](https://github.com/setcodes/gamedev-skills/blob/main/skills/godot-gamedev/SKILL.md) |

### 모델, 스프라이트, HUD, 오디오

| 스킬 | Futty에서 활용할 작업 | 원본 |
| --- | --- | --- |
| `blender-game-assets` | Blender에서 모델의 폴리곤과 리그를 정리하고 GLB를 내보냅니다. 자동 제작 단계는 Blender와 Blender Lab MCP에 연결합니다. | [Setcode](https://github.com/setcodes/gamedev-skills/blob/main/skills/blender-game-assets/SKILL.md) |
| `web-3d-asset-pipeline` | GLB의 축, 단위, 피벗, 텍스처를 점검하는 출고 절차를 가져옵니다. 내보내기 옵션은 Futty 임포터에서 검증합니다. | [OpenAI Game Studio](https://github.com/openai/plugins/blob/main/plugins/game-studio/skills/web-3d-asset-pipeline/SKILL.md) |
| `sprite-pipeline` | 투명 PNG의 기준점, 프레임 크기, 여백을 맞춰 Sprite3D용 에셋을 준비합니다. | [OpenAI Game Studio](https://github.com/openai/plugins/blob/main/plugins/game-studio/skills/sprite-pipeline/SKILL.md) |
| `game-ui-frontend` | 게임 화면을 가리지 않도록 로비, 메뉴, HUD의 정보 배치와 크기를 점검합니다. | [OpenAI Game Studio](https://github.com/openai/plugins/blob/main/plugins/game-studio/skills/game-ui-frontend/SKILL.md) |
| `building-era-authentic-game-audio` | 레트로 효과음, 배경음, 게임 이벤트별 오디오 큐와 테스트를 설계합니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/building-era-authentic-game-audio/SKILL.md) |
| `audio-setup-mixers` | 효과음·배경음·UI 음량 채널을 설계할 때 참고합니다. 믹서 조작은 Unity용입니다. | [Unity Skills](https://github.com/Unity-Technologies/skills/blob/main/skills/audio-setup-mixers/SKILL.md) |

### 플레이테스트, 물리, AI 이동

| 스킬 | Futty에서 활용할 작업 | 원본 |
| --- | --- | --- |
| `game-playtest` | 실행, 조작, 씬 전환, HUD를 순서대로 확인하고 스크린샷과 재현 절차를 남깁니다. 브라우저 자동화 단계는 Futty 플레이 세션에 맞춰 작성합니다. | [OpenAI Game Studio](https://github.com/openai/plugins/blob/main/plugins/game-studio/skills/game-playtest/SKILL.md) |
| `implementing-gameplay-invariants` | 문이 닫혀 있으면 통과할 수 없고, 같은 아이템에서 점수를 두 번 얻을 수 없는 등 규칙을 테스트로 바꿉니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/implementing-gameplay-invariants/SKILL.md) |
| `localizing-game-state-divergence` | 같은 시드와 행동으로 재현한 뒤, 점수나 이벤트가 처음 어긋난 틱을 찾습니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/localizing-game-state-divergence/SKILL.md) |
| `evaluating-gameplay-balance` | 시드와 플레이 스타일을 바꾸며 성공률, 진행 시간, 점수 분포를 비교합니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/evaluating-gameplay-balance/SKILL.md) |
| `verifying-turn-based-games` | 두 AI가 엄격히 교대로 행동하는 게임에서 규칙과 봇 대전 결과를 검증합니다. | [ABA Games](https://github.com/abagames/agentic-gamedev-skills/blob/main/.agents/skills/verifying-turn-based-games/SKILL.md) |
| `fix-3d-physics-collisions` | 충돌, 트리거, 레이캐스트 문제를 진단하는 순서를 참고합니다. 실제 판정과 설정은 Futty의 Rapier 기준입니다. | [Unity Skills](https://github.com/Unity-Technologies/skills/blob/main/skills/fix-3d-physics-collisions/SKILL.md) |
| `initialize-ai-navigation` | 순찰 지점, 장애물, 경로 연결 검사 방법을 Futty 길찾기 스킬에 반영합니다. | [Unity Skills](https://github.com/Unity-Technologies/skills/blob/main/skills/initialize-ai-navigation/SKILL.md) |
| `unity-pipeline` | 에디터 실행, Play 진입, 테스트, 스크린샷을 자동으로 연결하는 흐름을 참고합니다. | [Unity Skills](https://github.com/Unity-Technologies/skills/blob/main/skills/unity-pipeline/SKILL.md) |

### 라이선스 확인

- Setcode와 ABA Games 저장소는 MIT입니다. 패키지를 배포할 때 출처와 라이선스 고지를 포함합니다. [Setcode LICENSE](https://github.com/setcodes/gamedev-skills/blob/main/LICENSE) · [ABA Games LICENSE](https://github.com/abagames/agentic-gamedev-skills/blob/main/LICENSE)
- OpenAI Game Studio의 플러그인 메타데이터에는 MIT가 표시됩니다. 스킬 파일을 함께 배포할 때는 포함할 참조 문서, 스크립트, 에셋의 출처와 조건도 확인합니다. [Game Studio 플러그인 메타데이터](https://github.com/openai/plugins/blob/main/plugins/game-studio/.codex-plugin/plugin.json)
- Unity Skills는 Unity 의존 프로젝트용 Companion License를 명시합니다. Futty에서는 위 스킬의 작업 흐름을 참고하고, Futty MCP와 런타임용 절차를 작성합니다. [Unity Skills LICENSE](https://github.com/Unity-Technologies/skills/blob/main/LICENSE.md)

## Futty 기본 팩 후보

[게임 기획](../.agents/skills/futty-game-design/SKILL.md), [씬 제작](../.agents/skills/futty-scene-authoring/SKILL.md), [Rhai 규칙](../.agents/skills/futty-rhai-gameplay/SKILL.md), [AI 플레이](../.agents/skills/futty-ai-player/SKILL.md)는 저장소의 `.agents/skills/`에 들어 있습니다. [Beacon Bot Escape](../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron)에서 씬 제작부터 AI 탈출까지 실행할 수 있습니다.

[자동 플레이테스트](../.agents/skills/futty-playtest/SKILL.md)와 [리플레이 디버깅](../.agents/skills/futty-replay-debug/SKILL.md)은 탈출 판정과 첫 상태 차이를 확인하는 후속 작업에 사용합니다.

ABA Games의 공개 스킬을 참고해 Futty 도구로 실행할 수 있는 지침 세 가지를 추가했습니다. 각 스킬에는 참고한 원본의 커밋 링크가 있습니다.

| 탑재 스킬 | 적용 작업 |
| --- | --- |
| [Futty 레벨 설계](../.agents/skills/futty-level-design/SKILL.md) | 목표 경로를 배치하고 우회로와 장애물 판정을 검증 |
| [Futty 게임 피드백](../.agents/skills/futty-game-feel/SKILL.md) | 상호작용, 아이템 획득, 충돌 상태를 HUD와 시청각 효과로 전달 |
| [Futty 밸런스 검증](../.agents/skills/futty-balance/SKILL.md) | 같은 조건에서 전략별 성공률, 소요 틱, 기록된 점수를 비교 |

| 스킬 후보 | 입력 → 산출물 | 사용할 Futty 기능 |
| --- | --- | --- |
| `futty-game-design` | 장르·플레이 시간 → 핵심 조작, 승리·실패 조건, 규칙 문서 | 기획 템플릿과 예제 |
| `futty-scene-authoring` | 씬 요구사항 → 오브젝트, 프리팹, 충돌체, 카메라가 들어간 씬 | MCP 씬 편집·일괄 Undo |
| `futty-camera-lighting` | 시점·분위기 → 메인 카메라, Sky, 조명 배치, 캡처 | 카메라·조명, 화면 캡처 |
| `futty-rhai-gameplay` | 규칙과 오브젝트 태그 → 스위치, 문, 아이템, 씬 전환 스크립트 | Rhai, 이벤트, 태그, 씬 전환 |
| `futty-glb-assets` | Blender 모델 → 검증된 GLB와 씬에 연결할 UUID | GLB 임포트·재가져오기 |
| `futty-rig-animation` | 리그·클립 이름 → 시작 클립, 반복·속도 설정, 전환 테스트 | GLB 애니메이션, Rhai |
| `futty-sprite-assets` | 원본 이미지 → 크기와 투명도를 맞춘 PNG, 배치용 참조 | PNG, Sprite3D |
| `futty-game-audio` | 게임 이벤트 → WAV·OGG와 재생 조건 | 오디오 소스, 이벤트 |
| `futty-hud` | 점수·체력·목표 상태 → HUD 배치와 값·버튼 바인딩 | Game HUD, Rhai |
| `futty-ai-navigation` | 목표·이동 제한 → 웨이포인트와 순찰·복귀 규칙 | 캐릭터 컨트롤러, 길찾기 |
| `futty-ai-player` | 플레이어 관찰 → 이동·시점 회전·점프·상호작용 | 관찰·행동 API, 고정 틱 진행 |
| `futty-playtest` | 씬·시드·성공 조건 → 재현 절차, 판정, 마지막 관찰 | 헤드리스 실행, 판정 이벤트 |
| `futty-replay-debug` | 실패 리플레이 → 처음 어긋난 틱과 관련 오브젝트 | 행동 기록, 상태 비교 |
| `futty-windows-build` | 저장된 프로젝트 → 검사 결과와 Windows 빌드 경로 | 씬 검사, Windows 내보내기 |

## 탑재 순서

1. **제작** — `futty-game-design`, `futty-scene-authoring`, `futty-rhai-gameplay`로 Beacon Bot 방과 스위치·출구 규칙을 만듭니다.
2. **플레이** — `futty-ai-player`, `futty-playtest`, `futty-replay-debug`로 탈출 성공 여부를 기록하고 실패를 재현합니다.
3. **표현과 배포** — GLB, 스프라이트, HUD, 오디오, 애니메이션과 Windows 빌드 스킬을 추가합니다. 이후 길찾기와 밸런스 비교를 붙입니다.

팩에 추가할 때는 작업별로 `SKILL.md`에 이름, 호출 조건, 입력, 사용 도구, 결과물을 적습니다. 씬 예제와 검증 시나리오는 해당 스킬의 참조 파일로 연결합니다. [Agent Skills 규격](https://agentskills.io/specification)
