# Transactional selection rebasing, 2026-10-22

The preceding [normalization change](../normalization/README.md) removed four
full expression-arena clones. Of the four remaining copies in each original
workload, the last full-carrier clone protects rollback during authored-selection
rebasing. This iteration retains that rollback contract by validating all stores
before changing any store, then committing the already prepared expression
mapping and the existing statement/proof mappings to the owned carrier.

The ledger join and diagnostic order are unchanged. Expressions, tree statements,
normalized statements, transition targets and proof facts retain their independent
frontiers and occurrence checks. An invalid late store returns the complete
original carrier and the same error. No predecessor match is bypassed, no syntax
or placement input is filtered, and no semantic rule or fixture changes.

This removes a backup that exists solely because validation and mutation were
interleaved. The other copies that preserve concurrently retained predecessors
are outside this change. Construction scratch for the expression remapping is
retained, with the same append order. [Research and hypothesis](research.md).

The validation order is safe to separate because each validator reads only its
own occurrence IDs/frontier and the immutable rebase mapping; none reads the
changes an earlier store would make. Their order stays expressions → tree
statements → normalized statements/transition targets → proof facts. Writes
retain their former order and append behavior. This split adds no validation
pass: it moves the already existing checks ahead of the already existing writes.

## Provenance and acceptance

Base `fd83d7231c4752cbe59020c24c6a2a627d654c5f`. Baseline is the previously
validated normalization candidate, frozen at
`build/resolved-normalization/omega-candidate`; it already contains the previous
optimization. Candidate and diagnostic producers for this iteration are frozen
under `build/resolved-rebase/`, with exact binary/profile receipts and patches.
Do not delete those ignored binaries when archiving the worktree.

Focused validation comprises the representation crate check/tests, the unchanged
five positive and five invalid inputs from the previous experiment, exact native
byte comparisons, actual native execution and candidate native/Terminal agreement.
The new unit test creates valid earlier expressions and statements plus an invalid
late normalized statement, transition target or proof fact; it requires exact
original-carrier equality on each failure.

The temporary oracle generator reads the previous rebase methods directly from
Git. It compares the complete old/new `Result`: success trees, or the returned
original trees plus error. It runs both the crate tests (including refusals) and
the real compiler workloads with verification enabled. This covers the failure
contract beyond successful native output. Diagnostic work is not performance
measurement and is removed from production source after freezing the binary.

Run from this recorded worktree with its preserved baseline and output state:

```sh
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/queue.py
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/capture.py candidate
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/queue.py build_diagnostic.py
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/capture.py instrumented
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/summarize.py
```

Preserve existing immutable binaries before replay; scripts refuse producer
replacement.

## Retained result

- All 25 crate tests passed on the production candidate and again with the old/new
  oracle enabled. Six unit-test rebases compared complete results; four were
  rejections, including the three new late-store failures. Every returned carrier
  and error matched exactly. Nextest strips tab separators in its captured display;
  the summarizer accepts those lines without altering the raw log.
- Nine compiler-level rebase results compared equal across the ten inputs (an
  early-invalid input does not reach rebase). That makes 15 full-result comparisons
  total, all equal. This compares internal identity/provenance and rollback, not
  just emitted bytes.
- 30 compile/reject observations, 15 standalone native executions (all exit 0),
  five candidate native/Terminal agreements, and five unchanged invalid
  diagnostics. Native binaries are identical across the previous candidate, this
  candidate and the diagnostic producer for all five positive inputs.
- Affected crate check, formatting, production whitespace checks, script parsing,
  producer digests and exact restoration of the production patch passed. No
  diagnostic instrumentation remains in compiler source.

| Workload | Full-arena copies before → after | Copied expression rows before → after | Additional rows avoided |
| --- | ---: | ---: | ---: |
| Length | 4 → 3 | 17,300 → 11,725 | 5,575 |
| Dynamic dispatch | 4 → 3 | 17,278 → 11,719 | 5,559 |
| Nested record | 4 → 3 | 17,377 → 11,776 | 5,601 |

Each original workload reached the changed operation once. The oracle recorded
its live expression count and exact result equality; source inspection confirms
the one full-carrier clone was removed without a replacement production clone.
Previous totals come from the normalization record, whose baseline matches this
iteration's frozen baseline. Relative to the original eight-copy measurements,
the two retained changes leave three copies and roughly 66% fewer copied
expression rows. These are work counts, not bytes, RSS or wall-clock savings.

No end-to-end speedup is established. One-minute host load ranged approximately
16.4–24.4 during the single alternating A/B pass. The diagnostic producer adds
intentional cloning and equality work and is excluded from timing claims. Its
build waited 649 seconds for a shared build slot. That wait is workflow context,
not a compiler latency result.

Evidence: [summary](summary.json), [candidate observations](candidate-observations.json),
[compiler oracle observations](instrumented-observations.json),
[production tests](tests.log), [oracle tests](oracle-tests.log),
[candidate receipt](candidate-producer.json), [diagnostic receipt](instrumented-producer.json),
[implementation patch](candidate.patch), [complete diagnostic patch](instrumentation.patch),
[legacy source hashes](legacy-sources.json), and [environment](environment.json).
The complete diagnostic patch is relative to the recorded base, not an overlay
on the candidate patch. Both patches are bound to their producer receipts.

Fetched main was `8b048d43c2eb3a2e6beaa282292e4ef03f5a7f70`; there were no incoming
symbol-resolved representation changes since the previous fetch. This work
remains a local scoped candidate on its recorded base, not current-main,
full-baseline or cross-host qualification. Execution target: macOS arm64. No
language capability acceptance is subtracted. Claim:
`9993efce526e49e79059c0a71b73e9b8`.
