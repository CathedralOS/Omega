# Interpreted Forth-Gamma experiment

This experiment revisits a Forth-like Gamma after separating interpretation
from the former Gamma-to-Beta compilation path:

```text
Beta compiler -> fixed Forth-Gamma interpreter tape
Forth-Gamma Delta compiler + Delta source -> Forth-Gamma receipt
same fixed interpreter + receipt -> result
```

No per-program Beta or Alpha expansion participates. The editable symbolic
interpreter is reconstructed from the retained 753-line addressed evaluator and
initially reproduces its exact 4,312-byte tape. The experimental interpreter
adds named `value` storage and quoted `text` output. Those two operations replace
49 fixed-cell getter/setter pairs and 356 numeric output operations in the
retained Delta compiler.

## Measurements

```text
                              Lines  Instructions  Labels  Control  Tape bytes
functional Gamma evaluator    1,325         1,065     165      479       6,934
Forth-Gamma interpreter          890           723     122      312       5,145

functional Delta compiler      1,004 source lines, 90 definitions
Forth-Gamma Delta compiler      1,451 source lines, 555 definitions

total functional route         2,329 authored lines
total Forth route              2,341 authored lines
```

The Forth compiler contains 49 named values, 5,860 executable source tokens,
204 explicit branches, 171 explicit jumps, 77 stack-shuffle operations, and 20
remaining dynamic cell operations. The functional compiler contains 353 named
lexical `let` bindings and no source-level control-flow choreography. These
current totals are no longer a matched-coverage comparison: the functional
compiler also performs a complete declaration-order and exact namespace
uniqueness census that the retained Forth compiler lacks.

## Evidence

The pure interpreted route passes:

- exact recursive `Nat` result 3;
- exact two-field `List` result 9;
- three-field recursive rope result `0x42`;
- unknown fields, arity/binder errors, non-exhaustive matches, and out-of-order
  arms reject before output;
- empty-rope indexing traps; and
- 100,000-node construction and traversal complete through proper tail
  transfers.

The rope fixture is profile-adapted: helper names use `rope_` because the old
compiler reserves normative `bytes_*` builtins, and its unreachable Gamma I/O
trap is replaced with pure Delta division by zero. This preserves the recursive
three-field representation question but is not exact-source equivalence.

Scale is the largest operational loss. On the development host, interpreted
compilation took 2.38 seconds for 101 functions, 11.98 seconds for 301, and
119.29 seconds for 1,001. The selected 3,001-function witness exceeded its
600-second timeout. These timings are diagnostic, but the growth exposes the
compiler's repeated linear row scans.

## Finding

Interpretation fixes the old Forth route's catastrophic generated-Beta problem.
The trusted interpreter is 435 lines, 342 instructions, 43 labels, 167 control
transfers, and 1,789 tape bytes smaller than functional Gamma. Named values and
literal text also remove the most offensive numeric-cell and byte-emission
boilerplate.

It does not yet beat the selected architecture overall. The compiler is split
across 555 tiny words, explicit stack effects remain difficult to reconstruct,
and unreachable names or stack underflow are not statically rejected. Total
source is 12 lines larger despite lacking the selected route's newer global
census and any stack-effect checker, and large program compilation is
impractically slow.

The experiment is promising enough to retain, but not to select. A next attempt
would need declared stack effects plus indexed declaration lookup while keeping
the interpreter materially below functional Gamma. If those additions erase
the root-size advantage, the current functional rung remains the better trust
boundary.
