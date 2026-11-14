# Checked fence contracts

Owning tasks: **BUILD-OWNED-TARGET-SELECTION** for explicit target selection, and **CHECKED-ASM-LABEL-CONTRACTS** for checked fence contracts and execution.

Checked fence sequences retain their catalog target, ordering and exact empty clobber set while carrying input 37 to a result requiring 37.

The x86 bodies execute LFENCE, SFENCE, MFENCE in that order. The AArch64
bodies execute full-system DMB LD, ST, SY in that order. These retain distinct
catalog identities and access masks; DMB LD also orders later stores and is
not equated with LFENCE. All four hosted targets bind `ProgramEntry`.

The root and nine places call a consumer requiring exactly 37, before their
runtime equality oracle. Each `invalid/` changes only the declared clobbers:
rax on x86, x0 on AArch64. Both are invented changes for this catalog sequence,
so every twin must reject for the exact clobber-union obligation. The sequence,
bound, scalar guarantees and consuming operation are otherwise byte-identical.
Pins are `tests/omega/{pass,fail}/facts/checked_asm_fence_contracts_through_*`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [source](main.omg) | [invented clobber](invalid/main.omg) |
| local | [source](local/main.omg) | [invented clobber](invalid_local/main.omg) |
| field | [source](field/main.omg) | [invented clobber](invalid_field/main.omg) |
| parameter | [source](parameter/main.omg) | [invented clobber](invalid_parameter/main.omg) |
| call_result | [source](call_result/main.omg) | [invented clobber](invalid_call_result/main.omg) |
| match_join | [source](match_join/main.omg) | [invented clobber](invalid_match_join/main.omg) |
| const_name | [source](const_name/main.omg) | [invented clobber](invalid_const_name/main.omg) |
| other_state | [source](other_state/main.omg) | [invented clobber](invalid_other_state/main.omg) |
| other_machine | [source](other_machine/main.omg) | [invented clobber](invalid_other_machine/main.omg) |
| target_body | [source](target_body/main.omg) | [invented clobber](invalid_target_body/main.omg) |


- Establishment: the caller proves input == 37; entry and exit assertions are
  goals. Each catalog operation preserves that exact value and changes no
  general register or flag. Its ordering event remains a separate obligation.
- Identity, lifetime and composition: each place carries the same scalar
  subject through ordinary fields, parameters, returns, joins, constants,
  nested machines, non-nested machines and selected target bodies. Helpers
  retain the ordering events; a join executes only its selected branch.
- Terminal realization: exact target and fence kind cross the canonical wire.
  Execution requires the receiving host's explicit private sequential memory
  model, records each completed fence in order, and retains progress across
  work-budget pauses. Missing supply and mismatched targets must reject before effects.
  This model supplies neither shared/device memory nor synchronization proofs.
- Native realization: each emitted sequence passes the compiler's independent
  checks of instruction bytes, stack use and changed registers. This row
  observes actual execution on both Linux architectures. Wider catalog controls
  for missing, doubled, truncated, substituted or wrong-target bytes remain
  part of the parent acceptance; they are not all replayed by this milestone.

The older producer `3020fc62f3be` recorded Linux x86-64 execution and additional
Terminal target/fence checks. Those results are historical. They do not qualify
the current source or show native execution on another host.

The parent remains open for the other catalog families, hardware-effect linear
retirement, consumer-owned PortIo settlement, supplied device/interrupt/idle
models, external entry and customer canaries. Each host must be measured separately; foreign target interpretation and emitted images do not establish foreign
native execution, hardware timing, device completion or multi-core ordering.

The runtime model covers private entry memory constructed by the hosted runtime
and storage owned by this invocation. Unsupported placed-memory inputs and host
implementations refuse. Native helper execution still depends on its existing
admitted provider contract; adapters do not accept reference, atomic or device
values, reference results, callbacks or additional service access. Passing these
examples does not establish concurrent-provider behavior or ordering for external
memory. The governing instruction and environment rules are
[checked assembly](../../../../wiki/spec/language/assembly.md).

Additional local controls pin false exit goals, an invented clobber in an
unselected ARM body, mixed and wrong-body ISA demands, unsupported DMB scopes,
missing/numeric/extra DMB operands, and an operand on LFENCE. Malformed operands and false contracts refuse during source checking. Two source-valid controls instead select an instruction on an incompatible target and must refuse native publication. Neither kind of refusal proves hardware ordering or ownership.

## Current target-body migration

The target-body example now uses ordinary `X86Ordered` and `ArmOrdered` implementations of `SelectedOrdered`. Four typed Build rows choose the appropriate implementation, and one free `forward` helper retains the original input and result contracts. The x86 and AArch64 instructions remain distinct. All 31 projects and their pins now use direct inline assembly contracts, preserving each assertion and instruction; no colon-entry label or source target prefix remains in this row.

The match example calls both branches and checks independently initialized path markers. Both calls still produce 37 and the same six fence events. Its twin changes only the declared clobbers, just like the other nine twins.

Two controls now distinguish source validity from target compatibility. An ordinary instruction body is valid source even when one Build row selects it for the wrong architecture. Those controls use target-specific native-publication refusal files and exit-zero promises only for their successful profiles. They are program-role samples and build-only pass pins; the other 19 controls remain source-invalid. No dummy memory input or attempted execution supplies a publication refusal.

The original baseline at `da4770bf800fd60355e7f815dd151b02f6f3d0ca`, based on
published `7fc524eaec1a56fcd1b4e3809355f047b7d2d664`, failed
[run 41679589443](https://github.com/CathedralOS/Omega/actions/runs/41679589443)
before either engine executed. The direct-contract migration at
`0f7b5a2f97908e0ef15a15009466d4e82ca67244` reproduced the same return-lowering
failure on both Linux architectures. This second measurement establishes that
the valid migrated program still needed a compiler repair.

The compiler stopped looking for the return when it met an assembly marker.
It now uses the same completion boundary for graph planning and scalar return
discovery, keeping the existing value, result type and exit checks.

The independent fence observer also needed the same provider choices and actual
per-target entry as ordinary execution. The portable artifact has one default
entry; it cannot stand in for distinct Build-selected entries on every target.
The pins request the existing compiled-application cohort, which retains one
shared artifact and each target's checked entry. All missing/wrong-model,
exact-event, work-budget-pause and unchanged-artifact checks remain in force.

Fresh complete observations use source
`d4c693b8b0f429a49b3f2d77a331bad656d8cda4`, including main's updated calling and
wire formats. Both [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41705562039)
and [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41705566829)
pass the full selected scope: twelve source-valid programs, nineteen intended
source refusals, eleven actual native/Terminal exit-zero agreements and one
incompatible-ISA publication refusal. All 31 ordinary projects meet their
runtime or rejection oracle. The two source-valid ISA controls have ordinary
execution headers on compatible profiles and independent source-check plus
publication-refusal evidence on incompatible profiles.

All ten standard positives retain the exact fence streams on all four Terminal
target installations, missing/wrong-model refusals, work-budget-pause progress and
unchanged artifact identity. Every current record uses this measured source;
final README/record-only commits are not claimed executed. Earlier checkpoints,
including their failed full jobs and later header corrections, remain tied to
their original source identities in the retained qualification and
[PR #1725](https://github.com/CathedralOS/Omega/pull/1725) source archives.

These are Linux native observations and four-target Terminal observations.
Windows and macOS native execution, malformed-byte catalog controls beyond this
scope, and the broader parent acceptance remain open. The supplied private
sequential model establishes no shared/device-memory or hardware-timing claim.
