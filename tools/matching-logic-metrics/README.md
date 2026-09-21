# Matching-logic comparison metrics

Produces the metrics record that
[wiki/drafts/matching_logic.md](../../wiki/drafts/matching_logic.md#possible-bounded-comparison)
requires before any matching-logic comparison: **checker, translation, and
theory size; certificate size and checking time; and every imported rule,
assumption, or trusted bridge** — measured over identical pinned positive and
negative proof cases.

The record has two route columns. `route.current` is measured every run;
`route.matching_logic_encoding` is measured when the bounded slice under
`tools/matching-logic-slice` exists, and stays `pending` on checkouts where
it does not — the comparison keeps honest about which side is measured and
which is awaited.

When present, the encoding route's columns are:

| Axis | Measured surface |
|---|---|
| `checker` | `tools/matching-logic-slice` — the bounded-slice certificate checker |
| `translation` | `tools/matching-logic-sort-encoding` — the typed-to-one-sorted clause emitter |
| `trusted_derivation` | empty inventory with a note: no derivation outside the checker decides a leg; the remaining floor is the host `python3` runtime |
| `theory` | `rule_inventory.checkerRules` from the slice record, `rule_inventory.encodingClauses` summed over the sort-encoding corpus, and per-case clause/diagnostic rows |
| `certificate` | summed `certificate_bytes` reported by the slice checker over its pinned cases |
| `cases` | the slice checker's pinned corpus — verdict, `expect` polarity, per-run wall and in-checker elapsed time, admissions count |
| `check_time_ms` | median in-checker `elapsed_ms` and subprocess wall time per case |

## Axes and what they count

| Axis | Measured surface |
|---|---|
| `checker` | `omega-rust/psi/semantics/proof-admission/src` — the admission kernel that independently re-decides certificates (the trusted computing base a foreign checker would replace) |
| `translation` | `proof/src/checker/certificate*` + `proof/src/obligations*` — the producer that encodes checked obligations into `ProofNode` certificates |
| `trusted_derivation` | the rest of `omega-rust/psi/semantics/proof/src` — the derivation still deciding every certificate-uncovered leg |
| `theory` | variant counts of `ProofRule`, `AcceptedProofRule`, `PrimitiveJudgment`, `EvidenceRoute`, `ObligationClass`, `AcceptedFactRoute`, `ProofRuleFoundation`, plus source size of `integer_rules`, `mathematical_core`, `predicate_denotation`, and the `SemanticAxiom`/`Assumption` index-citation rule forms |
| `certificate` | Certificate bytes remain `unavailable`; when `OMEGA_PROOF_MEASUREMENTS` is set, the checker's emitted counts and kernel receipt figures are captured in `proof_measurements`. These measurements do not cover later conjunct lowering. |
| `check_time_ms` | median `total elapsed` and `compile: sources -> requested product` wall time over `--repetitions` runs of `omega --check --offline --timings` |

Every inventory also reports its test lines separately (`test_files` /
`test_lines` / `test_bytes`), so the trusted surface is measured apart from
its own regression corpus.

## Pinned cases

`pinned_cases.json` holds identical-subject positive/negative pairs from
`tests/omega/{pass,fail}/proofs/` — e.g. `proof_inductive_climbing_sum` against
`inductive_climbing_sum_unbounded_accumulator` and
`inductive_climbing_sum_step_false_twin`. Negative cases are expected to exit
nonzero; per-phase timings are only printed on success, so a reject row
carries `total_elapsed` and a `null` `compile` phase by design. A case whose
observed polarity differs from its expected one sets `match: false` and
contributes to `case_mismatches`; `measure` exits nonzero when any appear.

## Usage

From the repository root, after `cargo build -p omega` (or `mbx build -p
omega`):

```sh
python3 tools/matching-logic-metrics/run_metrics.py measure \
    --omega target/debug/omega \
    --out tools/matching-logic-metrics/records/$(git rev-parse --short HEAD).json
```

```powershell
python tools\matching-logic-metrics\run_metrics.py measure `
    --omega target\debug\omega.exe `
    --out tools\matching-logic-metrics\records\<rev>.json
```

`--skip-cases` records only the inventory axes (no omega binary needed).
`--repetitions N` changes per-case timing repeats (default 3).

```sh
python3 tools/matching-logic-metrics/run_metrics.py validate \
    tools/matching-logic-metrics/records/*.json
python3 tools/matching-logic-metrics/run_metrics.py report \
    tools/matching-logic-metrics/records/*.json
```

`validate` enforces schema `omega-matching-logic-comparison/1` so committed
records cannot silently drift; `report` renders one markdown row per record
for pasting into the draft or a PR.

## Records

One JSON document per measurement run under `records/`, named for the
measured revision. Records carry `revision`, `dirty`, `toolchain`, `host`,
and the evidence block the draft requires (logical fragment, rule/semantics
version, target capsule, observation profile, bridge graph, admissions), so a
row stays interpretable after the code moves.
