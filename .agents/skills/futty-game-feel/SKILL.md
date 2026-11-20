---
name: futty-game-feel
description: Improve feedback and readability for Futty game events such as a switch activation, pickup, landing, hit, or blocked action.
---

# Futty Game Feel

1. Reproduce the event and identify which information the player misses. Record the transition in the scene, HUD, and available sound cues.
2. Give the event one clear primary cue. Use a Rhai HUD value or a change to the relevant object's color for a local state change. For a sound cue, use a registered clip and existing audio components through the editor when audio setup is needed.
3. Trigger feedback on the state transition. Check repeated input, rejected actions, and rapid consecutive events so cues agree with the game rule and do not mask the next action.
4. Replay the inputs to confirm the event and HUD consistently update. Use the editor Play view to judge timing, visibility, and audio playback.

[Futty HUD guide](../../../docs/guide.md#게임-hud) · [Game feel reference](https://github.com/abagames/agentic-gamedev-skills/blob/4ed8d751d08b5c1be14ccca02ce0b9a9bea60593/.agents/skills/maximizing-game-feel/SKILL.md)
