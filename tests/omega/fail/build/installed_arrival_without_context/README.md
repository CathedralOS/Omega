A timer handler must be able to acknowledge an interrupt even when it never
requests a shared context view. This check verifies that the compiler reaches
the later startup-condition check for such a handler.

The program keeps the complete 33-entry exception/timer floor, exact callback
requirements and destinations, retained table/context,
TaskStateStorage and four guarded stack owners. Its timer only completes the
owned Pending occurrence; no context-view application is instantiated.

A port read selects direct, ordinary-helper, dynamic and nonreturning
forwarding paths. Each path must associate the same incoming Ack with the
Scope-selected PIC completion. The ordinary helper is a checked body, and the
nonreturning path forwards the Ack before halting.

The fixture intentionally supplies no installation environment assumptions.
The expected bootstrap-assumption refusal occurs after arrival-helper checks.
It is a positive control for those checks and an explicit refusal of physical
installation. It does not run privileged instructions or establish any CPU,
backing, lifetime, permission, publication or interrupt-delivery fact.

The existing uefi-storage-entry support package keeps these declarations in
its explicitly imported installed_interrupt_floor module. Both thin Application
roots choose their timer through an exact static machine argument and select
the same registrar/task providers.
The sibling installed_arrival_wrong_controller reuses the floor and
selects a timer that forwards to another completion operation. It chooses
another exact completion contract with the same Ack type and the same
underlying PIC action, which must
refuse before installation assumptions are considered.

The inherited InterruptEntry contract excludes an added opaque forwarding
service. Its attempted source route refused at that authored invokes ceiling;
this fixture does not widen it or claim installed-token execution coverage for
that disallowed route. The existing selected-provider graph control retains its
separate source and ordinary-runtime coverage.
