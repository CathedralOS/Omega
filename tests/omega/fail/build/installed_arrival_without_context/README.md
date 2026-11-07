A timer handler must be able to acknowledge an interrupt even when it never
requests a shared context view. This check verifies that the compiler reaches
the later startup-condition check for such a handler.

The program keeps the complete 33-entry exception/timer floor, exact callback
requirements and destinations, retained table/context,
TaskStateStorage and four guarded stack owners. Its timer only completes the
owned Pending occurrence; no context-view application is instantiated.

A port read selects direct, selected-provider, dynamic and nonreturning
forwarding paths. Each path must associate the same incoming Ack with the
Scope-selected PIC completion. The selected provider is an ordinary checked
body, and the nonreturning path forwards the Ack before halting.

The fixture intentionally supplies no installation environment assumptions.
The expected bootstrap-assumption refusal occurs after arrival-helper checks.
It is a positive control for those checks and an explicit refusal of physical
installation. It does not run privileged instructions or establish any CPU,
backing, lifetime, permission, publication or interrupt-delivery fact.

The sibling installed_arrival_wrong_controller reuses this same floor and
replaces only the selected forwarding body. It chooses another exact completion
contract with the same Ack type and the same underlying PIC action, which must
refuse before installation assumptions are considered.
