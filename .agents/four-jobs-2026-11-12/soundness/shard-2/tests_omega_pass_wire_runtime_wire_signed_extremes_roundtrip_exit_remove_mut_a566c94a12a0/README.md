A generated compact_binary codec encodes and decodes i64 minimum/maximum scalar
fields alongside a packed i32 array containing its minimum and maximum. The
fixture checks all 37 canonical wire bytes independently of decoding, then
checks the decoded values and consumed extent. Native and Terminal execution
must exit 0. This exposes lossy policy-qualified casts in both zigzag directions
and overflow in signed shifts. Existing ranged-field and wrong-era fixtures
retain invalid-input controls for the same generated scalar operations.
