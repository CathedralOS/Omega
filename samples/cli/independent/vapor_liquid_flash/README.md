# Vapor–liquid flash

This synthetic model holds temperature and pressure fixed and takes supplied
constant equilibrium ratios, K=y/x. It does not estimate K, fugacity, energy,
pressure, temperature, or real-fluid behavior. Feed is on a one-mole basis.

Run `omega update --project samples/cli/independent/vapor_liquid_flash --target linux_x86_64`
using the normal package acceptance flow, then
`omega run --both --keep samples/cli/independent/vapor_liquid_flash/main.omg < samples/cli/independent/vapor_liquid_flash/data/binary_half.txt`.
The explicit entry is `main`, bound for Linux x86-64, macOS ARM64 and Windows x86-64.

Stdin is `n z0 K0 z1 K1 ...`, with 2–8 ordered species. Tokens are unsigned
finite decimal numbers, including `.5` and `1.`; whitespace separates tokens.
Signs, exponents, NaN, infinity, malformed or missing tokens and extra tokens
refuse. There are at most 1023 bytes, 32 characters per token, and 17 tokens.
EOF terminates input. Fractions must be in [0,1], sum to one within 1e-12,
and K must be in [0.05,20]. Fractions are used as supplied, without normalization.
Decimal numbers are evaluated in binary64; numerical acceptance refers to the
frozen finite cases, not an arbitrary-precision promise for ill-conditioned K.

The Rachford–Rice residual is
`F(b)=sum(z_i*(K_i-1)/(1+b*(K_i-1)))`.
Positive K makes every denominator positive on [0,1]. F decreases when any
positive-abundance species has K unequal to one. `F(0)<=0` selects liquid;
`F(1)>=0` selects vapor; endpoint zero within 2e-15 times the sum of absolute residual terms selects
the corresponding bubble/dew boundary (a relative cancellation allowance). Otherwise 60 bounded bisections determine vapor fraction b.
For the split, `x_i=z_i/(1+b*(K_i-1))`, `y_i=K_i*x_i`.
For a single phase its composition equals the feed. Output explicitly marks
the other phase absent, including a zero-volume phase at a boundary.
Each component emits reconstructed feed and absolute closure error.

If every positive-abundance species has K=1, F is identically zero: vapor
fraction and phase existence are non-identifiable. Each phase, if present,
would have the feed composition. The report states that conditional fact and
emits no chosen phase compositions. Component closure is identically zero
for every admissible split; those ordered feed/closure identity rows do not
assign a vapor fraction or an absent phase. The separately frozen exact rational
constant and beta coefficients in `data/closure_oracle.json` pin this identity. A zero-abundance species with
K unequal to one does not resolve this ambiguity.

Exits: 0 for an identified physical result, 3 for non-identifiability, and 64
for refused input, and 65 for failed numerical closure or composition normalization. Source control `controls/component_bound` must reject its
ninth-component write; no runtime status substitutes for that rejection.

`python3 samples/cli/independent/vapor_liquid_flash/validate.py --omega /absolute/path/to/omega`
checks actual native and captured Terminal values against the checksum-frozen
80-digit independent Decimal/Newton oracle. Binary roots additionally satisfy
an analytic expression. The required error bounds are 1e-8 for vapor fraction
and present-phase composition and 1e-10 for each component's closure, including
closure recomputed from displayed compositions. It also checks normalization,
K relations, phase status, ordering, and absence markers. `reference.py --freeze`
is an explicit maintenance action; validation never rewrites expected results.

The bisection midpoint uses an owned mutable `f64` parameter. Its update and
return exercise invocation-local scalar storage; the caller keeps its bracket
endpoint. Compiler fixture `runtime_owned_mutable_float_parameter_exit` also
checks `f32`, nested-machine arrival, caller isolation, and signed zero, alongside
immutable-mutation and live-loan refusal controls.

For a faster finite suite, build the supported runner with
`mbx build -p omega --example qualify_inputs`, then pass its absolute executable
path as `--qualifier` to the same validator. The runner compiles the normally
accepted project once and uses a fresh native process and isolated Terminal
invocation for each input. The validator still checks both complete streams,
actual process-exit statuses, source-invalid refusal, and every frozen value.
