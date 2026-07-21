> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide and design briefs; this file tracks open
> engineering work only. Completed work belongs in git history and canary
> headers.

# Tasks

Last pruned: 2026-07-20.

Omega's first real consumer is Cathedral (`../Cathedral`). General language
work takes priority, with Cathedral vertical slices used as acceptance tests.
Fetch `main` before taking a compiler task and avoid a lane whose newest commit
is already changing the same subsystem.

## Immediate queue

### Checked assembly, inbound entry plans, and the Cathedral timer

This is the critical path from the current serial-only Cathedral milestone to
the first timer tick. The design is recorded in
`wiki/design_briefs/os_memory_and_hardware_foundation.md`, chapter 19, and
chapter 23.

The checked x86 catalog includes structured control-register, MSR, flags,
fence, and interrupt-mask operations. `iretq`, `sysret`/`sysretq`, and `eret`
are deriver-only. Do not expose source-level `lidt` before IDT2: the current
freestanding-root authority bridge cannot record installed inbound roots, so a
raw catalog entry would create an effect/WCSU audit hole.

The first ENT2 slice is implemented in `omega-calling-conventions`: normalized
register/value-placement vocabularies, deterministic `CallPlan + StatePlan`
identity, MS-x64/SysV-x64/AAPCS64/Linux-syscall evaluators, plan validation,
and a separately fingerprinted footprint-evidence carrier. Existing host
bindings can be evaluated through this model as an independent oracle while
their hardcoded encoders remain in service. The inbound process-entry
argument prologue now consumes the normalized native `CallPlan`'s exact
register and width on x86-64 and AArch64; incoming stack arguments and scalar
float register locations are covered as well. Integer and scalar-float
process-entry results now carry the plan-selected result register through both
native encoders. Flat
one-to-four-member AAPCS64 homogeneous floating-point aggregate entry
parameters are classified from their normalized record layout and spread
across the selected vector registers. Fixed non-HFA AAPCS64 records up to 16
bytes now use consecutive `x` fragments or aligned whole-value stack
fragments, and
normalized small-aggregate result plans select `x0`/`x1`. Small AAPCS64 entry
terminals now load fixed non-HFA results into `x0`/`x1` and flat HFA results
into their plan-selected vector registers. AAPCS64 aggregates
above 16 bytes now have normalized indirect placements, outbound calls use
caller-owned copies plus `x8` result destinations, and entry prologues copy
register- or stack-passed pointees into runtime-frame storage. Unsupported
mixed/general entry signatures retain the compatibility path without panicking
the compiler. Pure-integer SysV AMD64 records up to 16 bytes now use
consecutive GPR fragments when the whole value fits, otherwise roll wholly to
the stack without consuming the remaining argument register; normalized result
plans select `rax`/`rdx`. SysV entry parameters now also classify flat HFA and
recursive INTEGER/SSE records, consume the independent GPR/XMM banks
atomically, and fall wholly to their planned stack fragments on either-bank
exhaustion. Small SysV entry results now load terminal record fragments into
their normalized `rax`/`rdx` and/or `xmm0`/`xmm1` locations.
`Binding`-authored SysV integer imports now preserve
those records through selection, marshal their planned register or stack
fragments, spill small aggregate results from `rax`/`rdx`, and keep layout plus
data/call relocation walkers in lockstep. Authored scalar floats retain their
class from the selected storage descriptor; SysV calls marshal the independent
XMM bank or planned stack slot and spill results from `xmm0`. Generic Linux syscall
leaves now evaluate the normalized syscall policy at emission and pass its
exact parameter registers, number register, and supervisor-call immediate into
both ISA encoders; the legacy binding fields no longer choose those facts on
that path. The normalization seam also enforces policy, supervisor-call control,
stack/shadow facts, and the encoder scratch/clobber ceiling. Runtime-storage
x86-64 syscall arguments now stage through volatile `r11`/`rax` instead of
silently destroying callee-saved `r15`; AArch64 large-offset marshalling reuses
the plan-selected `x8` number register, which the plan now declares clobbered.
Composite runtime-text byte and line syscalls now consume the same normalized
placements:
the AArch64 encoders honor the plan-selected registers and supervisor-call
immediate, while the fixed x86-64 sequences reject plans they cannot realize
instead of silently choosing an ABI. AArch64 C/import calls and their results
now evaluate AAPCS64 from selected operand shapes and pass the plan's exact X/V
registers and stack placements to the ISA encoder, including scalar stack
arguments, flat HFA arguments/results, and fixed non-HFA arguments up to 16
bytes in consecutive `x` registers or whole-value stack fragments. AArch64
vtable and service-table calls
reuse that marshaller and dispatch through caller-saved `x16`; vtable receivers
remain planned `x0` arguments, while service-table pointers stay outside the
wire signature. Field-model returns route plan-selected scalar GPR/vector and
flat HFA results through matching stores and relocation accounting. The
general Microsoft x64 import path
now derives its policy from the concrete target, evaluates argument/result shapes,
consumes the plan's register and shadow-relative stack placements, and rejects
non-Microsoft x86 policies
instead of silently applying Win64. The Win64 normalization seam also enforces
policy, call/return control, 16-byte stack alignment, 32-byte shadow space, and
the `rax`/`r10`/`r11` encoder scratch/clobber ceiling. General imports, vtable
and service-table dispatch, and their result stores stage through
plan-clobbered `r11`/`rax` instead of silently destroying callee-saved `r15`.
Microsoft x64 vtable and firmware service-table calls now use the same
plan-driven register/stack marshaller,
with dispatch-only table pointers excluded from the wire signature and
plan-selected results checked before storage. The ordinary host-operation
Windows imports now consume evaluated Microsoft x64 placements. `GetStdHandle`,
`ExitProcess`, and `Sleep` use the plan-driven general marshaller with
byte-identical relocation layouts.
`GetAsyncKeyState` now consumes planned RCX/RAX placements while preserving
its required 16-bit zero-extension result transform.
Windows time out-parameter calls now evaluate their actual foreign signatures:
one planned RCX pointer plus an ignored planned RAX `BOOL` for QPC/QPF, or void
for `GetSystemTimePreciseAsFileTime`; the temporary stack slot remains an
encoder materialization detail. The x86 constant-result routing remains
operation-specific so QPF cannot be mistaken for AArch64's frequency constant.
Composite `ReadFile`/`WriteFile` calls now model their actual five-parameter
signature and ignored `BOOL` result; RCX/RDX/R8/R9, the shadow-relative fifth
argument, and the scratch-slot reservation all come from that evaluated plan.
The dedicated runtime line and byte Windows sequences reuse that exact layout
plus an actual one-DWORD/RAX `GetStdHandle` plan, preserving their fixed widths
and relocation sites. AArch64 fragmented compatibility calls and indirect-call
footprint contracts remain below. Ordinary/firmware entry lowering now validates a combined
boundary plan with no interrupted state, no save/restore obligation, a
provider-selected stack, non-preemptive entry semantics, and a transitive state
ceiling derived exactly from the ABI volatile-register classes.

The policy-result foundation now represents source evaluation explicitly as
`Accepted(BoundaryEntryPlan)` or a structured `Rejected` reason. Accepted plans
cross the common validator, canonicalize placement-fragment order before
contract hashing, and are the only result that can produce published identity;
policy rejection remains distinguishable from a malformed accepted plan for
declaration-site diagnostics.
The bundled `std::calling` module now exposes that closed signature, placement,
register, entry-control, and machine-state vocabulary to ordinary policy
authors. A policy implementation satisfying `CallingPolicy::plan` can already
be invoked through the compiler's purity-gated build-time interpreter: the
compiler materializes `BoundarySignature`, decodes the complete result, and
returns only a validated canonical plan. Focused end-to-end coverage exercises
signature-dependent acceptance and structured rejection. Concrete boundary
`Calling<C>` relationships are now discovered automatically in both compiler
entry paths; every inherited/declared boundary method is materialized and
evaluated, and its canonical plan fingerprint enters provider requirement
identity without perturbing identities that do not opt into calling policies.
Policy evaluation, authored rejection, and invalid accepted-plan diagnostics
retain the `Calling<C>` relationship source span. Generic boundary declarations
now evaluate only at concrete standalone conformance instances; the concrete
trait argument tuple keys internal lookup, forwarded method types are
substituted before signature materialization, and satisfies-derived provider
schemas recover the same instance without publishing policy type identity.

1. **ENT2c — lowering migration and concrete entry state.** Express the
   existing MS-x64, SysV-x64, AAPCS64, Linux-syscall, and firmware lowering
   choices through the normalized plan; continue beyond the completed
   register- and stack-resident process-entry argument paths, integer and
   scalar-float entry results, and generic and runtime-text Linux syscall paths
   to C/firmware
   outbound calls/results and compatibility-binding differential checks; the
   register-resident AArch64 C/import slice is complete, including exact
   plan-selected argument/result registers and fail-closed unsupported
   placements. The general Microsoft x64 import path likewise consumes exact
   planned register/stack/result placements and target-derived policy, as do
   Microsoft vtable and firmware service-table calls. Ordinary composite
   x86-64 host operations are now plan-checked through their actual foreign
   signatures, as are the dedicated runtime line/byte Windows sequences.
   The general Win64 encoder additionally rejects incompatible policy/control/
   stack/shadow/clobber contracts and keeps its call marshalling, dispatch, and
   result-store scratch inside the normalized ordinary-clobber ceiling.
   Direct 1-, 2-, 4-, and 8-byte Microsoft x64 entry records now consume the
   normalized boundary plan in both directions: declared parameters store from
   the plan-selected integer register or stack slot and terminal records load
   into `rax`. A source-to-PE UEFI canary pins the eight-byte `rcx`/`rax` path.
   Nondirect record widths now use the Microsoft hidden-result convention at
   entry: the destination arrives in `rcx`, declared arguments shift by one
   slot, the terminal copies through the preserved pointer, and returns it in
   `rax`. A 16-byte source canary pins the complete handoff. By-reference
   Microsoft aggregate entry parameters now consume their positional pointer
   from `rcx`/`rdx`/`r8`/`r9` or the shadow-relative stack slot and copy the
   complete pointee into runtime storage. Source-to-PE canaries pin both a
   register pointer between scalar arguments and the fifth-position stack
   pointer after the return address and 32-byte shadow space.
   `Binding`-authored outbound Microsoft records of 1, 2, 4, or 8 bytes now load
   by value into their positional GPR or the low bytes of their stack slot; a
   source-to-PE canary pins an eight-byte record in `rdx` between scalar
   arguments. All other record widths allocate 16-byte-aligned caller copies
   beyond shadow space and outgoing pointer slots, copy every fragment, and
   place the pointer in the normalized positional GPR or stack slot. General
   imports, vtable calls, and firmware service-table calls share the same
   reservation, width, and relocation accounting; a second source-to-PE canary
   pins a 24-byte record between scalar arguments. Outbound aggregate results
   now consume the same normalized Microsoft plan: 1-, 2-, 4-, and 8-byte
   records spill from `rax`, while every other width supplies the caller-owned
   destination in hidden `rcx`, shifts declared arguments (and vtable
   receivers), and leaves the callee to write in place. General imports,
   vtable calls, and service-table calls keep their encoders, widths, and
   relocation walkers in lockstep. Source-to-PE canaries pin both an eight-byte
   direct result and a 24-byte hidden-destination result with the first declared
   argument shifted to `rdx`.
   Authored Microsoft scalar `f32`/`f64` calls now retain their float class
   through the same path: arguments use the positional `xmm0`-`xmm3` register
   selected by the plan or the low bytes of its outgoing stack slot, and
   results spill from `xmm0`. General imports, vtable calls, and service-table
   calls share the float marshaller, width, and relocation accounting. A
   source-to-PE canary pins interleaved integer/float arguments in `rcx`/`xmm1`/
   `r8`/`xmm3`, a fifth-position stack float, and the `xmm0` result.
   Runtime-backed scalar `f32`/`f64` entry terminals likewise evaluate the
   declared primitive result shape instead of using the integer fallback, then
   load the plan-selected `xmm0` or `v0`. Source-to-PE and cross-target ELF
   canaries pin complete incoming/outgoing `xmm0` and `d0` round trips on
   Microsoft x64, SysV AMD64, and AAPCS64.
   Constant primitive-integer entry terminals now evaluate that same declared
   result shape instead of forcing the legacy four-byte fallback; exact
   one-, two-, four-, and eight-byte widths reach the plan-selected native
   result register. Source-to-image canaries pin full-width `rax` and `x0`
   writes for `u64`, and direct encoder tests pin the newly admitted `u16`
   shape on both architectures.
   Flat runtime scalar binary entry terminals now reuse the ordinary
   domain-aware binary writer through a reserved, width-aligned result scratch
   place, then load the plan-selected integer or vector result register. A
   native execution canary removes the former field-assignment workaround for
   `self.a + 100`, while the existing PE and cross-target ELF float canaries
   now compute before returning through `xmm0`/`d0`. Literal float terminals
   stage their exact IEEE bits through the same scratch-to-vector path; PE and
   Linux ARM64 source canaries pin both the bits and normalized register load.
   Record and case-literal entry terminals likewise materialize through a
   layout-sized, layout-aligned result place before consuming the same direct
   fragments or saved hidden destination as runtime-backed records. One
   source canary pins a two-word literal through SysV `rax`/`rdx`, AAPCS64
   `x0`/`x1`, and the Microsoft x64 indirect-result path.
   Runtime-indexed slice-element entry terminals now reuse the internal
   call-result indexed-copy shape to stage the selected element in result
   scratch, then load the normalized scalar register. A dependent-bound slice
   source canary pins `eax` and `w0` delivery on Linux x64 and ARM64.
   Runtime logical-NOT entry terminals now compare their byte-sized runtime
   operand with zero into result scratch before loading the normalized boolean
   result register. A native execution canary pins `!false` as process exit 1
   instead of the former silent natural-termination zero.
   Runtime numeric-cast entry terminals now reuse the ordinary scalar
   conversion writer in result scratch, deriving the destination primitive
   from the declared entry return type before its normalized register load. A
   native canary pins runtime `u8`-to-`i32` widening as process exit 70.
   Runtime binary entry terminals now build their operands through the shared
   recursive value-operand resolver, preserving nested arithmetic, conversions,
   signed operator selection, and arithmetic-domain witnesses. A native canary
   pins `a + ((b * c) / 10)` as process exit 70; nearby scalar-float and ABI
   result canaries remain green.
   Entry and internal call-result scalar operations now converge on the ordinary
   pre-resolved-place writer. Runtime comparison terminals return normalized
   booleans, while terminal `min`/`max`/`sqrt` builtins receive result scratch
   only when their builtin symbol matches. Native canaries pin `a < b` as exit 1
   and `max(a, b)` as exit 70.
   Authored scalar helper calls in entry-terminal position already preserve the
   callee's native result register through entry termination; the former
   compile-only free-standing `add_i32(3, 4)` canary now executes and pins exit 7.
   Fixed-array and text/slice descriptor entry results now wait on the explicit
   native-boundary policy decision in `OWNER_QUESTIONS.md` section 4; byte size
   alone must not silently define their public ABI.
   Direct scalar binary, numeric-cast, `min`/`max`/`sqrt`, runtime-indexed
   slice-element, and fixed-array indexed expressions in host-call argument
   position now materialize into bounded per-argument frame scratch before
   marshalling, preserving the scalar integer-versus-float register class.
   Indexed range expressions remain slice/address arguments and are checked by
   their descriptor-aware lowering rather than the scalar scratch blocker; a
   native canary pins `exit_process(self.a + self.b)` as exit 70. Direct nested
   authored value calls now participate in call-argument sequencing, execute
   their callee body before the host operation, and marshal from the resulting
   scalar slot; `exit_process(self.dbl(35))` is pinned natively and in the
   interpreter as exit 70.
   Compatibility syscall rows are differentially checked against normalized
   number-register and supervisor-call facts on both Linux architectures; the
   complete import/vtable/service-table compatibility mechanism matrix is
   checked against the target-derived native policy across Windows, UEFI,
   Linux, and macOS, while Linux-syscall mechanisms now reject non-ELF targets
   instead of choosing a policy from CPU architecture alone. The
   generic encoders additionally reject incompatible policy/control/stack/
   shadow/clobber contracts and keep all marshalling scratch inside the
   normalized ordinary-clobber ceiling.
   Scalar AAPCS64 outbound stack placements now reserve aligned outgoing space,
   materialize integer/pointer or float values through caller-saved scratch
   registers, store at plan-selected offsets, restore SP after the call, and
   feed the same overhead into layout and both relocation walkers. Flat
   two-to-four-member HFA arguments now remain one by-value operand through
   selection and consume every plan-selected vector-register fragment; grouped
   placements also drive layout and relocation accounting. When the vector bank
   is exhausted, the same operand copies each member into its contiguous planned
   stack area. Authored scalar-float arguments now retain their class from the
   selected storage descriptor rather than collapsing into same-width integers,
   and imports spill the plan-selected vector result through a matching relocated
   scalar store; a source-to-Mach-O canary pins both `d0` directions. Authored flat HFA results
   preserve one aggregate result place and spill every plan-selected
   vector-register fragment through one relocated base. The AArch64 import
   normalization seam now also rejects plans whose
   policy, call/return control, 16-byte stack alignment, zero-shadow-space
   contract, or ordinary-clobber ceiling cannot cover the encoder's fixed
   caller-saved scratch set; placement is no longer the only enforced plan
   facet. Continue making the plan authoritative across compatibility paths.
   Selected source policy plans now survive typed lowering beside their public
   fingerprints and attach only to the admitted provider's authored binding.
   `Binding`-authored x86-64 imports consume that exact plan for byte emission,
   width, and call/data relocation walks; a source-selected SysV placement on
   a Windows image is no longer replaced by the target-native Microsoft plan.
   The encoder rechecks the selected operand shapes and fails closed for
   unsupported policies. `Binding`-authored AArch64 imports now likewise consume
   the retained plan for emission, measured width, and call/data relocation
   walks. Non-default register and outgoing-stack placements are pinned across
   those consumers instead of being replaced by target-derived AAPCS64.
   `Binding`-authored syscall leaves now also retain authority: x86-64 and
   AArch64 emission plus measured width revalidate and consume the selected
   parameter, number-register, and supervisor-call control facts. Non-default
   argument-register canaries pin both architectures; unsupported result-bearing
   syscall signatures still fail closed.
   Evaluated source-policy identities now retain the complete canonical
   `BoundaryEntryPlan` through checked lowering instead of discarding the
   `StatePlan` after hashing it. Outbound bindings continue to project the
   call half, while inbound-stub lowering can recover the matching stack,
   preemption, save/restore, regime, and transitive-state obligations by the
   same semantic boundary key. Selected provider rows and backend host
   bindings now carry that complete plan as well; emission, layout, and both
   relocation walkers borrow its call half, leaving the state half available
   at the selected backend boundary instead of truncating it in orchestration.
   Compatibility bindings now resolve through the same complete-plan API:
   their existing call-plan entry point is only a projection, while selected
   source plans are fully revalidated against the concrete signature before
   either direction may consume them.
   The process-entry prologue now delegates register, incoming-stack,
   indirect-parameter, and hidden-result-pointer unmarshalling to a reusable
   inbound-stub derivation. That derivation consumes and revalidates the exact
   complete `BoundaryEntryPlan`; non-default selected registers reach its
   abstract writes, and a state-invalid carrier fails before any writes are
   produced. The matching reusable exit derivation now returns canonical
   result fragments plus entry/exit control from that same validated plan;
   process-entry scalar and aggregate result selection consumes it instead of
   reading a separately evaluated native result register.
   Fixed non-HFA AAPCS64 entry records up to 16 bytes now consume consecutive
   plan-selected `x` fragments with 16-byte register alignment, fall wholly to
   aligned stack fragments when the remaining register bank is too small,
   while normalized small-aggregate results select `x0`/`x1`; a
   source-to-object canary pins a mixed scalar + 16-byte record entry in `x0`,
   then `x1`/`x2`. `Binding`-authored outbound calls now preserve the same
   non-HFA records as one by-value operand, load every plan-selected `x`
   fragment, or copy the whole value into aligned outgoing stack fragments;
   cross-target canaries pin both realizations. Authored imports and indirect
   field calls now spill small aggregate results from their plan-selected
   `x0`/`x1` fragments through one relocated result base. Aggregates above 16
   bytes now use normalized indirect placements: outbound calls allocate and
   populate aligned caller-copy slots, pass their pointers in the planned `x`
   register or stack slot, and materialize large result destinations in `x8`;
   entry prologues load register- or stack-passed pointers and copy the complete
   pointee into its runtime-frame slot. Source-to-object and ISA tests pin both
   register and stack-pointer paths, records beyond the special 32-byte boundary
   handoff ceiling, the Microsoft-only scope of that exception, relocation
   position, and fragment stores.
   Small AAPCS64 entry results now consume those normalized result placements:
   fixed two-word records load `x0`/`x1`, while flat HFA members load `s`/`d`
   vector registers directly from runtime storage. Cross-target source
   canaries and direct encoder tests pin both families, including a 24-byte HFA
   that remains in `d0`/`d1`/`d2` rather than capturing `x8`. Large non-HFA
   AAPCS64 entry results now preserve the plan-selected hidden destination from
   `x8` before volatile work and copy the complete terminal record through it;
   unlike SysV, the entry emits no extra pointer return in `x0`. A Linux ARM64
   source-to-object canary pins the capture, three-word copy, and immediate
   terminal handoff.
   Pure-integer SysV AMD64 entry records up to 16 bytes now consume consecutive
   plan-selected GPR fragments when the complete aggregate fits. If the
   remaining register bank is too small, the complete aggregate moves to
   aligned stack fragments and the rolled-back register remains available to a
   following scalar; normalized small-aggregate result plans select
   `rax`/`rdx`. Linux x64 source-to-object canaries pin both the register and
   stack/rollback entry paths. `Binding`-authored outbound SysV integer and scalar
   float calls now preserve small records as one operand, consume the
   plan-selected GPR/XMM or whole-value stack fragments, spill small record
   results from `rax`/`rdx`, and spill scalar float results from `xmm0`; ISA and
   relocation tests pin the independent register banks, stack/rollback, result,
   alignment, clobber-ceiling, and fixup paths. A source-level Linux x64
   probe reaches that lowering and stops only at the existing ELF direct-image
   dynamic-binding gap. End-to-end import image/run acceptance is therefore
   platform-blocked until ELF dynamic imports exist. Ordinary SysV AMD64
   `VtableSlot`/`VtableField` and dispatch-only `TableFunction` calls now reuse
   the normalized integer/float/small-record marshaller and result spills; the
   receiver is required in planned `rdi`, service-table pointers stay outside
   the wire signature, and width plus data-relocation walkers consume the same
   layout. A freestanding source-to-ELF canary pins layout-resolved vtable
   dispatch. Pure-integer records above 16 bytes now take the SysV MEMORY path
   on outbound imports and indirect field calls: parameters copy by value into
   aligned outgoing stack fragments, results materialize their caller-owned
   destination through hidden `rdi`, and declared GPR arguments (including a
   vtable receiver) shift accordingly. The source-to-ELF canary pins the
   resulting `rsi` receiver dispatch. Entry prologues now copy MEMORY-class
   parameters from every normalized incoming stack fragment into runtime-frame
   storage, with a source-to-ELF canary pinning all three words of a 24-byte
   record. The first SysV SSE aggregate slice is also normalized: flat two- to
   four-member `f32`/`f64` records totaling at most 16 bytes are packed by ABI
   eightbyte into `xmm` registers (or roll wholly to aligned stack fragments),
   return through the corresponding `xmm` registers, and survive source
   selection, indirect field dispatch, result storage, width, and relocation
   accounting. Flat scalar records whose two eightbytes include SSE now carry
   that ordered INTEGER/SSE class pair through selection: mixed records reserve
   both register banks atomically (with whole-record stack rollback) and return
   through `rax` plus `xmm0`, while non-homogeneous SSE/SSE records such as
   `{ f64, f32 }` use `xmm0`/`xmm1`. Both survive the same source-to-ELF field
   dispatch path. MEMORY-result entries now save incoming hidden `rdi` before
   volatile work, shift declared parameters, copy a large terminal through the
   saved destination, and return that pointer in `rax`; a source-to-ELF canary
   pins the complete handoff. Named records nested within small SysV records
   are now classified recursively: scalar leaves merge into their containing
   eightbytes with INTEGER dominance, so a nested `{ u64, { f64 } }` record
   follows the same atomic mixed-bank parameter and result path. The indirect
   field-dispatch canary pins that recursive source-to-ELF selection. Fixed
   array members now recurse element-by-element with their canonical packed
   stride; an `{ [u32; 2], [f32; 2] }` source canary pins the resulting
   INTEGER/SSE class pair and completes the scalar/named-record/fixed-array
   leaf family. The same recursive classifier now covers a single occupied
   eightbyte: a nested f64 wrapper preserves SSE class through outbound
   argument/result selection instead of collapsing to INTEGER.
   SysV entry prologues now reuse those shapes: flat HFA parameters store
   their packed SSE eightbytes from consecutive `xmm` registers, mixed
   INTEGER/SSE parameters store from both independent banks, and exhaustion
   in either bank rolls the whole record to incoming stack fragments without
   consuming the other bank. Source-to-ELF canaries pin all three paths and
   the following scalar's rolled-back `xmm0` placement.
   Small record terminals now consume the same normalized result placement:
   INTEGER/INTEGER, SSE/SSE, and INTEGER/SSE entries load runtime storage into
   `rax`/`rdx`, `xmm0`/`xmm1`, and `rax`/`xmm0`, respectively. The shared
   return-copy operation now realizes scalar XMM loads with matching width and
   relocation accounting. Three source-to-ELF canaries pin those result
   combinations, while a 24-byte homogeneous-float canary proves the SysV
   two-eightbyte ceiling still selects the existing hidden-pointer MEMORY path.
   One-eightbyte nested SSE entry parameters and results likewise store from
   and load to `xmm0`; a source-to-ELF round-trip canary pins both directions.
   SysV MEMORY entry parameters no longer inherit the separate 32-byte
   Microsoft boundary-handoff ceiling: a 40-byte source canary pins all five
   incoming stack-fragment copies.
   Ordinary AArch64 `VtableSlot` and `VtableField` calls now evaluate AAPCS64
   from their selected operands, require the full-width receiver in planned
   `x0`, marshal every argument/stack slot through the shared plan consumer,
   and dispatch through caller-saved `x16` with no import relocation. Field
   calls separate a leading result place from the receiver and store a
   plan-selected scalar GPR result with layout and relocation accounting in
   lockstep. AArch64 `TableFunction` calls likewise exclude the dispatch-only
   table pointer from the AAPCS64 signature, marshal only declared arguments,
   and account for table/result fixups around the indirect dispatch. Both
   indirect mechanisms route plan-selected scalar integer, scalar float, and
   flat HFA results through matching relocated stores. `Binding`-authored
   `VtableSlot`/`VtableField` and `TableFunction` bindings now carry their
   retained source plan through emission, layout, and data relocation on both
   x86-64 and AArch64. The dispatch-only table stays outside the signature;
   a source-selected SysV plan can govern indirect calls in a PE image, while
   AArch64 preserves exact non-receiver register and outgoing-stack placements.
   The concrete x86 interrupt
   `StatePlan`, stack/IST, nesting, and acknowledgement policy used by Cathedral
   is OWNER-BLOCKED on `OWNER_QUESTIONS.md` section 2.
3. **ENT3 — constrained entry codegen.** Derive entry stubs, specialize/codegen
   under the state ceiling, emit a checkable final footprint certificate, and
   validate after relaxation, veneers, thunks, and generated stubs. The shared
   inbound-storage derivation now consumes and revalidates the complete
   `BoundaryEntryPlan`, produces target-specific register-clobber evidence for
   its generated x86-64/AArch64 copy fragments, and checks that evidence against
   the retained state ceiling before selection continues. Scratch identities
   live beside their ISA encoders, and selected inputs that alias scratch before
   capture reject instead of silently storing a frame-base value. Exit
   derivation likewise consumes the complete plan and preserves canonical
   result placement plus call/return control. Whole-body transitive evidence,
   save/restore specialization, and post-layout aggregation across handler code,
   exits, veneers, thunks, and admitted leaves remain.
4. **IDT1 — symbolic materialization (normalized foundation complete).**
   `LayoutPlan` now uses compiler-issued field keys normalized back to names;
   repeated `Bits` entries validate exact logical-source tiling plus
   destination bounds/overlap, while ordinary plan-laid values require one
   `At` per field. Normalized sealed `Data(DataSymbolId) | Entry(EntryStubId)`
   sources now derive resolved writes, native whole-pointer relocations, and
   post-handoff writer records while rejecting loader-consumed unresolved
   fragments. Object/image relocation sites are section-qualified, generic
   `Absolute64` relocations patch initialized data on both native families,
   and PE rebasing records data sites correctly. Native symbolic actions now
   lower with an explicit materialization origin rather than fake instruction
   metadata. Normalized placement constraints now join layout alignment with
   permitted address range, build/load/post-handoff phase, machine-regime
   identity, and artifact-installation scope, and validate concrete sites. The
   decoded constraint record is now bound into artifact admission and must match
   the exact record carried by the claimed placement at materialization, so a
   provider cannot substitute weaker constraints behind the admitted placement
   plan identity. Canonical executable-container v2 now requires a bounded entry
   set: unique compiler-issued `EntryStubId` values and in-code offsets are
   validated, the entry-set identity is bound into admission, and only an
   admitted artifact can yield a sealed entry materialization target from that
   set. The exact installed-code state now supplies the private resolver for
   atomic post-handoff entry writers: it resolves only entries in that admitted
   set against the installed placement, rejects foreign/data targets before
   publication, and never exposes the numeric address API. Lower these
   normalized writer programs to generated machine code (OWNER-BLOCKED on the
   provider boundary and atomic-publication contract in `OWNER_QUESTIONS.md`
   section 4).
5. **IDT2 — installed-root ledger.** Add `lidt` only as an installation path
   that consumes scoped IDT
   authority and records every installed entry as an external analysis root
   with effects, receipts, state plan, stack/IST class, nesting/WCSU, and
   component/version pins. The stack/IST policy is one fact consumed by both
   layout materialization and WCSU analysis.
6. **IDT3 — linear interrupt obligations.** Implement saved-mask guards and EOI
   obligations as provider-minted linear values with explicit consuming
   restore/complete operations. Do not use drop cleanup or interrupt-specific
   linearity rules.
7. **Cathedral timer acceptance.** Program PIT or LAPIC, install the IDT, post
    a bounded tick event, report ticks over the owned serial line, and `hlt`
    between ticks under QEMU. Negative rails: direct assembly cannot launder
    reach; user `iretq` rejects; incomplete fragment tiling rejects; forbidden
    final-artifact clobbers reject; omitted or double EOI rejects.

### Provider plans and retirement of `provides`

Provider plans are derived from `satisfies` closure. Checked adapters have
Omega bodies; irreducible leaves use
`satisfies Requirement via <Binding>;`. Target packages provide defaults and a
slot owner may override by type. The migration order remains load-bearing.

1. **PRV4b — Console adapters.** The honest owned-`String` to
   borrowed-byte-view runtime path (`as_view`/`bytes`) now runs in both engines,
   and standard `Console::write` and `Console::write_line` are checked Omega
   code: self-forwarding adapters walk that view with measured `Slice::Length`
   state transitions and reach only `write_byte`. Field-backed, literal-backed,
   and empty-line cases run differentially; the checked-tree canary pins both
   calls to their adapters, and the lossless built-in plan oracle remains green.
   More than 1,300 exact duplicate Console declarations now import that package.
   The compiler's dungeon lattice snapshot now shares the same standard import
   as the runnable sample instead of retaining a second String-based boundary.
   The remaining local declarations are intentionally different carrier,
   effect, or proof fixtures; migrate those with their owning surfaces, then
   remove the composite compatibility rows under PRV4f.
2. **PRV4c — target defaults and overrides.** Candidate plans are now keyed by
   provider type, unrelated conformance closures never combine, and only the
   selected covering candidate reaches adapter or leaf lowering. Explicit
   type-per-slot `build.omg` selection now validates the provider against the
   loaded dependency closure and is confined to the build root's slot-owner
   authority. Target-scoped package-owned `provider_defaults` machines now
   supply ordinary defaults, explicit build selection overrides them per slot,
   and conflicting target defaults reject. Extend the same selection contract
   to test/component slot owners. Coverage, signature conformance, transitive
   effect refinement, normalized identity, and selected-target-only ambiguity
   are already enforced.
3. **PRV4e — foreign format facts.** Move foreign offsets and bit constants
   from `Binding::Value` into programmable layout/format declarations and
   migrate filesystem leaves. Open-option flags have moved: portable code now
   supplies semantic `OpenOptions`, checked target-package machines own the
   Darwin/Linux/MSVCRT encodings, and no flag constant remains in a provider
   row. Target `open_with` implementations now copy the semantic record once
   and call their ordinary target encoder. Native lowering defers an outer
   value result until the complete nested statement-call splice has executed,
   and direct aggregate construction restores omitted fields and padding to
   their zero representation before applying named fields. The remaining
   `Binding::Value` filesystem consumers are `struct stat` offsets, which move
   when programmable foreign-record views can project the validated plan.
4. **PRV4f — compatibility deletion.** After the last consumers move, delete
   `Value`, populate tables, `provides` syntax, and every compatibility
   consumer. The unused `Binding::Instruction` carrier is already gone:
   instruction realizations are checked `asm` bodies, not provider rows. The
   test-only `HostOperations`/`call_shape` round-trip bridge is gone as well;
   checked adapters own composite behavior. The complete direct-import ABI
   canary family now derives bodyless `satisfies Requirement::method via
   Binding::DllImport(...)` leaves instead of authored `provides` rows: AArch64
   scalar, stack, HFA, and direct/indirect aggregate placement; SysV aggregate
   placement; Win64 scalar and direct/indirect aggregate placement; and the
   hosted import-argument runtime path all retain their exact lowering checks.
   The runnable UEFI hello sample and its slot/ref-argument canaries likewise
   use a target-scoped external leaf via `Binding::VtableSlot(1)` rather than a
   one-row compatibility table. Attached external leaves now carry their data
   type as the table-layout owner for `Binding::VtableField` and
   `Binding::TableFunction`; free table-field leaves reject. The UEFI field,
   out-parameter, and two-row service canaries plus the full SysV field-dispatch
   matrix have moved off `provides ... over Struct` while preserving layout-
   selected offsets and native aggregate calling plans.
   The obsolete standalone `host ... provides` syscall example is gone; the
   qualified external-leaf syscall canary already pins both x86-64 and AArch64
   admission, argument registers, syscall-number registers, and supervisor-call
   instructions.
   Remaining `provides` fixtures either intentionally pin compatibility
   grammar/diagnostics or still exercise vtable/syscall/value consumers that
   must migrate with their owning surfaces. Keep only the directed retirement
   diagnostic if useful.
### Compile-time machine parameters and generics

The source model is fixed: `<machine M>` requires an authored
`where machine M(args) -> Result` contract; selection such as
`map<Card::power>(items)` is compile-time symbol metadata, never a runtime
argument or inferred contract. MP4b now groups complete call-site tuples,
deep-copies each additional template body with fresh lexical symbols, rewrites
calls to their concrete states, and runs distinct type and static-machine
specializations in both engines. MP5 now captures a binder-positional universal
template-contract identity before substitution, spends one trust receipt for an
accepted template, binds every instance to the checked contract identities of
its selected static machines, and exports that relation in the machine-contract
manifest. Contract changes invalidate instances; implementation-body-only edits
remain contract-invisible.

The former blanket "machine parameters are unbuilt" blocker is retired.
Compile-time selection, modular checking, direct invocation, specialization,
and manifest identity are implemented and must not defer downstream customers.
The blocker audit leaves only genuinely stronger requests separate: turning a
machine into a stored runtime value, sealed entry reference, address-bearing
relocation source, or dynamically registered callback. `Calling<C>` remains a
policy-type relationship by design, not as a workaround for machine-parameter
support; provider row builders remain rejected because they duplicate
`satisfies`, not because they require an unavailable parameter form.
Static IDT/provider selection and generic replacement orchestration may consume
the implemented type-parameter form; only their runtime entry/relocation edges
remain fenced on reification. Any future deferral must state which of those
stronger operations it needs instead of citing machine parameters generally.

1. **MP6 — remaining consuming slices.** `Seq`'s consuming `map`/`filter` are
   now core machines: recursive static-machine selections specialize to direct
   calls, with no runtime callable, dictionary, or capture inference. Still add
   the nested proof schemas used by N5/N6, task-runtime machine selection, and
   the remaining build-surface canaries.

## Type, proof, and semantic-model work

### Dependent facts and frames

- **R5 — frames.** Direct and acyclic transitive internal calls, plus resolved
  boundary calls, now preserve linear arithmetic facts, recast and
  boundary-range witnesses, dependent entry and forwarding facts, and exact
  default-domain valuations outside their conservatively instantiated
  receiver/exclusive-argument may-write paths. Value-position calls use the
  same summaries recursively; when body analysis is unavailable (unknown,
  transitioning, static-machine, or cyclic callees), their ownership-bounded
  fallback invalidates the whole receiver plus explicit mutable arguments but
  preserves unpassed caller locals. Finish the `stores` clause, explicit
  state-arrival contracts, and broader Houdini-style inference for facts
  crossing sibling calls.

### Domain facets, effects, termination, and trust

- **DOM1 — facet kinds.** Enforce predicate versus semantic facets through
  merges, joins, casts, and generic substitution with per-axis composition.
- **DOM2 — binding-site operators.** Resolve operator theory from declarations,
  mints, and `requires`; never from flow facts. Resolve tuples deterministically
  and reject collisions.
- **DOM3 — introduction authority.** Implement sealed-by-default domains,
  `introduction open`, and `MintAuthority<D>` with distinct missing-proof and
  missing-authority diagnostics.
- **DOM4 — normalized identity.** Finish the deterministic domain-expression
  normalizer and make it own type/monomorphization identity.
- **DOM5 — weakening.** Add `weakens_to` certificates and sealed-theory hashes
  that detect stale operator theories.
- **STR — semantic carrier cleanup.** Finish termination-plan integration,
  validation/resolution from normalized domain/machine/permission plans,
  lowering only from checked selections, and deletion of compatibility bools.
- **EFX — kinded effects completion.** Resolve boundary-trait and core members;
  compute transitive recursive fixed points; enforce public ceilings and pinned
  provider subsets; split artifact/diagnostic/trust-ledger output; migrate core,
  std, and canaries away from the lowercase global table as semantic canon.
- **TPR4/TPR6 — publication and progress profiles.** Serialize public
  termination omission/default rules in artifacts. Resolve sealed profile
  domains, grant-backed admission and receipts, and pinned progress premises.
  Profiles are never flow-inferred ranking evidence.
- **GR6 — remaining trust consumers.** Finish qualification authority,
  ProgressProfile minting/premises, and MachineContractPlan permission/provider
  admission through the existing grant/receipt carrier.

### Carry, multiplicity, task lifecycle, and allocation

- **CRY1–CRY6 — four-axis carry policy.** The normalized
  suspension/CPU/thread/address record now survives syntax, resolved, typed,
  syntax snapshots, and a checked `CarryFacts` plan that separates authored
  minimums from effective derived policies. `[carry(...)]` requires all
  four axes, `[send]` has a directed retirement diagnostic, transparent
  aggregates derive per-axis intersections, and data/machine generic bounds
  compare the complete policy (including specialization admission). Concrete
  generic instantiations derive through symbol-keyed argument substitution,
  including nested wrappers. Opaque `boundary data` carriers now parse without
  a public shape or layout, cannot be constructed by ordinary code, default to
  the strict effective carry policy, and reject permissive property claims
  until admission can provide receipts. Statement-bound canonical liveness now
  rejects parameters and locals whose effective policy forbids suspension when
  they remain live across a direct or transitive `Suspend` call. Field-segment
  liveness also tracks attached-data fields and compatibility machine-owned
  cells through reachable state transitions without collapsing them into
  whole-`self`; effect, borrow, flow, and contract analyses join calls by the
  shared `(state, statement, ordinal)` identity. Intra-statement checking keeps
  that preorder identity while applying left-to-right evaluation: call
  arguments count as live during the call, and later operands cross an earlier
  nested suspending call. Call-carried generic parameters read the target
  declaration's normalized carry bounds rather than a same-spelled caller
  parameter. The legacy `Machine::contains` carrier has no source parser and
  must be deliberately retired or reintroduced before subtree carry semantics
  have a real customer. Continue with admitted and sealed per-mint facts,
  activation-demand joins against pessimistic admitted runtime behavior, and
  diagnostic and model-export consumers. Checked builds now emit
  `05_carry_manifest.json`, keeping authored minimums separate from effective
  derived policies with all four axes structured.
- **CML4 — finish multiplicity migration.** Remove downstream dependence on
  legacy move/drop arenas, cover remaining ownership forms and per-field debt,
  and lower semantic permission events into explicit backend transfers. Do not
  infer establishment from zero storage.
- **TR2b — transactional outcomes.** Preserve substituted linear debt through
  `Returned(T)` and `Rejected(Arguments)` rather than laundering it through an
  unconstrained generic payload.
- **TR3 — activation plans.** The normalized `omega-task-plans` candidate and
  validator are live for contract/entry/calling-plan IDs, argument/outcome
  layouts, continuation size/alignment, cancellation, distinct-versus-inline
  execution, local suspension safety, and separate safe-point/asynchronous
  migration-demand envelopes. Connect `runtime.start<M>(args)` elaboration,
  canonical liveness/carry derivation, and effect metadata.
- **TR4 — runtime requirement and admission.** The normalized demand/behavior
  join is live: provider storage/capacity, cancellation, inline behavior,
  preemption granularity, CPU/thread migration, and continuation movement fail
  closed against the activation plan; unknown runtimes are pessimistic. Add the
  `TaskRuntime` boundary requirement/provider-plan integration and ensure a
  rejected transactional start returns every moved argument and lease.
- **TR5 — custody and storage leases.** Track provider provenance and dependent
  child storage so close/reclaim rejects while claims remain live.
- **TR6 — continuations and first provider.** Lower continuations; admit inline
  completion only when the pinned contract permits it.
- **TR7 — suspension-safe loans.** Enforce the conservative moved/shared-
  immutable/synchronized subset and integrate carry checking.
- **TR8 — reference packages.** Build `ArenaTaskPool`, bounded mailbox, and
  supervisor packages, then migrate samples. Package ergonomics do not justify
  new core syntax without a semantic impossibility.
- **Allocator migration.** Replace ambient legacy `alloc` with explicit
  `Arena`/`Allocation` contracts and migrate Cathedral's obsolete bootstrap
  `Region`/`mint_region` carrier to `Extent`. Structural multiplicity, not a
  permanent semantic ban, governs debt-bearing `Allocation<T>`.
- **Vec and slices.** Implement owned dynamic `Vec<T>` storage plus
  `as_slice`/`as_mut_slice` over real allocation/extents.

### Mathematical and float libraries

- **N6 — quotients.** Implement the settled
  `data Real = CauchySeq % converges_together` type expression, carrier-only
  `as` construction, respect/congruence obligations, and equivalence laws.
- **N7 — nested schemas.** Support proof data parameterized by machines and
  machine-parameter signatures that themselves take machine parameters.
- **N8 — construction corpus.** Build Cauchy Real, order, completeness, and
  well-definedness, retiring axioms through the normal boundary-upgrade path.
- **F6 — total float order.** Add named `TotalOrder` satisfiers for f32/f64
  using sign-magnitude integer comparison once satisfier dispatch serves.
- **F7 — float format providers.** `FloatFormat::BINARY32` and
  `FloatFormat::BINARY64` now live in `omega::core` as ordinary semantic data.
  Replace the hardcoded IEEE lowering bootstrap with checked target
  conformances, derived provider plans, and checked assembly; there is no
  instruction-binding compatibility path.

## Layout, memory, and artifact foundation

- **L4/L5 — plan-laid views.** Derive projection over plan-laid byte views,
  complete non-scalar and mutable recast views, validate tiling beyond
  fact-free shapes, enforce validate/materialize mint exclusivity, and prove
  codec conformance through ordinary policy machines.
- **L6a — Extent.** The normalized conservation foundation is live in
  `omega-extents`: admitted one-shot root grants mint nonempty ranges;
  move-split preserves exact geometry; only compatible siblings from one
  split lineage merge; attenuation cannot add open-set rights; failed
  consuming operations return their authority; and one borrow-carrying loan
  derives shared/exclusive polarity from its parent. Fixed-destination mapping
  now consumes virtual authority while independently owning, shared-borrowing,
  or exclusive-borrowing its source; unmap returns reusable ranges only after
  an exact provider receipt releases stale translations and establishes its
  open completion facts. Connect these models to the opaque Omega `[linear]`
  carrier, sealed fact establishment, provider execution/effects, and source
  APIs.
- **L6b — AccessPlan and placed views.** The separate normalized validator is
  live: name-keyed entries pin exact transfer width, stable/external/atomic
  observation, ordinary and atomic permissions, exported versus
  provider-private access, and static service reach. Validation checks fixed
  layout geometry, rejects multi-container one-access laundering and public
  external RMW, and enforces borrow polarity at operation authorization. Add
  the Omega-authored policy surface, source-level borrow-carrying access
  values, and exact external/atomic lowering. Provider-admitted placed-view
  grants now check an actual Extent loan's space, provenance, open-set rights,
  size, and permitted static reaches; field authorization derives polarity
  from that loan and mints the only token primitive lowering may accept. Never
  expose arbitrary-offset access or per-access revocation probes.
- **L6c — symbolic materializer.** The normalized source/action plan and
  loader-versus-post-handoff validation are live. Range/alignment/phase/regime/
  installation-scope constraints are normalized, concrete-site validated, and
  bound through decoded artifact construction, admission evidence, placement,
  and materialization without permitting constraint substitution. Entry-source
  integration now reaches canonical executable entry-set decoding and
  admission-bound sealed entry targets. Exact installed code now resolves those
  targets privately while executing the atomic writer. Lower the normalized
  provider-resolved post-handoff writer programs to generated machine code
  (OWNER-BLOCKED: `OWNER_QUESTIONS.md` section 3).
  Writer programs already validate their
  concrete site, resolve each sealed target once, stage all writes, and publish
  atomically. Native whole-pointer actions already lower into section-qualified
  object relocations with materialization provenance.
- **External loans.** The normalized `omega-extents` model is live: a token
  borrow-carries the real Extent loan; device-read requires shared polarity;
  device-write requires exclusive polarity; admitted grants pin borrower,
  space, provenance, open-set rights, and an open set of completion facts; an
  exact provider receipt must establish borrower release plus every required
  fence/cache/provider fact. Connect it to Omega linearity/permission contexts
  and provider execution, then build the DMA slice. Bidirectional sharing
  remains an explicit atomic/coherence protocol, not ordinary lending.
- **EXI1–EXI5 — admitted executable installation.** The normalized
  `omega-executable-installation` ladder is live: immutable artifacts gain a
  reusable sealed admission only from exact evidence; one-shot extent-backed
  placement authority advances through frozen and exact-final-byte validated
  states; installation consumes artifact/placement/scope/audience-specific
  authority plus synchronous visibility evidence; W^X enforcement is reported;
  and every failed linear transition returns its inputs. The normalized container
  validator is live over checked-layout decode output: bounds and range
  arithmetic are checked, semantic sections are exact and non-overlapping,
  unknown required sections reject, and unknown optional sections remain
  informational with zero admission authority. Connect it to actual
  schema/layout byte decoding and the closed relocation validator; implement
  admission/PCC and final-footprint validators, materializer/installer
  providers, Omega linear integration, and provider-backed
  quiescence/replacement execution. Code-placement claims already validate the
  actual Extent base/length against normalized range, alignment, phase, regime,
  and installation-scope constraints before materialization. The normalized
  retirement path already distinguishes visibility from quiescence, requires
  X removal and write-authority restoration, and returns the exact placement
  for reuse only after an exact scoped receipt. PE/COFF remains only a firmware
  envelope; no arbitrary byte-to-code path exists.
- **Wire runtime.** Implement runtime layout for wire values, additional
  encoding families, compatibility reports, and version negotiation.

## Remaining language surfaces

- **Lifetimes.** Implement the decision-15 `'name` lifetime arc and borrow-
  carrying data needed by placed views and task storage.
- **Const data parameters.** Literal and scoped named-integer-const arguments
  now parse in generic type position, validate against the declared integer
  kind/range, and substitute into fixed-array layout, descriptors, runtime
  storage, and interpreter
  defaults; differential canaries pin direct and same-name, symbol-resolved
  forwarded const arguments through indexed storage. Const-specialized plain
  records now give distinct literal instances independent layout identity, and
  parameter-free attached mutating/value methods clone and dispatch by exact
  specialization. Attached methods whose same-name const parameters are covered
  by the container now specialize const-sized signatures and bare const values
  in executable bodies for each instance. Symbolic fixed-array lengths now
  reject undeclared names, ordinary type parameters, and non-integer const
  parameters instead of degrading to an unknown/default layout. Closed integer
  expressions in const generic arguments now fold across the signed/unsigned
  64-bit envelope with ordinary arithmetic precedence and grouping; non-negative
  values also support shifts and bitwise operations. Signed arguments retain
  their negative values through instance naming, fact discharge, attached
  methods, and both runtimes, while each const parameter's declared integer
  width rejects out-of-range values. Native and interpreter canaries pin
  distinct expression-derived layouts and signed specialization. Arithmetic expressions may
  now also use scoped literal integer const operands; the transient expression
  representation is eliminated before symbol resolution, and unknown symbolic
  operands reject loudly. Expressions over forwarded const parameters now
  evaluate for each concrete clone while the surviving generic template keeps
  a validated const-parameter dependency placeholder for its ordinary kind/type
  checks. Zero-argument machine calls in const-generic position now reuse the
  fixed-array const evaluator's typed transitive-purity gate, interpreter fuel,
  and target integer semantics before instance synthesis; call leaves compose
  with the checked arithmetic expression fold, while parameterized and
  effectful calls reject loudly. Boolean `where` facts whose operands are all
  const-bound now discharge once per synthesized instance (false instances
  reject); mixed field/const facts remain standing default-domain facts with
  their const operands specialized. Const-parameter membership in an integer
  domain defined by boolean `self` facts and/or evaluable machine-backed facts
  also discharges per instance; false membership rejects. Closed const-argument
  shifts and bitwise operations now wait for the parameter declaration and use
  its exact signed/unsigned width, including arithmetic signed right shift and
  overflow/range rejection. Const membership now evaluates the ordinary
  domain-body fact list directly, retaining inferred transitive-effect and
  signature checks, checked integer operands, logical negation, nested
  memberships, nested direct machine-backed facts, and conservative cycle
  handling. Continue with arithmetic-domain semantics and richer build-time
  fact operands.
- **Trait defaults (authored bodies complete).** Standalone data conformances synthesize a
  missing attached machine from the trait's authored body before resolution,
  including defaults inherited through `requires` and header parents. Ordinary
  calls dispatch in both engines, written methods override defaults, bodyless
  child declarations suppress inherited bodies, and conflicting inherited
  bodies require an override. Direct generic conformances retain and validate
  their explicit arguments, and concrete bindings compose through generic
  header parents before substitution into the synthesized signature and body.
  Reflection-driven trait generators remain under build-time evaluation below.
  Do not restore a `default` keyword.
- **Dynamic traits (OWNER-BLOCKED: `OWNER_QUESTIONS.md` section 5).**
  Closed-world parameter calls currently specialize per concrete call site.
  Runtime-varying construction/storage, descriptors carrying satisfier
  identity, vtable emission, true indirect dispatch, and object-safety await
  the runtime representation, ownership, ABI, and admissible-signature
  decisions recorded there.
- **Equatable synthesis.** A declared conformance now emits a callable
  compiler-owned `Type::equals` wrapper over the same structural expansion as
  `==`/`!=`; a written implementation still wins. Generalize this closed core
  privilege through build-time trait generators below.
- **Build-time evaluation.** Add compile-time evaluation and trait generators
  for effect-free machines in value/refinement position.
- **Separate compilation and component artifacts.** Normalize imports, pinned
  contracts, provider selections, artifact identities, and replacement
  certificates without hashing private implementation witnesses into public
  identity.
- **Hot swap.** Implement liveness pins, quiescence proofs, and borrows as swap
  barriers through packages and admitted runtime operations; add no `replace`
  syntax.
- **Serialized capabilities.** Implement attenuation and revocation across
  boundaries.
- **Text domains and String retirement.** Establish `Utf8`/`NoNul` over
  `[u8]`, add the compile-time/runtime mint paths and loop-invariant proofs,
  migrate the corpus, then delete builtin `string`/`String` and backend
  special cases. Follow `wiki/architecture/string_retirement_execution.md`.
- **Atomics remainder.** The closed ordering vocabulary and operation-specific
  legality rules now reject release-bearing loads, acquire-bearing stores,
  unknown names, and compare-exchange failure orderings that release or exceed
  the success ordering.
  Load/store/fetch_add/fetch_sub/fetch_xor/fetch_or/swap/compare_exchange now
  preserve their normalized order through target lowering on x86_64 and
  aarch64; RMW
  returns come from the atomic instruction itself, never a racing ordinary
  read, and the interpreter preserves that same returned-prior contract. Swap
  lowers to implicitly locked `XCHG` on x86_64 and ordering-selected LSE `SWP`
  on aarch64 without a synthetic arithmetic-domain obligation. Fetch-sub uses
  exact-width two's-complement negation plus one locked `XADD`/ordered `LDADD`,
  with native, interpreter, and ARM64 byte verification.
  Fetch-xor uses ordering-selected `LDEOR` on ARM64 and a locked `CMPXCHG`
  retry loop on x86_64; its returned prior is taken from the successful atomic
  attempt, with native, interpreter, and exact-encoding coverage.
  Fetch-or uses ordering-selected `LDSET` on ARM64 and the shared locked retry
  loop on x86_64, with the same native, interpreter, and encoding coverage.
  Complete standalone fences and the remaining fetch operations, and the
  cross-activation proof model beyond these first operations.
- **Proof engine.** Continue induction and proof-data support required by
  layouts, quotients, and Real.

## Vertical acceptance slices

- **Termination firewall.** Pin one public `terminates` requirement inherited
  by acyclic and cyclic providers; swap descending and bounded-increasing
  witnesses without changing caller/import-slot identity; reject runtime
  non-tail lowering and ungranted progress profiles.
- **Kinded effects.** Demonstrate separate service reach and `Suspend`/`Block`
  members, recursive inference, public-ceiling failures, provider subset
  admission, and stable normalized IDs independent of prover strength.
- **Units.** Before broad generic work, implement two units in one dimension
  and pin: explicit conversion, scaled dimensionless results, distinct
  Energy/Torque kinds, generic preservation, no silent forgetting, and package
  coherence for operator tuples.
- **OS gauntlet.** Validate the foundation against UART/MMIO, page tables,
  DMA, shared-page IPC, IDT/timer entry, and SMP AP bringup. A customer that
  needs a new keyword or customer-shaped primitive returns to design review.

## Owner-blocked

- **CFI3–CFI5 — protected returns and final CFI.** The forward edge can proceed
  with sealed entry references and descriptor identity. Protected returns,
  continuation/exception/interrupt preservation, final indirect-site
  certificates, and foreign-provider isolation/receipts wait on
  `OWNER_QUESTIONS.md`. Executable installation prevents injection; it does not
  prove legal control transfer.

## Platform-gated verification

- **Linux hosts.** Run filesystem/time structural rows on real x86_64 and
  AArch64 Linux. `clock_gettime` additionally needs composite `timespec`
  lowering before it can be verified.
- **macOS/x86 and other unavailable hosts.** Keep target emission structurally
  pinned; do not claim runtime verification without the host.
- **Windows GUI callback entry.** Implement WndProc inbound entry stubs and a
  real title-bar close path using the general entry-plan work above.

## Deferred until a real customer

- Richer measured-recursion guards and multi-subject lexicographic cycles.
- Reduced-Rat divisibility theory beyond what N5/N6 demands.
- Async extent revocation beyond provider quiescence.
- Non-blocking executable-visibility tokens.
- Runtime-generated host code/JIT and arbitrary self-modifying code remain
  intentionally unsupported, not backlog items.
- Universe levels wait for a full-mathlib replay goal.
- A serious SSA/register-allocation/SIMD backend is post-1.0; correctness of
  current native output remains the active bar.
