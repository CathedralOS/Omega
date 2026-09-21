# BENCHMARK-SELECTION-CONTRAST-ROWS — z70 re-verification

**Revision:** `7241e02227` (origin/main tip, linux x86_64 host).
**Verdict:** adjudication stands — covered, no independent slice. The row's
"every slice is claimed elsewhere" reading is still accurate at this tip.

## What the name asks for

Contrast rows = same subject, same target, *different* exact rule selections —
records keyed `subject__target__sel-<hash>` beside the `default` row.

## Re-verified at 7241e02227

- `tools/benchmark/records/` census is now **10 committed rows** (was six when
  the row was stamped). Both contrast rows this stub names remain on disk:
  `wrapping_square_sum__linux_x86_64__sel-885944b13b84` (CopyPropagation
  disabled, `7ec604d7ef`) and `wrapping_square_sum__linux_x86_64__sel-9c09e32a82fb`
  (`0969a3ea96a`), each beside `wrapping_square_sum__linux_x86_64__default` —
  selection-keyed contrast coverage is landed exactly as the sibling rows
  recorded.
- New since the row's stamp: `structural_proofs__linux_x86_64__default` subject
  row and `macos_x86_64` + `uefi_x86_64` cross-compile rows (runtime legs
  `skipped` per contract).
- `python3 tools/tests/test_benchmark.py` → **29/29 green** (was 21/21 at the
  earlier verifications — the matrix pins grew with the census).

## Fence state (why no new slice)

Producing a *new* contrast row writes `tools/benchmark/records/` and the
`wiki/drafts/benchmarks.md` matrix — both fenced this wave to
BENCHMARK-PROOF-SUBJECT-SELECTION (exp 14:19Z 2026-09-21). The remaining gap on
the cluster stays with its named owners: BENCHMARK-LINUX-X64-ROW-REFRESH
(re-measured default row at a newer revision) and the host-gated legs
(`linux_arm64` runtime needs arm64, `macos_*`/`windows_*` need their hosts,
`uefi_x86_64` needs QEMU/hardware). Selecting another `sel-` variant is
production work on the fenced surfaces, not an independent slice under this
name.
