# SAMPLES-COMPILE-MULTI-HOST — z142 verify+record

Board row: TASKS.md:16856 (verified-scope row; duplicate stub at :17126).
Re-verified at `5bb9a74842`.

## Mined-source row

Per-host gate = `compiler`'s `samples_compile` suite
(`all_samples_reach_checked_trees` + per-cohort authored-entry legs +
`samples_with_documented_exit_run_correctly`); "multi-host" means each
required host runs it, tracked per-sample in the cohort READMEs. The row's
recorded red classes at `e092723726`:

1. `windows_x86_64` authored-entry selection rejects the std entry —
   package-owned-binding surface (basics, fletcher_checksum, caesar_cipher,
   format_number legs).
2. `linux_x86_64`/`linux_arm64`/`macos_arm64` "selected ProgramEntry
   establishment rejoins 0 Terminal attachment identities" over a
   call-produced `&[T]` view local — owned by the split row
   SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT (TASKS.md:16994).
3. `cli/basics/print_number` — "cannot prove default-domain field
   requirement for return from Main::main … self.out requires [u8; N]::Utf8".

## State at 5bb9a74842

- Class (2)'s owning leg moved: the slice-view vocabulary landed since the
  row text — SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT records real descriptors
  replacing the nine "borrowed slice view has no Terminal descriptor"
  rejections, and `fletcher_checksum`/`recursive_sum` pass its acceptance on
  linux_x86_64.
- Re-witnessed class (1) at `5bb9a74842`:
  `cargo nextest run -p compiler --test samples_compile
  basics_samples_compile_from_authored_program_entry_bindings` FAILs in 11s —
  `brightness_control` rejects the std windows_x86_64 entry with the identical
  "require either the exact bundled Windows x86-64 contract or one accepted
  package-owned Windows x86-64 binding" diagnostic. Deep legs remain
  ~9min-to-first-assertion per the row; not re-run.
- The row's own residual is host execution: run the suite on
  windows_x86_64 / macos_arm64 / linux_arm64 hosts — none producible on this
  linux x86-64 worker. The named failure classes belong to their owning
  items (package-owned binding family, SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT,
  field-obligation rows).

## Verdict

Verify+record only — the gate is green-able only via per-host runs; no
independent unclaimed slice under this name. Duplicate stub at :17126 folds
here.
