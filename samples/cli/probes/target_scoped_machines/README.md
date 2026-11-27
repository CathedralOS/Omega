# Target-keyed provider selection

Four implementations return different integers. Build selects one for each
hosted target and binds an entry that checks the exact value. Selecting the
wrong implementation traps.

Run the host with `mbx xtask sample-gate --filter samples/cli/probes/target_scoped_machines`. Run another target with
`omega run --target windows_x86_64 build.omg`.
