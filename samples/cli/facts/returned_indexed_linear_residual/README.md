# Returned indexed linear residual

Seed.code == 7 survives into the unconsumed element of a returned array owner;
consuming element zero preserves the original residual claim.

This CML4 row performs `consume(issue(seed.code).items[0]) - 7`. The affine
Pair owns two linear Receipts in a fixed array. Both hooks require code 7;
issue fixes element zero at 7 and preserves its input in element one. Every
reject twin changes only SEED_CODE from 7 to 8, leaving the producer and
consumer contracts valid while breaking the residual hook prerequisite.

The root repeats local. The nine places are local, field, parameter, call
result, match join, const name, another state, another machine and another
target body. Each transports the same Seed fact into the same operation.
All positives bind four hosted entries and require native/Terminal exit 0.
Bindings alone do not establish execution on another host.

The indexed path exercises fixed-array element identity in contract-result
projection and completed-operand substitution. It is distinct from the
record-field path in returned_linear_residual. Neither row permits an
authored [] operator to acquire builtin array meaning.

1. Establishment: completed element constructors establish code values before
   the array and enclosing Pair become available.
2. Identity and lifetime: element zero transfers its original claim; the
   untouched element retains its original owner and claim until normal death.
3. Composition: declaration-owned result guarantees and captured call actuals
   name the same exact field/fixed-index path in all nine places.
4. Soundness: literal selectors need builtin selection and an in-range closed
   extent. Runtime selectors gain no new admission. The seed-8 twins reject
   Receipt::drop's fixed prerequisite, without a caller equality guard.
5. Checked-to-Terminal: preserve selected transfer, scalar capture before hook
   mutation, residual cleanup and separate original-home retirement.
6. Layout and ABI: carry the complete array result and selected Receipt through
   the ordinary ProgramEntry and call plans; no particular register is assumed.
7. Instructions and frames: preserve operand order, outgoing scalar values and
   original cleanup actions without runtime liveness flags.
8. Images and execution: native and Terminal must independently produce exit 0;
   fuel/resume and cross-host execution remain separate acceptance.

Baseline 55c5385c96f5 rejects public preparation at issue's indexed ensures,
consume's input requirement and the sibling hook prerequisite. Fresh main
30fbbc91026a still refuses the unchanged row. The repaired
compiler runs the unchanged root and all nine places with native/Terminal exit
0 on macOS ARM64; every seed-8 twin rejects the residual hook prerequisite.
Replay with `omega run --both <place>/main.omg` after ordinary package preparation.

The proof controls retain source-selected operator meaning: authored, lexical
and abstract-bound `[]` cannot borrow builtin array laws. Plain builtin indexing
in an inherited requirement remains valid. Inherited functional guarantees are
proved at ordinary exits under their exact applied scope and parameter mapping;
returning the wrong same-typed input still rejects. These controls establish
this row's proof and custody path. Operator applicability uses exact closed
generic multiplicity bindings; projecting a computed copy does not select
builtin storage. Extent guarantees retain their actual result/formal binders
and selected Exact arithmetic. These repairs do not close the remaining CML4
cleanup, fuel/retry or
Windows/Linux execution requirements.

Authority: [proof contracts](../../../../wiki/spec/proofs/contracts.md),
[structural array construction](../../../../wiki/spec/terminal-psi/structural_access.md#complete-structural-array-construction),
and [claim paths and residuals](../../../../wiki/spec/terminal-psi/ownership.md).
