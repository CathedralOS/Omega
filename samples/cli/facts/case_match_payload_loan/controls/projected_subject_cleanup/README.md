The computed subject keeps its original producer home through payload transfer
and disposes the anonymous residual on normal observation completion.

The named binding has a different lifetime: moving `receipt` leaves `bundle.note`
live. The positive explicitly drops that intact residual before borrowing the
counter again. The [invalid control](invalid/main.omg) omits that disposal and
must retain the counter-loan rejection. Its source preserves the original
`6ef02b8734676bc336bd122895f6c9ceeadc6c860bde880ceb5eb8d0c9ec78c3` probe;
the rejection is a language rule, not permission to end the named loan early.

This follows [partial ownership](../../../../../../wiki/spec/language/ownership.md)
and [root lifetimes](../../../../../../wiki/spec/terminal-psi/ownership.md#roots-continuations-and-order):
unselected siblings remain owned, and a partial ancestor cannot be disposed whole.
