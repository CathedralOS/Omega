# arm field variance

Runnable probe; no standard-library dependency. `omega run main.omg` must exit 0
natively after `omega update --project .` reviews the local package inputs.

`pack` places its inputs in different fields on each transition arm; the claim
map a caller binds does not depend on which arm ran. Both projected members are
consumed by `Acknowledgement::ack`, a one-member boundary trait requirement
whose selected provider `ReceiptSink` is an external sink (each target's C
library `abs`). The acknowledged codes show which arm placed which input.

Without a selected provider, native closure review refuses the program
(`reachable requirement ... resolves to 0 selected provider rows`), pinned by
`tests/omega/fail/providers/consuming_trait_requirement_without_provider`.
