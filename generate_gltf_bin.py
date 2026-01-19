# Script to generate a simple cube binary data for glTF
import struct

# Cube vertices (positions)
positions = [
    -1.0, -1.0, -1.0,
     1.0, -1.0, -1.0,
     1.0,  1.0, -1.0,
    -1.0,  1.0, -1.0,
    -1.0, -1.0,  1.0,
     1.0, -1.0,  1.0,
     1.0,  1.0,  1.0,
    -1.0,  1.0,  1.0,
]

# Normals (pointing outward from each vertex)
normals = [
    -0.577, -0.577, -0.577,
     0.577, -0.577, -0.577,
     0.577,  0.577, -0.577,
    -0.577,  0.577, -0.577,
    -0.577, -0.577,  0.577,
     0.577, -0.577,  0.577,
     0.577,  0.577,  0.577,
    -0.577,  0.577,  0.577,
]

# Colors (one per vertex - rainbow colors)
colors = [
    1.0, 0.0, 0.0,  # red
    0.0, 1.0, 0.0,  # green
    0.0, 0.0, 1.0,  # blue
    1.0, 1.0, 0.0,  # yellow
    1.0, 0.0, 1.0,  # magenta
    0.0, 1.0, 1.0,  # cyan
    1.0, 1.0, 1.0,  # white
    0.5, 0.5, 0.5,  # gray
]

# Indices for the cube (36 indices for 12 triangles)
indices = [
    # Front face
    0, 1, 2, 2, 3, 0,
    # Back face
    4, 6, 5, 6, 4, 7,
    # Left face
    4, 0, 3, 3, 7, 4,
    # Right face
    1, 5, 6, 6, 2, 1,
    # Top face
    3, 2, 6, 6, 7, 3,
    # Bottom face
    4, 5, 1, 1, 0, 4,
]

# Pack data as binary
data = b''

# Pack positions (96 bytes: 8 vertices * 3 floats * 4 bytes)
for v in positions:
    data += struct.pack('f', v)

# Pack normals (96 bytes)
for n in normals:
    data += struct.pack('f', n)

# Pack colors (96 bytes)
for c in colors:
    data += struct.pack('f', c)

# Pack indices (144 bytes: 36 indices * 4 bytes)
for i in indices:
    data += struct.pack('I', i)

# Write to file
with open('models/scene.bin', 'wb') as f:
    f.write(data)

print(f'Generated scene.bin with {len(data)} bytes')
print('Place this file in the models/ directory alongside scene.gltf')
