# Battery discharge capacity (NEW080, reconstructed)

A complete console application integrates delivered charge and energy through a
constant-resistance discharge test of a single cell. It emits per-step CSV
operating points, the blocking operating point with the terminal reason, and a
totals summary. The application owns its discharge integrator, parser, and
decimal reporter. It imports only settled core float operations, bindings, and
the existing standard Console provider.

## Provenance

This is a **reconstruction**, not recovered original bytes. The NEW080 original
candidate packet (`082650733f59`, archive
`battery-discharge-capacity-candidate-082650733f59.tar.gz`) is unreachable: the
Git ref is absent from the remote and the Library download route returns HTTP
401 on this executor. The model below is authored from the recorded outcome
name — battery-discharge-capacity qualification — against current repository
contracts, with an independent Decimal oracle frozen before the first Omega
run. It is presented as new provenance; nothing here claims original authorship.

## Scientific sample model

`domain/cell.omg` declares the `Cell` model and `domain/discharge.omg` the
integrator. The cell's open-circuit voltage declines linearly with delivered
charge:

```text
OCV(Q)   = V_full - (V_full - V_empty) * Q / Q_nominal
I        = OCV / (R_load + R_internal)
V_term   = OCV - I * R_internal
dQ [Ah]  = I * dt / 3600
dE [Wh]  = V_term * I * dt / 3600
```

The integrator evaluates the operating point at the start of each fixed
interval, then delivers that interval's charge and energy. A step whose
beginning terminal voltage is at or below the cutoff is a `cutoff` stop and
delivers nothing. Delivered charge reaching nominal capacity is `depleted`;
exhausting the step budget is `budget`. This is an ideal DC model: it excludes
Peukert rate dependence, temperature feedback, and diffusion overpotentials.
It is a sample calculation, not a calibrated cell prediction.

## Input

One record on stdin contains eight decimal integers with optional minus signs,
separated by spaces or tabs, terminated by LF or EOF. CR is accepted as
whitespace for CRLF input. The parser reads at most 160 bytes, rejects
malformed/extra/missing fields and decimal overflow, and stores exactly eight
bounded values.

| Field | Unit | Accepted values |
| --- | --- | --- |
| full open-circuit voltage | mV | 2000..15000 |
| empty open-circuit voltage | mV | 0..14999, strictly below full |
| nominal capacity | mAh | 1..100000 |
| internal resistance | mΩ | 1..10000 |
| load resistance | mΩ | 1..1000000 |
| terminal-voltage cutoff | mV | 0..14999, strictly below full |
| integration step | seconds | 1..600 |
| step budget | count | 1..10000 |

For example:

```text
4200 2500 3000 50 1000 2500 60 10000
```

Syntax errors exit 64; model/schedule range errors exit 65. A completed
discharge exits 0 under every stop reason.

## Output

The header names SI units. Numeric fields use signed integer mantissas and
powers of ten with nine significant decimal digits, for example `420000000e-9`
means 0.42 V. Each delivered step emits `index, elapsed_s, OCV, terminal
voltage, current, delivered mAh, delivered Wh`. A `stop,<reason>,OCV,terminal,
current` row reports the blocking or final operating point, and
`summary,steps,elapsed_s,mAh,Wh` closes the record.

## Independent acceptance

`validation/oracle.json` was frozen before the first Omega check/run. The
authoring script refuses to overwrite it. It uses 70-digit Decimal arithmetic,
an independent reimplementation of the same equations, per-case fixture hashes,
the stop reason, step and elapsed counts, and delivered totals. Principal
tolerances are 2e-9 V absolute plus 2e-8 relative for voltage; 5e-12 A absolute
plus 3e-8 relative for current; 1e-6 mAh absolute plus 3e-8 relative for
charge; 1e-9 Wh absolute plus 3e-8 relative for energy. Emitted numbers are
decoded exactly and compared against both the simulation and the frozen totals.

The fixtures exercise a nominal cutoff-limited discharge, depletion under a
light load, step-budget exhaustion, a heavy load collapse, a one-step small
capacity, a high-voltage pack, an immediate first-step cutoff, CRLF, EOF
termination, and twelve invalid runtime records.

The complete source in `controls/intended_invalid/main.omg` reads index 2 of a
two-element array. Acceptance requires rejection for that bounds violation.
Package-review, entry-slot, parse, or unrelated type errors do not pass this
control. It has no package dependency and is checked separately.
