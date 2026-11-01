# Target-scoped constant realization

Owning tasks: BUILD-OWNED-TARGET-SELECTION (retiring this row's remaining Source
target families/default selection) and PROVIDER-SELECTION-AFTER-TERMINAL /
TARGET-SEMANTIC-APPLICATIONS (its wider accepted application requirements).

Fact: a pure terminating boundary-trait provider selected per target closes the
fixed-array extent of `Main { values: [u8; limit()] }` before layout observation
(`size_of<Main>() == 4u64` on Linux and `8u64` on Windows/macOS), discharging exact
subtraction `observed - 4u64` and runtime target-size agreement across all nine
places. Every place copies this sentence; its reject twin changes only the
selected provider's extent to `2`, which cannot justify that subtraction.

The root and nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [another target body](target_body/main.omg). Each `<place>/invalid/main.omg`
is paired with the corresponding positive and corpus pin under
`tests/omega/{pass,fail}/facts/target_scoped_constant_realization_through_*`.
The primary [root](main.omg), [multi-method](multi_method/main.omg) and
[zero-extent](zero_extent/main.omg) controls remain part of the owned cohort.

Build selects the original `Small` provider (`4`) for both Linux profiles and
`Large` (`8`) for Windows and macOS. `Same` remains the separate equal-valued
provider. `Answer::get` keeps its unrestricted `u64`, pure, terminating contract;
no minimum extent is imposed on valid helpers. Each explicit single-target
compilation closes through its own provider. A multi-target compilation still
requires the same provider for a provider-derived constant: different target
rows do not make different values legal in one checked program.

Ordinary named implementations replace the four identical Source `target_size`
bodies. Their public `LayoutObservation` contract is exactly
`result == size_of<Main>()`, preserving every admitted extent. Build selects that
one implementation for all four profiles. The independent `ExpectedExtent`
requirement retains the original runtime expected values: `4 || 8` for the row,
`0 || 8` for the zero-extent control. It does not constrain `Answer` or justify the
caller's subtraction. An ordinary `expected()` forwarding machine preserves the
existing consuming call shape.

The [extent-two helper control](helper_two_extent/main.omg) uses the same exact
layout helper/contract and the twin's selected provider returning `2`. It exits
zero after observing `2`, without the row's unrelated subtraction demand. It is
a preservation control, not an additional nine-place row. The original
`target_body/invalid` must instead reject specifically on `2 - 4`, after the
helper's exact guarantee has been checked.
The [wrong-query opposite](helper_two_extent/invalid/main.omg) keeps that same
`size_of` body at extent two but promises `align_of<Main>()`; it must refuse the
exact difference `1` against required zero, preserving the query kind.

The match place exercises both arms. Each arm writes its own `17` or `23` route
marker through caller-owned storage, and `mark_size` preserves the original
observed value. The first route is captured before the second write. Both
subtractions and runtime expected-value checks remain, so flipped dispatch or an
always-selected arm cannot satisfy the authored result. The original weak
single-arm oracle is retained only as historical evidence.

Applicable obligations:

- **Establishment and composition:** ordinary checked pure terminating providers
  establish the extent. The array and layout query retain its exact application
  dependency. Calls, fields, joins, constants and nested machines transport that
  observation; each consumer proves its original exact subtraction directly.
- **Identity and lifetime:** the completed whole return is joined to its exact
  operand, state, statement and unique outgoing occurrence. Its occurrence recipe
  is consumed without rereading source after return effects. Projected constructor
  children keep their replay-stability check. No stale mutable read is accepted.
- **Formation and soundness:** public requirement contracts, selected builtin
  comparison meaning, exact integer carriers, endpoint formation and admitted
  target-dependent recipes remain required. An equality cannot form its own
  layout operand. Closed ordinary guarantees are checked immediately rather than
  published as deferred target debt. The unused crashing first method in
  `multi_method` remains outside evaluation of the pure second method, while
  the selected conformance must still be complete.
- **Terminal Psi and native realization:** all positive programs bind
  `ProgramEntry` for the four hosted targets and declare exit zero. Acceptance
  requires actual native/Terminal agreement on the host. The zero-extent control
  distinguishes a completed zero layout from unresolved layout; foreign artifact
  production is distinct from execution.
- **Historical provider closure:** Terminal carries the exact requirement,
  selected declaration/body, normalized canonical plan commitment and target
  scope. Receiving/native admission reconstruct those commitments. This folding
  history does not install runtime authority.

The previous implementation's baseline at
`53cd2938437a29356a48d6eaf29f14de07a4bb99` refused the original Linux program with
`build-time call from limit has no exact target-machine identity`. Its original
source hash, compiler identity and successful source-backed canonical admission
controls remain historical evidence. Those controls challenge equal-valued
provider replacement, absent historical closure, copied digests, repeated plan
inputs, target mismatch and provider-free folding; their historical observations
are not relabelled as fresh executions of this migration.

This migration's frozen fixed base is
`f66e17c47e5aa4cde70c084abdbb984865147ef5`, using the byte-identical optimized
`60ee163d4f0f5f8b9349ecf253660a917917a7c7` producer and its own SDK snapshot.
All ten original positive pins checked and ran with native/Terminal exits zero.
All ten original negative pins refused, but nine retained old diagnostic
fragments failed: the same selected extent-two subtraction now reports exact
constant bounds `[-2, -2]` outside `u64`. Those raw old matcher failures remain
recorded separately from proper negative acceptance.

The migrated canonical layout helper exposed a valid-source refusal at that
fixed base: `result == size_of<Main>()` could not be proved, including the legal
extent-two helper-only control. An existing protected diagnostic producer
attributed this to the return replay-stability gate. The repair reuses the
bounded completed-return/closed-endpoint owner already exercised by the
`target_layout_derived_constant` work, retaining its genuine target-dependency
fence. Older retained producer exploration is distinct from exact-candidate
qualification; fresh migration acceptance is pending.

Execution receipts, frozen original/migrated inputs and producer identities live
in the existing ignored qualification directory
`build/target-scoped-constant-migration-20261101`. The broader task's generic
application closure, supplemental intrinsic-plan reconstruction, provider-free
observation dependencies and other remaining acceptance stay open.

Governing contracts: [Build configuration](../../../../wiki/spec/build/configuration.md),
[evaluation](../../../../wiki/spec/language/evaluation.md),
[constants](../../../../wiki/spec/language/constants.md),
[Terminal product](../../../../wiki/spec/terminal-psi/product.md), and
[boundary calls](../../../../wiki/spec/terminal-psi/boundary_calls.md).
