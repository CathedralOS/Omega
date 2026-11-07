A timer handler must acknowledge an interrupt through the completion operation
chosen for its installation. This program deliberately chooses another
operation, even though it delegates to the same PIC action.

This root reuses the complete 33-entry floor from the explicitly imported
uefi-storage-entry::installed_interrupt_floor module and changes only the static
timer-handler selection. That handler forwards the unchanged Pending Ack to an
ordinary helper selecting another exact completion. The installation still
selects PicTimerAcknowledgement::complete. Native admission must report that the
helper selects another installation's controller, before conditions are checked.
No installation assumptions or runtime authority are supplied.
