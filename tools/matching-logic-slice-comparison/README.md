# Matching-logic slice comparison harness

Board item: `MATCHING-LOGIC-SLICE-COMPARISON`. Implements the bounded
comparison described in
[wiki/drafts/reference/matching_logic.md](../../wiki/drafts/reference/matching_logic.md)
("Possible bounded comparison") for the side of the comparison that exists
today, and defines the record both sides fill.

## What it produces

`compare.py` writes two artifacts:

- `record.json` — the machine-readable comparison record
- `wiki/drafts/reference/matching_logic_slice_comparison.md` — the rendered report

Measured on the current route:

- **Receiver checker size** — lines of Rust in `terminal-verifier` plus the
  `terminal-codec` proof-sidecar / proof-bundle / trust-graph sections and
  the `compilation-report` PCC admission surface. This is the code a
  receiver must run or trust; it is the honest analogue of "checker size"
  on the candidate side.
- **Theory size** — `proof-admission`'s `mathematical_core`,
  `integer_rules`, and `predicate_denotation` surfaces: the theory the
  kernel checks against, which a typed-to-one-sorted encoding must restate.
- **Translation size** — `0` on the current route; semantic products are
  checked natively. The axis exists for the candidate's encoder.
- **Imported rules / trusted bridge** — the enforced
  `AcceptedProofRule::foundation` inventory parsed out of
  `classicality.rs` (exhaustive match; a rule not classified does not
  compile). `SemanticAxiom` is the only `TrustedAdmission`.
- **Certificate size / checking time** — `--check` wall time per pinned
  case, plus `--native-sidecar <path>` to record produced `.proof` /
  `.psi.proof` artifact bytes. Bare-file checks emit no artifact, so the
  byte axis is pending until a native sidecar is supplied.
- **Identical pinned positive/negative cases** — `pinned_cases.json`: every
  `tests/omega/fail/proofs/*_twin` member that names its positive twin in
  its header comment. Positives must check (exit 0); negatives must reject
  and emit every `expected.txt` fragment — the same contract the fail
  corpus enforces. Pairs whose case dir carries a `build.omg` (std-package
  dependency, minutes per check) are flagged `heavy` and skipped unless
  `--include-heavy`.

## Usage

```bash
python3 tools/matching-logic-slice-comparison/compare.py \
    --omega target/debug/omega \
    --report wiki/drafts/reference/matching_logic_slice_comparison.md
```

Flags: `--skip-run` (static surfaces only), `--include-heavy`,
`--timeout <sec>`, `--record <path>`, `--native-sidecar <path>`.

Exit status is nonzero if any run pair violates the pinned discipline
(positive rejected, negative accepted, or expected fragment missing) —
that is a divergence on the current route, the class of finding this tool
exists to surface.

## Candidate side

`record["candidate"]` carries one measured column and two pending ones:

- **Sort encoding (measured)** — `candidate.sort_encoding` runs the
  typed-to-one-sorted encoder
  (`tools/matching-logic-sort-encoding/sort_encoding.py check`) over every
  case in its `cases/` corpus and records the emitted clause inventory —
  every clause is an axiom admission — plus per-case diagnostics and the
  checklist provenance (fragment, rule/semantics versions, subject,
  capsule, observation profile, bridge graph). `translation_surface` is
  the encoder's own physical line count: the candidate's translation
  size, counterpart to `0` on the current route.
- **Checker / theory / certificate (pending)** — these belong to
  `MATCHING-LOGIC-BOUNDED-SLICE` (`tools/matching-logic-slice`) and
  `MATCHING-LOGIC-COMPARISON-METRICS` (`tools/matching-logic-metrics`).
  Once they land, the same axes over the same pinned pairs fill the
  remaining columns; nothing in the schema changes.
