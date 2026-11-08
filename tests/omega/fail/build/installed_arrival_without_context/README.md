A timer handler must be able to acknowledge an interrupt even when it never
requests a shared context view. This check verifies that the compiler reaches
the later startup-condition check for such a handler.

The program keeps the complete 33-entry exception/timer floor, exact callback
requirements and destinations, retained table/context, TaskStateStorage and four
guarded stacks. A port read selects direct, ordinary-helper, dynamic and
nonreturning forwarding paths. Each forwards the same owned Pending Ack through
the Scope-selected PIC completion. No context-view application is instantiated.

This ordinary Application supplies no installation environment assumptions.
The expected bootstrap-assumption refusal occurs after arrival-helper checks.
It is a positive control for those checks and an explicit refusal of physical
installation. It does not execute privileged instructions or establish CPU,
backing, lifetime, permission, publication or delivery facts.

The sibling installed_arrival_wrong_controller copies the complete same floor
and differs in one completion operand in the dynamic forwarding body. Both
installations keep PicTimerAcknowledgement::complete. The alternate operation
has the same owned Ack type and delegates to the same PIC action, but must
refuse as another installation controller before conditions are considered.

The inherited InterruptEntry contract excludes an added opaque forwarding
service. Its attempted source route refused at the authored invokes ceiling;
this fixture does not widen that contract or claim installed-token execution
coverage for the disallowed route. The existing selected-provider graph control
retains its separate source and ordinary-runtime coverage.
