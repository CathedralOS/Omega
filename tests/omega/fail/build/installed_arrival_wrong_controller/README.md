A timer handler must acknowledge an interrupt through the completion operation
chosen for its installation. This program deliberately chooses another
operation, even though it delegates to the same PIC action.

This Application copies the complete 33-entry floor from its sibling. Its main
source differs in exactly one operand: the dynamic forwarding body chooses
OtherCompletion::complete. Scope still selects PicTimerAcknowledgement::complete,
and the two roots retain the same direct, ordinary-helper and nonreturning
paths. Native admission must report another installation controller before
conditions are checked. No installation conditions or runtime authority exist.
