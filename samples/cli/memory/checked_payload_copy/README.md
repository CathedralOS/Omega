# Checked payload copying

Run `omega run main.omg`. Expected exit: 0.

A runtime check establishes a nonempty extent in a case payload. Dispatch
copies that qualified record into a successor state and uses its fields there.
The same operation works beneath an envelope field and after a helper call;
the rejected case must not enter the payload continuation.

This is the small witness for Cathedral's checked boot geometry handoff. It
tests value copying, not firmware reference admission or storage ownership.
