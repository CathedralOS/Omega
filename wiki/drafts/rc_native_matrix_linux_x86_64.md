# RC native matrix — linux_x86_64 row

Witnessed row of the release-candidate native matrix on the Linux x86-64
host. Recorded at revision `7b259409073` (2026-09-21), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision; legs that only cross-emit here are marked.

Verdict: **red** — 22 pass / 16 fail across 38 legs. The 14 sysv legs
still refuse identically at product admission with `native-artifact
production requires one exact selected program entry` (the
ENTRY-CONTENT-ROOTS residual below). Two previously green legs went red
since `6ef64f6dd6`: one real emission regression landed upstream
(`9e20e91559`, details below), one refusal-stage drift whose fixture pin
is stale.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (4.3s) |

## Native legs

| leg | filter | result |
|-----|--------|--------|
| Hosted receiver bridge | `cargo nextest run -p compiler --test canary_suite -E 'test(~hosted_receiver_linux)'` (module `entry_and_abi::hosted_receiver_linux`, 5 tests) | 5/5 |
| Hosted receiver, aarch64 cross-emit | same run (module `hosted_receiver_linux_arm64`, 3 tests) | 3/3 |
| SysV x86-64 entry ABI | `test(~sysv_entry_abi)` (14 tests) | 0/14 |
| Entry calling policy | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (6 tests) | 6/6 |
| Source-evaluated native realization | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/linux_/) and not test(/arm64/)'` (8 tests) | 6/8 |

## Failure families

1. **Param-carrying boundary entry cannot select a root** (14 sysv legs,
   unchanged). All 14 legs refuse at product admission with
   `native-artifact production requires one exact selected program entry` —
   `builder.roots.bind(linux_x86_64::ProgramEntry, Main::main)` is the only
   bindable slot and its `HostedApplication` schema (`visible_parameters:
   None`) rejects any machine declaring arrival parameters or a result.
   Owner family: **ENTRY-CONTENT-ROOTS** (still open on the board); the
   aarch64 siblings share the residual.

2. **NEW REGRESSION — emitted dynamic ELF crashes the host loader**
   (`linux_dynamic_execution::boundary_requirement_executes_its_foreign_
   call_on_linux_x64`). Green at `6ef64f6dd6` (exited 70 natively); at
   `7b259409073` the emitted image SIGSEGVs before `_start` runs
   (`SEGV_MAPERR` at `si_addr=0x8`, inside `ld-linux-x86-64.so.2`).
   Root cause isolated on this host: `9e20e91559` "omega: admit general
   `.rela.dyn` relocations for imported data slots" emits unconditional
   `DT_RELA`+`DT_RELASZ=0` dynamic rows for the empty general-relocation
   table but never emits `DT_RELAENT` (tag 9 = `sizeof(Elf64_Rela)` = 24).
   glibc dereferences `l_info[DT_RELAENT]` whenever `l_info[DT_RELA]` is
   present — faulting insn `cmpq $0x18,0x8(%rax)` with rax=NULL. Proven by
   masking the `DT_RELA` row in the emitted bytes → identical image exits
   70. Fix direction: emit `DT_RELAENT=24` alongside (or skip the pair
   when the table is empty). Owner surface: `dynamic_table/dynamic_tags.rs`
   under the general-relocation-admission family.

3. **Refusal-stage drift** (`linux_dynamic_realization::aggregate_foreign_
   boundary_members_refuse_at_terminal_entry_establishment`). Still
   rejects, but the diagnostic moved: the fixture pin expects
   `ProgramEntry establishment rejoins 0 Terminal attachment identities`;
   the compile now reaches lowering and fails
   `Lowering(Unsupported("record store destination projected beyond its
   authored root"))`. The aggregate boundary member progressed past entry
   establishment into a deeper fence — the pin needs re-pointing to the
   lowering refusal (or the leg's intent reviewed by its owner).

## Green legs of note

- `linux_free_unit_entry_runs_and_completes_with_status_zero` and all
  hosted-receiver provisioning legs stay green natively (~28–106s each).
- `linux_dynamic_execution::boundary_requirement_declares_got_plt_import_
  slot_custody` (emission-side custody check) and
  `linux_dynamic_realization::{external_boundary_requirement_via_leaf_
  realizes_dynamic_elf_custody,import_bearing_linux_compiler_route_*
  (~88s),rejected_native_reentry_*}` pass.
- `hosted_receiver_linux_arm64` is cross-emit-only here — green, never
  executes an aarch64 binary.

## Row gaps

- Adjacent entry legs sampled for context and not counted above carry the
  same entry-selection residual (`program_entries_and_image_validation`
  x2, `aarch64_entry_abi` x11).
- `calling_policy_plans` counts include the module's linux_arm64 policy
  legs (host-independent plan evaluation); all green.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows),
  UEFI runtime (no QEMU/firmware on this host).
