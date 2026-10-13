# Fixed length preserves selector traps

`omega run --both main.omg` must trap in both engines. The final selected
array has length 3, but folding that count must not erase the overflowing
Trapping addition in its selector. Ordinary process exit 0 is a failure.
