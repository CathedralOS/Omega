# GAMMA-SEED-NATIVE-ACCEPTANCE — re-verification at 968e26855bd

Board item re-mines the gate `tests/gamma/seed-native-acceptance/` — the
Gamma chain's seeded-evaluator native acceptance gate (landed in
`7a29238279`, extended to every audited seed in `a1cfb00d75`). Owner:
Zergling-108. Host: Linux x86-64 (a seed-execution host).

## Lane

`sh tests/gamma/seed-native-acceptance/run.sh` — all three legs green:

- **identity**: canonical source/tape pair matches the bound Gamma evaluator
  records (`tools/bootstrap/gamma/evaluator_env.sh`).
- **acceptance**: evaluator tape (8575 bytes) stamped into every audited
  Alpha seed satisfies `tests/alpha/container.py`: host-selected
  `alpha_x64_linux` through the real materializer (ELF), `alpha_x64_windows.exe`
  (PE) and `alpha_arm64_macos` (Mach-O) each stamped at its recorded hole
  offset; `[length][tape][zeros]` hole contract + byte-exact outside-hole
  content hold on all three.
- **execution**: the materialized ELF executes the minimal Gamma source
  `(def main () Int 42)` natively and publishes the byte-exact `*` receipt —
  on this host the exec leg ran rather than skipping, since Linux x86-64 is
  an audited-seed execution host.

No residual: the gate already covers the shipped evaluator tape inside both
non-host audited seeds at their own hole offsets; macOS re-signing and the
Windows path are exercised by their own seed-exec hosts (this run verified
their stamping contract via `--pristine` byte checks, not their exec leg).
