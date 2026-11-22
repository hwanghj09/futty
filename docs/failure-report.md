# Futty 제작 실패 리포트

MCP의 `game_failure_schema`에서 실패 리포트의 JSON Schema를 조회할 수 있습니다. 현재 `schema_version`은 1입니다. AI나 제작 자동화는 `stage`와 `code`로 분기하고, `message`는 사람에게 표시합니다.

| 도구 | 리포트 위치와 단계 |
| --- | --- |
| `scene_validate` | `valid: false`일 때 구조화된 결과의 `failure_report`. 단계는 `scene_validation`입니다. |
| `project_validate` | `valid: false`일 때 구조화된 결과의 `failure_report`. 단계는 `project_validation`입니다. |
| `play_test` | `passed: false`일 때 구조화된 결과의 `failure_report`. 단계는 `playtest`입니다. |
| `scene_capture` | MCP 오류의 `error.data.failure_report`. 단계는 `camera_capture`입니다. |
| `project_build` | MCP 오류의 `error.data.failure_report`. 단계는 `windows_build`입니다. |

위 도구가 호출된 뒤 경로·읽기·쓰기 등 실행 중 오류를 만나면 같은 단계의 `error.data.failure_report`를 사용합니다. 기존 MCP 오류 코드와 메시지는 유지됩니다. 성공한 응답에는 실패 리포트가 생략됩니다.

| 코드 | 의미와 증거 |
| --- | --- |
| `validation_failed` | 포맷·에셋·스크립트 오류가 발견됐습니다. `evidence.issues`에는 직접 오류가, `evidence.invalid_scenes`에는 문제가 있는 프로젝트 씬과 해당 오류가 담깁니다. |
| `unexpected_status` | 플레이 종료 상태가 예상과 다릅니다. 예상·실제 상태와 실패한 검사만 담습니다. |
| `failed_checks` | 종료 상태는 일치하고 HUD 값, 충돌 또는 씬 전환 검사 중 일부가 실패했습니다. |
| `execution_failed` | 도구 실행 오류입니다. `context.path`와 해당되는 `context.output_path`를 확인합니다. |

플레이 리포트의 `context.tick`은 테스트가 종료된 틱입니다. `seed`는 플레이 요청에 쓴 시드이며, `replay_path`는 리플레이가 실제로 기록된 경우에만 들어 있습니다. 다음은 코인 수집 게임에서 이동 없이 120틱을 실행했을 때의 리포트 예입니다.

```json
{
  "schema_version": 1,
  "stage": "playtest",
  "code": "unexpected_status",
  "message": "Playtest ended with a different status",
  "context": {
    "path": "Coin/Coin Field.futty.scene.ron",
    "tick": 120,
    "seed": 17,
    "replay_path": "Idle.json"
  },
  "evidence": {
    "kind": "playtest",
    "expected": "success",
    "actual": "running",
    "failed_checks": [
      {"kind":"hud","binding":"coins","expected":3,"actual":0.0,"passed":false}
    ]
  }
}
```

`play_replay`의 `path`에 리포트의 `replay_path` 값을 전달하면 기록된 입력으로 같은 플레이를 재현할 수 있습니다.
