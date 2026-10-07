# Exact float-format narrowing

`omega run main.omg` exits 0 after narrowing a selected call's positive
binary64 singleton and a parameter's negative singleton to binary32 without
changing either value. Both source contracts establish representability; the
receiving binary32 ranges do not authorize rounding.

A binary64 parameter whose range endpoints retain a binary32-authored 0.1
also narrows unchanged. The endpoint reader must use its landed value, not
re-read the decimal spelling as binary64 0.1.

This differs from `F32::from_f64`, which explicitly requests nearest-even
rounding. Shared Terminal execution still needs the exact `F32::from_f64`
host-binding adapter owned by `TERMINAL-RUNTIME-DRIVERS`.
