# Declared remainder rank

Owner: `NESTED-MACHINE-UNIFICATION` in `TASKS.md`, remaining non-polynomial/non-monotone declared bodies.

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

`Remaining::LowBits(value)` produces `value % 4`. It is not globally monotone. Each recursive member independently requires its selected cell to lie in `4..=7`; preserving delivery and the decrement-by-one edge then establish strict descent of the produced rank. Both selectors exercise count `7 → 4` and the zero-rank count `4` path, with counter `3 → 6 → 6`. The selected cell must finish at `4`, the unselected neighbor keeps its original amount, and canary `641` survives. Each reject twin changes only decrement `1 → 0`.

Input, every intermediate and result formation must be independently proved; the requested `0..=3` rank range establishes nothing itself. The loop consumes the same exact captured input, selector bounds and storage version as the already-covered polynomial production. Declaration identity and selected remainder meaning must survive source checking, Terminal Psi and native realization.

At fixed base `787812b7685490345b98f4bfbeaae5d637531dcd`, the verified optimized retained C10 producer (actual producer revision `be61262f38428b56ba7f33b69fe537c2bd876ab2`, independently reviewed accessor-equivalent to this baseline) refuses all nine valid places with a ranking failure and missing source-complete component witness. The corrected frozen source and original malformed-selector probe remain distinct under `build/nonpolynomial-rank/`; the malformed source is not compiler-defect evidence.

All nine positive sources now run with native exit `0` and Terminal Psi exit `0` on macOS arm64 using optimized producer `6c059d7ebd5ea6014adc4fdd87f3d58616f7d1c7`. All nine zero-decrement twins reject because a preserving cycle lacks strict descent. The source/oracle bytes remain frozen from the corrected baseline. Fixture names under `tests/omega/{pass,fail}/facts/` start with `nested_machine_joint_rank_remainder_`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](./main.omg) | [invalid](./invalid/main.omg) |
| field | [program](../../../field/computed_measure/remainder/main.omg) | [invalid](../../../field/computed_measure/remainder/invalid/main.omg) |
| parameter | [program](../../../parameter/computed_measure/remainder/main.omg) | [invalid](../../../parameter/computed_measure/remainder/invalid/main.omg) |
| call_result | [program](../../../call_result/computed_measure/remainder/main.omg) | [invalid](../../../call_result/computed_measure/remainder/invalid/main.omg) |
| match_join | [program](../../../match_join/computed_measure/remainder/main.omg) | [invalid](../../../match_join/computed_measure/remainder/invalid/main.omg) |
| const_name | [program](../../../const_name/computed_measure/remainder/main.omg) | [invalid](../../../const_name/computed_measure/remainder/invalid/main.omg) |
| other_state | [program](../../../other_state/computed_measure/remainder/main.omg) | [invalid](../../../other_state/computed_measure/remainder/invalid/main.omg) |
| other_machine | [program](../../../other_machine/computed_measure/remainder/main.omg) | [invalid](../../../other_machine/computed_measure/remainder/invalid/main.omg) |
| target_body | [program](../../../target_body/computed_measure/remainder/main.omg) | [invalid](../../../target_body/computed_measure/remainder/invalid/main.omg) |

The additional controls reject a rollover `8 → 7` where the produced remainder increases, a zero divisor, an overflowing child hidden by the final remainder, a different selected measure declaration, a proof-only decreasing input whose produced rank stays constant, and an acyclic proof-only measure with an overflowing child. Named-domain-qualified measure applications remain unsupported; exploratory named-domain and retired anonymous-type syntax probes do not establish qualification support.

A signed shared-arithmetic control proves the remainder correlation for both divisor signs with variable operands, while its non-singleton-quotient control rejects an unjustified affine equality. A focused kernel test keeps nested remainder/quotient dependencies genuinely opaque across two symbolic arrivals and verifies both signs at concrete replay; a missing leaf refuses.

The requirement evidence follows the same complete path:

- **Establishment:** every member independently requires its exact selected cell in `4..=7`; the admitted `%` application establishes its produced rank. Its requested rank interval supplies no premise for operand or child formation.
- **Identity and lifetime:** entry snapshots retain the selected cell, selector bounds and measure identity. The selected write changes that cell; the other cell and canary remain unchanged. Opaque arithmetic denotations retain their operand dependencies when rebound.
- **Composition:** the delivery edge preserves the rank; the decrement edge decreases the actual produced value. The whole SCC, including the nested helper, must contain descent on every complete cycle. Runtime checks exercise both selectors, active cycles and the zero-rank completion path.
- **Soundness boundaries:** the six controls enforce application formation, declaration identity and produced-value comparisons, including erased proof-only uses. Removal of the raw-input decrease fallback prevents assuming a view is monotone.
- **Checked to Terminal Psi:** checked indexed observations and tail transfers lower to constant-stack control flow; `omega run --both` observes the authored counters and writeback under the Terminal interpreter.
- **Provider, layout and ABI:** all four hosted `ProgramEntry` bindings are authored. The executed macOS arm64 route passes explicit mutable references, selector and counter, and preserves record/array neighbor storage.
- **Instructions and activation storage:** native loop edges, indexed addresses, live backing and returning continuations must preserve the same final counters and canaries.
- **Encoding and native image:** direct native execution and Terminal Psi independently return zero. Other hosts are authored but were not executed by this macOS run.

This closes the scalar arithmetic remainder production of the joint-rank fact, not every declared body form or the parent migration. Structured/call-computed or cross-carrier measure bodies, nonscalar orders, dependent distances and the parent's other acceptance remain open. Focused signed/policy regression checks and independent review are recorded with the PR; an existing unsupported case is not relabeled as accepted.

Focused comparison covers 309 termination/remainder cases. The other 308 outcomes are unchanged; a constant-view rejection now reaches the actual descent judgment, so its obsolete classification diagnostic was migrated and replayed with zero lost outcomes. Historical unsupported source cases remain visible; this is not a claim that the whole termination domain is green. A proof-only polynomial probe also refuses identically on the baseline and candidate, so general proof-only scalar-view support remains open.
