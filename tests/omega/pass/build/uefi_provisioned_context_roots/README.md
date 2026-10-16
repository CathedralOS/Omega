# Provisioned context and semantic roots

An ordinary mutable context does not change the two semantic root arguments or
establish their physical backing. This variant must retain the same physical
entry obligation as `uefi_entry_schema_same_leaf`, after checking and staging
the context and semantic wrapper. Ordinary local record storage must not be
mistaken for another entry parameter. Native publication currently refuses because
the source bootstrap and installed-root bridge are not implemented.
