# RC native matrix — linux_x86_64 row

Witnessed row of the release-candidate native matrix on the Linux x86-64
host. Recorded at revision `8ae40607a3` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision; legs that only cross-emit here are marked.

Verdict: **red** — 15 pass / 23 fail across 38 legs. Every failure sits in
a fixture-migration residual; no leg that reached a produced executable
misbehaved at runtime.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (3.2s) |

## Native legs (canary_suite, `-p compiler --test canary_suite`)

| leg | filter | result |
|-----|--------|--------|
| Hosted receiver bridge | `hosted_receiver_linux` substring (module `entry_and_abi::hosted_receiver_linux`, 5 tests) | 1/5 |
| Hosted receiver, aarch64 cross-emit | same run (module `hosted_receiver_linux_arm64`, 3 tests) | 0/3 |
| SysV x86-64 entry ABI | `sysv_entry_abi` substring (14 tests) | 0/14 |
| Entry calling policy | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (module covers linux_x86_64 + linux_arm64 surfaces, 6 tests) | 6/6 |
| Source-evaluated native realization | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/linux_/) and not test(/arm64/)'` (8 tests) | 6/8 |

- `linux_free_unit_entry_runs_and_completes_with_status_zero` is the one
  green hosted-receiver leg: a free `Unit` entry binds no service receiver,
  produces the ELF, and executes it under `exit_group` status 0.
- Executed-on-host evidence that remains green: `linux_dynamic_execution::
  boundary_requirement_executes_its_foreign_call_on_linux_x64` runs the
  foreign call natively; GOT/PLT import-slot custody and dynamic ELF custody
  legs pass; `linux_dynamic_realization::import_bearing_linux_compiler_route`
  is slow (~97s) but passes.
- `hosted_receiver_linux_arm64` legs fail at fixture *compilation*, so they
  are red on every host — their runtime legs are aarch64-bound regardless.

## Failure families

All 23 failures reduce to two recorded residuals:

1. **Service<R> carrier spelling** (9 legs: hosted_receiver_linux x4,
   hosted_receiver_linux_arm64 x3, source_evaluated linux_hosted_receiver
   x2). Fixtures spell `console: Service<Console> in Bound` (or bare
   `Console` in the bare-interface leg); since `f705cbdb51` ("admit
   service carriers by exact closed `Service<R>` identity") the check
   rejects them with `the core Service carrier is closed; it admits no
   authored qualification` / `no domain named Bound is declared for
   Service<Console>` / `bare boundary trait Console in value position`.
   Sibling fixtures migrated in `0e1977994b`; `e1a0d10522` dropped the
   same spelling from the blocking executor. Owner family:
   **ENTRY-CONTENT-ROOTS** receiver-carrier migration (the Service<R>
   cluster in wiki/drafts/known_baseline_failures.md).

2. **Borrowed-storage ownership transfer** (13 sysv legs) + **missing
   exact selected program entry** (1 sysv leg). SysV aggregate entry
   fixtures transfer their non-copy result out of borrowed storage:
   `cannot transfer a non-copy value out of borrowed storage without
   replacing its owner in Main::main` (recent borrow-debt tightening,
   `48579a50d3`/`b33de114e4` family). `sysv_hfa_entry_argument` is the
   recorded harness-migration residual instead — the fixture builds a
   native request with no `build.omg` root bind; repair pattern from
   `runtime_integer_division_value` (`e5912f303a`): bind through
   `builder.roots.bind`. Owner family: **ENTRY-CONTENT-ROOTS** /
   fixture migration.

## Row gaps

- Adjacent entry legs sampled for context and not counted above:
  `program_entries_and_image_validation` x2 fail with family 2's
  entry-binding diagnostic.
- `calling_policy_plans` counts include the module's linux_arm64 policy
  legs (host-independent plan evaluation); they are green.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows),
  UEFI runtime (no QEMU/firmware on this host).
