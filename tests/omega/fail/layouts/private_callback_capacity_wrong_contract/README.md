# Reject a callback in a slot with a different contract

A program must put each callback in a table slot that expects its declared
contract. This test swaps two callbacks with different contracts while keeping
all 33 selected callbacks unique. The compiler must reject the mismatch before
writing their addresses or executing the receiving C function.

The positive is `pass/layouts/private_callback_descriptor_fragments_exit`.
Only the selection of those two callbacks changes. The expected diagnostic
names the callback binder and native-place demand because those are the
selected callback and destination records being compared.
