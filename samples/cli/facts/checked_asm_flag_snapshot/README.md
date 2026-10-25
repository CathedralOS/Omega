# Checked assembly flag snapshots

`sub eax, 1` after `mov eax, 1` establishes ZF; structured `pushfq snapshot`
carries bit 6 into the incoming obligation of the `observed` label. The runtime
oracle checks that bit and exits 0. The root witness remains byte-for-byte
unchanged from its frozen failing baseline.

Owning requirement: **CHECKED-ASM-LABEL-CONTRACTS**. The governing
[checked assembly contract](../../../../wiki/spec/language/assembly.md) requires
exact edge proofs, catalog clobbers, body-local scratch versions and executable
instruction semantics. [Numeric values](../../../../wiki/spec/language/numeric_values.md)
own the fixed-carrier bit operations; [structural access](../../../../wiki/spec/terminal-psi/structural_access.md)
owns exact field observations and write invalidation;
[storage](../../../../wiki/spec/resources/storage.md) owns the native temporary
stack footprint.

## Transport places and mechanism witnesses

The nine standard places (`local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) and
their `<place>/invalid/` reject twins accompany the mechanism variations below.

| Witness | Obligation or runtime distinction |
| --- | --- |
| [Root](main.omg) | Subtraction establishes the snapshot bit used by a label requirement. |
| [Local](local/main.omg) / [invalid](local/invalid/main.omg) | A local copy of the snapshot retains bit 64 into a subsequent label requirement; flipping bit 64 rejects. |
| [Field](field/main.omg) / [invalid](field/invalid/main.omg) | A completed store establishes the bit on the exact mutable record field consumed by the label. |
| [Parameter](parameter/main.omg) / [invalid](parameter/invalid/main.omg) | A completed snapshot supplies a callee precondition and its label requirement. |
| [Call result](call_result/main.omg) / [invalid](call_result/invalid/main.omg) | A helper call returning the snapshot with `ensures (result & 64u64) == 64u64` carries that guarantee into the caller's label requirement. |
| [Match join](match_join/main.omg) / [invalid](match_join/invalid/main.omg) | Exhaustive match arms producing distinct `pushfq` snapshots that both establish bit 64 discharge the post-join label requirement; breaking one arm rejects. |
| [Const name](const_name/main.omg) / [invalid](const_name/invalid/main.omg) | Named `const` mask `ZF_MASK = 64u64` preserves the snapshot bit obligation; wrong mask rejects. |
| [Nested machine](other_state/main.omg) / [invalid](other_state/invalid/main.omg) | State transition carries the snapshot obligation into a nested machine's label requirement. |
| [Other machine](other_machine/main.omg) / [invalid](other_machine/invalid/main.omg) | Cross-machine forwarding preserves the snapshot precondition and label requirement. |
| [Target body](target_body/main.omg) / [invalid](target_body/invalid/main.omg) | Target-qualified `target (windows_x86_64)` / `target (linux_x86_64)` helper bodies preserve the snapshot guarantee across host x86-64 targets. |
| [Retained snapshots](retained/main.omg) | An earlier snapshot retains ZF after a later subtraction establishes a different flag version. |
| [Aliased operands](alias/main.omg) | `sub eax, eax` captures both operands before writing; the six-flag image is 68 under mask 2261. |
| [Dynamic operand](dynamic_input/main.omg) | An opaque observed carry supplies a runtime operand; subtraction checks the complete flag mask against the independent 0-minus-1 or 1-minus-1 hardware result. |
| [Arithmetic boundaries](hardware_calls/main.omg) | Independent checked bodies cover borrow, signed overflow, sign, auxiliary carry and low-byte parity boundaries; every body proves the complete six-flag mask and checks its result. |
| [Observation before production](pre_sub/main.omg) | The initial snapshot retains architectural fixed bits; later subtraction replaces the arithmetic flags. |
| [Unreachable observation](unreachable/main.omg) | A skipped `pushfq` leaves its destination unchanged. |
| [First-label backedge](backedge/main.omg) | A two-iteration loop re-enters the authored first label and observes the prior iteration's flag version. |

The x86 instruction fixtures bind Linux x86-64 explicitly so the checked corpus
selects an applicable target. The sample Build files bind only Linux and
Windows x86-64 ProgramEntry: an ARM entry binding would demand an instruction
catalog the realized target cannot supply, so these programs are not
host-runnable on ARM machines.

Each positive is pinned under `tests/omega/pass/facts/checked_asm_flag_snapshot*`.
The [root invalid twin](invalid/main.omg) changes only the required bit from
64 to 0. The [parameter twin](parameter/invalid/main.omg) changes only the
actual's bit; the [field twin](field/invalid/main.omg) changes only its label
requirement. Separate fail fixtures preserve rejection after a direct field
overwrite, a scalar-leaf alias write and a whole-record reborrow write. An
opaque environmental snapshot without arithmetic production cannot establish
the bit. Omitted `rflags` and omitted `r10` controls each remove one clobber
without changing the instructions or values. General bitwise/shift/cast proof
formation controls live under `tests/omega/{pass,fail}/arithmetic/checked_flag_proof_*`.

## End-to-end contract

The instruction model establishes all six u32 subtraction flags and snapshots
them as immutable scalar values; the label consumes those values directly,
without a caller guard supplying its proof. Canonical field roots, complete
declared field paths and carriers preserve identity; overlapping writes and
alias-closed call frames retire current field associations. Calls and loop
edges retain ordinary preconditions, ownership and termination obligations.

Checked operations lower through the existing Terminal register-event and
scalar/control paths. A raw RFLAGS observation has no invented service demand,
but interpretation requires an explicitly installed x86 flags environment.
Missing supply and wrong-target supply remain refusals. Native realization
uses the exact balanced `pushfq; pop r10; mov r15, r10; mov destination, r15`
envelope and retains its clobbers, eight-byte temporary stack demand and
register preservation through normal allocation and image emission. Existing
provider, ABI, frame, branch and artifact validation remain applicable to the
realized functions; this witness introduces no memory authority or linear
retirement rule.

## Development evidence

```text
omega run --both samples/cli/facts/checked_asm_flag_snapshot/main.omg
omega --check samples/cli/facts/checked_asm_flag_snapshot/invalid/main.omg
```

Frozen baseline: revision `53cd2938437a29356a48d6eaf29f14de07a4bb99`, optimized
level-3 producer SHA-256
`d4b532f665183e9c9630d5247f900f7231b7ea2f3e7a36afbb28527df65a00f5`.
The unchanged root rejected with “scratch-register CFG flags cannot yet be
combined with hardware pushfq/popfq snapshots or restoration.” A raw snapshot
also exposed missing executable lowering, and an ordinary masked OR/AND
observation exposed missing proof precision.

The frozen build-12 development producer has SHA-256
`b69bc7ed0782549a44a88dbd7244de91b2ea8f2a5efccf5c28e928d83b41bf74`
and optimized level 3. On Linux x86-64, the unchanged root completed both
Terminal and native legs with exit 0 in 7.204 seconds. Eight additional mechanism probes completed both legs with exit 0
(7.106–13.045 seconds). The dynamic-input probe also completed both legs
with exit 0; its separate duration was not captured.
Root, parameter and field false controls, field overwrite/alias-write and
opaque-observation controls rejected for their authored proof obligations.
The additional whole-record reborrow rejected the retired field fact in
6.434 seconds; omitting `r10` rejected the exact clobber mismatch in 0.016
seconds. Actual additional host: Linux 6.18.55 x86-64, glibc 2.41.

Invocation-local commands, exact source hashes, producer/library identities
and raw outputs are retained under `build/slot7/producer-build12/`, with the
additional observations in `additional-host/`. The fixture pin map records
where an Expected-exit header was added to a probe copy; those source copies
are inputs to the exact-candidate gates. Final qualification is recorded
separately in `build/slot7/qualification.json`; the observations above remain
development evidence.

The former rejecting `asm/scratch_hardware_flag_mix` source is unchanged under
`tests/omega/pass/asm/`. Its guard deliberately takes `Trap`: the development
run observed native signal 4 and Terminal typed `Trap` agreement, with CLI
exit 255. Its corpus oracle is `trap`, not an ordinary numeric success.

This instruction cohort applies to x86 target bodies. The Build entries bind
x86-64 targets only; only Linux x86-64 executed here.
Windows x86-64 execution and other hosts remain unmeasured. Flag restoration,
other ISA/register views, indexed-memory composition, ranked termination and
hardware-effect retirement of exact linear obligations remain broader parent
scope. This mechanism does not close the entire checked-assembly requirement.

## Per-function temporary storage

[Mixed functions](stack_functions/main.omg) places an ordinary return, a
structured PUSHFQ snapshot and directed floating-point division in one object.
Its [invalid twin](stack_functions/invalid/main.omg) omits only the snapshot's
required r15 clobber. Both are pinned under `tests/omega/{pass,fail}/facts`.

Selected instruction IDs restart in each function. Temporary storage joins the
complete selected function/block/instruction roster, including terminators, to
the ordered encoding rows with `(MachineId, SelectedInstructionId)` identity.
An unrelated function's PUSHFQ or directed-FP envelope contributes no local
demand. The accounting regressions pin ordinary/PUSHFQ/directed-FP peaks at
0/8/16 bytes in both function orders; exact stack replay rejects understated or
inflated peaks and adds the full live frame. This is physical storage scoping
for the existing snapshot mechanism, not another transport-place row.
