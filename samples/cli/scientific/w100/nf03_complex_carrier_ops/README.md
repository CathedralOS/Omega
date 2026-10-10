# NF03 complex carrier operations

Two standalone hosted applications exercise one reusable borrowed-input API:
`phasor_power` computes F64 electrical complex power and `quadrature_rotation`
computes an F32 four-step rotation and sum. Both require exit 0.
`invalid_alias` is a separate complete source control intended to fail with an
overlapping mutable/shared loan diagnostic. It must never be counted as a
passing sample. No interactive input or console output is required.

The independent mathematical oracles and thresholds were frozen in
[ORACLES.md](ORACLES.md) before NF03 execution. Large scaled magnitudes test
representable results even when the ordinary squared norm overflows.

Once NF01 supplies `source/library/numerics/build.omg`, from repository root:

```sh
omega update --project samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power --target linux_x86_64
# Review the generated dependency decisions and resume their acceptance.
omega --check samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power/main.omg
omega run samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power/main.omg
omega run --both samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power/main.omg </dev/null
```

Repeat update/check/native/Terminal comparison for `quadrature_rotation`.
Check `invalid_alias/main.omg` separately, requiring the intended loan rejection.
For Windows redirect finite empty stdin from a file for `--both`.

NF01 metadata and NF02 policy are unavailable on the recorded baseline.
`evidence/qualify.py` stages byte-identical carrier/main sources in a temporary
local qualification package with separate harness metadata, executes one Omega
process at a time, and retains exact commands and complete logs. These results
qualify owned source behavior, not integrated package acceptance.
