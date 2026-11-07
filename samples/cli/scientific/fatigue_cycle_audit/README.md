# Fatigue cycle audit (NEW072)

A complete Omega console application for bounded synthetic load histories. It
extracts reversals, emits ordered cycles and exact cumulative Miner damage under
the explicit four-point model in [ORACLE.md](ORACLE.md). These nondimensional
fixtures illustrate a scientific sample model, not a calibrated material or
service-life prediction. No shared numerics implementation is imported.

Input is whitespace-separated unsigned decimal integers until EOF:

```
exponent C count load_0 ... load_(count-1)
2 100 7 0 9 2 7 4 6 0
```

Exponent must be 1 or 2, C 1..1000, count 2..16, loads 0..9. The parser accepts
space, tab, LF and CR, bounds total input to 256 bytes, bounds tokens to 19 and
rejects magnitudes above 1000 before another digit is read. Leading zeroes are
accepted. Negative, missing, extra or malformed tokens are rejected with exit
64 and one error line. EOF with an unfinished valid decimal token completes it.
Hosted console input can wait for the caller; bounded storage does not bound
waiting. Console Trap remains the documented host failure path.

The declared curve is N(R)=C/R^p. Full cycles have two half units; residual
adjacent pairs have one. CSV columns contain the range, half units, individual
damage numerator, common denominator 2C, and cumulative numerator. The final
`total,numerator,denominator` row records the sum. Four-digit decimal formatting
preserves leading zeroes deliberately. Fractions remain unreduced to expose
addition. All zero/constant histories emit no cycle rows.

`domain/input.omg` owns parsing and curve/history validation;
`domain/cycles.omg` owns private bounded reversal/cycle storage and counting;
`domain/report.omg` owns output. `main.omg` owns dispatch and process status.
Bindings use the ordinary Linux x86-64 std Console provider and explicit Build
entry binding. Dynamic indices are guarded before every access. Value arithmetic
uses declared Wrapping carriers because array element refinements are unavailable;
validated loads are <=9, there are <=16 reversals and emitted half units <=15,
so range <=9, each damage term <=162, total <=1215 and denominator <=2000.
Thus reachable calculations cannot wrap. Index/capacity arithmetic stays Exact.

Existing-sample comparison: `algorithms/insertion_sort` demonstrates fixed
sorting; `interpreters/stack_calculator` demonstrates bounded push/pop;
`rendering/histogram` counts categories; `text/parse_number` and
`text/format_number` illustrate two-digit conversion. None extracts reversals,
closes nested cycles, handles residual halves, validates a material curve or
reports rational cumulative damage. This workflow combines actual stdin parsing
with an application-specific reduction and audit trail.

The complete application passed source checking, all 26 exact native runtime
cases, and all 26 Native/Terminal termination comparisons on the recorded Linux
producer. The separate bounds control rejected for its intended reason. Package
review was accepted on the original project after explicit authorization of the
complete baseline; later identity refreshes must preserve all accepted meanings.
See [VALIDATION.md](VALIDATION.md) and the review bundle for qualification identity.
From the repository root, using a verified compiler and matching standard library:

```
omega update --project samples/cli/independent/fatigue_cycle_audit --target linux_x86_64
# Inspect any pending review and obtain its required authorization before acceptance.
# A consumer prepares its own transaction; evidence locks are not transplanted.
omega --check --target linux_x86_64 samples/cli/independent/fatigue_cycle_audit/main.omg
omega --target linux_x86_64 --build-dir build/fatigue-audit samples/cli/independent/fatigue_cycle_audit/main.omg
omega run --both --keep samples/cli/independent/fatigue_cycle_audit/main.omg < samples/cli/independent/fatigue_cycle_audit/data/nested.stdin
python3 samples/cli/independent/fatigue_cycle_audit/reference.py
python3 samples/cli/independent/fatigue_cycle_audit/validate.py --executable build/fatigue-audit/omega-program --artifacts build/fatigue-audit-results
omega --check --target linux_x86_64 samples/cli/independent/fatigue_cycle_audit/controls/unguarded_index/main.omg
# The last command must exit 1 with the sole expected bounds diagnostic.
```

Read [VALIDATION.md](VALIDATION.md) for actual commands, source/binary identity,
route outcomes and remaining blockers. `data/cases.json`, the input/output files
and `data/FROZEN.sha256` freeze independent hand-counted results before Omega
execution. The Python reference independently confirms those handwritten cycles;
it does not replace the delivered Omega application or supply expected files.
Tolerance is zero; generated output belongs in ignored `build/`.

`controls/unguarded_index` is a separate complete intended-invalid program with
its own Build entry. It deliberately selects the third element of a two-element fixed array
without a valid bound proof. Verify rejection separately and require the documented
array-bound diagnostic; other rejection reasons do not pass this control.
