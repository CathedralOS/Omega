# External boundary contracts

Calls libc `abs` through a private external boundary, an ordinary private
external and a public external boundary. The program exits 0 only when all
three results match, including a field/local operand and state forwarding.

`omega run main.omg` must prepare package review and execute on macOS ARM64.
This pins contract exposure separately from source visibility and executable
supply. It does not claim a Terminal interpreter implementation of libc.
