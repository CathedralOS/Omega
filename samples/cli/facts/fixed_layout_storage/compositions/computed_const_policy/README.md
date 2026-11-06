# Computed layout arguments use the caller's bound

A caller may select `Policy<Value + 1>` after proving that `Value` is below the
maximum u64 value. The bound belongs to the caller; choosing a layout must keep
it available when checking the supplied argument.

This [program](main.omg) writes a two-field payload through the selected layout
and checks that its value byte is 7. The [control](invalid/main.omg) removes only
the bound from the requirement and its implementation. It must reject for
unproved arithmetic even though the executable caller supplies 7.

The focused alias composition under
[tests/omega/pass/layouts/fixed_layout_computed_alias_policy_exit](../../../../../../tests/omega/pass/layouts/fixed_layout_computed_alias_policy_exit/)
passes the same argument through a transparent `PolicyStorage` alias. Its
[control](../../../../../../tests/omega/fail/layouts/fixed_layout_computed_alias_policy_rejected/)
removes the same bound. The original program, Build and oracles stay unchanged.

The [runtime-input control](../../../../../../tests/omega/fail/layouts/fixed_layout_runtime_alias_policy_rejected/)
keeps the arithmetic bound but supplies the value as an ordinary runtime
parameter. A safe addition does not make that input a const argument; it must
still reject even though the executable caller supplies 7.

Owning task: **LIBRARY-REPRESENTATION-FAMILIES**. Governing contracts:
[representation applications](../../../../../../wiki/spec/layouts/plans.md#representation-applications)
and [generic arguments](../../../../../../wiki/spec/language/generics.md).
The existing nine-place layout row and the parent's remaining acceptance remain
separate from this computed-argument repair.
