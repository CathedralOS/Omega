# Exact division across a Console call

An exact unsigned dividend survives a Console write between division and
remainder. The original value's flexible home is distinct from the RAX/RCX
instruction-local inputs. The helper checks `288 / 10 == 28` and
`288 % 10 == 8`; the complete entry prints `A` plus LF and exits 70.
