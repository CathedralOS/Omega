# Exact conversion needs unchanged denotation

Checking must reject binary64-to-binary32 `as` for lossy 0.1, unknown input,
finite-only input, and a window whose representable endpoints enclose lossy
values. A finite helper's 1.0e100 still overflows binary32, and an asserted
receiving range supplies no premise about the operand. Each rejection is an
Exact coercion obligation, independently of native provider availability.
