# Authored fixed layout

`SignedByte` is an ordinary named `FixedLayout<PortableByte>` implementation.
Its `IntegerAt` placement stores a signed value in one byte. `RawView` selects
that representation with `LayoutStorage<PortableByte, SignedByte>`; unrelated
plain byte storage retains its own representation.

This remains a checking probe. Its local success/failure values are not a
process-exit oracle, so it does not establish native/Psi runtime agreement.
