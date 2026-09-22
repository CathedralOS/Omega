# External proof import for matching logic

Bounded-comparison leg drafted by
[matching_logic.md](matching_logic.md): *one small external arithmetic proof
with its exact source axiom closure and checked proof-object translation.*
Semantic expressibility alone is insufficient — the imported object must be
replayed by the local checker, and its axiom closure is part of the evidence.
Delete this draft once the bounded comparison reaches a verdict on the
matching-logic route, or when the importer is retired.

Tool: `tools/matching-logic-external-proof-import/external_proof_import.py`
(Python 3.9+, standard library only). It reads a foreign flat-step export
(`external-arithmetic-proof/1`), translates it into the bounded slice
checker's `omega-matching-logic-certificate/1`, and re-checks the result with
`tools/matching-logic-slice/slice_checker.py`. Nothing about the imported
statement is trusted; only the translated derivation, replayed against the
case's declared theory, can produce an `accept`.

## What was imported

The pinned reference case (`cases/reference.json`) carries a Peano-style
external proof of `add(2,1)=3`: axioms `add_z` (`forall x. add(x,z)=x`) and
`add_s` (`forall x y. add(x,s(y))=s(add(x,y))`), ten flat foreign steps, one
conclusion reference. The importer translates `UI`/`SUBST`/`TRANS`/`REFL`/
`AXIOM`/`DEF_CLOSED` steps into certificate nodes and the slice checker
verifies the tree. `record.json` carries the measured record:
`export_bytes`, `certificate_bytes`, `export_steps`, and
`certificate_nodes` per case, plus the per-run bridge graph.

## Design decisions

- **Checked translation, not trusted import.** The importer never judges
  soundness itself; `translation-recheck` means the real checker rejected
  the translated node. This matches the draft's distinction: a checked
  source proof with a trusted translation retains a *translation*
  admission — here the translation is mechanical rule renaming, and the
  admission is listed under `bridge_graph`/`admissions` in the record.
- **Exact axiom closure.** `axiom_closure` in the case declares precisely
  which source axioms the export may consume. `undeclared-axiom` rejects
  a citation outside the case theory; `axiom-outside-closure` rejects one
  outside the declared closure; `closure-overstated` rejects a closure
  naming an axiom the proof never cites. The closure is evidence, not a
  courtesy — it is the axiom list a reviewer must audit.
- **The producer cannot weaken the goal.** The importer rebuilds the goal
  from the case's declared `obligation` (`statement` pattern, or the slice
  checker's `refinement_after_transition`) and requires syntactic equality
  with the derived conclusion — `goal` fires on any substitution.
- **Fragment fences are named, not silent.** `mu`/`nu`/`fixpoint` rules
  reject as `fragment-escape` (the completeness fragment has no fixpoint
  symbols); `lem`/`dne`/`classical_choice` reject as
  `classical-rule-import` per
  [classicality.md](../../spec/proofs/classicality.md) — importing classical
  reasoning would change the accepted foundation. Hypothesis carriers
  (`HYP`/`IMPI`/`GEN`/`ORE`) reject as `unsupported-external-rule`: this
  bounded importer covers the hypothesis-free subset only, which is all a
  flat arithmetic export needs.
- **Bounded by construction.** Exports beyond 64 steps refuse with
  `certificate-bound` before translation.

## Diagnostic inventory

| Rule | Rejects |
| --- | --- |
| `schema` | wrong case/export schema tag, missing steps or conclusion |
| `malformed-step` | non-object step, duplicate id, missing rule field |
| `premise-order` | premise cites an id that is not an earlier step |
| `unknown-external-rule` | rule name with no bridge entry |
| `unsupported-external-rule` | hypothesis-carrying rule outside the slice |
| `fragment-escape` | fixpoint/mu/nu introduction |
| `classical-rule-import` | LEM/DNE/classical principles |
| `undeclared-axiom` | citation outside the case theory |
| `axiom-outside-closure` | citation outside the declared closure |
| `closure-overstated` | closure names an axiom never cited |
| `translation-recheck` | slice checker rejected the translated node |
| `goal` | conclusion differs from the declared obligation |
| `certificate-bound` | export exceeds the step bound |

## Evidence record

`record` writes `record.json` (`omega-external-proof-import-record/1`):
logical fragment, rule version, semantics version (sha256 of the source
draft), exact subject and case digest inputs, target capsule, observation
profile, bridge graph (external rule → local rule), the admissions the
replay consumed, and per-case diagnostics. This is comparison input for the
bounded-slice harness; it admits nothing into an Omega proof.
