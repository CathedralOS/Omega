# Pre-execution oracle freeze

Frozen UTC: 2026-10-11T00:35:06.986806+00:00

This freeze precedes all Omega check, update, native and Terminal invocations.
Integer outputs and exits are exact; absolute and relative tolerances are zero.
The schedule stays constant over the supplied bounded horizon.

Cases: 23

`reference.py` SHA-256: `15e2150932e79112f7b3193fa6792c838fb804efc8cd9b26b92ed0210f36b7c4`
`fixtures/oracle.json` SHA-256: `e1fe875c7b1eecae75af5defd9ac5de3cb7b0dbb06ec9186e996153548e557ed`

The independent recurrence uses exact rational expected reproduction and survival,
followed by the declared integer floors. No result is derived from Omega output.
Golden fixture bytes and exits are committed; validation rejects hash changes.
