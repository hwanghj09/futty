"""Smoke tests for Futty's one-command procedural prototype asset pack."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
import tempfile
import unittest
import wave
import zlib
from pathlib import Path


SOURCE = Path(__file__).with_name("generate-prototype-assets.py")
spec = importlib.util.spec_from_file_location("futty_asset_pack", SOURCE)
assert spec and spec.loader
asset_pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(asset_pack)


class PrototypeAssetPackTests(unittest.TestCase):
    def test_pack_contains_all_expected_assets(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "game"
            manifest = asset_pack.create_pack(out, "Test", "fantasy", 42)
            self.assertEqual(len(manifest["assets"]), 8)
            self.assertTrue(manifest["not_ai_generated"])
            for entry in manifest["assets"]:
                content = (out / entry["path"]).read_bytes()
                self.assertEqual(hashlib.sha256(content).hexdigest(), entry["sha256"])
            self.assertEqual(
                json.loads((out / "art-direction.json").read_text())["theme"],
                "fantasy",
            )

    def test_png_headers_crc_and_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "game"
            asset_pack.create_pack(out, "Test", "horror", 7)
            expected = {
                "sprites/hero_walk.png": (128, 32),
                "sprites/coin.png": (32, 32),
                "sprites/tiles.png": (64, 32),
                "ui/hud_panel.png": (96, 24),
            }
            for rel, dimensions in expected.items():
                data = (out / rel).read_bytes()
                self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
                position = 8
                chunks = []
                compressed = b""
                while position < len(data):
                    size = struct.unpack(">I", data[position:position + 4])[0]
                    tag = data[position + 4:position + 8]
                    payload = data[position + 8:position + 8 + size]
                    crc = struct.unpack(">I", data[position + 8 + size:position + 12 + size])[0]
                    self.assertEqual(crc, zlib.crc32(tag + payload) & 0xFFFFFFFF)
                    position += 12 + size
                    chunks.append(tag)
                    if tag == b"IHDR":
                        self.assertEqual(struct.unpack(">II", payload[:8]), dimensions)
                    if tag == b"IDAT":
                        compressed += payload
                self.assertEqual(chunks, [b"IHDR", b"IDAT", b"IEND"])
                width, height = dimensions
                self.assertEqual(len(zlib.decompress(compressed)), height * (width * 4 + 1))

    def test_glb_has_mesh_and_two_animation_clips(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "game"
            asset_pack.create_pack(out, "Test", "scifi", 10)
            data = (out / "models/hero.glb").read_bytes()
            magic, version, size = struct.unpack("<4sII", data[:12])
            self.assertEqual((magic, version, size), (b"glTF", 2, len(data)))
            length, tag = struct.unpack("<I4s", data[12:20])
            self.assertEqual(tag, b"JSON")
            gltf = json.loads(data[20:20 + length])
            self.assertEqual([anim["name"] for anim in gltf["animations"]], ["Idle", "Action"])
            self.assertEqual(len(gltf["meshes"]), 1)
            self.assertEqual(gltf["meshes"][0]["primitives"][0]["mode"], 4)
            self.assertEqual(gltf["buffers"][0]["byteLength"] % 4, 0)
            binary_length, binary_type = struct.unpack(
                "<I4s", data[20 + length:28 + length]
            )
            self.assertEqual(binary_type, b"BIN\x00")
            self.assertEqual(binary_length, gltf["buffers"][0]["byteLength"])

    def test_wav_files_are_decodable_and_nonempty(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "game"
            asset_pack.create_pack(out, "Test", "fantasy", 42)
            for rel in ("audio/jump.wav", "audio/pickup.wav", "audio/ambience.wav"):
                with wave.open(str(out / rel), "rb") as audio:
                    self.assertGreater(audio.getnframes(), 0)
                    self.assertEqual(audio.getframerate(), 22050)
                    self.assertEqual(audio.getnchannels(), 1)
                    self.assertEqual(audio.getsampwidth(), 2)

    def test_seed_reproducible_and_existing_directory_protected(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            first = asset_pack.create_pack(a, "Test", "horror", 99)
            second = asset_pack.create_pack(b, "Test", "horror", 99)
            self.assertEqual(first, second)
            with self.assertRaises(ValueError):
                asset_pack.create_pack(a, "Test", "horror", 99)


if __name__ == "__main__":
    unittest.main()
