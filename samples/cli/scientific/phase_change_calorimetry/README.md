# Phase change calorimetry

Two insulated lumped bodies exchange heat through constant conductance G. This
educational model assumes uniform body temperature, reversible equilibrium
phase change, constant mass and heat capacity in both sensible phases, and no
loss, supercooling or hysteresis. It extends the workflow excluded by
`heat_exchanger_balances`; it does not model that sample's flowing streams.

Whitespace-separated signed decimal stdin, terminated by EOF:

```
m1 c1 L1 Tm1 h10 m2 c2 L2 Tm2 h20 G count t0 ... t(count-1)
```

Mass is kg, heat capacity J/(kg K), latent heat and specific enthalpy J/kg,
melting temperature and output temperature degrees Celsius, conductance W/K,
time seconds and transferred energy joules. Enthalpy zero is the solid melting
onset, rather than an absolute temperature-energy datum. Each body's enthalpy
law uses its own reference:

```
T(h) = Tm + h/c              h < 0 (solid)
T(h) = Tm                   0 <= h <= L (isothermal mixture)
T(h) = Tm + (h-L)/c          h > L (liquid)
f(h) = min(1, max(0, h/L))
m1 dh1/dt = -G (T1-T2); m2 dh2/dt = G (T1-T2)
```

The finite domain is m,c,L in [1,1000], Tm in (0,1000], initial h in
[-1000,1000], G in [0,1000], integer count in [0,8], and nondecreasing
observation times in [0,1000]. Repeated times and zero observations are valid.
Numbers require an integer digit before an optional decimal point, and one to
nine fractional digits after a point. Exponents, plus signs, NaN and infinities
reject. The parser consumes at most 4096 bytes and 13 digits per number; it
rejects malformed tokens, missing/extra tokens and over-budget input before
emitting observations. EOF must follow the complete supplied record.

The solver evolves one cumulative transfer q (positive from body 1 to 2),
with h1=h10-q/m1 and h2=h20+q/m2. Within a phase interval q has an affine ODE,
solved by exponential response. Four transfer-coordinate phase boundaries
bound the event sequence. Sixty bisections locate each event time. Phase slopes
at endpoints use transfer direction and exact boundary coordinates; a floating
enthalpy cancellation cannot choose the wrong phase. A 16-event safeguard
rejects failed progress. Exponential reduction and a small-argument response
series preserve accuracy without a relaxed floating mode or external numerics.

Each observation emits eight integer fields suffixed `_pico`, in order: time,
temperature1/2, fraction1/2, enthalpy1/2 and transferred_energy. Divide by 1e12
for the units above. Conversion truncates toward zero, with quantum 1e-12.
All bounded outputs fit i64; no successful result relies on saturation.
`status=ok` ends valid output and exit 0. Invalid input prints
`INVALID calorimetry input` and exits 64. Failed numerical progress or internal
energy conservation prints `FAILED calorimetry convergence or conservation`
and exits 65. Empty stdin is deliberately an exit-64 sample-discovery control.

Before sample execution, `validation/reference.py` froze independent two-enthalpy
SciPy DOP853 integrations at two accuracy settings, analytic solid and liquid
single-regime exchange, and equilibrium energy checks. The analytic phase partition is frozen separately below. The ODE does not
share production's one-transfer/event implementation. `validation/FREEZE.json`
locks the reference, oracle and fixture bytes. Initial oracle SHA-256:
`d431c4af9337181cad06725e81b631efca5cf7a21a256f4512fccdc7d3347f09`.
Fixtures include melting and freezing transitions, distinct melting points,
isothermal intervals, endpoint heating/cooling, extreme conductance, equilibrium,
zero conductance and mass/conductance scaling. Temperature tolerance is 1e-5;
enthalpy and transfer tolerance is 1e-6 times max(1, absolute reference value).
Melt fractions use 1e-8. Energy drift must be <=1e-10 times
max(1, |m1*h10|+|m2*h20|, m1*L1+m2*L2), also checked from displayed numbers.

Use a compiler built from the qualification revision, with this checkout's
`source/library`, and the ordinary package review/acceptance workflow. Full
access to the execution host does not approve package authority. Then:

```
omega update --project samples/cli/independent/phase_change_calorimetry --target linux_x86_64
omega --check --target linux_x86_64 samples/cli/independent/phase_change_calorimetry/main.omg
omega run --both samples/cli/independent/phase_change_calorimetry/main.omg < samples/cli/independent/phase_change_calorimetry/data/melting.txt
mbx build --manifest-path samples/cli/independent/phase_change_calorimetry/validation/Cargo.toml --release --target-dir target
python3 samples/cli/independent/phase_change_calorimetry/validation/validate.py --omega /absolute/path/to/omega --route all
```

The Linux numerical SDK runner compiles once through normal package preparation,
executes the exact retained Terminal artifact with its admitted provisions and
isolated domain-exit delegation, then publishes and runs the native image.
Every input stream is finite, each native child is bounded, and the outer
validation invocation is timed. It saves actual native and Terminal stdout,
stderr and statuses; termination-only `--both` agreement is insufficient.
The separate source-invalid project writes observation 8 into an eight-element
array and must reject specifically for its bounds obligation.

The sample first exposed a guard-call typing defect already repaired upstream
by `658da8228c736fba8b460dad4382117a8e30e0d4`. This assignment preserves that
implementation rather than duplicating it. Its natural mixed-format controls
then exposed a distinct correctness defect on fetched main
`5b32c4eb36ffa2a6161794b068e9e0282b0bb482`: concrete f64/f32 comparisons and
retained contracts were accepted without explicit conversion. The shared compiler
repair checks both actual operand formats before admitting builtin operations,
in runtime expressions and concrete/abstract specification formation. Resolved
heterogeneous authored operators retain their own meaning and requirements.
Unknown generic carriers remain unresolved. Focused conversion, guard, field,
index, authored-domain operator and requires controls live in
`tests/omega/{pass,fail}/float/`. Canonical contracts are
`wiki/spec/language/numeric_values.md` (literal landing, floating formats and
mixed-integer comparison limits), `wiki/spec/language/expressions.md` (operand
selection and ordinary call obligations), and `wiki/spec/terminal-psi/product.md`.

Independent review requested a nontrivial analytic phase equilibrium, rather
than treating an integrated terminal temperature as an analytic partition.
`validation/supplement_reference.py` freezes an additional distinct-melting-point
case: at equilibrium body 2 melts at 40 Celsius, body 1 has specific enthalpy
140, and conservation fixes body 2's specific enthalpy at 30, strictly inside
[0,60]. The total energy is 200 J, the transfer is -160 J, and both melt
fractions are determined (1 and 1/2). Both captured routes must match this
partition directly. The separately frozen addendum preserves the original oracle.
