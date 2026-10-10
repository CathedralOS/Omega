# interior_loop_header

Result-bearing machine whose loop header is not its entry state: the entry runs once as a prologue, then every iteration re-enters `step` — loop values travel as `step` parameters. Square-and-multiply computes 3^7 mod 100 = 87 with every factor reduced first.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
