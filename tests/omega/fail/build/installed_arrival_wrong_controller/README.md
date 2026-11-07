A timer handler must acknowledge an interrupt through the completion operation
chosen for its installation. This program deliberately chooses another
operation, even though it delegates to the same PIC action.

This root depends on the complete 33-entry floor from uefi-storage-entry::installed_interrupt_floor and changes
only ArrivalForwarding's checked provider. The replacement calls a different
exact completion contract, despite using the same owned Pending type and
delegating to the same PIC operation. Native admission must report that it
selects another installation's controller. No installation assumptions or
runtime authority are supplied.
