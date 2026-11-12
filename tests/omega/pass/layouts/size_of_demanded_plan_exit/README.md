# A size query demands its layout

A query of `LayoutStorage<Flags, CompactBits>` evaluates and validates the selected
named policy even when no stored instance exists. The plan places the Boolean at
byte zero and reports one byte. The entry checks that size and exits 0; a different
size exits 1.

This replaces the retired negative assertion that a query without an earlier
stored instance must refuse. The current demand rule requires the query itself
to obtain the plan. The nearby `size_of_invalid_plan_rejected` control changes
only the plan alignment from 1 to 0 and must reject during plan validation.

Parent: LIBRARY-REPRESENTATION-FAMILIES. Rules:
[layout demand](../../../../../wiki/spec/layouts/plans.md#layout-demand-and-validation)
and [representation applications](../../../../../wiki/spec/layouts/plans.md#representation-applications).
