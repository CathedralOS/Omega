# Selected physical firmware entry

Build selects the checked physical adapter separately from the semantic
two-root continuation. The emitted PE entry transports the firmware handle
into an ordinary helper and returns its status unchanged. A real non-null
image handle yields EFI_INVALID_PARAMETER; a lost/zero handle yields
EFI_UNSUPPORTED. The image performs no storage installation and claims no
successful OS handoff.

The selected opaque representation describes the consumed System Table header;
no source operation dereferences it or claims the table's storage. This keeps
the ABI probe independent of firmware protocol providers. The sibling
`uefi_entry_schema_same_leaf` control has no selected physical adapter and
must still refuse native publication.
