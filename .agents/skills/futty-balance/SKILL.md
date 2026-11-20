---
name: futty-balance
description: Compare Futty gameplay strategies, scoring, and completion times using controlled playtests and saved replay evidence.
---

# Futty Balance

1. Define the intended strategy, a simple baseline, and one alternative that might farm rewards or bypass the objective. Express fixed strategies using the same `play_test` input format.
2. Run each strategy with the same scene version, seed, and tick budget. Repeat with relevant variations in inputs or supported game state. Futty's runtime seed stabilizes spawned prefab IDs. Verify how the game controls randomness before treating a seed change as a new gameplay condition.
3. Report runs per strategy, terminal status, elapsed ticks, and any score exposed in `last_observation.hud`. Use separate runs when comparing one rule change. Save a replay where a strategy unexpectedly succeeds or fails.
4. Distinguish what the scripted policies demonstrate from what requires human play. Keep reaction-time and subjective difficulty claims grounded in human trials.

[Futty playtest guide](../../../docs/guide.md#자동-플레이테스트와-리플레이) · [Gameplay balance reference](https://github.com/abagames/agentic-gamedev-skills/blob/4ed8d751d08b5c1be14ccca02ce0b9a9bea60593/.agents/skills/evaluating-gameplay-balance/SKILL.md)
