# REGISTERED-CALLBACK-LIFETIME — lane status at 5958706064e

Board row: `TASKS.md` (linear external-root registration, unregister ends it
before code/component leases; `component-publication` ledger +
`registered_callback_lifetime.rs` authored-contract chain). Owner:
Zergling-108. Host: Linux x86-64, revision `5958706064e`.

## Re-verified legs (all green)

`cargo nextest run -p checked-trees-to-lowered-psi --test suite -E
'test(registered_callback_lifetime)'` — 7/7:

- `unregister_route_lowers_capacity_bounded_program_local_schema`
- `verified_catalog_exposes_the_unregister_producer_row`
- `interpreted_unregister_dispatch_forwards_the_live_registration`
- `interpreted_register_unregister_round_trip_drives_the_ledger`
  (check → lower → verify → codec → interpret, both boundary effects in order)
- `sum_reply_case_payload_authorizes_the_routed_domain` + both controls —
  the routed-domain grammar seam (`3cfe2d969d`) is live.

## Frontier measured on this host

I probed `lower_machine(&checked, "Customer::run")` on `REPLY_SUM_SOURCE`
(the authored sum customer with `Registered(registration: Registration in
Live)` + `Rejected`, driving retry-then-unregister through `transition`):

```
InvalidUnitMachinePlan { machine: "Customer::run",
  reason: "attached Unit closure is missing a checked transitive machine plan",
  omission: "`Customer::run` has no admitted body (local construction stopped
    at statement sequence: local data: structural call binding, state 0,
    statement 0)" }
```

i.e. checked admission passes (the row's grammar-seam fix holds) but the unit
machine plan still will not admit `let reply: Reply = Registrar::register(..)`
as a local structural call binding — the "unit-machine plan admission for the
affine-classified sum result" leg the test file assigns to a sibling item.

Second confirmed gate, unchanged: installed-provider boundary results are
capped to `Structural` with `multiplicity == Affine` and empty
qualifications/projected_qualifications/claims in terminal-interpreter
`call_operations.rs:354` (`supported_result`); the sum's qualified
`Registered` payload cannot ride the installed path yet — only the
uninstalled effect path admits it. Native callback entry remains
CALLBACK-PRIVATE-MATERIALIZATION's (host-gated foreign invocation witness).

## Residual fence map

No live claim fences this item's named surfaces at probe time
(`registered_callback_lifetime.rs`, `component-publication`,
`private_callbacks`, `call_operations.rs` all unclaimed) — but the two open
legs above are explicitly sibling-owned in-tree, so the honest slice under
this claim is the measured frontier, not new machinery.
