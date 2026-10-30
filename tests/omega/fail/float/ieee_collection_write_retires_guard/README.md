# A collection store retires its saved IEEE guard

This is the exact source twin of the indexed disjoint-store runtime witness,
changing only the assignment target from `reading.other` to
`reading.values[index]`. The replacement is outside the integer conversion
range. The saved Boolean cannot justify converting that changed storage.
The compiler must reject the unqualified conversion for missing finite,
representable truncation evidence.
