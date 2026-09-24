# Physical entry end-to-end audit

Audit at `03be2ee302` of the physical entry route — authored `Main` → hosted
process adapter → image entry symbol → container header entry field → host
process execution — read against the board row
`BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS` ("Host acceptance on
macOS/Windows/QEMU remains a host leg") and the mined stubs
PHYSICAL-ENTRY-BRIDGES / PHYSICAL-ENTRY-END-TO-END. Delete this audit once the
macOS, Windows, and QEMU/arm64 host legs report their own runs on that row.

## Route

1. `program-entry-plan` binds `Main::main` to the target's `ProgramEntry`
   root declared in `build.omg` (`builder.roots.bind`).
2. `image-emission::hosted_unit_entry::prepare` selects the physical entry
   adapter by exact target: `LinuxUnit` / `LinuxArm64Unit` / `DarwinUnit` /
   `WindowsUnit` (free Unit entries) and hosted-receiver bridges. The
   adapters are product-owned bytes; each calls the semantic entry and
   completes with status zero. A `ProgramEntry` has no result, so there is
   no scalar-returning entry adapter.
3. `image` binds `object.layout.entry_symbol` into
   `FinalImage.symbol_table.entry_symbol` and `function_linkage` validates
   the binding (entry symbol must be a bound text function).
4. `image-elf::entry_symbol::elf_entry_address` /
   `image-pe::entry_symbol::pe_entry_rva` resolve the entry symbol to the
   file address/RVA written into `e_entry` / `AddressOfEntryPoint`; the
   symbol must exist and live in text or emission fails closed.
5. `final_image_validation` replays the produced image: it decodes the
   emitted adapter bytes and checks the `e_entry`→file-bytes round-trip
   against the bound semantic function.

## Evidence on linux x86-64

Host execution of emitted images runs through
`image-emission/tests/artifacts/hosted_exit_runtime.rs` (`assert_exit`
writes the emitted image to a temp file, chmods, and spawns it).

`cargo nextest run -p image-emission --all-targets -E 'test(~exit) or
test(~entry) or test(~runs)'` @ `03be2ee302`: 19/19 pass, including

- `object_custody::hosted_exit_process_object_validation_replays_exact_scalar_and_trap_bytes` —
  emits an ELF and executes it on this host (scalar exit).
- `object_custody::linux_write_line_then_exit_survives_object_image_and_installation_replay` —
  write_line + exit end-to-end on host.
- `hosted_unit_entry::tests::{linux_x86_64_unit_entry_calls_entry_and_exits_with_zero_status,
  linux_arm64_unit_entry_..., windows_unit_entry_..., darwin_unit_entry_...}` —
  exact adapter bytes per target.
- `hosted_unit_entry::tests::{linux_elf_entry,windows_pe_entry}_selects_the_exact_adapter_bytes` —
  container entry selection for ELF and PE.

CLI-level check (`omega --accept-admissions --build-dir /tmp/omega-build-cli-mvp
samples/cli/basics/cli_mvp/main.omg` @ `03be2ee302`): the full native pipeline
ran (~25 min dev-profile compile incl. `omega-language-std`) to the package
review boundary, which refused publication with `cannot accept fresh package
review evidence` and staged three pending decision rows. That gate is the
project-acceptance transaction (wiki/spec/packages/acceptance.md) — unrelated
to the entry route — and `omega run` has no admission flag of its own, so the
CLI exec witness needs a one-time per-project review settlement
(`omega update --project … --target …` then edit `pending`→`accept`, `--resume`).
The artifact-level exec evidence above already covers emitted image → host
process → exit status, which is the physical entry contract itself.

## Host-gated legs

- **macOS arm64** — Mach-O entry selection and unit adapter bytes are
  emission-validated; process execution requires a Darwin host
  (`assert_exit` is `cfg(target_os = "macos")`-gated).
- **Windows x86-64** — PE entry RVA resolution and unit adapter bytes are
  emission-validated; `assert_exit` has no Windows lane (would need
  CreateProcess semantics).
- **linux arm64** — `bl` + `exit_group(94)` adapter is emission-validated;
  no QEMU harness on a x86-64 host.
- **UEFI** — `program-entry-plan` leg held by UEFI-OS-HANDOFF at audit time.

## Finding

No implementable gap remains on a linux x86-64 host: every leg of the
route — adapter selection, entry-symbol binding, header field resolution,
final-image replay, and host execution — is wired and exercised end-to-end.
The remaining legs are host-acceptance work (macOS, Windows, QEMU/arm64)
that must report their own platform runs; this record is the linux
x86-64 closure of the end-to-end claim.
