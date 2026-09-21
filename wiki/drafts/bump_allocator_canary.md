# BUMP-ALLOCATOR-CANARY

Re-verified at `c924529921dd` (linux x86-64). The row's premise is now
substantially stale: the package allocator is no longer fixture-local —
`source/library/alloc/` hosts a real package (`build.omg`, `bump.omg` 359
lines, `main.omg`) and `tests/omega/pass/memory/bump_allocator_canary`
reaches it through an authored `builder.depend(Source::Path)` row instead of
hand-instantiated copies.

## Landed since the row's premise was written

- `22ea73fa34cc0` — strategy hosted in the alloc package via depend edge;
  `canary_suite.rs::bump_allocator_canary_consumes_the_alloc_package_through_the_depend_edge`
  compiles the consumer with real `package_inputs` and asserts the package's
  own declarations arrive (`Split`, `Bump`, `Issued`, `Attempt`, `BumpVec`,
  `Recomposed`, `Growth`, `RetiredSlot`).
- Fixture surface now exercises: two coexisting infallible allocations with
  exact counted-residual accounting (`remaining` kept distinct from tail
  placement authority), `check_remaining` scalar bounds surviving backing
  consumption, `try_allocate`/`Attempt` exhaustion returning the strategy
  unchanged, tail-ward `release` of the newest region only, `reset`
  restoring the full original backing, `BumpVec` growth retaining one
  `RetiredSlot` buffer, in-place `shrink` of the live buffer's suffix, and
  a concrete `ResidentStorage`/`ResidentContentTransfer<P,T>` resident
  route (place → read view → retire) over `Extent in Granted & Vacant`.
- Fail controls green and rostered: `grow_with_live_resident`,
  `release_with_live_resident`, `reset_with_live_resident`,
  `resident_dropped`, `place_into_occupied`,
  `restated_resident_index_mismatch`, `shrink_beyond_buffer`,
  `cast_minted_{resident,vacant}`, `live_allocation_dropped`,
  `borrowed_strategy_carve` (the `&mut` field carve still rejects — the
  temporary-borrow leg).

## Still open (row's acceptance is not met)

- `Main::main` remains empty — the fixture is checked-only; no interpreter
  or native-host execution leg exists.
- Growth carries no elements: content-preserving movement waits on the
  placed element operations; `merge`/`reset` still demand `Vacant` on every
  part.
- Retained storage is the finite `RetiredSlot` checking example — an
  unbounded retained list needs placed-storage indirection + ranking proof.
- `ResidentStorage` establishment is still a pinned custody shape, not an
  evaluated-layout/provider route through PLAN-LAID-VIEWS; the
  `Initialize` family is undeclared.

## Verification at this tip

- `cargo nextest run -p compiler --test canary_suite -E
  'test(~bump_allocator_canary)'` — 1/1 PASS (depend-edge consumer reaches
  checked trees, 11.9s).
- `OMEGA_FAIL_CANARY_FILTER=bump_allocator` fail_canaries lane — PASS
  (all rostered controls reject with their pinned fragments).
