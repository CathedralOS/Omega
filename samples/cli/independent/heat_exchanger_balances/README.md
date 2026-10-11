# Heat exchanger balances (NEW073)

A complete stdin-driven synthetic scientific application using the existing std
Console provider and application-private parsing, stage solving and reporting.
No experimental numerics imports, shared exports or worker dependencies.

Input is whitespace-separated unsigned decimal integers, terminated by EOF:

```
1 stages hot_flow hot_cp cold_flow cold_cp hot_in cold_in G0 ... G(stages-1)
```

Direction `1` means counterflow. Flows are kg/s, heat capacities J/(kg K),
conductances W/K, temperatures degrees Celsius. Stages are 1..8; flows and
specific capacities 1..100; temperatures 0..200 with hot_in >= cold_in;
each conductance is 0..200 and no larger than either stream capacity rate.
At most 16 tokens and fewer than 256 input bytes are consumed. Any missing or
extra token, invalid byte, direction, range or non-counterflow temperature order
prints `INVALID bounded counterflow input` and exits 64. Numerical balance
failure exits 65. Valid input emits all node temperatures and stage duties,
then total duty and a balance verdict, and exits zero.

This explicitly declared model uses ideal constant-capacity streams, no losses,
no phase change, and midpoint temperatures for each finite-volume stage. It is
an educational discretization, not equipment sizing or a continuous NTU model.
Hot flow follows increasing node j; cold flow follows decreasing j. With
capacity rates Ch=hot_flow*hot_cp and Cc=cold_flow*cold_cp:

```
q[j] = G[j] * ((H[j]+H[j+1])/2 - (C[j]+C[j+1])/2)
H[j+1] = H[j] - q[j]/Ch
C[j+1] = C[j] - q[j]/Cc
H[0] = hot_in; C[N] = cold_in
```

The private solver propagates the affine two-stream transfer for trials with
(H0,C0)=(hot_in,0) and (0,1), solves the far cold boundary for C0, and retains
a third pass. The per-stage denominator is
`1 + G/(2 Ch) - G/(2 Cc)`, bounded below by 1/2 by the declared conductance
limit. Up to eight stages bound storage and computation. Guarded accesses keep
all stage and node indices within their arrays.

Before any Omega execution, `reference.py` independently froze exact rational
solutions using simultaneous duty equations and Fraction Gaussian elimination.
It checks every midpoint relation, monotonicity and both enthalpy sums.
`data/oracle.json` preserves exact fractions, input hashes and tolerances;
its initial SHA-256 is
`fba8b0b7e3bde8a7795425d6691f1cdeeb64d29174fcd32fe22ea9d4c241a32c`.
Internal absolute balance and temperature tolerance is 1e-7. Display uses
integer millidegrees/milliwatts (quantum 0.001). Computed values within 1e-7
degrees Celsius or watts of a half-milli threshold are treated as ties and
rounded upward; elsewhere nearest rounding applies. This small display tie
band accounts for binary64 propagation error and does not claim exact rational
rounding for every bounded input. Authored stdout files remain exact byte
oracles. Independent review's additional exact-tie fixture and its display
policy were frozen separately in `data/supplemental_oracle.json` before its
Omega execution; the original oracle was preserved. The reference is supporting evidence, while
`main.omg` and the three domain modules are the delivered application.

Nine valid fixtures cover zero conductance, one and multiple equal-capacity
stages, unequal streams in both directions, heterogeneous conductances,
maximum stage count, maximum capacities and equal inlet temperatures. Thirteen
invalid fixtures cover physical/range, syntax, arity and input-budget errors.
The separate complete source in `controls/out_of_bounds/` deliberately writes
node 9 of a nine-element temperature array. It must reject specifically for
unproved/out-of-range indexing, not a package, entry or syntax error.

Semantic overlap was checked against `collections/heat_grid` (coordinate
accumulation), `simulation/particle_sim` (Euler position/velocity stepping),
`arithmetic/vector_distance` (scalar float math), and `stdin_checksum` (byte
input). None solves a two-boundary coupled stream balance; this adds that
workflow rather than changing their constants.

From the repository root, select a compiler built from the recorded base and
set `OMEGA_LIBRARY_ROOT` to this checkout's `source/library`. Review packages
with `omega update --project samples/cli/independent/heat_exchanger_balances
--target linux_x86_64`, inspect the emitted review, and resume accepted decisions
through the documented package workflow. A denied acceptance is a blocker and
must not be retried or bypassed. Then:

```
omega --check --target linux_x86_64 samples/cli/independent/heat_exchanger_balances/main.omg
omega --target linux_x86_64 samples/cli/independent/heat_exchanger_balances/main.omg
omega run --both samples/cli/independent/heat_exchanger_balances/main.omg < samples/cli/independent/heat_exchanger_balances/data/unequal_hot_larger.txt
omega --check samples/cli/independent/heat_exchanger_balances/controls/out_of_bounds/main.omg
python3 samples/cli/independent/heat_exchanger_balances/validate.py --cli /absolute/path/to/omega
```

`validate.py` runs each fixture sequentially, compares native stdout and exits
with frozen files, and records Terminal agreement or explicit decline. The CLI's
`--both` contract compares termination only; it does not expose interpreted
stdout, so native output verification does not establish Terminal output identity.
Build products and validation logs belong in ignored `build/`.
