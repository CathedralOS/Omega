# Owned local computed float stores

A complete Console entrypoint writes direct call results into owned local f64
and f32 fields, updates the f64 field with selected arithmetic, and reads a
later scalar local. Native and Terminal must both exit 70; incorrect values
exit 71. The expected exit was declared before execution.

Before the repair, the current-main compiler at
`9fa24a5de5dab0498097bf2000eb6031754c09fd` rejected the first assignment with
`statement sequence: call: flow call` (source unit 0, bytes 622..635).
The application exposing this gap was travel time tomography: its owned `Fit`
received the objective call result. Checked computations already retain their
selected operations and call correspondence; value admission must not discard
that evidence solely because the destination is owned local storage. The
existing destination authority, type, range and computation-root checks remain.
