---
name: futty-level-design
description: Design and verify Futty levels when planning routes, pacing player decisions, placing obstacles, or comparing alternative layouts.
---

# Futty Level Design

1. Record the spawn, goal, player abilities, intended route, and the obstacle that teaches or tests each ability. When exploring alternatives, change route structure or required actions and describe the trade-off for each layout.
2. Inspect the saved scene with `scene_objects`. Use `scene_apply` for the smallest playable layout. Place a visual cue where the player makes a decision and put the collider where the visible obstacle ends.
3. Run `scene_validate`. For a standard scene with a character camera, exercise the intended route with `play_test` and try one plausible shortcut or blocked route with `play_step`. Inspect player position or HUD when a terminal result alone cannot distinguish the two paths.
4. Report reachable goals, unintended paths observed, input sequences, and the first position where a blocked or unclear route needs adjustment.

[Futty guide](../../../docs/guide.md#ai-플레이-세션) · [Design exploration reference](https://github.com/abagames/agentic-gamedev-skills/blob/4ed8d751d08b5c1be14ccca02ce0b9a9bea60593/.agents/skills/exploring-game-design-space/SKILL.md)
