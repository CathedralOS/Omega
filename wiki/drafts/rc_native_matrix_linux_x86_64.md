# RC native matrix — linux_x86_64 row

Witnessed row of the release-candidate native matrix on the Linux x86-64
host. Recorded at revision `e76d715c8e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision; legs that only cross-emit here are marked.

Verdict: **red** — 22 pass / 16 fail across 38 legs. The Service<R>
carrier-spelling family from the `8ae40607a3` row is closed on the
canary-suite hosted-receiver fixtures (`ff93300f44d` respelled them onto
the closed service carrier); the borrowed-storage residual shrank and the
sysv aggregate-entry fixtures now surface a newer exact-arithmetic
obligation gate. No leg that reached a produced executable misbehaved at
runtime.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (3.5s) |

## Native legs (canary_suite, `-p compiler --test canary_suite`)

| leg | filter | result |
|-----|--------|--------|
| Hosted receiver bridge | `hosted_receiver_linux` substring (module `entry_and_abi::hosted_receiver_linux`, 5 tests) | 5/5 |
| Hosted receiver, aarch64 cross-emit | same run (module `hosted_receiver_linux_arm64`, 3 tests) | 3/3 |
| SysV x86-64 entry ABI | `sysv_entry_abi` substring (14 tests) | 0/14 |
| Entry calling policy | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (module covers linux_x86_64 + linux_arm64 surfaces, 6 tests) | 6/6 |
| Source-evaluated native realization | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/linux_/) and not test(/arm64/)'` (8 tests) | 6/8 |

- `linux_free_unit_entry_runs_and_completes_with_status_zero` remains the
  green free-entry leg; the two bare-interface rejection legs and both
  provisioning legs are now green on both hosts' modules.
- Executed-on-host evidence that remains green: `linux_dynamic_execution::
  boundary_requirement_executes_its_foreign_call_on_linux_x64` runs the
  foreign call natively; GOT/PLT import-slot custody and dynamic ELF custody
  legs pass; `linux_dynamic_realization::import_bearing_linux_compiler_route`
  is slow (~83s) but passes.
- `hosted_receiver_linux_arm64` legs now compile the fixture fine and are
  cross-emit-only — they are green everywhere since they never execute an
  aarch64 binary.

## Failure families

The 16 failures reduce to three recorded residuals:

1. **Service<R> carrier spelling — source_evaluated residue** (2 legs:
   `linux_hosted_receiver::linux_hosted_receiver_normal_return_provisions_
   zii_storage_and_console`,
   `linux_hosted_receiver_explicit_exit_process_preserves_its_distinct_
   outcome`). Those fixtures still spell `console: Service<Console>
   in Bound`, rejected with `the core Service carrier is closed; it admits
   no authored qualification` / `no domain named Bound is declared for
   Service<Console>`. The canary-suite siblings were migrated in
   `ff93300f44d`; these two want the same respell. Owner family:
   **ENTRY-CONTENT-ROOTS** receiver-carrier migration (Service<R> cluster in
   wiki/drafts/known_baseline_failures.md).

2. **Exact-arithmetic proof obligation on sysv aggregate-entry fixtures**
   (7 legs): `sysv_small_aggregate_entry_spreads_consecutive_gprs`,
   `sysv_erased_small_aggregate_entry_spreads_only_relevant_fields`,
   `sysv_mixed_aggregate_entry_uses_independent_register_banks`,
   `sysv_mixed_aggregate_entry_rolls_wholly_to_stack`,
   `sysv_small_aggregate_entry_rolls_wholly_to_stack`,
   `sysv_large_aggregate_entry_copies_the_memory_class_stack_value`, and
   `sysv_wide_aggregate_entry_uses_general_memory_classification` now refuse
   at checking with `exact arithmetic in machine Main::main state main
   assignment may overflow u64` (decision 17 — exact arithmetic is a proof
   obligation; fix per the diagnostic: bound, guard, cast, or a defined-
   overflow domain). These legs previously reached the borrowed-storage
   family; a landing between `8ae40607a3` and `e76d715c8e` moved their first
   refusal earlier. Owner family: fixture migration under the exact-
   arithmetic obligation gate.

3. **Borrowed-storage ownership transfer** (6 legs) + **missing exact
   selected program entry** (1 leg), unchanged from the `8ae40607a3` row:
   `sysv_small_result_entry_loads_rax_and_rdx`,
   `sysv_large_result_entry_saves_and_uses_the_hidden_pointer`,
   `sysv_hfa_result_entry_loads_xmm0_and_xmm1`,
   `sysv_mixed_result_entry_loads_rax_and_xmm0`,
   `sysv_large_hfa_result_entry_remains_memory_class`, and
   `sysv_wrapped_float_entry_uses_xmm0_in_both_directions` transfer their
   non-copy result out of borrowed storage:
   `cannot transfer a non-copy value out of borrowed storage without
   replacing its owner in Main::main`. `sysv_hfa_entry_argument_packs_
   eightbytes_into_xmm_registers` remains the harness-migration residual —
   the fixture builds a native request with no `build.omg` root bind;
   repair pattern from `runtime_integer_division_value` (`e5912f303a`):
   bind through `builder.roots.bind`. Owner family: **ENTRY-CONTENT-ROOTS**
   / fixture migration.

## Row gaps

- Adjacent entry legs sampled for context and not counted above:
  `program_entries_and_image_validation` x2 fail with family 2's
  entry-binding diagnostic.
- `calling_policy_plans` counts include the module's linux_arm64 policy
  legs (host-independent plan evaluation); they are green.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows),
  UEFI runtime (no QEMU/firmware on this host).
