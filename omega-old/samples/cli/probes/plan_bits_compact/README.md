# Compact field layout

`CompactBits` is a named `FixedLayout<PackedFlags>` implementation selected by
`LayoutStorage<PackedFlags, CompactBits>`. The three logical fields share one
byte; two fragments form `code`. Field reads and writes must preserve sibling
bits and reconstruct the complete scalar.

This remains a checking probe. Both local outcomes return normally, so it is
not runtime evidence that the packed fields were preserved.
