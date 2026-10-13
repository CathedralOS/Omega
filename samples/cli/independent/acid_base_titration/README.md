# Ideal diprotic acid titration

Read an ideal H₂A acid solution and independently specified cumulative additions
of a strong monovalent base. Volumes are litres; concentrations and output
species are mol/L. This is a bounded ideal-equilibrium demonstration: complete
mixing, additive volumes, unit activity coefficients, no precipitation, no
complexation, and no other ions. It is not an activity model or laboratory advice.

The whitespace-delimited packet is:

```
acid_volume acid_concentration Ka1 Ka2 Kw base_concentration
added_volume ...
```

Supply 1..16 added volumes; ordering and duplicates are allowed because each
addition is evaluated independently from the original acid solution. Input
accepts signed decimal tokens with an optional decimal point and `e`/`E`
exponent. Mantissas contain 1..18 digits, exponents 1..2 digits with magnitude
at most 30. The complete stream, including whitespace, must contain fewer than
4096 bytes. EOF after a complete token is valid. No count field is required.

The declared domain is acid volume `[1e-6,1000]`, each added volume `[0,1000]`,
acid/base concentration `[1e-6,1]`, Ka1 `[1e-12,1]`, Ka2 `[1e-14,Ka1]`, and
Kw `[1e-16,1e-12]`. All quantities must be finite. Invalid ordering, zero or
negative constants, out-of-domain volumes, nonfinite spellings, overflow,
malformed/truncated/extra tokens and byte exhaustion print `error=input` and
exit 2. Empty input also refuses. Valid packets exit 0 with `status=ok`.

For added volume Vb, `Ct=Va Ca/(Va+Vb)` and `Na=Vb Cb/(Va+Vb)` account for
dilution and the base's counterion. With `D=H²+Ka1 H+Ka1 Ka2`, species are
`Ct H²/D`, `Ct Ka1 H/D`, and `Ct Ka1 Ka2/D`; `OH=Kw/H`. The sample bisects
`H+Na-OH-HA-2A=0` for 160 steps in `[1e-20,10]`. Protonation increases with H
and OH decreases, giving one root. The declared bounds strictly bracket it.
A range-reduced logarithm computes pH. Each row reports added volume, pH, H,
OH, H2A, HA, A, total acid, counterion, signed mass closure and signed charge
residual. Scientific notation retains very small species. The parser canonicalizes an
integer coefficient and combined decimal exponent before a single float scaling
operation, so equivalent spellings of domain endpoints behave identically.

Run from the repository root with a recorded optimized compiler:

```
omega update --project samples/cli/independent/acid_base_titration --target linux_x86_64
omega run samples/cli/independent/acid_base_titration/main.omg < samples/cli/independent/acid_base_titration/data/curve.txt
python3 samples/cli/independent/acid_base_titration/validation/validate.py --omega /absolute/path/to/omega
```

Normal package review remains required; accepting package source declarations
does not grant runtime host authority. The bound entry is Linux x86-64.

`validation/oracle.json` freezes independent 90-digit Decimal hydrogen roots
and derived species before product execution. Its reference solves the cleared
quartic rather than the sample's rational residual. Cases include zero and tiny
titrant, both equivalence regions and excess base, equal Ka constants, domain
extremes, diluted concentrations, 100-fold volume scaling, 16 additions and EOF
without a newline. `FREEZE.json` and the validator bind the reference, fixtures
and oracle by SHA-256. Do not regenerate to match a failing implementation.

The validator requires absolute pH error <=1e-6; species and mass closure error
<=1e-8 times diluted acid concentration; H/OH/counterion/volume relative error
<=1e-8; and charge residual error <=1e-8 times the largest participating charge
concentration. It additionally reconstructs mass and charge from printed values
and checks scaling invariance. Runtime refusals live in `validation/refusals.json`.
The supplemental `endpoint_oracle.json` additionally freezes equivalent decimal
endpoint spellings and the long integer coefficient case uncovered by review.
The separate `invalid` project must reject reading species index 3 from length 3.

`terminal_validate.py` compiles an output observer against exact recorded producer
libraries. It consumes the actual admitted application's retained canonical
Terminal artifact and provisions via shipped APIs and validates captured stdout
against the same oracle and refusals. Fuel and wall-clock sponsor bounds are
explicit. Native/Terminal exit parity alone is not numerical qualification.
