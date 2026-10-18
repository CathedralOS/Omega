# Private physical-entry stack

The selected source adapter retains a 160,000-byte workspace through an
ordinary helper call. It exceeds the closed 128 KiB firmware stack premise;
the image shell must switch to accounted private backing before allocating
that frame. The source and firmware guarantee are not rewritten to fit.

A nonzero firmware image handle returns EFI_INVALID_PARAMETER; zero returns
EFI_UNSUPPORTED. Neither path installs storage roots or calls the semantic
continuation. The smaller `uefi_selected_physical_entry` exercises the same
entry route without a large frame. `uefi_entry_schema_same_leaf` remains the
missing-physical-selection control.
