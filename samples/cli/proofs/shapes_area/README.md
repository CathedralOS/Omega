# Shapes Area

Computes integer areas for three shape kinds using value-position machine
calls and scalar arithmetic. Exercises multi-machine value-position calls
with multi-arg parameters, integer multiply/divide, and guard-ladder
comparison. Runs to exit **70**.

```
omega --target windows_x86_64 --build-dir build samples/cli/proofs/shapes_area/main.omg
./build/omega-program.exe   # exit 70
```

Area calculation:
- `triple(3*3)` = 9 × 3 = 27   (circle radius 3, approximated as r²×3)
- `product(5, 8)` = 40          (rectangle 5×8)
- `halve(6, 1)` = 3             (triangle base=6 height=1, integer b×h÷2)
- Total: 27 + 40 + 3 = **70**

The circle computation is done in the caller to avoid it.
