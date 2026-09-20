# RC native matrix — linux_arm64 row

Witnessed row of the release-candidate native matrix for the linux_arm64
target. Recorded at revision `6ef64f6dd6` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision. Suite legs are cross-compile/plan evidence — canary
runtime legs are `#[cfg(target_arch = "aarch64")]`-gated and skip here. One
execution leg ran under userspace emulation, `qemu-aarch64-static`
`1:6.2+dfsg-2ubuntu6.31` (qemu-aarch64 6.2.0, Debian), named per the closure
rule that emulated runs must name the emulator and version; it is reported
separately and not folded into the suite counts.

Verdict: **red** — 13 pass / 73 fail across 86 witnessed legs, plus one
passing emulated execution leg. Against the prior row at `e76d715c8e`
(11/75), the source-evaluated hosted-receiver pair closed — the
`Service<R>` carrier-spelling residual is gone there — and the same
frontier residuals carry the rest. No leg that reached a produced artifact
misbehaved on the AArch64 side of its assertions.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (4.0s) |

## Native legs

| leg | filter | result |
|-----|--------|--------|
| Entry calling policy (linux x86_64 + linux_arm64 shared module) | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (6 tests) | 6/6 |
| Hosted receiver bridge | `hosted_receiver_linux_arm64` substring (module `entry_and_abi::hosted_receiver_linux_arm64`, 3 tests) | 3/3 |
| `linux_arm64` substring cohort (adds the float-semantic-twin leg) | `cargo nextest run -p compiler --test canary_suite -E 'test(/linux_arm64/)'` (4 tests) | 3/4 |
| `aarch64` substring cohort (AAPCS64 entry ABI, cross-aarch64 import custody, artifact footprints, asm aarch64 refusals, aarch64 float rewrites) | `cargo nextest run -p compiler --test canary_suite -E 'test(/aarch64/)'` (72 tests) | 0/72 |
| Source-evaluated hosted receiver | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/arm64/)'` (2 tests) | 2/2 |

- The six `linux_entry::*` calling-policy legs (AAPCS64 physical/semantic
  replay + signature rejection for both linux profiles) are
  host-independent and green.
- `hosted_receiver_linux_arm64`'s two runtime legs compile the authored
  `Service<Console>` receiver to an ELF and print `SKIP: hosted receiver
  runtime requires Linux ARM64` — execution is `#[cfg]`-gated; the bare
  `Console` field leg fails closed on the `Service<R>` carrier rejection.
- The `hosted_receiver_linux_arm64` module's three tests also appear inside
  the `linux_arm64` substring cohort; distinct leg counts dedupe them.
- The source-evaluated arm64 pair was 0/2 at `e76d715c8e` on the closed
  `Service<Console>`/`Bound` spelling; it is green at this revision.

## Emulated execution leg (not a suite count)

| leg | command | result |
|-----|---------|--------|
| `cli_mvp` default selection compile | `omega --timings --target linux_arm64 samples/cli/basics/cli_mvp/main.omg` (after `tools/benchmark/benchmark.py prepare` settled `omega.lock`) | pass — `published native output` (ELF 64-bit LSB aarch64, statically linked), 1279539 ms |
| `cli_mvp` run under named emulator | `qemu-aarch64-static 6.2.0 <executable>`, stdin `/dev/null` | pass — stdout `Hello, Omega.` + `[press Enter to close]` prompt, exit 0 |

First recorded execution of an emitted aarch64 artifact for this row. A
native `aarch64-unknown-linux-gnu` host re-run remains the closure
evidence: qemu-user does not cover kernel-ABI paths the emulator
short-circuits, and the suite's `#[cfg]`-gated runtime legs still wait on
real hardware.

## Failure families

All 73 failures reduce to these residuals:

1. **Selected ProgramEntry rejoin** (41 legs, dominant): `selected
   ProgramEntry establishment rejoins 0 Terminal attachment identities;
   expected one` — the live selected-dispatch service-custody frontier
   recorded on the prior row and the linux_x86_64 row. Covers the
   cross-aarch64 import legs, most artifact-footprint legs, and
   `bounded_carrier_regressions_compile_on_aarch64`.

2. **Exact-arithmetic proof obligation** (8 legs): `exact arithmetic ...
   may overflow u64/u32/i32: the operands are not provably in range
   (decision 17)` — five `aarch64_entry_abi` argument fixtures plus three
   artifact-footprint fixtures; the fixture-spelling respell family the
   linux_x86_64 row already applied on its side.

3. **Entry selection refuses param/result fixtures** (8 legs):
   `native-artifact production requires one exact selected program entry`
   — three `aarch64_entry_abi` result/argument fixtures and the five
   `inline_asm` x86-asm-refusal legs. The hosted `ProgramEntry` slot
   admits no visible parameters or result; ENTRY-CONTENT-ROOTS residual.

4. **Borrowed-storage ownership transfer** (7 legs): `cannot transfer a
   non-copy value out of borrowed storage without replacing its owner` —
   three `aarch64_entry_abi` result fixtures plus four footprint legs.

5. **Unsupported lowering arm** (3 legs): `native-artifact Terminal
   production failed: Lowering(Unsupported(...))` — two convert-write
   footprint legs and `cross_aarch64_stack_import_compiles_with_planned_
   layout`.

6. **Named float rewrite selection** (2 legs): the
   `*_selects_aarch64_fmadd_and_executes` pair asserts on the selected-
   rewrites/provider-plan surface — the fmadd plan still does not land.

7. **Ensures-contract / domain-field proof** (1 leg):
   `compiler_body_to_indexed_copy_footprints_...` cannot prove the
   `FixedVecI32x4::push` exit contract.

8. **Unit-closure machine plan** (1 leg): `place_guard_footprints_...`
   — `InvalidUnitMachinePlan`.

9. **Missing footprint evidence file** (1 leg):
   `compiler_body_storage_bit_field_write_footprints_...` panics
   `Os { code: 2 }` reading expected evidence — fixture gap.

10. **Operator-occurrence resolution** (1 leg, `linux_arm64` cohort):
    `linux_arm64_float_semantic_edge_twin_retains_artifact_evidence`
    stops at `authored Operator declaration selection occurrence 178
    remained unresolved after successful checking` — unchanged from the
    prior row.

## Row gaps

- No suite leg executed an AArch64 image: this host is x86-64. The qemu
  leg above is the only execution evidence and is deliberately kept
  outside the counts.
- Runtime-status legs inside `pass_canaries_compile`'s monolithic
  aggregate are not separately filterable and were not run, same as the
  prior row.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows).

## Re-run condition

Re-run on a linux/arm64 host once the ProgramEntry-rejoin frontier and
the ENTRY-CONTENT-ROOTS fixture respells close; that host also converts
the emulated leg into native `#[cfg]`-gated execution evidence.
