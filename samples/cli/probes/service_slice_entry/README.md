# service_slice_entry

First sample lending a fixed array field from a Fused
entry receiver: `Ping` is a static provider slot selected
by `build.omg`, while `Worker::drain(&mut main.items, ...)`
projects `[u64; 3]` into `buf: &mut [u64]` through the
ordinary borrowed-view presentation.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

## Migration

Until 2026-10-20 `Main` carried a `Binding<Ping>` field named
`console`, and the probe described that field as riding provider
selection. The specification retired source-visible `Binding<R>`
(`wiki/spec/build/component_publication.md:198`), so the field and
its import are gone. Provider selection is unchanged —
`Build::select_provider<Ping, PingProvider>` — because it never
needed the carrier; the borrowed-slice lending this probe exists
to demonstrate was always on ordinary data.
