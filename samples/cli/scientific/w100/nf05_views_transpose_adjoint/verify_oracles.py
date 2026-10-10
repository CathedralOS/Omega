"""Independent integer-coordinate oracle; never a shipped descriptor implementation."""
import json
from pathlib import Path

fixtures = json.loads(Path(__file__).with_name("oracles.json").read_text())
checks = 0

def equal(actual, expected):
    global checks
    assert actual == expected, (actual, expected)
    checks += 1

def flatten(backing, shape, offset, strides):
    rows, columns = shape
    return [backing[offset + row * strides[0] + column * strides[1]]
            for column in range(columns) for row in range(rows)]

for name in ("packed", "padded"):
    fixture = fixtures[name]
    rows, columns = fixture["shape"]
    row_stride, column_stride = fixture["strides"]
    equal(flatten(fixture["backing"], [columns, rows], fixture["offset"],
                  [column_stride, row_stride]), fixture["transpose_flat"])
    equal(flatten(fixture["backing"], [rows, columns], fixture["offset"],
                  [row_stride, column_stride]), fixtures["packed"]["backing"])
packed = fixtures["packed"]
equal(flatten(packed["backing"], packed["reshape_shape"], 0, [1, 3]),
      packed["reshape_flat"])
padded = fixtures["padded"]
patch = fixtures["patch"]
equal(flatten(padded["backing"], patch["shape"], patch["offset"], patch["strides"]),
      patch["flat"])
mutated = padded["backing"].copy()
mutated[patch["offset"]] = 17
equal(mutated, patch["mutated_backing"])
# Empty corner (2,3) has a formal address 12 beyond the 10-element backing.
# Enumeration of zero coordinates must perform no read, for each empty shape.
for shape in ([0, 0], [0, 3], [2, 0]):
    equal(flatten(padded["backing"], shape, padded["offset"], padded["strides"]), [])
repeated = fixtures["readonly_alias"]
equal(flatten(packed["backing"], repeated["shape"], repeated["offset"],
              repeated["strides"]), repeated["flat"])
addresses = [repeated["offset"] + row * repeated["strides"][0]
             + column * repeated["strides"][1]
             for column in range(3) for row in range(2)]
equal(len(set(addresses)) == len(addresses), False)
# An overlapping nonzero-stride mapping also cannot grant mutable access.
equal(len(set(row + column for column in range(2) for row in range(2))), 3)
fixture = fixtures["adjoint"]
values = [complex(real, imaginary) for real, imaginary
          in zip(fixture["real"], fixture["imaginary"])]
adjoint = [values[column + row*2].conjugate()
           for column in range(2) for row in range(2)]
equal([int(value.real) for value in adjoint], fixture["adjoint_real"])
equal([int(value.imag) for value in adjoint], fixture["adjoint_imaginary"])
equal([adjoint[column + row*2].conjugate()
       for column in range(2) for row in range(2)], values)
gram = [sum(values[index + row*2].conjugate() * values[index + column*2]
            for index in range(2)) for column in range(2) for row in range(2)]
equal([int(value.real) for value in gram], fixture["gram_real"])
equal([int(value.imag) for value in gram], fixture["gram_imaginary"])
equal(gram[1], gram[2].conjugate())
print(f"{checks} independent frozen oracle comparisons passed")
