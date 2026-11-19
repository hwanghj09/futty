"""Build the original Beacon Bot GLB and its three skeletal clips."""

import json
import math
from pathlib import Path
import struct


data = bytearray()
doc = {
    "asset": {"version": "2.0", "generator": "Futty Beacon Bot"},
    "scene": 0,
    "scenes": [{"nodes": [0, 5]}],
    "accessors": [],
    "bufferViews": [],
}


def accessor(rows, code, component, shape, bounds=False):
    while len(data) % 4:
        data.append(0)
    offset = len(data)
    for row in rows:
        data.extend(struct.pack("<" + code * len(row), *row))
    view = len(doc["bufferViews"])
    doc["bufferViews"].append({
        "buffer": 0, "byteOffset": offset, "byteLength": len(data) - offset,
    })
    attributes = {"bufferView": view, "componentType": component,
                  "count": len(rows), "type": shape}
    if bounds:
        attributes["min"] = [min(column) for column in zip(*rows)]
        attributes["max"] = [max(column) for column in zip(*rows)]
    doc["accessors"].append(attributes)
    return len(doc["accessors"]) - 1


names = ["Torso", "Right arm", "Left arm", "Right leg", "Left leg"]
pivots = [(0, .91, 0), (.47, 1.27, 0), (-.47, 1.27, 0),
          (.20, .74, 0), (-.20, .74, 0)]
doc["nodes"] = [
    {"name": name, "translation": list(pivot) if index == 0 else
     [value - origin for value, origin in zip(pivot, pivots[0])]}
    for index, (name, pivot) in enumerate(zip(names, pivots))
]
doc["nodes"][0]["children"] = [1, 2, 3, 4]
doc["nodes"].append({"name": "Beacon Bot", "mesh": 0, "skin": 0})
bind = [(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -x, -y, -z, 1)
        for x, y, z in pivots]
doc["skins"] = [{"name": "Beacon Bot rig", "skeleton": 0, "joints": list(range(5)),
                 "inverseBindMatrices": accessor(bind, "f", 5126, "MAT4")}]

colors = [
    ("Teal enamel", (.10, .60, .65)),
    ("Steel", (.14, .23, .30)),
    ("Amber", (.98, .43, .08)),
    ("Beacon", (.53, .96, .82)),
]
doc["materials"] = [
    {"name": name, "pbrMetallicRoughness": {"baseColorFactor": [*color, 1]}}
    for name, color in colors
]
parts = [[[], [], [], [], []] for _ in colors]


def box(center, size, joint, material, shoulder=False):
    positions, normals, joints, weights, indices = parts[material]
    for axis in range(3):
        tangent, bitangent = (axis + 1) % 3, (axis + 2) % 3
        for sign in (-1, 1):
            base = len(positions)
            for u, v in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
                point = list(center)
                point[axis] += sign * size[axis] / 2
                point[tangent] += u * size[tangent] / 2
                point[bitangent] += v * size[bitangent] / 2
                normal = [0, 0, 0]
                normal[axis] = sign
                positions.append(point)
                normals.append(normal)
                if shoulder and point[1] > center[1]:
                    joints.append((joint, 0, 0, 0))
                    weights.append((.7, .3, 0, 0))
                else:
                    joints.append((joint, 0, 0, 0))
                    weights.append((1, 0, 0, 0))
            face = (0, 1, 2, 0, 2, 3) if sign > 0 else (0, 2, 1, 0, 3, 2)
            indices.extend((base + index,) for index in face)


box((0, 1.07, 0), (.72, .55, .43), 0, 0)
box((0, .79, 0), (.55, .18, .36), 0, 1)
box((0, 1.54, 0), (.53, .31, .42), 0, 0)
box((0, 1.55, .213), (.43, .16, .032), 0, 1)
box((-.106, 1.557, .235), (.092, .05, .019), 0, 3)
box((.106, 1.557, .235), (.092, .05, .019), 0, 3)
box((.146, 1.746, 0), (.052, .105, .052), 0, 2)
box((.13, 1.122, .222), (.13, .096, .05), 0, 2)
for sign, arm, leg in ((1, 1, 3), (-1, 2, 4)):
    box((sign * .50, 1.205, 0), (.24, .20, .35), arm, 2, shoulder=True)
    box((sign * .56, .98, 0), (.17, .31, .24), arm, 0)
    box((sign * .56, .755, .015), (.195, .16, .26), arm, 1)
    box((sign * .203, .455, 0), (.25, .51, .30), leg, 1)
    box((sign * .203, .42, .158), (.19, .24, .053), leg, 0)
    box((sign * .203, .108, .077), (.28, .20, .44), leg, 0)
    box((sign * .203, .135, .291), (.15, .059, .025), leg, 2)

primitives = []
for material, (positions, normals, joints, weights, indices) in enumerate(parts):
    primitives.append({
        "attributes": {
            "POSITION": accessor(positions, "f", 5126, "VEC3", bounds=True),
            "NORMAL": accessor(normals, "f", 5126, "VEC3"),
            "JOINTS_0": accessor(joints, "B", 5121, "VEC4"),
            "WEIGHTS_0": accessor(weights, "f", 5126, "VEC4"),
        },
        "indices": accessor(indices, "H", 5123, "SCALAR"),
        "material": material,
    })
doc["meshes"] = [{"name": "Beacon Bot", "primitives": primitives}]


def rotation(axis, degrees):
    angle = math.radians(degrees) / 2
    return (*[math.sin(angle) if coordinate == axis else 0 for coordinate in range(3)],
            math.cos(angle))


def clip(name, times, tracks):
    timeline = accessor([(time,) for time in times], "f", 5126, "SCALAR", bounds=True)
    samplers, channels = [], []
    for joint, axis, angles in tracks:
        samples = [rotation(axis, angle) for angle in angles]
        samplers.append({"input": timeline, "output": accessor(samples, "f", 5126, "VEC4"),
                         "interpolation": "LINEAR"})
        channels.append({"sampler": len(samplers) - 1, "target": {"node": joint, "path": "rotation"}})
    return {"name": name, "samplers": samplers, "channels": channels}


doc["animations"] = [
    clip("Idle", (0, .6, 1.2, 1.8, 2.4), [
        (0, 1, (0, 11, 0, -11, 0)), (1, 2, (0, 8, 0, 8, 0)),
    ]),
    clip("Walk", (0, .24, .48, .72, .96), [
        (1, 0, (30, 0, -30, 0, 30)), (2, 0, (-30, 0, 30, 0, -30)),
        (3, 0, (-32, 0, 32, 0, -32)), (4, 0, (32, 0, -32, 0, 32)),
    ]),
    clip("Signal", (0, .35, .61, .87, 1.13, 1.65), [
        (1, 2, (0, 113, 145, 111, 139, 0)),
    ]),
]
doc["buffers"] = [{"byteLength": len(data)}]
json_bytes = json.dumps(doc, separators=(",", ":")).encode("utf-8")
json_bytes += b" " * (-len(json_bytes) % 4)
data.extend(b"\x00" * (-len(data) % 4))
glb = struct.pack("<4sII", b"glTF", 2, 28 + len(json_bytes) + len(data))
glb += struct.pack("<I4s", len(json_bytes), b"JSON") + json_bytes
glb += struct.pack("<I4s", len(data), b"BIN\x00") + data
output = Path(__file__).parent / "Assets/Futty/6c5cb6d0-3ec5-4142-8af3-368c0c033dde.glb"
output.parent.mkdir(parents=True, exist_ok=True)
output.write_bytes(glb)
