---
name: futty-scene-authoring
description: Build or revise saved Futty scenes via MCP using object queries, atomic edit batches, registered assets, prefabs, cameras, and colliders.
---

# Futty Scene Authoring

1. Inspect the target with `scene_inspect` and `scene_objects`. Use returned UUIDs for existing objects and asset references. Use `scene_create` for a new saved scene.
2. Keep saved scenes closed in the editor while writing with MCP. Use workspace-relative `.ron` paths with forward slashes.
3. Apply one coherent change with `scene_apply`. A new object's `key` can be used as an object reference by later edits in that batch. Position and scale use three components; rotation input uses XYZ degrees. A parent edit keeps the local transform. Use root objects for solid colliders and character controllers.
4. Inspect the saved scene again and open it in Futty when visual validation is needed. Keep the returned latest-batch `undo_token` if reverting that applied change may be necessary. Use `scene_undo` only for that token and scene.

Read the [MCP guide](../../../docs/guide.md#codex에서-futty-mcp-사용하기) for the operation list and request examples. The [Beacon Bot Escape scene](../../../examples/agent-escape/Beacon%20Bot%20Escape.futty.scene.ron) shows a playable layout with a fixed floor, player capsule, switch, and kinematic door.
