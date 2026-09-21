# EFI-MATRIX-PROMOTION — record

Re-verified at `94e764a6da` on linux x86-64. The row re-mines the
hosted-matrix clause of `wiki/drafts/rust_compiler_completion.md`: the
required matrix is exactly four hosted rows (linux x86-64, linux aarch64,
macOS aarch64, Windows x86-64) and "freestanding EFI work remains a
separately stated target milestone until it is promoted into this hosted
matrix" (line 21).

## Why no slice exists

- Promotion is a deliberate matrix revision: the doc's own rule requires
  a support-matrix change to revise the finite matrix in the same change,
  and the hosted gates already keep EFI out — there is no silent deficit
  to repair.
- The promotion's precondition legs are the board's own UEFI items:
  UEFI-PHYSICAL-SEMANTIC-ENTRY (source-authored two-surface entry —
  Loaded Image's evaluated `EfiLoadedImageLayout` schema/plan landed in
  `targets/uefi_x86_64/tables.omg`; System Table and Boot Services
  catalogs remain duplicate production definitions) and UEFI-OS-HANDOFF
  (the handoff that makes an EFI host row possible at all). Both remain
  open, so there is no EFI row to promote.
- The promotion decision itself is a milestone statement owned by the
  completion doc, not a lane task.

Conclusion: item is an authorization gate, not implementable work. The
correct state is "open — waits on UEFI legs"; no code or matrix change is
warranted while they stand.
