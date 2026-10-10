# Replaced returned reference record

Returning a mutable holder uses its current reference, not the local backing
from its first initializer. Both native and Terminal execution must observe 17.
Anonymous forwarding and forwarding after two caller-side replacements retain
that current backing. Each replacement retires the old occurrence once; the
final forward must not revive the holder's initial backing of 23.

Run `omega run --both main.omg` in this directory.
