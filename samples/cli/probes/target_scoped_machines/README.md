# Target-keyed provider selection

Four implementations return different integers. Build selects one for each
hosted target and binds an entry that checks the exact value. Selecting the
wrong implementation traps.

Run the host with `omega run --both build.omg`. Run another target with
`omega run --engine psi --target windows_x86_64 build.omg`.
