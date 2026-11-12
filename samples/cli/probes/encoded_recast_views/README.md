# Encoded recast views

`RawView` selects `LayoutStorage<PortableByte, SignedByte>`. Recasting an
exclusive byte loan to that view preserves its original backing. Reading the
signed field and replacing it must preserve the neighboring byte.

This remains a checking probe without a process-exit oracle. It does not
establish runtime agreement or a separate second-recast capability.
