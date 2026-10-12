# NF03 complex carrier operations

Two complete hosted applications exercise one reusable borrowed-input API:
`phasor_power` computes F64 electrical complex power and `quadrature_rotation`
computes an F32 four-step rotation and sum. Each exits 0 only when every numeric
assertion succeeds, otherwise 1. Both expect empty stdout and stderr with empty
stdin. `invalid_alias` is a separate complete source control intended to reject
overlapping mutable/shared loans, never a passing numerical sample.

The source was recovered from the stopped W100 owner's retained NF03 commit
`ea2a50a27f8743d41fc02b7787e90228b1162a50`. The original mathematical oracles and
intervals in [ORACLES.md](ORACLES.md) remain unchanged. `verify_oracles.py` checks
those frozen results independently using rational and high-precision decimal
arithmetic; it does not establish Omega runtime acceptance.

NF01 package metadata and NF02 numeric policy are now present. The complex
kernel uses NF02's format-specific scalar classification for magnitude input
admission and preserves its own `NonFiniteInput` and `Overflow` result cases.
The consumers additionally use NF02's reference-additive comparison with the
original tolerance, and still require the original frozen interval.

From repository root, prepare each authored project through the normal route:

```sh
omega update --project samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power --target linux_x86_64 --offline
# Read each proposed dependency/source decision before recording acceptance.
omega update --resume --project samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power --offline
omega --check samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power/main.omg
omega run samples/cli/scientific/w100/nf03_complex_carrier_ops/phasor_power/main.omg
```

Repeat for `quadrature_rotation`. Check `invalid_alias/main.omg` separately and
require its intended loan diagnostic. Genuine Terminal qualification must use
the normal prepared native report's retained artifact and runtime provisions,
and compare captured stdout, stderr and external exit status with the same
expectations. CLI `--both` termination agreement alone does not establish those
stream observations. The owned validator performs both direct native execution
and genuine buffered Terminal execution from each normal compilation report:

```sh
python3 samples/cli/scientific/w100/nf03_complex_carrier_ops/validation/build_runtime.py --omega /absolute/path/to/verified/omega --output samples/cli/scientific/w100/nf03_complex_carrier_ops/build/runtime-validator
python3 samples/cli/scientific/w100/nf03_complex_carrier_ops/validate.py --omega /absolute/path/to/verified/omega --runtime-harness samples/cli/scientific/w100/nf03_complex_carrier_ops/build/runtime-validator
```

The builder verifies CLI bytes against its recorded build unit and selects the
Rust dependency closure by exact fingerprints. It retains a build receipt.
The validator requires already reviewed project decisions and writes actual
native and Terminal streams and exits under each project's ignored
`build/runtime-validation/`. Its isolated containing-domain exits must be
`ExternalCompletion`, and every authored numeric assertion must succeed.

The restored `evidence/` records describe historical producers and qualification
attempts. Four historical source text dumps have trailing empty lines normalized;
the retained source commit identifies their original bytes. In particular, `evidence/qualify.py` creates temporary qualification
metadata; its historical results do not represent acceptance of these current
integrated projects. Current qualification comes from the validator’s verified
compiler receipt and actual native and captured Terminal observations; source
checks and oracle calculations alone are not completion.
