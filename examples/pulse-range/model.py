"""Build the original Pulse Range models and audio with Blender's Python runtime."""

import math
from pathlib import Path
from random import Random
import struct
import sys
import wave

import bpy
from mathutils import Vector


OUT = Path(sys.argv[sys.argv.index("--") + 1]).resolve()
OUT.mkdir(parents=True, exist_ok=True)


def space(xyz):
    x, y, z = xyz
    return Vector((x, -z, y))


def material(name, rgb):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*rgb, 1.0)
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = mat.diffuse_color
    return mat


def paint(obj, mat):
    obj.data.materials.append(mat)
    return obj


def box(name, pos, size, mat, bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=space(pos))
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = (size[0], size[2], size[1])
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        edge = obj.modifiers.new("Rounded edges", "BEVEL")
        edge.width = bevel
        edge.segments = 2
    return paint(obj, mat)


def barrel(name, pos, radius, length, mat, direction=(0, 0, 1), sides=12):
    bpy.ops.mesh.primitive_cylinder_add(vertices=sides, radius=radius, depth=length, location=space(pos))
    obj = bpy.context.object
    obj.name = name
    obj.rotation_euler = space(direction).to_track_quat("Z", "Y").to_euler()
    return paint(obj, mat)


def ring(name, pos, radius, thickness, mat):
    bpy.ops.mesh.primitive_torus_add(
        major_segments=24, minor_segments=6, location=space(pos),
        rotation=(math.pi / 2, 0, 0), major_radius=radius, minor_radius=thickness,
    )
    obj = bpy.context.object
    obj.name = name
    return paint(obj, mat)


def sign(text, pos, size, mat):
    bpy.ops.object.text_add(location=space(pos), rotation=(math.pi / 2, 0, 0))
    obj = bpy.context.object
    obj.data.body = text
    obj.data.align_x = "CENTER"
    obj.data.align_y = "CENTER"
    obj.data.size = size
    obj.data.extrude = 0.004
    obj.data.resolution_u = 4
    bpy.ops.object.convert(target="MESH")
    paint(bpy.context.object, mat)


def export(name):
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.convert(target="MESH")
    bpy.ops.object.join()
    bpy.context.object.name = name
    bpy.ops.export_scene.gltf(filepath=str(OUT / f"{name}.glb"), export_format="GLB")
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)


bpy.ops.wm.read_factory_settings(use_empty=True)
coal = material("Carbon blue", (0.025, 0.043, 0.060))
steel = material("Slate panels", (0.100, 0.155, 0.188))
stone = material("Cool concrete", (0.259, 0.324, 0.350))
cream = material("Ceramic white", (0.725, 0.784, 0.764))
mint = material("Pulse teal", (0.030, 0.740, 0.602))
orange = material("Safety amber", (0.944, 0.281, 0.066))
shadow = material("Rubber", (0.019, 0.025, 0.031))

box("Deck", (0, -0.22, -1.75), (15.6, 0.4, 26.0), coal)
for col in range(6):
    for row in range(10):
        x = (col - 2.5) * 2.59
        z = (row - 4.5) * 2.59 - 1.75
        tile = steel if (col + row) % 5 == 0 else stone
        box("Deck tile", (x, -0.045, z), (2.56, 0.09, 2.56), tile, 0.015)

for x in (-7.65, 7.65):
    box("Side wall", (x, 1.32, -1.9), (0.42, 2.65, 26.1), steel, 0.08)
    box("Wall cap", (x, 2.70, -1.9), (0.56, 0.20, 26.1), cream, 0.05)
    box("Amber rail", (x * 0.974, 1.52, -1.9), (0.055, 0.06, 26.0), orange)
    for z in (-12.0, -8.0, -4.0, 0.0, 4.0, 8.0, 10.5):
        box("Side pier", (x * 0.97, 1.60, z), (0.62, 3.2, 0.42), coal, 0.07)
        box("Side lamp", (x * 0.924, 2.08, z), (0.04, 0.28, 0.15), mint, 0.018)

box("Backstop", (0, 2.39, -14.52), (15.6, 4.8, 0.52), steel, 0.10)
box("Backstop cap", (0, 4.82, -14.52), (15.6, 0.24, 0.58), coal, 0.04)
box("Rear railing", (0, 0.67, 11.08), (15.6, 1.3, 0.35), steel, 0.06)
box("Rear railing cap", (0, 1.35, 11.08), (15.6, 0.12, 0.39), orange, 0.03)
for x in (-5.45, -2.73, 0.0, 2.73, 5.45):
    box("Backstop damper", (x, 1.75, -14.19), (2.55, 3.25, 0.24), coal, 0.075)
    for h in (0.36, 0.66, 0.96, 1.26, 1.56, 1.86, 2.16, 2.46, 2.76):
        box("Damper slat", (x, h, -14.045), (2.35, 0.08, 0.07), steel, 0.02)
for x, number in [(-5.45, "01"), (0, "02"), (5.45, "03")]:
    box("Lane light frame", (x, 3.44, -14.05), (1.64, 0.41, 0.16), coal, 0.06)
    sign(number, (x, 3.42, -13.95), 0.28, mint)

box("Sign panel", (0, 4.19, -14.06), (8.43, 0.90, 0.28), coal, 0.07)
sign("PULSE RANGE", (0, 4.19, -13.90), 0.68, cream)
for x in (-4.05, 4.05):
    box("Sign end cap", (x, 4.19, -13.90), (0.09, 0.64, 0.05), orange)

box("Firing line", (0, 0.018, 3.38), (14.15, 0.035, 0.27), coal, 0.025)
for x in range(-7, 8):
    dash = box("Firing line dash", (x * 0.94, 0.04, 3.38), (0.60, 0.013, 0.12), orange)
    dash.rotation_euler.z = 0.17
for x in (-3.35, 3.35):
    box("Lane guide", (x, 0.013, -3.5), (0.06, 0.022, 13.5), cream)
    for z in (0.4, -3.6, -7.6, -11.6):
        box("Distance mark", (x, 0.018, z), (0.36, 0.019, 0.12), mint)
for side in (-1, 1):
    for z in (5.25, 8.25):
        x = side * 7.12
        box("Storage case", (x, 0.40, z), (0.82, 0.8, 1.34), coal, 0.08)
        box("Case lid", (x, 0.81, z), (0.88, 0.16, 1.38), steel, 0.055)
        for end in (-0.48, 0.48):
            box("Case band", (x, 0.84, z + end), (0.89, 0.13, 0.11), orange, 0.026)
export("Courtyard")

bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.43)
body = bpy.context.object
body.scale = (1, 0.80, 0.91)
paint(body, steel)
for side in (-1, 1):
    box("Stabilizer", (side * 0.38, 0.0, -0.05), (0.36, 0.23, 0.31), coal, 0.063)
    box("Pulse cell", (side * 0.47, 0.0, 0.085), (0.12, 0.12, 0.037), mint, 0.015)
    box("Ceramic cap", (side * 0.31, 0.22, -0.015), (0.18, 0.15, 0.31), cream, 0.04)
barrel("Armored face", (0, 0, 0.31), 0.36, 0.13, coal, sides=16)
ring("Amber target ring", (0, 0, 0.385), 0.28, 0.045, orange)
barrel("Recess", (0, 0, 0.396), 0.231, 0.025, shadow, sides=20)
ring("Inner ring", (0, 0, 0.416), 0.151, 0.021, cream)
barrel("Pulse core", (0, 0, 0.438), 0.092, 0.044, mint, sides=16)
export("Drone")

box("Pulse housing", (0, 0.025, 0.015), (0.216, 0.181, 0.611), coal, 0.043)
box("Ceramic body", (0, 0.082, -0.030), (0.247, 0.171, 0.443), cream, 0.048)
box("Grip", (0, -0.15, 0.116), (0.137, 0.249, 0.173), shadow, 0.035).rotation_euler.x = -0.21
box("Cell chassis", (0, -0.078, -0.105), (0.173, 0.168, 0.216), steel, 0.026)
box("Cell stripe", (0, -0.125, -0.119), (0.182, 0.027, 0.162), orange, 0.008)
for side in (-1, 1):
    for z in (-0.148, -0.079, -0.01):
        box("Pulse channel", (side * 0.122, 0.084, z), (0.027, 0.046, 0.038), mint, 0.007)
barrel("Emitter shell", (0, 0.052, -0.325), 0.102, 0.21, steel)
barrel("Amber rim", (0, 0.052, -0.403), 0.113, 0.068, orange)
barrel("Pulse aperture", (0, 0.052, -0.444), 0.071, 0.022, coal)
barrel("Emitter", (0, 0.052, -0.459), 0.042, 0.016, mint)
box("Sight base", (0, 0.191, 0.011), (0.142, 0.046, 0.158), coal, 0.012)
for x in (-0.046, 0.046):
    box("Sight post", (x, 0.231, -0.04), (0.022, 0.055, 0.05), coal, 0.007)
box("Sight indicator", (0, 0.215, -0.041), (0.044, 0.017, 0.039), mint, 0.005)
export("Blaster")


def sound(name, duration, sample):
    rate = 24000
    noise = Random(15)
    data = bytearray()
    for frame in range(round(rate * duration)):
        t = frame / rate
        envelope = min(1.0, t / 0.004) * (1.0 - t / duration) ** 2
        data.extend(struct.pack("<h", round(23500 * envelope * sample(t, noise))))
    with wave.open(str(OUT / f"{name}.wav"), "wb") as file:
        file.setparams((1, 2, rate, 0, "NONE", "not compressed"))
        file.writeframes(data)


sound("Pulse", 0.17, lambda t, noise: 0.62 * math.sin(math.tau * (460 * t - 790 * t * t)) + 0.17 * noise.uniform(-1, 1))
sound("Confirm", 0.20, lambda t, _: 0.52 * math.sin(math.tau * 920 * t) + 0.27 * math.sin(math.tau * 1380 * t))
