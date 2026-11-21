---
name: futty-game-design
description: Design a Futty game or prototype around a playable loop, a small scene list, player actions, and testable win and failure rules.
---

# Futty Game Design

Turn the user's premise into a playable first scene. Reuse existing Futty input actions, collision bodies, tags, Rhai events, and HUD bindings when they fit.

- Specify what the player can see and do, the goal, and the events that prove success or failure.
- Describe each scene by entry point, required objects, and transition conditions. For each gameplay object, specify a tag and its role, such as a switch that sends an event to a door.
- Use the smallest scene that can exercise the main rule. Keep optional art, progression, and extra levels out of the first playable slice unless the user requests them.
- When designing for an AI player, list the observations available at decision time and the existing input actions that can express each move. Include a successful sequence and a sequence that should fail.

Use the [Futty roadmap](../../../docs/FUTTY_FINAL_GOAL.md) to check which AI interfaces are planned. Inspect the authored scenes and scripts under [examples](../../../examples/) before proposing replacement engine systems.
