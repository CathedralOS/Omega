# callback_slot_materialization

First sample on `PrivateCallbackSlot<Layout, machine
Requirement>` — a compiler-known relationship whose
second trait argument is a MACHINE
(`WindowProcedure::call`). Named implementations are
inert until a layout policy cites them through
`LayoutPlan::place_private<Implementation>(plan, offset)`;
runtime code receives no slot identity.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
