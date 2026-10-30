# Saved reference and reseated descriptor

Owner: `NESTED-MACHINE-UNIFICATION` in `TASKS.md`. This continues the same
[producer-written reference-payload rank fact](../README.md).

A private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

The producer writes 7 or 0 to the original Leaf, moves its mutable reference
through `current` into `saved`, reseats `current` to a distinct Leaf, and writes
count 19 and stamp 929 through that reseated descriptor. It returns `saved` inside Pending.
The public result bound remains 7; the cycle consumes it directly without a
caller guard rebuilding the bound. The original Leaf begins with count 55 and stamp 413; seven
iterations take the counter from 3 to 10, and the empty path preserves 10. Callers check the
original leaf, guard 827, canary 641 and the replacement'scount 19 and stamp 929. Payload
observations finish before the replacement is inspected, respecting the public
lifetime's possible-origin loan frontier.

Each of the nine sibling places has a twin changing only the producer write from 7 to 8.
Both source checking and the own-Ensures diagnostic are frozen against main
`71d80181a45253213d2f167b812ebf4e0ff902a9`: all nine valid positives currently
refuse `input` statement 6, as do the deliberately invalid twins. The exact
source/Build hashes and optimized baseline identity are retained in
`build/saved-reference-postcondition/`.

Acceptance is all nine positives running with native/Terminal agreement and
all nine fact-sensitive twins rejecting at the producer. It includes precise
snapshot/backing identity and effect invalidation, independent case and loan
formation, canonical Terminal production and cleanup, ordinary target layout/
ABI realization, native instruction/storage realization and emitted-image
execution. Omitting or redirecting the reseat/write changes the observed
replacement or original backing and makes main trap. Other-host runtime
acceptance and the parent capability's other requirements remain open.
