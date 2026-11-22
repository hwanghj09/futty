#!/usr/bin/env python3
"""Generate a small original Futty prototype asset pack in one command.

Standard-library-only fallback for AI agents. It makes *placeholder* artwork,
an animated low-poly GLB and WAV audio; it does not call an image/3D/audio model.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import re
import struct
import wave
import zlib
from pathlib import Path


PALETTES = {
    "fantasy": {"primary": "#54C68A", "secondary": "#F0C46E", "dark": "#24344C",
                "accent": "#F56F87", "ground": "#586D45"},
    "horror": {"primary": "#A598C7", "secondary": "#E0DAD2", "dark": "#211D30",
               "accent": "#E65A66", "ground": "#464253"},
    "scifi": {"primary": "#48CAE4", "secondary": "#F9E670", "dark": "#172A45",
              "accent": "#FF718A", "ground": "#344967"},
}


def rgb(code: str) -> tuple[int, int, int]:
    return tuple(bytes.fromhex(code.removeprefix("#")))  # type: ignore[return-value]


def png(path: Path, width: int, height: int, pixels: bytearray) -> None:
    if len(pixels) != width * height * 4:
        raise ValueError("Invalid RGBA size")

    def chunk(tag: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + tag + body + struct.pack(
            ">I", zlib.crc32(tag + body) & 0xFFFFFFFF
        )

    rows = b"".join(
        b"\x00" + pixels[y * width * 4:(y + 1) * width * 4]
        for y in range(height)
    )
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )


def canvas(width: int, height: int) -> bytearray:
    return bytearray(width * height * 4)


def rect(pixels: bytearray, width: int, height: int, x0: int, y0: int,
         x1: int, y1: int, color: tuple[int, int, int], alpha: int = 255) -> None:
    for y in range(max(0, y0), min(height, y1)):
        for x in range(max(0, x0), min(width, x1)):
            idx = (y * width + x) * 4
            pixels[idx:idx + 4] = bytes((*color, alpha))


def character_sheet(path: Path, palette: dict[str, str]) -> None:
    width, height = 128, 32
    pixels = canvas(width, height)
    body, face, dark, accent = (rgb(palette[k]) for k in
                                ("primary", "secondary", "dark", "accent"))
    for frame in range(4):
        offset = frame * 32
        bounce = [0, 1, 0, 1][frame]
        rect(pixels, width, height, offset + 10, 7 + bounce,
             offset + 22, 17 + bounce, face)
        rect(pixels, width, height, offset + 8, 16 + bounce,
             offset + 24, 26 + bounce, body)
        rect(pixels, width, height, offset + 12, 11 + bounce,
             offset + 14, 13 + bounce, dark)
        rect(pixels, width, height, offset + 19, 11 + bounce,
             offset + 21, 13 + bounce, dark)
        rect(pixels, width, height, offset + 9, 5 + bounce,
             offset + 23, 8 + bounce, accent)
        rect(pixels, width, height, offset + 5 + frame % 2 * 2, 17,
             offset + 9 + frame % 2 * 2, 25, body)
        rect(pixels, width, height, offset + 24 - frame % 2 * 2, 17,
             offset + 28 - frame % 2 * 2, 25, body)
        rect(pixels, width, height, offset + 10 + (frame % 2) * 2, 25,
             offset + 15 + (frame % 2) * 2, 30, dark)
        rect(pixels, width, height, offset + 18 - (frame % 2) * 2, 25,
             offset + 23 - (frame % 2) * 2, 30, dark)
    png(path, width, height, pixels)


def coin_png(path: Path, palette: dict[str, str]) -> None:
    width = height = 32
    pixels = canvas(width, height)
    gold = rgb(palette["secondary"])
    edge = rgb(palette["accent"])
    for y in range(32):
        for x in range(32):
            d = (x - 15.5) ** 2 + (y - 15.5) ** 2
            if d < 13.5 ** 2:
                rect(pixels, width, height, x, y, x + 1, y + 1,
                     gold if d < 11.0 ** 2 else edge)
    rect(pixels, width, height, 14, 7, 18, 24, edge)
    png(path, width, height, pixels)


def tiles_png(path: Path, palette: dict[str, str], seed: int) -> None:
    pixels = canvas(64, 32)
    base = rgb(palette["ground"])
    dark = rgb(palette["dark"])
    light = rgb(palette["primary"])
    rng = random.Random(seed)
    rect(pixels, 64, 32, 0, 0, 64, 32, base)
    rect(pixels, 64, 32, 0, 0, 32, 5, light)
    for _ in range(32):
        x, y = rng.randrange(0, 64), rng.randrange(5, 32)
        rect(pixels, 64, 32, x, y, x + 2, y + 2, dark, 210)
    png(path, 64, 32, pixels)


def ui_png(path: Path, palette: dict[str, str]) -> None:
    pixels = canvas(96, 24)
    border = rgb(palette["primary"])
    fill = rgb(palette["dark"])
    rect(pixels, 96, 24, 0, 0, 96, 24, border, 255)
    rect(pixels, 96, 24, 2, 2, 94, 22, fill, 245)
    rect(pixels, 96, 24, 6, 7, 9, 17, rgb(palette["secondary"]))
    png(path, 96, 24, pixels)


def make_glb(path: Path, palette: dict[str, str], name: str) -> None:
    # glTF 2.0: one cube mesh with two named node-transform animation clips.
    vertices = [
        (-0.45, -0.5, -0.45), (0.45, -0.5, -0.45),
        (0.45, 0.5, -0.45), (-0.45, 0.5, -0.45),
        (-0.45, -0.5, 0.45), (0.45, -0.5, 0.45),
        (0.45, 0.5, 0.45), (-0.45, 0.5, 0.45),
    ]
    triangles = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7,
        0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2,
        1, 2, 6, 1, 6, 5, 0, 4, 7, 0, 7, 3,
    ]
    blob = bytearray()
    views = []
    accessors = []

    def add(values: list[float] | list[int], fmt: str, component: int,
            kind: str, target: int | None = None,
            extrema: tuple[list[float], list[float]] | None = None) -> int:
        while len(blob) % 4:
            blob.append(0)
        start = len(blob)
        blob.extend(struct.pack("<" + str(len(values)) + fmt, *values))
        view = {"buffer": 0, "byteOffset": start, "byteLength": len(blob) - start}
        if target is not None:
            view["target"] = target
        view_index = len(views)
        views.append(view)
        count = len(values) // {"SCALAR": 1, "VEC3": 3}[kind]
        accessor = {"bufferView": view_index, "componentType": component,
                    "count": count, "type": kind}
        if extrema is not None:
            accessor["min"], accessor["max"] = extrema
        accessors.append(accessor)
        return len(accessors) - 1

    position = add([n for vertex in vertices for n in vertex], "f", 5126, "VEC3",
                   34962, ([-0.45, -0.5, -0.45], [0.45, 0.5, 0.45]))
    index = add(triangles, "H", 5123, "SCALAR", 34963)
    times = add([0.0, 0.5, 1.0], "f", 5126, "SCALAR",
                extrema=([0.0], [1.0]))
    idle = add([0., 0., 0., 0., 0.12, 0., 0., 0., 0.],
               "f", 5126, "VEC3")
    action = add([0., 0., 0., 0., 0.35, 0.18, 0., 0., 0.],
                 "f", 5126, "VEC3")
    color = [component / 255 for component in rgb(palette["primary"])]
    gltf = {
        "asset": {"version": "2.0", "generator": "Futty procedural prototype pack"},
        "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [{"name": name, "mesh": 0}],
        "meshes": [{"name": name, "primitives": [
            {"attributes": {"POSITION": position}, "indices": index,
             "material": 0, "mode": 4}]}],
        "materials": [{"name": "Theme", "pbrMetallicRoughness": {
            "baseColorFactor": [*color, 1.0], "metallicFactor": 0.0,
            "roughnessFactor": 0.85}}],
        "animations": [
            {"name": clip_name,
             "samplers": [{"input": times, "output": output,
                            "interpolation": "LINEAR"}],
             "channels": [{"sampler": 0, "target": {
                 "node": 0, "path": "translation"}}]}
            for clip_name, output in (("Idle", idle), ("Action", action))
        ],
        "buffers": [{"byteLength": len(blob)}],
        "bufferViews": views, "accessors": accessors,
    }
    source = json.dumps(gltf, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    source += b" " * (-len(source) % 4)
    blob.extend(b"\x00" * (-len(blob) % 4))
    total = 12 + 8 + len(source) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<I4s", len(source), b"JSON") + source
        + struct.pack("<I4s", len(blob), b"BIN\x00") + blob
    )


def wav(path: Path, kind: str, seed: int) -> None:
    sample_rate = 22050
    duration = {"jump": 0.23, "pickup": 0.34, "ambience": 1.8}[kind]
    rng = random.Random(seed)
    samples = []
    for i in range(round(sample_rate * duration)):
        t = i / sample_rate
        if kind == "jump":
            frequency = 290 + 210 * t / duration
            signal = math.sin(2 * math.pi * frequency * t)
            amplitude = 0.25 * (1 - t / duration) ** 2
        elif kind == "pickup":
            frequency = 440 + 340 * t / duration
            signal = math.sin(2 * math.pi * frequency * t)
            amplitude = 0.24 * (1 - t / duration) ** 2
        else:
            # Quiet seamless-ish placeholder: not a mastered music track.
            signal = (math.sin(2 * math.pi * 110 * t)
                      + 0.5 * math.sin(2 * math.pi * 165 * t)
                      + rng.uniform(-0.13, 0.13))
            amplitude = 0.085 * (0.5 - 0.5 * math.cos(2 * math.pi * t / duration))
        samples.append(int(max(-1, min(1, signal * amplitude)) * 32767))
    with wave.open(str(path), "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(sample_rate)
        audio.writeframes(struct.pack("<" + str(len(samples)) + "h", *samples))


def create_pack(destination: Path, name: str, theme: str, seed: int) -> dict:
    if destination.exists() and any(destination.iterdir()):
        raise ValueError("Destination exists and is not empty; choose a new directory")
    destination.mkdir(parents=True, exist_ok=True)
    palette = PALETTES[theme]
    outputs = [
        ("sprites/hero_walk.png", "texture", "2D 4-frame 32x32 sprite sheet"),
        ("sprites/coin.png", "texture", "2D collectible sprite"),
        ("sprites/tiles.png", "texture", "2D 2-tile 32x32 atlas"),
        ("ui/hud_panel.png", "texture", "HUD panel graphic"),
        ("models/hero.glb", "model", "Low-poly placeholder; Idle and Action clips"),
        ("audio/jump.wav", "audio", "Jump effect"),
        ("audio/pickup.wav", "audio", "Pickup effect"),
        ("audio/ambience.wav", "audio", "Loopable ambient placeholder"),
    ]
    for relative, _, _ in outputs:
        (destination / relative).parent.mkdir(parents=True, exist_ok=True)
    character_sheet(destination / outputs[0][0], palette)
    coin_png(destination / outputs[1][0], palette)
    tiles_png(destination / outputs[2][0], palette, seed)
    ui_png(destination / outputs[3][0], palette)
    make_glb(destination / outputs[4][0], palette, name + " Hero")
    for n, rel in enumerate(["audio/jump.wav", "audio/pickup.wav", "audio/ambience.wav"]):
        wav(destination / rel, rel.split("/")[-1].removesuffix(".wav"), seed + n)
    art_direction = {
        "project": name, "theme": theme, "seed": seed, "palette": palette,
        "2d": {"sprite_cell": [32, 32], "pixels_per_unit": 32,
               "hero_walk_frames": 4, "tiles_cell": [32, 32]},
        "3d": {"units": "meters", "up_axis": "Y", "style": "simple low-poly",
               "hero_animation_clips": ["Idle", "Action"]},
        "audio": {"format": "WAV PCM16 mono", "sample_rate": 22050},
        "quality": "procedural prototype placeholders only; replace for polished games",
    }
    (destination / "art-direction.json").write_text(
        json.dumps(art_direction, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    entries = []
    for relative, kind, description in outputs:
        content = (destination / relative).read_bytes()
        entries.append({
            "path": relative, "kind": kind, "description": description,
            "sha256": hashlib.sha256(content).hexdigest(),
        })
    manifest = {
        "schema_version": 1, "project": name, "theme": theme,
        "generator": "Futty standard-library prototype asset pack",
        "generator_type": "procedural", "not_ai_generated": True,
        "art_direction": "art-direction.json", "assets": entries,
        "integration": "Use Futty MCP asset_import with an existing saved scene; "
                       "then assign returned asset IDs using scene_apply and script/HUD tools.",
        "license_note": "Generator-authored placeholder assets; inspect project "
                        "and third-party content licenses before distributing a game.",
    }
    (destination / "asset-manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path,
                        help="New output folder; keep inside Futty MCP workspace")
    parser.add_argument("--name", default="Futty Prototype")
    parser.add_argument("--theme", choices=sorted(PALETTES), default="fantasy")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()
    if not re.fullmatch(r"[\w ._-]{1,70}", args.name, flags=re.UNICODE):
        parser.error("Project name must be 1-70 letters, digits, spaces, '.', '_' or '-'")
    try:
        manifest = create_pack(args.output, args.name, args.theme, args.seed)
    except (ValueError, OSError) as exc:
        parser.error(str(exc))
    print(json.dumps({
        "success": True, "directory": str(args.output.resolve()),
        "manifest": str((args.output / "asset-manifest.json").resolve()),
        "asset_count": len(manifest["assets"]),
        "kind": "prototype placeholders; NOT final AI image/3D/audio generation",
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
