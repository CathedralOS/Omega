# CANARY-ACQUIRES-THROUGH-HELPER-RETURN — record

Re-verified at `e70748c995` on linux x86-64.

## Resolution

Already landed. The stub names the canary
`tests/omega/pass/capabilities/acquires_through_helper_return` — chapter 18
nested-acquires: `Vault::pick` mints `Folder::Writable` at the `Desktop`
boundary; `Backup::stage` and `Main::main` never touch the boundary but
receive the minted `Folder` through nested helper returns, so the
authority-flow report must propagate `acquires` up the call graph with the
helper recorded as provenance.

The machinery is `propagate_nested_capability_flows` in
`typed-trees-to-checked-trees/src/facts/capabilities.rs`: it collects call
edges from the service-reach plan and fixpoints `returns`/`derives`/`acquires`
verbs up to callers whose received value reaches the authority, recording
the helper state as `via_state_symbol`.

Board notes grouped this canary into the InvalidUnitMachinePlan family
(with CANARY-RUNTIME-LITERAL-DISPATCH-EXIT et al.), but the moved
ProgramEntry-attachment failure never touched it — it compiles and its
flow facts satisfy the roster.

## Verified at this stamp

- `OMEGA_PASS_CANARY_FILTER=acquires_through_helper_return cargo nextest
  run -p compiler --test canary_suite -E 'test(pass_canaries_compile)'`
  → PASS (18.3s).
- `capability_flows_retain_exact_direct_and_propagated_sites` → PASS; the
  roster pins both propagated routes: `Backup::stage acquires via
  Vault::pick`, `Main::main acquires via Backup::stage`.
