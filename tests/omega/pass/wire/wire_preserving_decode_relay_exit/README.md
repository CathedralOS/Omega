# Preserving codec relay

From a checkout with the `omega` executable available, prepare this project's
package lock with `omega update --project tests/omega/pass/wire/wire_preserving_decode_relay_exit`,
accept its package review, and resume that update. Then run:

```sh
omega run --both tests/omega/pass/wire/wire_preserving_decode_relay_exit/main.omg
```

Both native execution and the interpreter must return 70. The authored decoder
rejects an empty input and decodes `[41, 171, 205]` into the known value 41 and a
borrowed opaque tail `[171, 205]`. The encoder first relays the original bytes,
then relays `[42, 171, 205]` after the known value changes and the remainder moves
into the new value. The input stays unchanged. A substituted codec identity
rejects before writing output.

The companion `fail/wire/returned_preserving_remainder_input_write` fixture
rejects mutation of the input while the returned remainder's loan is live.
These are finite regression checks of ordinary authored machines; they do not
establish general codec agreement or earn `Derived` trust. The governing
contract is [Codecs and format policies](../../../../../wiki/spec/layouts/codecs.md).
