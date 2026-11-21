# Futty AI 원스톱 게임 제작: 디자인 · 모델링 · 애니메이션 · 사운드

> 기준일: 2026-11-21  
> Futty는 사람보다 **AI 에이전트의 도구 사용**을 우선한다.  
> **현재 상태**: GLB/PNG/WAV/OGG를 불러오고 애니메이션/효과음을 재생할 수 있지만, 생성형 AI로 고품질 디자인·모델·음악을 직접 만드는 기능은 아직 없다. 아래의 대부분은 개발 계획이다.

관련 문서: [AI 로드맵](ai-roadmap.md) · [2D/3D·멀티플레이](2d-multiplayer-roadmap.md) · [개발 체크리스트](development-checklist.md).

## 1. 최종 사용 경험

사용자의 한 문장:

> 3D 4인 공포 게임을 만들어줘. 폐병원 맵, 괴물 모델링, 걷기·공격 애니메이션, 발소리, BGM, UI, 온라인 서버까지 완성하고 실행 파일로 만들어줘.

AI 에이전트의 자동 처리 단계:

1. 기획: 장르, 핵심 플레이, 2D/3D, 플레이 인원, 장면/에셋 목록 생성.
2. 아트 디렉션: 팔레트, 조명, 형태/재질, 캐릭터 스타일, UI, 사운드 톤 정의.
3. 에셋 생성: 2D PNG/시트/타일, 3D GLB/텍스처/리그, 애니메이션 클립, SFX/앰비언스/BGM.
4. 검증: 형식, 해상도, 폴리곤, UV, 리그, 애니메이션, 클리핑, 라이선스 검사.
5. 적용: Futty MCP를 통해 임포트/씬 배치/애니메이션·사운드 이벤트 연결.
6. 테스트: 씬 검증, 카메라 캡처, 플레이테스트, 멀티플레이 동기화.
7. 개선: 실패한 에셋만 재제작하며 UUID/씬 참조 보존.
8. 빌드: Game.exe, 필요한 에셋, 멀티일 때 Server.exe, 테스트/출처 보고서.

사람이 Blender, 이미지 편집기, 음악 편집기를 직접 조작하는 단계가 **필수가 되지 않게** 설계한다.

## 2. 아키텍처

~~~text
사용자 요구
   ↓
AI Agent (기획/제작 총괄)
   ↓
GameBrief → ArtDirectionSpec → AssetPlan
   ↓
Asset Generation Service (모델 중립)
   ├─ 2D Image Adapter
   ├─ 3D Model + Rig Adapter
   ├─ Animation Adapter
   ├─ SFX / Music Adapter
   └─ Offline Procedural Fallback
   ↓
Normalize → Validate → Asset Registry / SHA-256
   ↓
Futty MCP / CLI: Import → Bind → Test → Build
   ↓
Game.exe / Server.exe / Source Assets / Report
~~~

생성형 모델의 추론과 콘텐츠 제작은 외부 에이전트/연결된 제공자가 수행할 수 있고, **Futty는 제작 API, 자산 규격, 검증, 연결, 테스트, 빌드**에 집중한다. 특정 AI 제공자의 API에 종속되지 않는다.

| 생성 모드 | 목표 | 평가 |
| --- | --- | --- |
| prototype | 오프라인, 빠른 게임 프로토타입 | 플레이스홀더임을 표시 |
| assisted | 연결된 이미지/3D/음악 생성 모델 사용 | 생성 도구·라이선스·품질 기록 |
| production | 일관된 디자인, 리깅, 후처리, 반복 검수 | 통과할 때만 final |

외부 유료 생성 API는 사용자 연결과 승인된 예산 범위에서만 호출한다. API 키는 저장소나 게임 패키지에 넣지 않는다.

## 3. 실제 엔진 호환성

| 콘텐츠 | 기존 Futty 지원 | 추가 구현 |
| --- | --- | --- |
| PNG | 텍스처 임포트, Sprite3D, HUD | 완전한 2D 렌더링, 스프라이트 시트/타일 생성 |
| GLB | 메시/재질/애니메이션 임포트·재가져오기 | 텍스트→3D 모델/UV/자동 리깅·정리 |
| 3D 애니메이션 | GLB 클립 재생, Rhai 전환 | 자동 동작 생성·리타게팅·전환 상태 머신 |
| WAV/OGG | 파일 임포트, 오디오 소스 재생 | SFX/음악 생성, 오디오 믹서, 루프 검수 |
| MCP | 기존 asset_import, asset_reimport, scene_apply, 스크립트/HUD | 생성 서비스 호출, 일괄 가져오기, 게임 전체 오케스트레이션 |

Futty의 기존 **asset_import**는 이미 생성된 로컬 파일을 씬에 등록하는 도구다. 자체 생성형 모델 기능은 아니다.

## 4. 하나의 디자인 규칙으로 통일

**ArtDirectionSpec**을 먼저 만들어 모든 이미지/모델/애니메이션/사운드 생성 요청에 전달한다.

- 게임: 장르, 플레이어 경험, 분위기, 시대/세계관, 화면비.
- 색감: 주색, 보조색, 강조색, 명암 범위, 대비·가독성.
- 2D: 픽셀아트/일러스트, pixels-per-unit, 프레임 크기, 피벗, 아웃라인, 타일 크기.
- 3D: low-poly/스타일라이즈/리얼, Y-up, 미터 단위, 폴리곤 예산, UV·텍스처 사양.
- 애니메이션: Idle/Walk/Run/Attack/Hit/Death, 각 프레임/길이, 루프, 전환, 이벤트 타임.
- 사운드: 장르/음색, SFX·앰비언스·BGM 구분, 음량 범위, 재생 조건.
- UI: 안전영역, 버튼 상태, HUD 체계, 접근성.
- 생성 출처: 제공자, 프롬프트 해시, 사용 권한, 생성 버전, 품질 등급.

각 에셋의 색상/스타일/길이/크기를 동일한 기준으로 검사한다.

## 5. 세부 개발 작업

### A. 2D 그래픽·디자인

- [ ] 텍스트로 배경/캐릭터/적/소품/UI 디자인 요구를 생성.
- [ ] PNG 배경 제거·알파·캔버스/피벗/팔레트 정규화.
- [ ] 2D 스프라이트 시트: Idle/Walk/Jump/Attack/Hit 프레임 제작 및 정렬.
- [ ] 타일셋 경계 이음새/타일맵/레이어 검수.
- [ ] 생성 PNG를 Futty의 전용 2D 렌더러에 배치하는 MCP API.
- [ ] 재생성 시 기존 에셋 ID/바인딩 보존.

**완료 기준**: AI가 전부 제작한 맵/캐릭터/프레임/HUD가 2D 게임에서 재생된다.

### B. 3D 모델링

- [ ] ModelSpec에 크기·형태·폴리곤 상한·재질·리그 요구 작성.
- [ ] 외부 3D 생성기 또는 Blender 헤드리스 제작 어댑터.
- [ ] GLB default scene/mesh/normal/UV/축·스케일·피벗/텍스처 검증.
- [ ] 자동 리깅, 본/가중치/애니메이션 호환성 검사.
- [ ] 모델 재가져오기 시 씬 UUID 및 클립 번호 보존.

**완료 기준**: AI 생성 캐릭터와 환경 모델이 정상 로딩·렌더링된다.

### C. 애니메이션

- [ ] 2D 프레임 애니메이션 + 3D GLB 스켈레톤/노드 애니메이션 통합 사양.
- [ ] Idle/Walk/Attack 등 클립 생성·검증 및 게임 상태 바인딩.
- [ ] 리그 호환/루트 모션/루프·속도·전환 검사.
- [ ] 애니메이션은 시각 표현, 공격/충돌의 공식 판정은 런타임/서버가 수행.
- [ ] 멀티플레이 시 애니메이션 상태/이벤트만 네트워크 동기화.

**완료 기준**: 이동·공격·피격 등 상태별 애니메이션 전환과 재가져오기 검증.

### D. 사운드·음악

- [ ] 점프, 공격, 충돌, 수집, UI 이벤트별 효과음 자동 생성.
- [ ] 장소별 앰비언스 및 메뉴·전투·승리 BGM 생성.
- [ ] WAV/OGG 유효성/재생 시간/샘플레이트/피크/클리핑/루프 이음새 검사.
- [ ] 볼륨/음악/SFX 버스·페이드·공간음 향상.
- [ ] 게임 이벤트에 클립 ID를 자동 연결하고 중복 재생 방지.
- [ ] 멀티플레이는 오디오 파일이 아닌 재생 이벤트를 필요한 클라이언트에 전송.

**완료 기준**: 완성된 게임에서 모든 주요 이벤트의 소리와 BGM이 정상 재생된다.

## 6. 에셋 원본/버전/라이선스 관리

매니페스트는 파일 경로뿐 아니라 종류, 출력 크기, 생성기, 해시, 스타일, 버전, 클립/음원 정보, 라이선스, 테스트 결과를 기록한다.

~~~json
{
  "schema_version": 1,
  "project": "haunted-hospital",
  "art_direction": "art-direction.json",
  "assets": [
    {
      "id": "monster-01",
      "path": "assets/3d/monster.glb",
      "kind": "model",
      "provider": "user-configured-generator",
      "sha256": "actual-content-hash",
      "license": "review_required",
      "clips": ["Idle", "Walk", "Attack"],
      "status": "generated"
    }
  ]
}
~~~

상태: planned → generated → validated → imported → bound → tested → final. **generated**가 곧 게임에서 정상 동작하거나 상업적 배포 가능하다는 뜻은 아니다.

### 신규 MCP/CLI 설계안(아직 구현 안 됨)

| 기능 | 제안 API | 결과 |
| --- | --- | --- |
| 기획/스타일 | game_brief_create, art_direction_set | 게임 구성과 아트 규칙 |
| 에셋 기획 | asset_plan_create, asset_plan_inspect | 파일/의존성/품질 목표 |
| 제작 | asset_generate, asset_batch_generate | 작업 ID, 경로, 에러, 비용 |
| 검수 | asset_validate, asset_quality_report | 규격, 스타일, 출처, 재생성 여부 |
| 적용 | asset_import_batch, asset_bind | UUID 보존 및 씬에 연결 |
| 애니메이션 | animation_bind | 클립→상태/이벤트 |
| 오디오 | audio_cue_bind | SFX/BGM→재생 조건 |
| 통합 | game_produce, game_review | 생성→검증→테스트→빌드 리포트 |

API는 워크스페이스 상대 경로, JSON 스키마, 오류 필드/코드, 파일 해시, 원자적 업데이트, 비용/타임아웃/재시도 제한을 지원해야 한다.

## 7. 먼저 실제로 추가한 기능: 원커맨드 프로토타입 제작기

[scripts/generate-prototype-assets.py](../scripts/generate-prototype-assets.py)는 Python 3 표준 라이브러리만 사용한다.

한 번 실행하면 **2D PNG 4개**, **Idle/Action 두 애니메이션을 포함한 단순 3D GLB 1개**, **WAV 3개**, **아트 디렉션 JSON**, **SHA-256 에셋 매니페스트**를 생성한다. 예시는 다음과 같다.

~~~bash
python scripts/generate-prototype-assets.py --output generated/my-prototype --name "My Prototype" --theme horror --seed 42
~~~

지원 테마: fantasy, horror, scifi. 매니페스트를 읽은 에이전트는 기존 Futty MCP asset_import로 등록하고, 반환받은 UUID를 사용해 씬/스크립트/HUD와 연결한다. 산출물 폴더는 워크스페이스 내부의 새 폴더를 사용하고 기존 파일은 덮어쓰지 않는다.

[구조 검증 테스트](../scripts/test_prototype_asset_pack.py)에는 PNG CRC/크기, GLB 메시·애니메이션, WAV 디코딩, 재현성, 경로 무단 덮어쓰기 방지 사례가 포함되어 있다.

**범위 제한**: 이 생성기는 외부 AI 모델을 호출하지 않으며 작품 수준의 모델링·리깅·음악을 생성하는 기능이 아니라 프로시저럴 플레이스홀더를 만드는 첫 단계다. Futty 자체 디코더/에디터 통합 테스트는 별도로 필요하다.

## 8. 개발 순서와 완료 기준

| 단계 | 항목 | 완료 기준 |
| --- | --- | --- |
| A0 | 프로시저럴 패키지 생성기 + 기본 검증 테스트 | PNG 4, GLB 1, WAV 3, 디자인 규칙/매니페스트 자동 생성 |
| A1 | Futty 에셋 검사·일괄 임포트·바인딩 | 생성 에셋이 씬에 ID 유지로 적용되고 게임에서 동작 |
| A2 | 2D 이미지/타일/애니메이션 제작 어댑터 | 2D 캐릭터·맵·UI를 AI가 생성 |
| A3 | 3D 모델·리그·애니메이션 제작 어댑터 | 모델/Walk/Attack를 검수하고 배치 |
| A4 | 사운드 생성/믹싱 어댑터 | SFX, 앰비언스, BGM 자동 제작·연결 |
| A5 | 원스톱 제작 디렉터 | 생성 실패 복구 → 게임/멀티 테스트 → 빌드 자동 완주 |

A2~A4는 병렬 진행 가능. A5는 2D/3D 멀티플레이 로드맵과 통합한다.

### 바로 다음 개발 작업

- [x] 오프라인 원커맨드 프로토타입 생성기 소스 추가
- [x] PNG/GLB/WAV/매니페스트 단위 테스트 소스 추가
- [ ] GitHub CI에서 생성기 테스트 실제 통과 확인
- [ ] Futty GLB/오디오 디코더를 이용한 실제 임포트/재생 테스트
- [ ] 일괄 임포트/씬 바인딩 MCP API
- [ ] 사용자 연결 이미지/3D/음향 모델 어댑터와 비용/권리 정책
- [ ] 아트/애니메이션/사운드 품질 검사 및 재생성
- [ ] 2D/3D 싱글/멀티를 포함한 E2E 원스톱 게임 제작

## 9. 최종 성공 판정

사용자 요청 한 번으로 **기획·디자인·2D/3D 모델링·애니메이션·효과음·BGM·UI·게임 로직·멀티플레이 테스트·패키징**이 GUI 조작 없이 완결되어야 한다. 최종 산출물은 플레이 가능한 Windows 게임/서버, 원본 에셋, 출처·권리 내역, 재현 가능한 테스트 보고서를 포함한다.

실제로 생성되지 않은 요소나 플레이스홀더를 최종 완성품이라고 표시하지 않는다.
