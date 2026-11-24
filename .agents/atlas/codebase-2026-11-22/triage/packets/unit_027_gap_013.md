# Triage packet: unit_027 gap_013 (area 12, `unit_validation.rs` lines 9601-15600)

Status: In progress. Receipts are added as each claim is settled.

This packet reviews the correctness observations in
[record unit_027_gap_013](../../records/unit_027_gap_013.md) and the matching
lines of [digest 12](../../digests/12-native-representations.md). The baseline
is `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. `unit_validation.rs`
is unchanged between that baseline and `a0152d26`, so the record's line numbers
still apply. No open pull request touches this file.

`UV` is
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs`.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| A | Record §2 item 9; digest 12 line 192: `validate_function` takes an optional reference-membership validator, and `None` skips that check (`UV:9896`) | `UV:9896`, `:9932-9940`; caller `UV:1543-1575`; consumer `UV:8181-8210` | When a function forms a reference, its projected qualifications must equal the qualifications the source storage carries, including the memberships recorded in the descriptor graph. | Trace. `validate_function` only forwards the option. The unit driver builds `Some` whenever any function contains `EstablishReference` and `descriptor_results::optimized_graph(unit)` succeeds (`UV:1543-1558`). When no function forms a reference, the consumer arm cannot run, so `None` skips nothing. When the graph cannot be built, the consumer refuses a `SharedBorrow` reference that needs graph memberships (`UV:8205`). For other accesses it skips the graph-derived memberships (`UV:8192-8201`) and then requires exact equality between projected and captured rows (`UV:8211-8218`). That route compares against fewer rows. | Pass-through intended. The graph-failure route stays open. Its consumer lies in `UV:7510-8838`, outside this slice. Next step: a fixture in this crate where `catalog_graph` returns `None` for a unit with a `MutableBorrow` `EstablishReference` over storage with a domain membership. |
