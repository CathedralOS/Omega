# Record: unit_085 (area 17-omega-source: `source/library/core` and `source/library/std`)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`, detached at worker-2. Read-only pass; no program was built or run.
Unit rows: 113 files, 21,976 lines (core 65 files / 11,981 lines; std 48 files / 9,995 lines). Every line of every range was read.
Paths below are relative to `source/library/`. "Proof machine" means a machine whose value is never run; only its `requires`/`ensures` matter.
Correction to the crate map: `core/native_execution_authority.omg` is one line (a trait, `:1`), not empty.

Citation words used: "read" = I read the lines; "grepped" = a Grep with the stated pattern; "inferred" = concluded from reading, not traced.

---------------------------------------------------------------------------------------------------------------------

# PART A: `core/` (65 files)

## A1. Layout, access, resource and extent vocabulary

### core/layout.omg (599)
1. Purpose: declares the data a layout policy reads and returns (`Schema`, `LayoutPlan`, `FieldPlan`), the access-policy vocabulary (`AccessPlan`, `ResourceProfile`, `AccessLifecycle`), and the traits policies satisfy (`FixedLayout`, `AccessPolicy`).
2. Shape: `SchemaField` (`:29`; fields key, size, align, identity `Optional<u64>`, `number: i64`, kind, layout_source), `FieldLayoutSource` (`:45`), `FieldKind` (`:57`), `SchemaCase` (`:67`), `Schema` (`:76`, 32 fields, 32 cases), `FieldPlan` (`:107`: At, IntegerAt, Bits, Varint, LengthPrefixed), `LayoutPlan` (`:139`, 64 entries), `AccessPlan` (`:313`, 32 fields/readers/writers), `ResourceProfile` (`:404`, 32 regions), `AccessLifecycle` (`:456`), `NativeAccess` (`:484`, the only machine bodies: `initialize :485`, `move_out :501`), `IssuedBy` (`:327`), `IssuedByAny` (`:333`), `layout_plan_place_private` (`:166`), `layout_plan_place_private_field` (`:174`), `Accessor<T>` (`:547`), `LayoutTarget` (`:563`), `LayoutStorage` (`:584`), `FixedLayout` (`:586`), `AccessPolicy` (`:597`).
3. In/out: reads nothing at run time; `NativeAccess` calls `ResourceProfileSupply::<Role>::profile` then `|access|::initialize_from_profile` / `extract_resident` (`:495-498`, `:507-510`), which live in `core/access/` (not in my unit).
4. Decides: array bounds are `requires` clauses (`:67`, `:76-77`, `:285`, `:314`, `:357`, `:376`, `:404`); `SchemaField requires align >= 1, align <= 16` (`:29`). Nothing here selects at run time.
5. Cases by shape: fixed capacities everywhere: 32 schema fields/cases (`:76-77`), 64 plan entries (`:140`), 16 transfer rules (`:357`, `:376`). "At most 32 fields in the pilot slice (plan validation rejects more)" (`:52-53`).
6. Same job elsewhere: `layout_plan_place_private` and `_field` are identity functions by design (`:166-176`); the compiler records the receipt. Two copies of the `use ...access` import (`:1`, `:181`).
7. Unused: `IssuedBy` / `IssuedByAny` I did not grep; `layout_plan_place_private` (grepped name over `source-rust/` and `tools/`: not run; mark "suspected" used by the compiler for the sealed identity).
8. Stated limits: "Concrete proved-fit mutation remains fail-closed." (`:98-99`); "Tagged/wire placements remain consumers of the wire-plan path today." (`:99-100`); "Transitional compatibility mirror; -1 means unnumbered." (`:34-35`).
9. Fail-open: none seen. `AccessCallerRule`/`Issuance` default ("first case is the ordinary zero value ... permits nobody", `:290-291`) fails closed (stated).

### core/c_layout.omg (109)
1. Purpose: a layout policy `CAbiPolicy<T>` that places a record's scalar fields the way a C compiler would (`:7`).
2. Shape: `CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>` (`:5`); `plan` (`:8`); `c_abi_unsupported` (`:20`); `c_abi_padding` (`:25`); `c_abi_fields` (`:36`) with nested `place :44`, `represented :59`, `checked :69`, `extent :78`, `append :87`, `finish :95`, `complete :104`.
3. In/out: reads `Schema` (`fields[i].align/size/key/identity/layout_source`), writes a `LayoutPlan` with `FieldPlan::At{offset}` per field (`:91`), `size_fixed` and `align` (`:105-106`).
4. Decides on: `schema.case_count == 0 && schema.field_count > 0` (`:11`); `identity in Optional::None` (`:52`: numbered fields refused); `layout_source` Scalar or Explicit (`:63-67`: Default refused); `u64` overflow guards (`:73`, `:82`, `:99`). Refusal is `entry_count = 65` (`:21`), which "The ordinary receiving validator rejects" (`:18-19`).
5. Cases by shape: at most 32 fields (`index < 32`, `:40`), alignment 1 to 16 (`:27`, `:38`), 33 steps (`:12`); "Numbered schemas ... require an explicit policy instead of guessed C order" (`:57-58`).
6. Same job elsewhere: `StatLayout` (`std/targets/linux_x86_64/filesystem_impl.omg:24`) and `LinuxArm64StatLayout` (`std/targets/linux_arm64/filesystem_impl.omg:51`) hard-code offsets instead of using a policy; `DarwinStat` uses `CLayout` (`std/macos_arm64_filesystem_abi.omg:33`).
7. Unused: `CLayout` grepped only by name in std (3 files read: `macos_arm64_filesystem_abi.omg:5,20-23,33,37`).
8. Stated limits: comment at `:1-3` "nested fields require an explicitly selected child representation".
9. Fail-open: none seen. A schema with more than 32 fields would produce a 32-entry plan (`:40`/`:42`), but `Schema requires field_count <= 32` (`layout.omg:76`) means that input cannot be built (traced to the `requires`).

### core/matrix_layout.omg (181)
1. Purpose: rank-two strided index arithmetic over a caller slice, with checks for bounds and (for mutation) non-aliasing.
2. Shape: `Layout2` (`:20`), `IndexResult` (`:24`), `ReadResult` (`:25`, `i32` only), `try_offset` (`:27`), `valid_readonly` (`:73`), `greatest_common_divisor` (`:86`, `terminates by right -> Nat::Descending`), `valid_mutable` (`:95`), `transpose` (`:133`), `read_checked` (`:138`), `try_read` (`:143`), `try_write` (`:164`).
3. In/out: reads `Layout2` and `backing_length`; returns `IndexResult`/`ReadResult`/`bool`; `try_write` writes `backing[index]` (`:178`).
4. Decides on values: bounds by dividing capacity before multiplying (`:41`, `:61`); mutable layouts "fit iff rows exceeds the first magnitude and columns the second" via gcd (`:8-11`, `:120-129`); zero stride with count > 1 refuses (`:108`).
5. Cases by shape: only `i32` element type (`:25`, `:138`, `:143`, `:164`).
6. Same job elsewhere: none seen in unit.
7. Unused: grepped `Layout2|valid_mutable|matrix_layout` over the checkout (excluding `.git` and the atlas): 47 occurrences in 4 files: this file (23) and three `samples/cli/scientific/w100/nf01_shape_layout_contract/*/main.omg`. No use in `source-rust/` or `tools/`.
8. Stated limits: none.
9. Fail-open: none seen. `try_offset` returns `Invalid` on any guard failure (`:28-31`, `:44`, `:64`).

### core/extent.omg (300)
1. Purpose: the linear `Extent` record (base address, length) and the proof domains that say what a given extent is allowed to be used for (`Granted`, `Vacant`, `Resident<P,T>`).
2. Shape: `Extent` (`:15`), domains `Vacant` (`:24`), `Resident` (`:28`), `Granted` (`:235`); traits `VacantContentTransfer` (`:35`), `ResidentContentTransfer` (`:214`), `ProgramStorageEntry` (`:251`), `ExtentRootProvider` (`:289`); machines `Vacant::content` (`:47`), `split_vacant_extent` (`:66`), `merge_vacant_extents` (`:105`, four nested helpers `:128-188`), `Granted::content` (`:275`); `no_wrap` (`:228`); `ExtentSlot` (`:297`); native provider `ProgramStorageEntryNativeProvider` (`:262`).
3. In/out: proof machines return `IntervalSet<Nat>` or `Extent` values built from fields; no external reads.
4. Decides: `merge_vacant_extents` branches on `left.length == 0` / `right.length == 0` (`:123`, `:163`); `split_vacant_extent` chooses `rest_base` on `requested < length` (`:89-92`). `Granted` is established only by named provider routes (`:239-242`), not by structure.
5. Cases by shape: empty-interval special cases (`:73`, `:87-88`, `:99-102`).
6. Same job elsewhere: `Vacant::content` (`:47`) and `Granted::content` (`:275`) have identical bodies.
7. Unused: `ExtentSlot` (`:297`, not public); not grepped.
8. Stated limits: "an exhausted range has no one-past address" (`:87`).
9. Fail-open: none seen; domain evidence is issuer-restricted (stated `:20-22`).

### core/run.omg (141)
1. Purpose: `Run<P,T>`, a linear run of `count` cells over a granted extent, with routes to make an empty run, initialize or extract a single cell, and relocate a run.
2. Shape: `Run` (`:5`), domain `Live` (`:10`), `Singleton` (`:43`), `Run::empty` (`:15`), `release_empty` (`:28`), bodyless `retire_empty` (`:36`), `initialize_cell` (`:46`), `extract_cell` (`:66`), bodyless `initialize_from_profile` (`:82`), `extract_from_profile` (`:90`), `relocate` (`:102`), `relocate_from_profiles` (`:134`), `RunRelocation` (`:97`).
3. In/out: wrappers call `ResourceProfileSupply::<Role>::profile` and pass `@profile` to the bodyless primitives.
4. Decides: contracts only: `requires storage.length == 0` (`:17`), `run.count == 1` (`:68`, `:93`); `ensures match result {...}` (`:55-58`, `:71-73`, `:111-120`).
5. Cases by shape: single-cell and empty runs only; relocation keeps count (`:113`).
6. Same job elsewhere: `AccessLifecycle::initialize/move_out` (`layout.omg:456-472`) cover the same transfer for a whole extent.
7. Unused: none grepped.
8. Stated limits: "The compiler realizes these exact declarations." (`:79`).
9. Fail-open: the three `boundary machine` declarations without bodies (`:36`, `:82`, `:90`, `:134`) accept whatever the compiler provider does; stated at `:79-81` ("the primitive must bind that completed call ... before publishing"). Classification: stated.

### core/device.omg (168)
1. Purpose: linear proxy types (`DeviceLoan`, `DeviceOrderingEvent`), domains for DMA direction and ordering roles, and the `DeviceLoanProvider` boundary trait.
2. Shape: `DeviceLoan` (`:44`), domains `DeviceReads :46`, `DeviceWrites :49`, `Publication :59`, `CacheMaintenance :62`, `Notification :65`, `Completion :72`; `DeviceCompletionAttempt` (`:77`), `DeviceAcquisitionAttempt` (`:84`), `DeviceLoanProvider` (`:94`: reserve, release, lend_for_read, lend_for_write, publish, maintain_cache, notify, settle, complete, acquire).
3. In/out: none at run time (all rows are bodyless boundary requirements).
4. Decides: domains `established by` named provider rows (`:46-67`).
5. Cases by shape: none.
6. Same job elsewhere: none in unit.
7. Unused: not grepped.
8. Stated limits: "A hosted entry supplies no arrival extents; this is the only `Granted` source a hosted program reaches today" (`:97-99`).
9. Fail-open: none seen.

### core/content.omg (39), core/drop.omg (15), core/equality.omg (5), core/never.omg (3)
- content.omg: declares `IntervalSet<CoordinateSpace>` (`:12`), `CountedQuantity<Unit>` (`:17`) and trait `Content<Owner,A>::project` (`:21`); the rest is comments saying `old`, `separate`, `embed` are compiler proof terms with no declaration (`:25-39`). No run-time behavior. Stated: "It has no machine declaration, provider, or runtime implementation." (`:38-39`).
- drop.omg: `drop<T>(value: T)` with an empty body (`:14`); the checked cleanup edge does the work (`:11-12`). One caller count not grepped.
- equality.omg: trait `Equatable<T>::equals` (`:3`); comment says only this declaration permits structural equality synthesis (`:1`).
- never.omg: `data Never requires false` (`:2`), uninhabited.
Fail-open: none seen in these four.

### core/ptr.omg (25), core/fence_ordering.omg (5), core/code_layout.omg (11), core/program_image.omg (14)
- ptr.omg: opaque `boundary data Ptr<T>` and `PtrRange<T>` (`:16-17`), trait `PointerArithmetic` (`:21`: `offset`, `range`).
- fence_ordering.omg: `FenceOrdering [copy]` with Receive, Publish, ReceivePublish (`:1`).
- code_layout.omg: `CodeRegion` (`:3`) and `CodeLayout<'code>` (`:8`) plain copy records.
- program_image.omg: trait `ProgramImage::retained_suffix_start` (`:6`) and `ProgramImageNativeProvider` (`:10`, a boundary machine satisfying it).
No decisions; no fail-open.

## A2. Hardware control, interrupts, tasks, process, authorities

### core/code_segment.omg (38), core/data_segment.omg (49), core/task_register.omg (46)
1. Purpose: x86 control surfaces for reloading CS, reloading data segments, and loading the task register, each as a boundary trait with an `X86...Control` implementation calling a compiler-realized `boundary machine`.
2. Shape: `CodeSegmentControl` (`code_segment.omg:12`), `X86CodeSegmentControl` (`:22`), `reload_code_segment` (`:35`), `read_code_selector` (`:38`); `DataSelectors` (`data_segment.omg:12`), `DataSegmentControl` (`:14`), `X86DataSegmentControl` (`:24`), `reload_data_segments` (`:42`), five `read_*_selector` (`:45-49`); `TaskRegisterControl` (`task_register.omg:9`), `X86TaskRegisterControl` (`:21`), `activate_task_register` (`:40`), `read_task_register` (`:46`).
3. In/out: take `@mutable [u64; Entries]` GDT and a selector; write a 10-byte scratch (`let mutable observed: [u8; 10]`, e.g. `code_segment.omg:26`); return `bool` (`== 1` on the u8).
4. Decides on value: the compiler-side primitive returns 1 or not; the wrapper compares `== 1` (`code_segment.omg:27`, `data_segment.omg:29`, `task_register.omg:29`).
5. Cases by shape: `Entries >= 1 && Entries <= 8192` (`code_segment.omg:14`, `data_segment.omg:16`), `>= 2` for the task register (`task_register.omg:13`); task state is `[u8; 104]` (`:11`).
6. Same job elsewhere: three near-identical wrappers (same scratch, same `== 1`).
7. Unused: not grepped.
8. Stated limits: "Build selects the implementation. This implementation requires x86-64." (`code_segment.omg:21`, `data_segment.omg:23`, `task_register.omg:20`). The admitted scope (CPL0, interrupts masked, no CET) is prose only (`code_segment.omg:3-8`).
9. Fail-open: none seen; false means "rejected check, state unchanged" (stated `code_segment.omg:9`).

### core/interrupt.omg (66), core/static_interrupt.omg (154)
1. Purpose: linear obligations for interrupt masking and acknowledgement, and the x86 static-interrupt installation surface.
2. Shape: `InterruptMaskGuard`, `InterruptAcknowledgement` (`interrupt.omg:20-21`), domains `Active` (`:23`), `Pending` (`:43`), traits `InterruptMaskControl` (`:28`), `InterruptEntry` (`:46`), `InterruptMaskRestore` (`:52`), `InterruptAcknowledgementCompletion` (`:60`). In static_interrupt: carriers (`:8`, `:121`), `StaticInterruptTableValidation` (`:25`), `StaticInterruptWork` (`:35`), `StaticInterruptStack` (`:44`), `StaticInterruptFrame` (`:52`), `StaticInterruptArrival` (`:59`), `X86StaticInterruptArrival` (`:68`), `X86StaticInterruptScope::run` (`:84`, bodyless boundary), `view_static_interrupt_context` (`:111`), `begin_static_interrupt_completion` (`:129`), `finish_static_interrupt_completion` (`:137`), `complete_installed_interrupt` (`:143`, the only body: begin, call `Completion`, finish, `:150-153`).
3. In/out: `complete_installed_interrupt` consumes the pending acknowledgement.
4. Decides: `run` has `ensures result == false` (`:103`); `requires GdtEntries >= 2 && GdtEntries <= 8192` (`:98`).
5. Cases by shape: `[u8; 104]` task state (`:93`).
6. Same job elsewhere: none.
7. Unused: not grepped.
8. Stated limits: validation "permits no crash route and makes no termination promise" (`:23-24`).
9. Fail-open: `begin/finish` completion primitives are bodyless (`:129`, `:137`); stated: "A failed or nonreturning controller body cannot authorize IRET." (`:141-142`).

### core/task.omg (72), core/wait_wake.omg (61), core/process_exit.omg (20), core/process_exit_native.omg (7)
1. Purpose: vocabularies: `Task<T>` linear claim and outcomes (`task.omg:11-41`), `TaskLifecycle` (`:47`), `TaskRuntime` (`:58`); `WaitOutcome` and `WaitWake` (`wait_wake.omg:31`, `:40`); `ProcessExit` trait (`process_exit.omg:14`) and `ProcessExitPrimitive` (`process_exit_native.omg:3`).
2. Shape/decisions: only contracts. `TaskRuntime::start/try_start` carry `ensures true` (`task.omg:63`, `:70`) next to `requires machine Target ... suspends blocks`.
3. Cases by shape: `wake_one` returns 0 or 1 (`wait_wake.omg:50-52`); `exit_process` returns `Never` with `crashes none terminates` (`process_exit.omg:15-19`).
4. Same job elsewhere: `NativeProcessExit` in `std/process_exit.omg:16` wraps `ProcessExitPrimitive`.
5. Stated limits: "provider admission/dispatch remains the next runtime layer" (`task.omg:56-57`); the integer fields are "the bootstrap carrier" (`:8`).
6. Fail-open: `ensures true` (`task.omg:63`, `:71`) is vacuous but states nothing; none accepted by it. No other.

### core/filesystem_authority.omg (9), core/presentation_authority.omg (5), core/process_authority.omg (5), core/time_authority.omg (2), core/native_execution_authority.omg (1)
Empty boundary traits used as reach names: six filesystem names (`filesystem_authority.omg:4-9`), five presentation names (`presentation_authority.omg:1-5`), `StandardInput`/`StandardOutput` (`process_authority.omg:2`, `:5`), `ClockObservation`/`ExecutionDelay` (`time_authority.omg:1-2`), `NativeCodeExecution` (`native_execution_authority.omg:1`). No decisions. Their only consumers in my unit are the reach lists in `std/` (e.g. `std/macos_gui.omg:42-47`).

### core/defaults/build.omg (2), core/main.omg (1)
`defaults/build.omg:2` is an empty `machine build(builder: @mutable Build) {}`; `main.omg:1` is one comment line.

## A3. Floating point

### core/float_operations.omg (1409)
1. Purpose: the public IEEE operation surface: operator tokens bound to named `Float::*` boundary machines whose contracts equate the result's `FloatMeaning` with the proof-level `FloatSemantics` result.
2. Shape: `FloatClass` (`:15`); bodyless `Float::meaning32/64` (`:29-30`) and `FloatSemantics::*` (`:55-290`, about 60 signatures including `from_integer` x8, `to_*` x8, `to_*_saturating` x8, directed `*_toward_zero/positive/negative` for add, subtract, multiply, divide, square root, fma); `F32::meaning`/`F64::meaning` (`:35`, `:41`, the only bodies); token-bearing boundary machines `+ - * / == != < <= > >=` (`:299-465`); `F32/F64::negate`, `multiply_then_add`, `fused_multiply_add`, `square_root`, `minimum`, `maximum` (`:474-589`); `from_f64`/`from_f32` (`:593-610`); `from_i8..u64` x2 (`:614-694`); `I8..U64::from_f32/from_f64` each in three variants (default with `requires`, `in Trapping` with `crashes Trap`, `in Saturating`) (`:703-957`); classification queries (`:960-1019`); directed-rounding variants (`:1021-1409`).
3. In/out: no run-time reads; every row is realized by the compiler catalog (`:4-7`).
4. Decides: each float-to-integer row's range is spelled by literal bounds: e.g. `I8` `value > -129.0f32 && value < 128.0f32` (`:705`), `I32::from_f32` uses `>= -2147483648.0f32` (`:769`) while `I32::from_f64` uses `> -2147483649.0` (`:785`); comment `:696-701` explains "Use minimum minus one only when representable".
5. Cases by shape: exactly eight integer types x two float types x three policies; "Wrapping deliberately has no declaration." (`:698`).
6. Same job elsewhere: the same 48 float-to-integer rows are written again in `float_provider.omg` twice (trait rows `:57-168`, `NativeFloats` rows `:370-545`). Three copies of every bound.
7. Unused: n/a (declarations).
8. Stated limits: bodyless `Float::meaning32` etc. are "exact compiler primitives ... because this sealed toolchain source declares them with exactly this path and shape" (`:24-28`).
9. Fail-open: bodyless primitives (`:29-30`, `:55-290`) are accepted on the strength of path and shape (stated `:24-28`, `:47-54`: "Merely naming a declaration ... elsewhere grants no primitive"). Classification: stated.

### core/float_provider.omg (690)
1. Purpose: lists every `NativeFloats` member (`:219`) and binds each by `satisfies` to its public `Float::*` / `F32::*` / `I8::from_f32` requirement; Build never selects it (`:1-4`).
2. Shape: private `trait FloatNativeOperations` (`:6-217`, about 190 member signatures) and `NativeFloats satisfies FloatNativeOperations` (`:219-690`, one `boundary machine ... satisfies X ;` per row).
3. Decides: names are keyed on spelling: member `i8_from_f32_f32_trapping` satisfies `I8::from_f32` with `in Trapping` (`:374-377`); selection of the right row relies on the `in Trapping/Saturating` result domain and `requires`/`crashes` text.
4. Cases by shape: all rows listed, one per (operation, format) pair.
5. Same job elsewhere: contracts duplicated with `float_operations.omg:703-957` (see above).
6. Unused: n/a.
7. Stated limits: "The compiler catalog supplies intrinsic execution only for this sealed map." (`:3-4`).
8. Fail-open: none seen (rows carry no bodies; correspondence is compiler-checked, inferred).

### core/float.omg: `core/float/float.omg` (348)
1. Purpose: bit-level IEEE helpers for f32/f64: `from_bits`, `to_bits`, constants, classification, NaN construction.
2. Shape: `Sign` (`:6`), `NaNKind` (`:10`), `Classification<Payload>` (`:14`); `from_bits` (`:22`, `:29`), `to_bits` (`:36`, `:42`); constants `min`, `max`, `min_positive_normal`, `min_positive_subnormal`, `max_subnormal`, `epsilon`, `infinity`, `negative_infinity`, `quiet_nan`, `radix`, `precision_bits`, `fraction_bits`, `exponent_bits`, exponents, digits (`:49-200`); `sign`, `is_nan`, `is_finite`, `is_signaling_nan`, `classify`, `quiet_nan`, `signaling_nan`, `quiet`, `canonicalize_nan` per format (`:202-348`).
3. In/out: reads through `@bits as @f32` views (`:25`); each result `ensures` a bit pattern.
4. Decides on bits and on the type parameter: `transition T == f32 { true -> ..., _ -> f64 }` (e.g. `:53-56`); `classify` selects on masked exponent and fraction (`:226-238`, `:300-312`).
5. Cases by shape: only `f32`/`f64` (`requires T == f32 || T == f64`, e.g. `:50`); the f32-only machines take `value: f32` with `requires T == f32` (`:203`).
6. Same job elsewhere: `Sign` here (`Positive, Negative`, `:6-9`) and `core/float_meaning.omg:13-16` (`Negative, Positive`): same name, opposite case order, inferred same namespace (not resolved). `is_finite` here (`:211`, `:285`) vs `math.omg:486` vs `linear_algebra.omg:21` vs `F32::is_finite` (`float_operations.omg:978`): grepped `machine is_finite` in `source/`: 4 hits (float.omg 211, 285; linear_algebra.omg 21 as `is_finite_f64`; math.omg 486). `classify<T>` (`:221`) vs `F32::classify` (`float_operations.omg:960`) vs `classify_f32` (`float_accuracy.omg:49`).
7. Unused: not grepped.
8. Stated limits: "The static carrier equations admit only unqualified f32/f64." (`:3-4`).
9. Fail-open: none seen. Note `canonicalize_nan` calls `quiet_nan::<T>()` which resolves the zero-argument overload (`:271`, `:345`), traced by reading.

### core/float_accuracy.omg: `core/float_accuracy/float_accuracy.omg` (257)
1. Purpose: caller-visible comparison policy: `NumericClass`, symmetric-any and reference-additive tolerance comparisons for f32 and f64.
2. Shape: `NumericClass` (`:26`), `AccuracyComparison` (`:34`), tolerance records (`:39-42`), `classify_f32` (`:49`), `classify_f64` (`:107`), `magnitude_f32/64` (`:70`, `:128`), `compare_symmetric_any_f32/f64` (`:77`, `:135`), `compare_reference_additive_f32/f64` (`:170`, `:217`), `widen_f32_to_f64` etc. (`:44-47`).
3. In/out: values and tolerances in; `AccuracyComparison` out.
4. Decides on value: tolerance validity first (`:79-82`), NaN, equality, finiteness, then difference vs absolute then relative scaling (`:88-104`); additive form halves operands when `actual - reference` overflows (`:185-188`, `:198-209`).
5. Cases by shape: f32 and f64 copies, line for line.
6. Same job elsewhere: f32 and f64 bodies duplicate each other (`:77-105` vs `:135-163`; `:170-210` vs `:217-257`).
7. Unused: grepped `compare_reference_additive|compare_symmetric_any|ComplexF64|ComplexF32` (checkout excluding `.git`/atlas): 173 occurrences in 8 files, all in `samples/cli/scientific/w100/` and `core/complex`, `core/float_accuracy`; no use in `source-rust/` or `tools/`. `classify_f32/f64` are called by `complex.omg:85-86,144-145`.
8. Stated limits: "It is a rounded format computation, not a proof of exact real error." (`:8`).
9. Fail-open: `InvalidTolerance` is returned before equal values can hide a bad policy (`:78`, `:136`); none seen.

### core/complex.omg: `core/complex/complex.omg` (173)
1. Purpose: `Complex<T>` carrier plus add, multiply, conjugate, squared norm and scaled magnitude for f32 and f64.
2. Shape: `ComplexF32/F64` namespaces (`:38-39`), `Complex<T [copy]>` (`:41`), `ComplexMagnitude<T>` (`:49`), `admitted_finite` (`:28`), `ComplexF32::add/multiply/conjugate/norm_squared/try_magnitude` (`:55-113`), the same for F64 (`:115-173`).
3. In/out: write to a `destination: @mutable Complex<f32>`; results of `try_magnitude` are `Value`, `NonFiniteInput` or `Overflow`.
4. Decides on value: `NumericClass` from `float_accuracy` (`:28-35`); finite check twice (input then magnitude, `:84-86`, `:110-111`).
5. Cases by shape: f32 and f64 only; the f64 half duplicates the f32 half.
6. Unused: see float_accuracy line 7 (samples only).
7. Stated limits: "It promises no correctly rounded hypot, no relative-error bound for all inputs" (`:14-16`).
8. Fail-open: none seen.

### core/float_format.omg (62), core/float_meaning.omg (55), core/float_order.omg (52)
- float_format.omg: `FloatSpecialValues` (`:14`), `FloatFormat` (`:21`), constants `FloatFormat::BINARY32` (`:34`) and `BINARY64` (`:49`).
- float_meaning.omg: proof sum `FloatMeaning` (`:25`), domain `Rat::NonZero` (`:21`), `nonzero_rat` (`:35`), `finite_nonzero` (`:53`); both helper machines are private and have no caller in this unit (not grepped).
- float_order.omg: trait `Order<T>` (`:14`), `F32::TotalOrder`, `F64::TotalOrder` (`:34`, `:44`) implement IEEE totalOrder by turning bits into sortable keys (`:18-32`). Decides on bits, never on float comparison (`:5-11`). `TotalOrder` is used by `float_accuracy.omg:63`, `:121`.
Fail-open: none seen. (`float_meaning` stated: "NaN intentionally erases representation-specific payload bits", `:5-6`.)

### core/numeric_conversion.omg (1450)
1. Purpose: named integer conversions among u8..u64 and i8..i64, in four policies (exact, wrapping, saturating, trapping), plus widening helpers.
2. Shape: five private helpers (`:24-47`); then, per pair, `narrow_<src>_to_<dst>_{exact,wrapping,saturating,trapping}` and `widen_<src>_to_<dst>` (`:51-1450`). Examples: `narrow_u16_to_u8_exact` (`:51`), `..._trapping` with `machine invalid() { crash Trap; }` (`:73-82`); `widen_u8_to_u16` (`:84`); signed-to-unsigned `narrow_i8_to_u8_*` (`:918-949`); `narrow_i64_to_u64_*` (`:1420-1450`).
3. Decides on value: threshold compares (`value > 255`, `:67`, `:76`); signed `wrapping` goes through `numeric_conversion_signed_bits` (`:24`, e.g. `:302`) and a mask; the exact rows `requires` the range and `ensures embed(result) <= embed(value) && embed(value) <= embed(result)` (`:55`).
4. Cases by shape: a fixed matrix; widening names exist only where the full source range fits ("A widening name is used only when the complete source range fits the target", `:21-22`). Pairs like u8 to i8 are `narrow_*` (`:551`).
5. Same job elsewhere: inconsistent form: `narrow_i32_to_u8_wrapping` computes `(value & 0xFF) as u8` directly (`:1191-1193`), every other signed-to-unsigned wrapping row goes through `numeric_conversion_signed_bits` (e.g. `:1058-1062`, `:1224-1228`). `narrow_u64_to_u8_wrapping` begins `let bits: u64 = value as u64;` (`:194`), a no-op cast.
6. Unused: callers in this unit: `sha256.omg:13-14` (two rows), `std/console.omg:5-6`, `std/posix_filesystem_adapter.omg:15-19`, `std/macos_gui.omg:10-11`, linux/darwin time files. Not counted overall.
7. Stated limits: none ("These named machines implement those contracts visibly", `:5`).
8. Fail-open: none seen: the trapping rows route to `crash Trap`; saturating rows clamp by endpoint; wrapping rows are defined by mask.

### core/integer.omg: `core/integer/integer.omg` (162)
1. Purpose: static queries `min`, `max`, `width_bits`, `is_signed` over the eight fixed-width types, and carrier-family queries.
2. Shape: `min` (`:7`), `max` (`:32`), `width_bits` (`:57`), `is_signed` (`:82`), `Carrier` (`:113`), `carrier<T>` (`:122`, body never its answer: "This body is never its answer", `:119-121`), `is_integer`, `is_signed_integer`, `is_unsigned_integer`, `is_fixed_width_integer` (`:132-162`, each `crashes none terminates` with an `ensures` equal to the carrier field).
3. Decides on the type argument via `transition T == u8 -> ...` chains (`:20-29`); `carrier<T>` is elaborated by the compiler from the type's identity.
4. Cases by shape: exactly u8,u16,u32,u64,i8,i16,i32,i64; any other `T` violates `requires` (`:8-9`).
5. Same job elsewhere: `integer\max::<u32>()` conflicts with nothing; `float.omg` repeats the `T == f32 || T == f64` pattern.
6. Stated limits: "Only this exact toolchain declaration is elaborated by Psi" (`:119`).
7. Fail-open: `carrier<T>` returns `Carrier{false,false,false}` if ever run (`:122-124`), which would claim "not an integer". Classification: stated (never the answer, `:119-121`).

### core/linear_algebra.omg: `core/linear_algebra/linear_algebra.omg` (195)
1. Purpose: f64 slice kernels: dot, scaled norm, axpy and self-axpy, with all-or-nothing writes.
2. Shape: private `is_finite_f64` (`:21`), `try_dot_f64` (`:26`), `try_scaled_norm_f64` (`:59`), `try_axpy_f64` (`:115`), `try_axpy_self_f64` (`:159`).
3. In/out: `try_dot_f64(left,right,output)` writes `$output` only on success (`:51`); axpy validates all elements then writes (`:112-113`, `:121-154`).
4. Decides on value: finite-ness through `x - x == 0.0` (`:22-23`); length equality (`:27`, `:116`).
5. Cases by shape: f64 only; `index: u64 in Wrapping` loops.
6. Same job elsewhere: `is_finite_f64` duplicates `math.omg:486` and `F64::is_finite` (`float_operations.omg:981`).
7. Unused: grepped `try_axpy_self_f64|try_dot_f64|try_scaled_norm_f64|try_axpy_f64`: 28 occurrences in 5 files: this file (5), `samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/*`. No use in `source-rust/` or `tools/`.
8. Stated limits: "false means unequal lengths or a nonfinite input, intermediate or result" (`:7-9`).
9. Fail-open: none seen; a failing call leaves outputs unchanged (stated `:7-9`).

### core/math.omg (498)
1. Purpose: numeric helpers: `min_*`, `max_*`, `clamp_*`, `abs_*` for each fixed integer type, float `min/max/clamp/sqrt/abs`, plus `sin`, `cos`, `is_finite` for f64.
2. Shape: integer helpers for i8..i64 and u8..u64 (`:12-290`); float helpers (`:298-405`); `sin` (`:417`, nested `reduce :423`, `fold :439`, `poly :452`); `cos` (`:477`); `is_finite` (`:486`).
3. Decides on value: integer min/max by `<`; `abs_i*` `requires value > MIN`; `sin` reduces by a binary ladder of `2*pi*2^k` for `k` from 16777216 down (`:418-435`), folds to `[-pi/2, pi/2]` (`:441-443`), then a degree-17 Taylor (`:456-473`).
4. Cases by shape: a copy per carrier; `abs_u*` return the value (`:184-188`).
5. Same job elsewhere: `is_finite` (`:486`) duplicates `linear_algebra.omg:21`; `cos(x) = sin(x + pi/2)` (`:482`).
6. Unused: not grepped.
7. Stated limits: "Large arguments and sub-ulp accuracy are the recorded follow-on" (`:414-415`); accuracy "~5e-14 polynomial error" (`:413`). Comment cites a deleted test (`:493`, `tests/omega/pass/float/runtime_std_is_finite_exit`).
8. Fail-open: `sin` of an argument larger than about 6.7e7 skips the ladder and feeds `fold`/`poly` an out-of-range value, returning a wrong finite number without refusal (read `:418-436`; stated limit `:412-415`). Classification: stated. Also observation (inferred): `sin(+inf)` recurses with `x - step == inf` forever (`:432`); `sin` has no `terminates` clause.

### core/units.omg (184)
1. Purpose: library units: `Unit` record, constants, domain `Quantity<U>` and closed named conversions over `i64`/`f64`.
2. Shape: `Unit` (`:10`), `Units` namespace, constants `METER`...`KILOMETER_PER_HOUR` (`:20-62`), `domain<T,const U: Unit> T::Quantity<U>` (`:64`), operator machines `+ Quantity::add_i64_meters` (`:74`), `add_f64_meters` (`:82`), `/ divide_i64_meters_by_seconds` (`:90`), `divide_f64_meters_by_seconds` (`:98`), `divide_f64_kilometers_by_hours` (`:106`), constructors (`:114-140`), conversions (`:144-184`).
3. Cases by shape: "closed named combinations rather than computed index expressions" (`:6-8`); only these pairs (meters+meters, m/s, km/h).
4. Fail-open: none seen: i64 ops use `Trapping` and `crashes Trap` (`:77`, `:93`, `:146`).

### core/nat.omg (888), core/nat_metric.omg (553), core/rat.omg (512), core/int.omg (238), core/cauchy.omg (245), core/seq.omg (303), core/relation.omg (64), core/real.omg (21)
These are proof-library files; the machines have `terminates by`, `requires` and `ensures` and bodies that exist only to give the checker something to unfold.
- nat.omg: `Nat` (`:34`), bodyless `Nat::Descending` (`:18`), `Nat::BoundedDistance` (`:21`), token machines `<=` (`:48`) and `-` (`:56`), `add`, `mul` (`:70`, `:80`), `zero`, `one`, ring lemmas (`add_zero_right :122`, `add_comm :194`, `add_assoc :217`, `mul_comm :382`, `mul_assoc :332`, `mul_distributes :290`), `add_cancel :410`, `pred :435`, `saturating_sub :452`, monus lemmas (`:463-701`), `mod :711`, `gcd :854`, `div :873`. Decides: structural recursion on constructor (`transition a { Nat::Zero -> ..., Nat::Succ {prev} -> ...}`). Stated: "TRANSITIONAL TRUNCATED SUBTRACTION" (`:443`); `Nat::Descending` is a compiler primitive known by "exactly this path and shape" (`:3-9`).
- nat_metric.omg: `nat_gap` (`:5`), `nat_max` (`:403`), order and gap lemmas (`add_le_add_right :53`, `le_trans :79`, `mul_le_mul_right :140`, `mul_le_cancel_right :170`, `nat_gap_triangle :259`, `nat_max_le :491`). `mul_swap_trailing` has an empty body (`:391-395`), discharged by the semiring license.
- rat.omg: `Rat` (`:20`), `mk_rat` (`:26`), `mk_signed_rat` (`:48`), `rat_eq` (`:81`, returns `Nat` 0/1), `rat_gap` (`:106`), `rat_close` (`:153`), reflexive/symmetric/triangle lemmas (`:120-512`). Stated limit: "the carrier does not yet enforce that discipline structurally" (`:14-17`).
- int.omg: `IntPair` (`:21`), `add_int`, `mul_int`, `neg_int`, quotient lemmas (`:28-238`). Several lemma machines return a dummy `add_int(a, b)` / `neg_int(a)` / `mul_int(a, b)` body (`:158`, `:175`, `:191`, `:237`) that only gives the `ensures` something to state; the value is never meaningful.
- cauchy.omg: `cauchy_at` (`:13`), `converges_together_at` (`:30`), symmetric/triangle lemmas (`:64`, `:108`), `doubled_nat_max_modulus` (`:185`), `doubled_nat_max_threshold` (`:202`). Stated: "Heterogeneous transitivity, `converges_together_at_composed`, is not yet provided here." (`:242-243`).
- seq.omg: `Seq<T>` (`:6`), `BagView` (`:25`), `Bag` (`:29`), `map` (`:38`) and `filter` (`:51`) (generic, no `public`), `length`, `append`, `reverse` and lemmas (`:77-303`). Stated: Bag "is the slice view, so the views of `[2,1]` and `[1,2]` are not identical" (`:18-24`). `Seq` lemmas are u64-only (`Seq<u64>`).
- relation.omg: traits `Reflexive`, `Symmetric`, `Transitive`, `Antisymmetric`, `Equivalence`, `Preorder`, `PartialOrder` (`:9-64`); no bodies.
- real.omg: `Real` opaque (`:12`), `Real::add`, `Real::multiply` (`:14-15`) and two accepted law rows `real_add_commutative`, `real_multiply_commutative` (`:17-21`). Stated: "temporary axiomatic surface" (`:1`); "each law is a separate accepted declaration and therefore a separate trust row" (`:6-7`). Fail-open: classification stated: these two `boundary machine ... ensures` rows are assumed true, not proved.
Same job elsewhere: Nat gap/le lemmas partly overlap `rat_gap` (`rat.omg:106`) and `nat_gap` (`nat_metric.omg:5`) (`rat_gap` re-spells the Nat gap by hand). Unused: not grepped. Fail-open: none other seen.

## A4. Core vocabulary: numeric/option/data types, collections, schema, wire

### core/option.omg (15), core/conversion.omg (35), core/format_lineage.omg (14), core/representation.omg (77), core/wire.omg (151), core/iteration.omg (19), core/vec.omg (95), core/fixed_vec.omg (107), core/slice.omg (71), core/slice_views.omg: `core/slice_views/slice_views.omg` (15), core/indexing.omg: `core/indexing/indexing.omg` (23), core/external_binding.omg (45), core/sha256.omg: `core/sha256/sha256.omg` (462), core/reflect.omg: `core/reflect/reflect.omg` (340)
- option.omg: `Optional<T>` with `#0 case None`, `#1 case Some` (`:6-9`); private `zero_is_none` proof (`:11-15`).
- conversion.omg: `Result<Value,Error>` (`:6`), `TryFrom` (`:11`), `convert` (`:15`), `compose` (`:26`; the second map runs only after `Ok`, `:31-34`).
- format_lineage.omg: trait `FormatMigration<Lineage,Old,New>::migrate` (`:12`).
- representation.omg: `OpaqueRepresentation` (`:12`), request/error records (`:17-28`), `RepresentationModel` (`:32`), encoding/decoding traits (`:46`, `:60`), `Representation` (`:72`).
- wire.omg: codec requirement traits `Encode`, `StrictDecode`, `ProjectingDecode`, `PreservingDecode` (`:54-72`), `DecodeResult` with `Invalid` as the zero case "so zero-initialized verdict/result storage remains fail-closed" (`:17-22`), read/frame records (`:78-151`). No bodies.
- iteration.omg: trait `ElementStep` (`:1`) and `for_each` (`:6`), recursive over `items[1..]`, `terminates by items -> Slice::Length` (`:9`).
- vec.omg: `data Vec<T> {}` (`:23`), state-shaped `Vec::as_slice`, `Vec::push` with empty bodies (`:31`, `:34`), boundary trait `VecAccess` (`:40-95`). Stated: "The actual runtime storage mechanics are OUT OF SCOPE here" (`:11-12`).
- fixed_vec.omg: `FixedVec<T, const N: u64>` (`:55`) and boundary trait `FixedVecAccess` (`:65-107`: Length, push, pop, get, as_slice, each with `requires`). Stated: bodies "blocked on the generic-machine frontier above" (`:61-62`). Comments cite deleted `tests/omega/pass/...` paths (`:20`, `:34`).
- slice.omg: `slice_index` (`:6`, requires `index < items.length`), `slice_range` (`:11`), boundary trait `SliceAccess` (`:21`: Length, index_mut, tail, tail_mut, from, from_mut, to, to_mut, range_mut). Same job: `indexing.omg:3` (`read`), `:10` (`range`), `:18` (`view`) and `slice_views.omg:1` (`element`), `:9` (`subslice`). Grepped `slice_index|slice_range|slice_views` over the checkout: 43 matches in 29 files, of which in `source/`: `core/slice.omg` only (5); `source-rust/.../type_system.rs` has 8 matches (name list), rest samples/docs. 
- indexing.omg: bodyless `[] read`, `[..] range`, `view` (`:3-23`), "The sealed compiler supplies these operations" (`:1`).
- external_binding.omg: `DllImport` (`:9`, cases PeByName, PeByOrdinal, ElfVersioned, MachODylibSymbol) and `ForeignBinding` (`:36`, DllImport or Syscall). Consumed by `std/build.omg`.
- sha256.omg: `Sha256` record (`:16`, `msg: [u8; 8192]`, scratch `u64 in Wrapping` index fields), `sha256_round_k` (`:56`), `Sha256::reset` (`:90`), `stage_block` (`:103`), `compress` (`:134`), `digest` (`:254`). Decides on staged index guards: each array access sits in a state entered by a transition that restates `field < extent`; a violation sets `bad` (`:9-11`, e.g. `:128-130`). Cases by shape: message at most 8192 bytes; tail of up to two blocks (`:295-300`, `:334-346`). Same job elsewhere: Rust has a separate SHA-256 (grepped `Sha256::digest|Sha256::reset`: matches under `source-rust/` and `tools/` are Rust crates, not this machine). Unused: grepped `sha256\\Sha256|core\\sha256|Sha256` in `**/*.omg`: 63 matches, all in this file; no `.omg` consumer. Fail-open: `sha256_round_k` returns 0 when `i >= 64` (`:85-87`, unreachable since callers guard `w0 < 64`, `:213`; inferred); `digest` with `msg_n > 8192` ends with `bad = true` but still writes `out32` from a partial hash (`:116-119`, `:285-288`; open).
- reflect.omg: `TypeSchema`, `SchemaNode`, `SchemaMembers` (`:7-9`) over a byte image; bodies `schema<T>() { TypeSchema { storage: "" } }` (`:13`) and `callable` (`:35`), which return empty placeholders because "Only this exact toolchain declaration is elaborated by Psi" (`:11`); `schema_word` (`:48`), `schema_span` (`:75`), `graph_node` (`:115`), accessors (`:142-189`), `members_count` (`:208`), `members_get` (`:230`), `field` and `parameter` that `crash Trap` as fallback (`:275-300`, stated "no description is returned"), bodyless visitors (`:306-340`). Decides on image version == 2 and `graph.length >= 64` (`:118`), `count <= (graph.length - 64) / 16` (`:123`). Fail-open: `graph_node.select` (`:128-133`): a handle `>= count` falls to `read(graph, 0)`, so a bad handle yields node 0 instead of `Trap`; `members_get` (`:233-236`): an index past the member list returns `graph_node(graph, 0)` as well. Classification: open (no comment at either site; neighbors `bad()` do trap).

### core/CompactBinary and others not in this unit are not covered.

---------------------------------------------------------------------------------------------------------------------

# PART B: `std/` root files

### std/build.omg (833)
1. Purpose: the package Build: names the package and, for each target, selects providers and binds raw requirements to syscalls or DLL symbols.
2. Shape: `build` (`:4`), `configure_filesystem_providers` (`:85`), `configure_clock` (`:103`), `configure_time_host` (`:127`), `configure_windows_filesystem` (`:178`), `configure_posix_canonical` (`:353`), `configure_posix_filesystem` (`:384`), `configure_posix_filesystem_darwin` (`:500`), `configure_console` (`:644`), `configure_windows_gui` (`:696`), `configure_macos_gui` (`:751`), `configure_macos_image` (`:791`), `configure_process_exit` (`:826`), plus ObjectiveC binds inline (`:19-77`).
3. In/out: calls `Build::package`, `Build::select_provider::<Trait, Provider>(builder, target)`, `Build::bind::<Requirement>(builder, target, ForeignBinding...)`.
4. Decides keyed on target identifiers (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `macos_x86_64`, `windows_x86_64`, `alpha_bootstrap`) and on the spelling of the trait/member names. System call numbers are literal: x86 `openat 257, read 0, write 1, ...` (`:385-436`), arm64 `56, 63, 64, ...` (`:437-495`).
5. Cases by shape: per-target lists. Coverage gaps (read): `Clock` and `FilesystemService`/`PosixFilesystemHost` are selected for the four non-bootstrap targets but not `macos_x86_64` (`:104-120`, `:86-97`); `TimeHost` and `Console`/`ProcessExit` are selected for `macos_x86_64` (`:148`, `:682`, `:831`) and bound there to `macos_arm64` traits (`:157-164`); `ProcessExit` is selected for `alpha_bootstrap` (`:832`).
6. Same job elsewhere: arm64 reuses `targets\linux_x86_64\PosixFilesystemLinux::*` requirement names with arm64 numbers (`:437-495`), but `read/write/pread64/pwrite64/readlinkat/getdents64` use arm64-specific traits (`:439-446`, `:465-468`); stat is bound twice on arm64 (x86 trait `:472-475` and `PosixFilesystemLinuxArm64Metadata` `:476-479`).
7. Unused: n/a (build rows).
8. Stated limits: none; comment `:469-471` explains the 144 vs 128 byte stat window.
9. Fail-open: a requirement with no `Build::bind` for a target is not caught here (Build behavior is outside this file); `macos_x86_64` has console, time and exit rows but no clock or filesystem row (read). Classification: open (not traced to the Build checker).

### std/calling.omg (298)
1. Purpose: the compiler-owned vocabulary for calling policies: value shapes, register names, call plans, state plans, and the `CallingPolicy` and `Calling<Policy>` traits.
2. Shape: domain `[u8]::Utf8` (`:15`), `ValueClass` (`:26`), `ValueShape` (`:34`), `AbiValueClass`/`AbiValueShape` (`:50`, `:62`), `NativePlace` with `requires` (`:84-94`), `BoundarySignature` (`:118`, 256 shapes/fields, 32 parameters), `MachineRegister` (`:139`), `ValueLocation` (`:166`), `ValuePlacement` (`:187`), `CallingConvention` (`:193`, six conventions), `CallPlan` (`:222`), `StatePlan` (`:266`), `BoundaryEntryPlan` (`:276`), `BoundaryPlanResult` (`:286`), `CallingPolicy` (`:291`), `Calling` (`:297`).
3. Decides: none at run time; `NativePlace requires` field path 1 to 16 (`:85`).
4. Cases by shape: capacities 256/256/32/32/64/64 (`:118-120`).
5. Unused: not grepped.
6. Stated limits: "Array tails beyond each explicit count are ignored" (`:4-5`).
7. Fail-open: none seen.

### std/darwin_calling.omg (248)
1. Purpose: the calling policy `DarwinArm64ScalarCalling` for fixed scalar Darwin arm64 C calls, and the shared `darwin_arm64_entry` plan.
2. Shape: `DarwinArm64ScalarCalling` (`:8`), `darwin_arm64_scalar_signature` (`:21`) with nested `result`, `result_index`, `result_shape`, `result_integer`, `result_place`; `darwin_arm64_integer_shape` (`:70`); `darwin_arm64_scalar_parameters` (`:75`); `darwin_arm64_scalar_parameter` (`:91`); `darwin_arm64_entry` (`:192`, public).
3. In/out: reads a `BoundarySignature`, writes into a `BoundaryEntryPlan`; unfit signatures give `Rejected` with the text at `:15`.
4. Decides on shape: parameter and native counts equal, at most 32, no callbacks (`:23-25`); class Integer or Reference (`:114-118`); integer sizes 1, 2, 4, 8 with equal alignment (`:71-72`); first eight in `X0..X7` and only size 4 or 8 (`:146-158`); others on the stack, offset aligned to natural alignment (`:182`); stack extent must stay at most 256 (`:82`, `:146`); `257` is the reject sentinel (`:89-90`, `:99`).
5. Cases by shape: quote "expected at most 32 fixed integer or sized-reference parameters, Unit or 32/64-bit integer result, ... and no callbacks" (`:15`).
6. Same job elsewhere: parameter placement in `posix_byte_view_calling.omg:123-217` (six word registers only); clobber table 42 registers written out per entry (`:204-246`) and again in `targets/macos_arm64/entry.omg:121-162`, `targets/linux_arm64/entry.omg:161-202`.
7. Unused: `darwin_arm64_entry` called by `posix_byte_view_calling.omg:49`, and in `darwin_calling.omg:10` (grepped by reading).
8. Stated limits: "Narrow register scalars need separate caller-extension evidence." (`:5`).
9. Fail-open: none seen (every miss reaches `257`/`false`).

### std/posix_byte_view_calling.omg (319)
1. Purpose: four calling policies (`LinuxArm64ByteViewCalling :6`, `LinuxX86ByteViewCalling :19`, `SystemVByteViewCalling :34`, `DarwinArm64ByteViewCalling :47`) that pass a byte slice as pointer then length in two consecutive words.
2. Shape: `posix_byte_view_parameters` (`:59`), `posix_byte_view_parameters_from` (`:123`), `posix_byte_view_parameter` (`:135`), argument and result register choosers (`:219`, `:236`), entry plans for linux arm64 (`:244`), linux x86 (`:264`) and System V (`:281`).
3. Decides on shape: parameter_count equals native count, at most 6 (`:61-62`); no callbacks; reference of size 16/align 8 needs two words (`:163`), size 8/align 8 one word (`:164`), integers 4 or 8 bytes (`:171-172`); result integer 4/8 (`:99-100`); register by convention: `LinuxSyscallX86_64` uses `R10` for the 4th (`:221`), `SystemVAMD64` uses `Rcx` (`:222`), everything else `Aarch64X` (`:223`).
4. Cases by shape: "expected bounded byte views, integer or sized-reference formals, and Unit or integer result" (`:13`, repeated `:26`, `:41`, `:54`).
5. Same job elsewhere: four `plan` machines are identical except which entry builder they call.
6. Fail-open: `posix_byte_view_argument_register` falls to `Aarch64X{index: word}` for any convention that is not the two named (`:223`); the only callers pass Linux arm64/Darwin arm64/Linux x86/SystemV, so no other convention reaches it (inferred by reading `:244-281`). Classification: traced to four callers.

### std/entry_shapes.omg (110)
1. Purpose: shared helpers for hosted entry calling policies: integer-shape test, extent-record test, and placement builders.
2. Shape: `hosted_entry_integer_shape` (`:7`), `hosted_entry_extent_shape` (`:18`), `hosted_entry_named_word` (`:61`), `hosted_entry_arm_extent_value` (`:74`), `hosted_entry_register_extent_value` (`:93`).
3. Decides on shape: extent = record of byte size 16, alignment 8, two fields at offsets 0 and 8, each an 8-byte integer (`:24-26`, `:47-56`); field index bound `first_field < 255` (`:32`).
4. Same job elsewhere: `macos_x86_64_entry_extent_shape` (`targets/macos_x86_64/entry.omg:216-251`) is a second copy of `hosted_entry_extent_shape`; grepped `hosted_entry_extent_shape|macos_x86_64_entry_extent_shape`: 10 occurrences in 6 files: `entry_shapes.omg` (1), `linux_arm64/entry.omg` (2), `linux_x86_64/entry.omg` (2), `macos_arm64/entry.omg` (1), `macos_x86_64/entry.omg` (2, its own copy only), `windows_x86_64/entry.omg` (2).
5. Fail-open: none seen.

### std/clock.omg (23), std/process_exit.omg (23), std/filesystem_host.omg (5), std/main.omg (2)
- clock.omg: `Clock` trait with `sleep(milliseconds: u32)` and four `invokes` (`:10-22`).
- process_exit.omg: `NativeProcessExit satisfies ProcessExit` whose body calls `ProcessExitPrimitive(return_code)` (`:16-22`); `terminates` is claimed (`:20`).
- filesystem_host.omg: domain `[u8]::Path requires byte_observation\no_nul(@self[..])` (`:4-5`).
- main.omg: comments only.
Fail-open: none seen.

### std/console.omg (443)
1. Purpose: the `Console` boundary trait, six provider implementations, and shared adapters that turn a byte reader/writer into line operations.
2. Shape: `ByteRead` with `requires` (`:13-19`), `LineReadResult` (`:23`), `Console` (`:31`), `ConsoleByteOperations` (`:63`), `PosixConsoleRawOperations` (`:71`), four per-target `*ConsoleIo` boundary traits (`:81`, `:91`, `:101`, `:111`), providers `WindowsConsole :120`, `AlphaConsole :141`, `LinuxX86Console :161`, `LinuxArm64Console :182`, `DarwinArm64Console :203`, `DarwinX86Console :224`, raw and byte adapters per POSIX target (`:245-339`), `posix_console_read_byte` (`:344`), `posix_console_write_byte` (`:361`), `console_read_line` (`:371`), `console_write_bytes` (`:410`).
3. Decides on value: read count `1 -> byte`, `0 -> end`, else `Error` (`:348-352`); line scan stops on `value == 10` (`:393`); buffer full returns `Full(count)` before reading (`:375-378`).
4. Cases by shape: raw read/write move one byte through an `i32` cell; `descriptor == 0 && count <= 4` and `descriptor == 1 && count <= 4` (`:73`, `:76`).
5. Same job elsewhere: six `Console` providers are line-for-line copies with one name changed; four `*ConsoleIo` traits differ only by calling policy; three Raw/Bytes pairs repeat.
6. Unused: provider names are used only by `std/build.omg:662-690`.
7. Stated limits: "Finite storage does not promise a bound on waiting for host input." (`:369-370`).
8. Fail-open: `posix_console_write_byte` ignores the count returned by `Raw::write` (`:364`, `let written` unused), so a failed or short write is not reported. Classification: open. `posix_console_read_byte` maps any count other than 0 or 1, including a negative errno, to `ByteRead::Error` with no retry on interrupt (`:351`; read).

### std/filesystem.omg (461)
1. Purpose: portable filesystem value types and the `FilesystemService` boundary trait.
2. Shape: `File` (`:14`), `ErrorKind` (`:18`), results (`:46-104`), `DirEntry` (`:68`, name 765 bytes), `OpenOptions` (`:106`), `Permissions` (`:128`), `Metadata` (`:199`), `FilesystemService` (`:276-461`, about 50 requirements), helper machines `error_kind_reconstruct` (`:31`), `open_options_truncate_conflict` (`:120`), `permissions_readonly` (`:134`), `permissions_set_readonly` (`:142`), `posix_mode_with_readonly` (`:158`), `metadata_is_*` (`:211-232`), metadata getters (`:234-264`).
3. Decides: `open_options_truncate_conflict` is `truncate && (!write || append) && !create_new` (`:121`); readonly test for POSIX `(mode & 146) == 0` (`:138`); mask `4294967149` clears write bits (`:159`).
4. Cases by shape: `DirEntry.name: [u8; 765]` bound "255 UTF-16 code units, three UTF-8 bytes per unit" (`:65-67`).
5. Same job elsewhere: `TryLockResult` repeats `WouldBlock`, `Unsupported`, `InvalidInput`, `WriteZero` as its own cases next to `Error{kind}` (`:97-104`), duplicates of `ErrorKind` cases (`:18-29`).
6. Unused: `metadata_is_*` callers not grepped.
7. Stated limits: "Runtime acceptance is recorded per host" (`:4-5`).
8. Fail-open: `exists(path) -> bool` (`:317`) has no error channel; the POSIX adapter returns `stat_rc == 0` (see adapter `:780-785`): a stat failure other than not-found reads as "does not exist". Classification: traced (adapter). `try_exists` is the precise twin (`:321`).

### std/macos_gui.omg (651), std/macos_image.omg (104)
1. Purpose: macOS window, presentation, key-state and clock surfaces over Objective-C, Core Graphics and libSystem.
2. Shape: `AppKit` (`:39`), `ObjectiveC` (`:53`), `CoreGraphics` (`:164`), `macos_objc_class` (`:178`), `macos_objc_selector` (`:194`), `Window` (`:205`), `macos_window_create` (`:224`), `macos_window_destroy` (`:358`), `macos_window_get_dc` (`:378`), `msg_peek` (`:383`), `msg_translate` (`:406`), `msg_dispatch` (`:411`), `is_window` (`:424`), `blit` (`:438`), `present` (`:451`), `foreground_window` (`:524`), `macos_map_keycode` (`:540`), `macos_key_state` (`:604`), `DarwinClock` (`:615`), `macos_sleep` (`:626`), `macos_get_tick_count` (`:635`), `macos_perf_counter` (`:644`), `macos_perf_frequency` (`:651`). macos_image: `MacosImageData` (`:7`), `MacosImageObjects` (`:17`), `macos_image_snapshot` (`:44`).
3. In/out: GUI calls pass selector strings built as byte arrays; `present` copies pixels into a CGImage (`macos_image.omg:52-103`).
4. Decides on value: `present` accepts `1..=8192` width and height with `pixels.length == w*h` (`:462`), otherwise returns 0; key map is a 30-way chain on a Windows virtual-key number (`:541-570`) with 65535 for unmapped (`:601`); `msg_peek` returns 1 only if an event was captured (`:402`).
5. Cases by shape: `macos_window_blit` fixes the pixel array at `[i32; 4096]` and ignores `device`, `dest_width`, `dest_height`, `info` (`:438-448`); `macos_window_create` takes `class_name: [u8; 7]` and `style` and uses neither (`:224`, stated `:8`). Comment: "create subtracts 16/39 from outer dimensions, while blit stretches 64x64 pixels" (`:6-7`).
6. Same job elsewhere: three sleeps/poll traits: `DarwinClock::poll` (`:618`), `DarwinClockWait::poll` (`targets/macos_arm64/clock_impl.omg:17`), `DarwinTimeRead::poll` (`targets/macos_arm64/time_host_impl.omg:15`); `macos_sleep` (`:626`) wraps `u32 -> i32` with wrapping (`:630`), while `MacosArm64Clock::sleep` (`macos_arm64_clock.omg:8`) splits large waits (stated "retains the former u32-to-i32 wrapping contract", `:624-625`). The 22 repeated reach lists of 17 names (e.g. `:42-47`, `:57-62`) are written out again for each machine.
7. Unused: grepped `macos_map_keycode|macos_key_state|macos_window_blit|macos_window_present` over the checkout: 36 occurrences, in `std/macos_gui.omg` (8) and samples under `samples/gui/*` only (28). Grepped `macos_sleep|macos_get_tick_count|macos_perf_counter|macos_perf_frequency` (first 12 rows): 12 occurrences in `std/macos_gui.omg` (4) and four sample files; no other consumer.
8. Stated limits: "Class/style inputs do not select Cocoa classes or grant foreign authority." (`:8-9`).
9. Fail-open: `macos_window_create` ignores `AppKit::load()` (`let loaded`, `:238`) and the result of each `send` (`rc`); a failed class lookup yields handle `0` that is stored in `Window` (`:341-356`); `macos_window_msg_translate` always returns 1 (`:406-410`); `macos_objc_class` returns 0 for names that do not fit (stated `:174-177`). Classification: stated for the name case, open for the rest.

### std/posix_filesystem_host.omg (278)
1. Purpose: `PosixFilesystemHost` (raw POSIX service trait), `PosixFilesystemTarget` (per-target layout/flag/errno trait), and path termination helpers.
2. Shape: `PosixFilesystemHost` (`:33-167`, 36 requirements), `PosixFilesystemTarget` (`:175-202`, 11 requirements), `posix_terminated_path` (`:214`), `posix_copy_path_from` (`:229`), `posix_put_path_byte` (`:244`).
3. Decides on position: a path fits when `path.length < out.length` (`:216`); on a miss it writes a terminator at index 0 (`:222`) so the call sees `""`.
4. Cases by shape: `read_metadata` takes `@mutable [u8; 144]` (`:125`), `set_file_times` takes `@[u8; 32]` (`:144`); `directory_type_offset` bounded 18..20 and `directory_name_offset` 19..21 (`:188`, `:192`).
5. Same job elsewhere: terminated-path helper used by every target adapter (about 90 call sites in the three `posix_filesystem_impl.omg` files, by reading).
6. Stated limits: "A path that does not fit leaves the empty string, which every POSIX call refuses, never a truncated prefix" (`:205-207`).
7. Fail-open: none seen: the miss case is the empty string; verified for open, unlink, mkdir, stat calls (reading).

### std/posix_filesystem_adapter.omg (1858)
1. Purpose: the shared POSIX implementation of the portable `FilesystemService`: `FilesystemPosixProvider` forwards each requirement to a `posix_filesystem_*` helper that uses the target's `PosixFilesystemHost` and `PosixFilesystemTarget`.
2. Shape: `posix_metadata_kind` (`:23`); `FilesystemPosixProvider` (`:36-356`: every method makes `let mutable context: PosixFilesystem;` then calls one helper); `PosixDirectoryKind` (`:357`); `PosixFilesystem` scratch record (`:363`, `stat_buf [u8;144]`, `dir_buf [u8;512]`, `entry_name [u8;765]`, many `rd_*`/`rda_*` cursors); helpers: create/open/append (`:447-521`), read/write (`:525-559`), `posix_filesystem_write_descriptor` (`:570`), `write_all` (`:620`), `read_all` (`:646`), `copy` (`:686`), exists/try_exists/metadata (`:780-819`), errno mapping (`:832-851`), directory creation (`:853-943`), permissions (`:959-1034`), `posix_filesystem_file/descriptor` (`:1124-1142`), `open_with` (`:1145`), `read_dir_count` (`:1172`), `read_dir_nth` (`:1241`), `read_dir_entry_fd` (`:1257`), `remove_dir_all` (`:1423`) with `rda_drain/rda_step` (`:1461`, `:1473`) and `release_directory_stack` (`:1605`), `set_times` (`:1685`), `canonicalize` (`:1711`/`:1800`), locks (`:1720-1769`), `PosixCanonicalLibc` (`:1773`), `PosixErrno` (`:1786`).
3. In/out: reads `PosixFilesystemHost::*` returns (negated errno or counts), writes the scratch; converts to portable results through `PosixFilesystemTarget::*`.
4. Decides on value: success is `fd >= 0` / `n >= 0` / `rc == 0` (e.g. `:454`, `:535`, `:825`); error kind is looked up by `err_code` through the target (`:843-851`); hard-coded errno numbers in the shared file: `-4` retry on interrupted write (`:603`), `2` not found (`:797`), `17` exists (`:927`); mode constants `438` (0666, `:453`, `:475`, `:625`, `:746`, `:1156`) and `493` (0755, `:856`, `:875`); lock operations `2`, `1`, `6`, `5`, `8` (`:1723-1765`); readdir record geometry `rd_off < 495` (`:1194`) vs `< 489` (`:1288`), record length minimum `> 0` (`:1205`) vs `>= 24` (`:1301`); entry name limit 255 (`:1363`) though `DirEntry.name` holds 765 (`filesystem.rs`-level comment `filesystem.omg:65-67`); d_type `4` dir, `8` file, `0` unknown (`:1375`, `:1398`, `:1405`).
5. Cases by shape: `File.handle = descriptor + 1`, with `handle <= 2147483648` (`:1124-1141`, absent gives -1); `remove_dir_all` allows 4096 open levels (`:1431`, `:1586`); canonicalize and all paths use 4096-byte scratch (`:1804-1805`); `read_dir_count` returns `count - 2` assuming both dot entries are seen, `rd_count > 2` (`:1215-1221`).
6. Same job elsewhere: `FilesystemPosixProvider` methods are one-to-one wrappers (`:36-356`); the three `*_posix_filesystem_impl.omg` provide the inverse shims; `exists` (`:780`) and `try_exists` (`:787`) both issue the same stat; `read_dir_count` (`:1172`) and `read_dir_nth` (`:1241`) both walk `getdents` records with different magic bounds. `open_options_truncate_conflict` is used once (`:1149`).
7. Unused: `posix_filesystem_native_error_code_i64` (`:837`) is used by `posix_filesystem_last_error_i64` (`:848`) only (read).
8. Stated limits: "it promises no termination of its own, since another process may keep adding entries" (`:1459-1460`); "not atomic against concurrent replacement or mode changes" (`:968-969`).
9. Fail-open: (a) `posix_filesystem_exists` returns false for any stat failure (`:780-785`), traced above; (b) `read_dir_count` assumes `.` and `..` are both present while `read_dir_nth` explicitly refuses that assumption (`:1239-1240`, `:1310-1311`): an entry listing without both dot entries gives a count short by the missing ones, open; (c) `copy` keeps only `details.mode & 511` (`:721`), dropping setuid/sticky bits (read; behavior, stated nowhere); (d) `rds_parent_go` removes "entry 0" of the parent after popping a child, assuming that entry is the child just drained (`:1544-1557`), inferred; a concurrent insert at the front of the directory makes it delete the wrong entry or fail, open.

### std/darwin_calling.omg, std/entry_shapes.omg covered above.

---------------------------------------------------------------------------------------------------------------------

# PART C: `std/` per-target root files (time, clock, filesystem host maps)

### std/linux_x86_64_clock.omg (31), std/linux_arm64_clock.omg (21), std/macos_arm64_clock.omg (31)
`LinuxX8664Clock::sleep` (`:8-31`), `LinuxArm64Clock::sleep` (`:9-21`) and `MacosArm64Clock::sleep` (`:8-31`). x86 and macOS split a `u32` above 2147483647 into three thirds and a remainder of `poll(0,0,t)` calls (`linux_x86_64_clock.omg:14-29`, `macos_arm64_clock.omg:14-29`); arm64 builds a `timespec` and calls `nanosleep` once (`linux_arm64_clock.omg:15-19`) with a null remainder. None retries after an interrupted wait (stated `linux_x86_64_clock.omg:6-7`, `linux_arm64_clock.omg:6-8`). Data `LinuxArmClockSleepRequest` (`linux_arm64_clock.omg:4`).

### std/linux_x86_64_time_host.omg (100), std/linux_arm64_time_host.omg (95), std/macos_arm64_time_host.omg (58)
Providers satisfy `TimeHost` (the trait is defined in `std/time_host.omg`, not in my unit). Linux x86 and arm64 read `clock_gettime(1)` for monotonic and `clock_gettime(0)` for wall clock (`:32`, `:55`), convert `seconds * 1e9 + nanoseconds` in `Wrapping` after `narrow_i64_to_u64_saturating` (`linux_x86_64_time_host.omg:18-24`, arm64 `:14-20`). A failed read returns 0 ("Failed reads return zero; the std clock reads own degraded-clock handling", `:26`). Darwin passes selectors 8 (monotonic) and 0 (wall) (`macos_arm64_time_host.omg:10`, `:22`). All three report `1000000000` units per second and epoch offset 0 (`:45-76` etc.). Sleep: x86 and Darwin split large waits and call a "wait_part" that retries `poll` until it returns 0 (`linux_x86_64_time_host.omg:78-99`, `targets/linux_x86_64/time_host_impl.omg:47-53`; `darwin_time_host_wait_part` `targets/macos_arm64/time_host_impl.omg:29-35`); arm64 retries `nanosleep` with the full original duration until it returns 0 (`linux_arm64_time_host.omg:81-91`). Same job elsewhere: seven sleeps in total (the three clock files, these three, `macos_gui.omg:626`). Fail-open/other: a persistent non-zero outcome from `poll` or `nanosleep` recurses without a bound (`linux_arm64_time_host.omg:86-90`), so a failing call hangs; a failed clock read reports tick 0 (stated). Two unused data names inside: not grepped.

### std/linux_x86_64_posix_filesystem.omg (77), std/linux_arm64_posix_filesystem.omg (77), std/macos_arm64_posix_filesystem.omg (77)
Each is 37 `use` lines plus one `public ... satisfies PosixFilesystemHost` block of 34 assignments `PosixFilesystemHost::<op> = <target>_posix_<op>;` (e.g. `linux_x86_64_posix_filesystem.omg:39-77`). No logic. The three files are the same text with the target prefix swapped.

### std/macos_arm64_filesystem_abi.omg (37)
`DarwinTimespec` (`:7`), `DarwinStatRecord` (`:12`, `CLayout<DarwinTimespec>` for four timestamps), `DarwinStat = CLayout<DarwinStatRecord>` (`:33`), `DarwinFileTimes = [CLayout<DarwinTimespec>; 2]` (`:37`). The comment says "fixes the stat extent at144/alignment8" (`:3`, spacing typo in source). Stated: "A sparse record must use its own named policy." (`:4`).

---------------------------------------------------------------------------------------------------------------------

# PART D: `std/targets/**`

### std/targets/alpha_bootstrap/console.omg (42)
`AlphaConsoleIo` boundary trait (`:10`), `AlphaConsoleBytes` (`:15`), `alpha_console_read_result` (`:26`): `-1` is EOF, `0..255` a byte, anything else `Error` (`:27-41`). Writes narrow the `i32` byte by wrapping (`:22`). `use ByteRead;` and `use ConsoleByteOperations;` import by bare name (`:3-4`).

### std/targets/linux_arm64.omg (4), linux_x86_64.omg (4), macos_arm64.omg (4), macos_x86_64.omg (4), uefi_x86_64.omg (3)
Comment-only target definition files (e.g. `linux_arm64.omg:1-4`: "Target filtering keeps non-selected implementations inert").

### std/targets/linux_x86_64/clock_impl.omg (21), std/targets/linux_arm64/clock_impl.omg (18), std/targets/macos_arm64/clock_impl.omg (22), std/targets/linux_x86_64/time_host_impl.omg (56), std/targets/linux_arm64/time_host_impl.omg (35), std/targets/macos_arm64/time_host_impl.omg (38)
Raw boundary traits: `LinuxClockWait::poll` (`linux_x86_64/clock_impl.omg:15`), `LinuxArmClockWait::nanosleep` (`linux_arm64/clock_impl.omg:12`), `DarwinClockWait::poll` (`macos_arm64/clock_impl.omg:16`), `LinuxX64TimeRead` with `clock_gettime` and `poll` (`linux_x86_64/time_host_impl.omg:31`), `LinuxArm64TimeRead` (`linux_arm64/time_host_impl.omg:27`), `DarwinTimeRead` (`macos_arm64/time_host_impl.omg:11`); wait helpers `linux_x86_64_time_host_wait_part` (`linux_x86_64/time_host_impl.omg:41`) and `darwin_time_host_wait_part` (`macos_arm64/time_host_impl.omg:23`). Same job: `LinuxClockWait::poll` and `LinuxX64TimeRead::poll` bind the same syscall 7 (`build.omg:117-118`, `:167-168`); `DarwinClockWait::poll`, `DarwinTimeRead::poll`, `DarwinClock::poll` bind `_poll` (`build.omg:111-114`, `:153-156`, `:782-786`). Stated: "Retry failed waits instead of reporting sleep complete early; retrying may oversleep." (`macos_arm64/time_host_impl.omg:21-22`); "Native execution still requires general syscall lowering and emission" (`linux_x86_64/clock_impl.omg:8-10`).

### std/targets/linux_x86_64/entry.omg (235), linux_arm64/entry.omg (258), macos_arm64/entry.omg (199), macos_x86_64/entry.omg (253)
1. Purpose: per-target calling policy for the hosted program entry (`LinuxX86_64CallingPolicy :18`, `LinuxArm64CallingPolicy :24`, `MacosArm64CallingPolicy :16`, `MacosX64CallingPolicy :18`) and the application traits (`LinuxX86_64Application :235`, `LinuxArm64Application :258`, `MacosApplication :199`, `MacosX64Application :253`).
2. Shape: `*_entry_plan` machine with nested `physical`, `physical_indices`, `physical_shapes`, `build_physical`, `semantic`, `semantic_indices`, `semantic_shapes`, `build_semantic`, `finish`, `seal`, `reject`; word builders (`linux_arm64_entry_named_word :228`, `linux_arm64_entry_stack_word :241`, `macos_arm64_entry_word :186`).
3. Decides on shape (the one entry decision): parameter count selects the arm: Linux 1 = physical (initial stack word) and 2 = semantic (image + storage extents), others rejected (`linux_x86_64/entry.omg:27-31`); macOS 4 = physical (argc, argv, envp, apple) and 2 = semantic (`macos_arm64/entry.omg:25-29`). Shapes must be exact integers of 8 or 4 bytes (`linux_x86_64/entry.omg:56-57`, `macos_arm64/entry.omg:57-61`), extents by `hosted_entry_extent_shape`; physical needs a result, semantic must have none (`:34-37`, `:79-82`). Entry control is a supervisor call (`exit_group`) for Linux physical entry (rax 231 on x86, x8 94 on arm64) and a plain return otherwise (`linux_x86_64/entry.omg:133-148`, `linux_arm64/entry.omg:138-153`).
4. Cases by shape: quote "requires exact physical or program-storage entry shapes" (`linux_x86_64/entry.omg:229`, `linux_arm64/entry.omg:222`, `macos_arm64/entry.omg:180`, `macos_x86_64/entry.omg:210`).
5. Same job elsewhere: the four `*_entry_plan` machines are the same program with registers changed; the 25- and 42-entry clobber tables are typed out four times (`linux_x86_64/entry.omg:154-211`, `macos_x86_64/entry.omg:135-192`, `linux_arm64/entry.omg:160-202`, `macos_arm64/entry.omg:120-162`). `macos_x86_64/entry.omg:216-251` copies `hosted_entry_extent_shape`.
6. Unused: the four application traits are used by Build rows outside my unit (not grepped).
7. Stated limits: "This calling contract supplies no bridge, loader admission, receiver, or numeric stack guarantee." (`linux_x86_64/entry.omg:11-13`, `macos_arm64/entry.omg:10-11`).
8. Fail-open: none seen; every miss goes to `reject()`.

### std/targets/uefi_x86_64/handoff.omg (140)
Wrapper machines over firmware calls `uefi_os_handoff_legs_acquire/grow/free_map/attempt_exit` (`:27-86`), cycle forwarders (`:88-128`) and `valid_map_geometry` (`:130-140`; stride at least the minimum, version equal to the constant, `size > 0 && size <= capacity && size % stride == 0`). The header names `complete_boot_services` and `run` (`:14-16`), but they are not defined in this file (grepped `complete_boot_services`: 3 hits, `targets/uefi_x86_64/handoff.omg` (1, the comment) and `std/uefi_x86_64_handoff.omg` (2), the latter outside my unit). Fail-open: none seen.

### std/targets/linux_x86_64/filesystem_impl.omg (184), std/targets/linux_arm64/filesystem_impl.omg (195), std/targets/macos_arm64/filesystem_impl.omg (144)
1. Purpose: the `PosixFilesystemTarget` implementations: stat decoding, open-flag encoding, errno kinds, directory constants.
2. Shape: x86 `StatLayout` (`:24`), `StatRecord` (`:59`), `StatView` (`:75`), `linux_x86_64_posix_decode_metadata` (`:79`), `permission_bits` (`:111`), `encode_open_options` (`:46`), `decode_directory_kind` (`:119`), flags (`:129-143`), `kind_of_stored_error_code` (`:148`), `LinuxX8664PosixFilesystemTarget` (`:172`); the same set for arm64 (`LinuxArm64StatLayout :51`, `LinuxArm64StatRecord :28`, `LinuxArm64StatView :46`, `...PosixFilesystemTarget :183`) and Darwin (`DarwinStat` view, `macos_arm64_posix_*`, `MacosArm64PosixFilesystemTarget :132`).
3. Decides on constants: Linux open flags `write -> 1`, `read+write -> 2` shifted, append `<<10`, create `<<6`, truncate `<<9`, excl `<<7` (`linux_x86_64/filesystem_impl.omg:51-56`); Darwin uses `<<3`, `<<9`, `<<10`, `<<11` (`macos_arm64/filesystem_impl.omg:68-73`); no-follow flag `256` Linux, `32` Darwin; remove-dir flag `512` Linux, `128` Darwin; descent flags `196608` x86, `49152` arm64, `1048832` Darwin; errno catalog per target: Linux `2,13,1,17,9,21,11,22,34,36,38,95`, Darwin `2,13,1,17,9,21,35,22,34,63,45,78,102`; all others `Other` (`:150-163`, `macos_arm64:109-124`). Darwin `would_block` 35, Linux 11.
4. Cases by shape: hand-written layout plans place 13 `At` offsets keyed on field index (`linux_x86_64/filesystem_impl.omg:29-41`, `linux_arm64/filesystem_impl.omg:56-68`), declared sizes 144 and 128 (`:42`, `:69`). Decode rejects negative `blocks` or `blksize` with `InvalidInput` (`:84-87`, `:78-81`). Linux has no birth time (`TimestampResult::Error{Unsupported}`, `:106`); Darwin reads `btime.seconds` (`macos_arm64/filesystem_impl.omg:52`).
5. Same job elsewhere: three metadata decoders; x86 and arm64 differ in offsets and `nlink`/`blksize` width only; Linux errno tables are the same list twice (`linux_x86_64:150-163` = `linux_arm64:164-178`).
6. Stated limits: "Its reserved tail is not a birth timestamp." (`linux_arm64/filesystem_impl.omg:16`); "Linux stat has reserved words here, not a birth timestamp." (`linux_x86_64/filesystem_impl.omg:104`).
7. Fail-open: `StatLayout.plan` and `LinuxArm64StatLayout.plan` read `schema.fields[0..12]` without checking `field_count` (`linux_x86_64/filesystem_impl.omg:29-41`); correctness depends on the compiler's plan validation (`core/layout.omg:130-137`), stated there. Classification: stated. `kind_of_stored_error_code` maps any unlisted errno (including `ENOTEMPTY`, `EINTR`, `ENOSPC`) to `Other` (open, behavior).

### std/targets/linux_x86_64/posix_filesystem_impl.omg (574), std/targets/linux_arm64/posix_filesystem_impl.omg (517), std/targets/macos_arm64/posix_filesystem_impl.omg (785)
1. Purpose: the target kernel or libc adapters for every `PosixFilesystemHost` row; each terminates path arguments into a 4096-byte local buffer, calls the raw leaf, and returns negated errno.
2. Shape: x86: `PosixFilesystemLinuxX86ByteViews` (`:21`), `PosixFilesystemLinux` leaves (`:32-97`: openat, close, lseek, mkdirat, unlinkat, fchmodat, fchmod, renameat, linkat, symlinkat, newfstatat, fstat, ftruncate, utimensat, fsync, fdatasync, dup, flock, fchownat, fchown), then 34 `linux_x86_64_posix_*` machines (`:98-574`). arm64: `PosixFilesystemLinuxArm64ByteViews` (`:19`), `PosixFilesystemLinuxArm64Metadata` (`:30`), 34 `linux_arm64_posix_*` machines (`:41-517`) reusing `targets\linux_x86_64\PosixFilesystemLinux` (`:6`). Darwin: `PosixFilesystemDarwinByteViews` (`:40`), `darwin_timestamp_word` (`:54`), `PosixFilesystemDarwin` (`:72-160`), `darwin_failure_i32/i64` (`:163`, `:169`), `darwin_normalize_i64` (`:176`), 34 `macos_arm64_posix_*` machines (`:185-785`).
3. Decides on value/position: every path machine sets up the terminated copy, issues the raw call, and only then checks `fits`, returning `-36` (Linux) or `-63` (Darwin) if the path did not fit (e.g. `linux_x86_64_posix_filesystem... :102-108`); `read` and `pread` clamp to `min(count, buffer.length)` (`:153`); `readlinkat` and `getdents64` further clamp to `2147483647` (`:388`, `:398`); the stat-based machines check `fits` before the call (`:473-480`); `creat` uses flags `577` (`:138`); Darwin libc failures read `errno` through `posix_errno` and negate it (`macos_arm64_posix_filesystem_impl.omg:163-173`, `:197-201`); `___getdirentries64` with a buffer of at least 1024 bytes saves and restores its last four bytes (`:540-571`).
4. Cases by shape: paths up to 4095 bytes; Darwin modes narrowed to `u16` by wrapping (`:211`, `:231`, `:327`, `:427`, `:445`); uid/gid narrowed to `u32` (`:742`); `-100` is `AT_FDCWD` on Linux (`:104`), `-2` on Darwin (`:618`).
5. Same job elsewhere: x86 and arm64 files are the same 34 machines with the prefix changed; Darwin repeats them with `settle(outcome)` calling `darwin_failure_i32` (about 34 copies). Linux syscall numbers live in `std/build.omg`, not here.
6. Unused: leaf traits are bound only in `std/build.omg` (`:385-495`, `:501-641`); grepped by reading.
7. Stated limits: "an authored import cannot request that placement yet" (`macos_arm64/posix_filesystem_impl.omg:23-24`, variadic arguments, so `___open` is used).
8. Fail-open: (a) the raw call runs before the fit check, with `""` as the path, and its result is discarded when the path did not fit (e.g. `linux_x86_64/posix_filesystem_impl.omg:102-108`); stated at `posix_filesystem_host.omg:205-207`. (b) `darwin_failure_i32` returns `0 - errno`; if the libc call returned negative but `errno` reads 0, the machine returns 0, which every caller reads as success (`macos_arm64/posix_filesystem_impl.omg:163-167`, `:198-201`); inferred (libc sets errno on failure), classification open. (c) Darwin mode narrowing wraps silently (`:211`).

---------------------------------------------------------------------------------------------------------------------

# Cross-unit summary of duplicated jobs (all grepped or read, counts given where I ran a command)

- Float-to-integer conversion guards: `float_operations.omg:703-957`, `float_provider.omg:57-168`, `float_provider.omg:370-545` (three spellings of the same 48 rows).
- `is_finite`: `float.omg:211`, `float.omg:285`, `math.omg:486`, `linear_algebra.omg:21`, `float_operations.omg:978,981` (Grep `machine is_finite` in `source/`: 4 matches in 3 files, plus the boundary rows).
- `Sign`: `float.omg:6` vs `float_meaning.omg:13`.
- Slice index and range: `indexing.omg:3,10`, `slice.omg:6,11`, `slice_views.omg:1,9`.
- Console providers: `console.omg:120,141,161,182,203,224` (six copies).
- Sleep: seven bodies (`linux_x86_64_clock.omg:8`, `linux_arm64_clock.omg:9`, `macos_arm64_clock.omg:8`, `linux_x86_64_time_host.omg:78`, `linux_arm64_time_host.omg:74`, `macos_arm64_time_host.omg:36`, `macos_gui.omg:626`); differing in whether a failed wait is retried.
- Poll requirement traits: `LinuxClockWait`, `LinuxX64TimeRead::poll`, `DarwinClockWait`, `DarwinTimeRead::poll`, `DarwinClock::poll`.
- Entry plans: four copies; extent-shape test twice (`entry_shapes.omg:18`, `macos_x86_64/entry.omg:216`).
- POSIX target adapters: 34 machines per target; stat decoders three times; errno tables three times.
- Byte-view calling policies: four `plan` machines, identical but for the entry builder (`posix_byte_view_calling.omg:6-58`).

# Coverage

Read in full, every line of every range in `units/unit_085.tsv` (113 rows, 21,976 lines):
- core: rows 1 to 65 (11,981 lines) in full.
- std root: rows 66 to 90 (6,308 lines: 833, 298, 23, 443, 248, 110, 461, 5, 21, 77, 95, 31, 77, 100, 31, 37, 77, 58, 651, 104, 2, 319, 1,858, 278, 23) in full.
- std targets: rows 91 to 113 (3,687 lines) in full.
Skimmed: none. Not read: none.
(Sum check: 11,981 + 6,308 + 3,687 = 21,976.)

# Five things a maintainer most needs to know about this unit

1. The files are mostly contracts and proof text, not running code. The running code is std (`console.omg`, `posix_filesystem_adapter.omg`, the target adapters, `macos_gui.omg`, the time and clock files), `sha256.omg`, `math.omg`, `linear_algebra.omg`, `numeric_conversion.omg` and the `core` byte helpers. Bodyless `boundary machine` rows are accepted on the strength of the compiler catalog (`float_operations.omg:24-28`, `real.omg:6-7`, `run.omg:79-81`).
2. The same table is typed out several times with only names changed: 48 float-to-integer rows three times, six console providers, four entry plans (42-register clobber tables inline), 34 POSIX adapters for each of three targets, three stat decoders, seven sleep bodies. Any fix has to be made in every copy; the macos_x86_64 entry also has its own copy of the extent-shape test.
3. Error handling in std is by convention, not by type: negative errno numbers, hard-coded 2, 4, 17 in the shared adapter (`posix_filesystem_adapter.omg:603`, `:797`, `:927`), each target mapping errno through its own short list with everything else `Other`; `exists` and `posix_console_write_byte` discard failures (`:780-785`, `console.omg:364`).
4. Several places answer a bad input with something that looks valid: `reflect.omg` returns node 0 for a bad handle (`:128-133`, `:233-236`); `read_dir_count` assumes both dot entries (`posix_filesystem_adapter.omg:1215-1221`); the Darwin errno path can return 0 after a failed call (`macos_arm64/posix_filesystem_impl.omg:163-167`); `sin` past about 6.7e7 returns a wrong value (`math.omg:412-415`).
5. Several vocabulary files in this unit have no consumer outside `samples/` or none at all: `linear_algebra.omg`, `matrix_layout.omg`, `complex.omg`, `float_accuracy.omg` (samples only), `sha256.omg` (no `.omg` consumer, 63 matches all inside the file), `macos_gui.omg` window machines (samples/gui only). `std/targets/uefi_x86_64/handoff.omg` header describes `complete_boot_services` and `run`, which are not in that file.

# What I am unsure about

- Whether `float.omg`'s `Sign` and `float_meaning.omg`'s `Sign` live in the same namespace (both in core, different directories); I did not read the resolver.
- Whether the compiler checks that `StatLayout` and `LinuxArm64StatLayout` offsets match the real kernel layout; I only compared the numbers with my memory of the x86-64 and asm-generic stat layouts (they look right).
- `posix_filesystem_set_times` writes only the seconds halves of `times_buf` (`:1688-1703`); I assumed the nanosecond halves stay zero because a fresh `let mutable context: PosixFilesystem;` is zero-initialized. Not verified.
- Public/pub caller counts for most core vocabulary traits (`Equatable`, `Order`, `Content`, `DeviceLoanProvider`, `WaitWake`, `TaskRuntime`) were not grepped; I did not state they are unused.
- `std/build.omg` does not show how Build treats a requirement with no row for a target; the `macos_x86_64` gap (no `Clock` or filesystem rows, `build.omg:104-120`, `:86-97`) may be handled elsewhere.
- Whether `rds_parent_go`'s "remove entry 0 of the parent" (`posix_filesystem_adapter.omg:1544-1557`) can ever delete a different entry; I reasoned it is correct for a quiescent directory only.
