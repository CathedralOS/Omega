# Zero-established locals before ranked arrivals

Run `omega run --both main.omg` after preparing this project. Expected exit: 0.

The countdown declares array scratch storage in its entry and record/scalar
storage in a named state without authored initializers. Those fresh declarations
must preserve the independent `remaining` premise. The named state writes the
record, saves its field, and forwards that value beside the immutable scalar's
zero; the final observation catches scalar-binding ordinal drift.

This is ordinary runtime execution of the same prefix operation used by the
Spread16 layout policies. The corresponding negative
`fail/termination/zero_local_does_not_decrease_rank` keeps the counter unchanged:
zero establishment supplies no descent. Existing zero-domain and prefix-write
controls independently require establishment and reject invalidated rank facts.
