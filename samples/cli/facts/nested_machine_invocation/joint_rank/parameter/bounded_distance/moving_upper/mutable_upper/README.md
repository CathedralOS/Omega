# Mutable distance endpoint

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../TASKS.md).

The parent joint-rank fact also uses an upper endpoint stored in a mutable
slice. All nine places contain `bounded_distance/moving_upper/mutable_upper/`:
local, field, parameter, call result, match join, const name, nested machine
(`other_state`), another machine and a Build-selected target body. The source
fact sentence, interval and counter operation are identical in each place.

The selected cell starts at 7. The cycle advances its lower cursor three times,
then decrements that cell four times. The caller observes `Counts { value: 10,
climbs: 3, shrinks: 4 }` and `[641, 3, 919]`; a second idle invocation preserves
every value. Each twin changes only the selected-cell decrement from 1 to 0.
Branch inversion, an omitted store or a neighboring-slot write breaks the
caller oracle even if the total step count happens to remain seven.

The clean fixed base `0d5180a7d8bbdedca136fb88f1c1dc2a9b39db23` admits all
nine positives but refuses native production at `state graph: natural ranks`.
The repaired path retains both distance operands as exact prepared coordinates
with their own selector/index receipts. A proof expression alone cannot supply
an executable indexed entry observation. Computed upper expressions keep their
expression semantics; user-selected indexing is not physical-coordinate evidence.

- Establishment: every invocation independently requires a formed selector and
  the lower/upper ordering. Missing-index and missing-endpoint controls reject.
- Identity: an arrival captures its own cell value. The decrement invalidates the
  current observation, and the recurrent edge reads the replacement; keeping the
  same descriptor and selector does not keep the previous cell's value.
- Composition: explicit helper arguments and each helper's own requirements justify
  the complete cycle. Lexical nesting grants no parent premises.
- Soundness: scalar formation, selected operator meaning, loans and write authority
  remain independent. Zero decrement supplies no strict edge.
- Psi production: the two operands keep the existing checked coordinate/receipt
  representation through ordinary scalar computations and Terminal production.
- Omega realization: each positive binds all four hosted entry targets. Execution
  evidence here covers macOS ARM64 only; other-host execution and portable WCSU
  remain outside this repair.

Fixtures are `tests/omega/{pass,fail}/facts/nested_machine_joint_rank_mutable_upper_*`.
Raw baseline/candidate commands, producer identities and engine observations live
in this workbench's `build/mutable-upper/`. This production does not close the
parent task's remaining normal-return, nonscalar, migration or product-compiler
acceptance.
