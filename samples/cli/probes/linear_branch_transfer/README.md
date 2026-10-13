# linear branch transfer

Runnable probe; no standard-library dependency. `omega run main.omg` must exit 0
natively after `omega update --project .` reviews the local package inputs.

Transition arms are alternatives: both arms transfer the same linear receipt
into a consuming state, and the checker joins them as one live claim.
`Acknowledgement::ack` is a one-member boundary trait requirement; the selected
provider `ReceiptSink` is an external sink (each target's C library `abs`), so
the acknowledged value is the receipt's code. `Receipt` has no `Receipt::drop`
plan, so an arm that skips the requirement call is rejected.

Without a selected provider, native closure review refuses the program
(`reachable requirement ... resolves to 0 selected provider rows`), pinned by
`tests/omega/fail/providers/consuming_trait_requirement_without_provider`.
