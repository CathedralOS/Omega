# Research record linkage

NEW064 is a complete bounded two-source linkage workflow over synthetic research
metadata. It uses private scoring, enumeration, consensus and audit modules.
There are no external datasets, unfinished numerics imports or worker dependencies.
This is a declared teaching model, not a validated bibliographic identity model.

Each source has three records, with numeric identifier, normalized title token,
normalized author token and publication-year category. Digits 1–9 name distinct
synthetic values; zero means missing, including missing year. Equal nonzero
identifier/title/author/year contributes 12/6/3/1 respectively. Conflicting
nonzero identifiers veto an edge regardless of other similarities. Retain scores
at or above the supplied threshold (1–9).

A matching selects at most one right record per left record and vice versa.
Unmatched costs zero. The program enumerates all 64 base-four assignments,
rejects duplicate right selections and unavailable edges, and maximizes total
score. It emits only links present in every optimum. A left choice differing
between optima is ambiguous, including a link-versus-unmatched disagreement.
The consensus code is 0 for always unmatched, 1–3 for a right record, or 4 for
ambiguous. It never resolves ties using arbitrary input order.

The report includes best score, number of optimal assignments, candidate edges,
identifier-conflict vetoes, linked/ambiguous/unmatched left counts and true
positive/false positive/false negative counts against declared one-to-one truth.
A false positive is an emitted consensus link different from truth; a false
negative is a nonzero truth link not emitted. Ambiguous rows count as missed
truth where appropriate. Precision and recall can be calculated from these
counts; no floating-point approximations or undefined zero-denominator rates
are reported.

## Input and output

Supply exactly 28 single-digit tokens followed by EOF: threshold, the four fields
of each of three left records, the four fields of each of three right records,
and three truth choices. ASCII space/tab/CR/LF separate tokens. The input parser
rejects adjacent digits, nondigits, too few/many tokens, zero threshold, truth
outside 0–3, repeated nonzero truth choices and input requiring more than 256
bytes including the observed EOF. All other record values are legal. Reading
may block waiting for the finite input stream to finish.

`data/*.txt` are complete stdin fixtures. `data/*.stdout` are exact expected
output. `data/frozen.json` contains independently enumerated optima and matrices.
The process exit is a fingerprint of all 13 computed report fields, using the
weights and modulus in `frozen.json`; it is deliberately not a universal success
code. Invalid runtime input prints `INVALID input` and exits 64.

The Omega score/counter additions use bounded modulo-1000 arithmetic. The model
admits scores <=22, totals <=66, at most 64 assignments and at most nine edges;
no legitimate value reaches that modulus. No numerical tolerance is allowed.

## Independent expectations and validation

The oracle uses left subsets and right permutations, independently of Omega's
base-four decode and streaming consensus. Expectations and zero absolute/relative
tolerances were frozen before the first Omega execution. `data/oracle.sha256`
protects the oracle and expected results from drift. Run:

```text
python3 samples/cli/independent/research_record_linkage/reference.py
```

There are nine original named scientific fixtures, 20 deterministic held-out
synthetic fixtures, and a separately frozen `greedy_trap` challenge (greedy score
15 versus optimal score 17). `competition` exercises competing targets; `ties` challenges arbitrary
tie breaking; `ambiguous_unmatched` challenges link-only ambiguity detection;
`identifier_conflict` challenges similarity overriding incompatible identifiers;
`false_link` records both an actual false positive and missed truth;
`low_threshold`/`high_threshold` challenge threshold behavior with the same input.

Use a compiler built from current main and the existing std provider closure.
From repository root, substitute its absolute path for OMEGA. First review the
ordinary project package proposal:

```text
OMEGA update --project samples/cli/independent/research_record_linkage --target linux_x86_64
```

Read the generated review, accept only the concrete intended std/provider
proposal if authorized, then resume once. Do not retry denied acceptance or
bypass cache custody. After successful package preparation:

```text
OMEGA --check --target linux_x86_64 samples/cli/independent/research_record_linkage/main.omg
OMEGA --target linux_x86_64 --report-file /tmp/new064-compile.txt samples/cli/independent/research_record_linkage/main.omg
python3 samples/cli/independent/research_record_linkage/validate.py --executable ABSOLUTE_PUBLISHED_EXECUTABLE
python3 samples/cli/independent/research_record_linkage/validate.py --terminal --omega ABSOLUTE_OMEGA
```

The validator compares all native stdout bytes, empty native stderr and authored
exit for every valid/invalid fixture. Terminal `run --both` must report an actual
agreement for ten named variants and all malformed cases; a decline fails that
route. The current CLI compares termination, so the field fingerprint supplies a
computed semantic observation for Terminal. Exact Terminal stdout is not exposed
by this CLI and is not claimed as independently verified.

## Separate intended-invalid source

`controls/unproved_index` has its own complete entrypoint and Build binding.
Its `select` machine indexes `[u64; 3]` with an unrestricted `u64`. This violates
[the scalar bounds obligation](../../../../wiki/spec/language/counts_and_addresses.md#bounds-and-lowering)
regardless of the actual caller passing zero.
Prepare its package policy separately if required, then run:

```text
OMEGA --check --target linux_x86_64 samples/cli/independent/research_record_linkage/controls/unproved_index/main.omg
```

Rejection must contain `cannot prove index` at `records[position]`; a parse,
package, custody or entry-selection rejection is not acceptance of this control.
The ordinary source uses guarded indices and remains separate from the control.

## Scope comparison

Existing `algorithms/bubble_sort` demonstrates guarded array mutations;
`algorithms/dutch_flag` partitions an enum array; `rendering/histogram` counts
categories; `arithmetic/stats_compute` computes scalar statistics; and
`interpreters/calculator_rpn` parses a stack program. None performs candidate
scoring, identifier-conflict vetoes, exhaustive injective matchings, consensus
across global optima or truth-based linkage auditing. The new workflow shares
ordinary Console/input/array facilities, not another application's algorithm.

See `VALIDATION.md` for the exact executor, compiler identity, observed routes,
artifact hashes and any unresolved acceptance boundary. Generated artifacts and
package decisions remain in ignored local build state.

The original source and all39fixtures have been recovered from the original
author commands and retained corrections. Current source/native/captured
Terminal qualification is pending the general bounded arithmetic proof repair.
See VALIDATION.md for current evidence and historical producer boundaries.

Direct Terminal output is captured by validation/terminal_capture.rs through
the public admitted product runtime and checked by validation/check_capture.py.
All39input streams must match every expected byte and authored exit; CLI exit
agreement alone is not full Terminal output coverage.

Link the private observer against the exact compiled CLI dependency closure:

```text
python3 -B validation/build_capture.py --omega ABSOLUTE_OMEGA --omega-sha256 COMPILER_HASH --output ABSOLUTE_CAPTURE
ABSOLUTE_CAPTURE ABSOLUTE_SAMPLE_ROOT ABSOLUTE_CAPTURE_DIRECTORY
python3 -B validation/check_capture.py ABSOLUTE_CAPTURE_DIRECTORY
```

The linker matches the public CLI byte for byte to its build unit, then follows
its recorded dependency fingerprints. Every linked library and observer source
hash is recorded. It never selects libraries by timestamps or whichever
duplicate appears first, and it does not rebuild compiler dependencies.
