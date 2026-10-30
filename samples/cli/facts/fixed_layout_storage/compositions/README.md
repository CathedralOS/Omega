# Fixed policy compositions

Owning task: `LIBRARY-REPRESENTATION-FAMILIES`, named FixedLayout policy applications with complete static operands, nested references and portable physical observations. These are compositions of the existing storage row, not additional transport rows.

- [Closed requirement policy](closed_requirement_policy/main.omg) checks the same declared finite 7/11 contract at both calls and reads the authored byte at offset 8. Its [twin](closed_requirement_policy/invalid/main.omg) asks for an incompatible physical extent.
- [Finite dynamic policy](finite_dynamic_policy/main.omg) retains both finite Width applications through ordinary dynamic dispatch. Its [twin](finite_dynamic_policy/invalid/main.omg) makes the selected plan alignment zero.
- [Reference descriptor](reference_descriptor/main.omg) writes a fitting IntegerAt field in a record containing a reference to a generic representation, then checks both the value 7 and the unchanged pointee 43. Its [twin](reference_descriptor/invalid/main.omg) changes only the store to 256; a separate pinned wrong-actual control changes the pointee policy argument from 7 to 8 without changing geometry.
- [Computed const policy](computed_const_policy/main.omg) forms `Value + 1` under the genuine upper bound published by both the public requirement and implementation. Its [twin](computed_const_policy/invalid/main.omg) removes that bound while leaving the policy, actual caller and operation unchanged. An unused policy parameter does not discharge operand formation.

The [invariant open policy](unbounded_invariant_policy/main.omg), [semantic-only varying policy](semantic_only_policy/main.omg), [valid overridden policy](overridden_valid_policy/main.omg) and [unmentioned valid policy](unmentioned_valid_policy/main.omg) retain distinct Source checking contexts. A semantic description can remain abstract; a physical operation still needs complete geometry. Existing partial-target coverage remains in `tests/omega/pass/layouts/unused_partial_target_layout_exit`.

Each positive declares exit zero and ordinary hosted ProgramEntry rows. Runtime acceptance is pending for the combined const/formation repair; earlier immutable producer90 observations establish the closed requirement and dynamic cases, not the newer repairs. Source-invalid programs are never executed. Additional pinned controls cover uncalled finite members, unavailable universal geometry and invalid dormant or overridden policy bodies.

The nine-place storage and fragmented-byte rows remain the transport evidence. Aliases, conversion families, complete C ABI migration, product producer parity and the other remaining task requirements are not closed by these compositions.
