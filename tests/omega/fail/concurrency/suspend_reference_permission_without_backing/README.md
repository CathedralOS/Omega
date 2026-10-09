# Carry permission does not establish loan preservation

`omega --check tests/omega/fail/concurrency/suspend_reference_permission_without_backing/main.omg`
must reject the suspension crossings. A reference, a generic carrier containing
a reference, a slice, and a call-borne reference have `Carry::AcrossSuspend`
permission, but no checked join establishes their backing lifetime, outstanding
access restrictions and address preservation. The descriptor permission must
not bypass those independent obligations. The argument has no later caller use;
it remains live for the suspending invocation itself.
An unbounded generic parameter also cannot assume the absence of stored views
merely because its value has suspension carry permission.

This is a fail-closed implementation frontier, not a language prohibition on
borrowed values across suspension. Valid retention needs the per-value loan
relations in checked carry, the canonical Terminal frontier and the selected
runtime's preservation/backing join.
