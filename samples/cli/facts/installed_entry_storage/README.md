# Explicitly installed entry storage

Owning task: **EXPLICIT-ENTRY-STORAGE** in [TASKS.md](../../../../TASKS.md).
This row establishes the static-backed exclusive-state and installed-handoff
requirements of [entry roots](../../../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge).

**Fact:** An explicitly installed private `Registers` allocation lends its exact
exclusive placed view, reading status 41 and writing transmit 9 into the same
backing before handback. Every standard place copies that sentence and operation.
The code byte 9 travels through each place before the same placed write consumes
its proved contract. Each twin changes that input to 8 and refuses the unchanged
write contract; no caller guard re-establishes it.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [unproved write](local/invalid/main.omg) |
| field | [program](field/main.omg) | [unproved write](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [unproved write](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [unproved write](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [unproved write](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [unproved write](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [unproved write](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [unproved write](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [unproved write](target_body/invalid/main.omg) |

The row is pinned under `tests/omega/{pass,fail}/facts/` as
`installed_entry_storage_through_<place>_{exit,unproved}`. Every positive binds
all four hosted targets. `entry_ram.txt` explicitly requests initialization for
source occurrence 0; `expected_entry_ram.txt` is the independent authored original
backing oracle. The portable test entrypoints consume those same files:

```sh
omega run --both --keep --entry-ram 0=2900000003000000 main.omg
python3 tools/sample_native_gate.py --filter facts/installed_entry_storage
python3 tools/corpus_gate.py --both --filter installed_entry_storage
```

`entry_ram.txt` rows are `source-position=hex-bytes`, never addresses. The runtime
checks each exact canonical ingress row, scalar codec, complete extent, alignment,
canonical padding, placement, access and no-code cleanup before construction.
Absence provides no storage. This grants neither MMIO nor host pointers, callbacks,
external objects, compound/atomic/erased custody or unsupported qualifications.

Establishment uses those actual typed bytes, not Build selection or a mutable
parameter's shape. The runtime owns distinct writable image residences for the
saved loader continuation, checked activation stack, each original allocation
and each observation header. The callee receives only its declared occurrences.
No ambient mutable global or hidden receiver enters source. Ordinary lifetime,
loan, qualification, termination and cleanup checks remain in force.

After normal return, the runtime retires the entry loans and observes the same
original allocations. The dedicated FD 3 protocol binds the canonical artifact,
placement seal, occurrence and complete byte count; guest stdout/stderr are not
this evidence. A partial write, wrong header, missing/extra frame or absent
normal return cannot pass. The public CLI compares actual native and Terminal
backing independently of their exit agreement. With `--keep`, inspect
`native.entry-ram.0.bin`, `terminal.entry-ram.0.bin`, `native.entry-ram.frames`,
`native.identity.txt`, `program.oterm` and `terminal.effects`; both backing files
must contain `29 00 00 00 09 00 00 00`.

Checked-to-Terminal lowering retains the exact placed loan, scalar read/write
and selected entry identity. Terminal installation constructs its own private
allocation from the checked recipe and observes the original after execution.
Provider settlement, layout geometry and ABI planning retain source/physical
coordinates and each pointer's exact record geometry. Native realization charges
the callee's complete WCSU to a disjoint stack partition. ISA selection and final
Mach-O/ELF emission establish initialization before lending and observation after
return. A separate instruction reader reconstructs the final pointer addresses,
argument registers, exact semantic call, stack switch/restore, full writes and
loader continuation, and the final image header must select that exact adapter.

The [two-occurrence control](two_occurrences/main.omg) additionally checks argument
order and distinct backing: occurrence 0 remains `2900000003000000`, and occurrence
1 changes from `3500000005000000` to `3500000009000000`. The negative controls are:

- [missing backing](controls/missing_backing/main.omg);
- [wrong source occurrence](controls/wrong_occurrence/main.omg);
- [incomplete initialized extent](controls/invalid_extent/main.omg);
- [noncanonical initialized padding](controls/invalid_padding/main.omg);
- [overlapping exclusive root uses](controls/overlapping_exclusive_roots/main.omg).

The initial frozen macOS ARM64 producer at `436add4eac90` refused the valid root
at native admission because it had no matching installed-argument lender. The
repaired macOS bridge executed the root, all nine places and two-occurrence
control with native/Terminal exit 0 and exact original-backing observations;
all twins and storage controls refused their own obligations. Final producer-bound
qualification records retain actual sources, target and engine observations.
Linux x86-64 retains its existing physical lender; actual native observations on
other hosts are unverified. Unsupported native lenders still refuse explicitly.

The companion [free local-state row](../free_entry_local_state/README.md) proves
ordinary local ownership and executable cleanup without supplied parameters.
[Applied zero establishment](../applied_zero_establishment/README.md) retains
invalid initialization controls when construction moves into ordinary locals.
These establish separate obligations of the same parent outcome.
