> `OWNER_QUESTIONS.md` contains unresolved owner decisions only. Settled
> language rulings live in the guide and design briefs; this file tracks open
> engineering work only. Completed work belongs in git history and canary
> headers.

# Tasks

Last pruned: 2026-07-24.

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
fence, and interrupt-mask operations. `iretq`, `sysret`/`sysretq`, `eret`, and
the `lidt` contract are deriver-only. `lidt` requires distinct `IdtControl` and
the private descriptor through deriver scratch R10. Its generated-only
target/machine carrier retains the exact materialized-table, destination,
content, root-ledger, and control identities, records R10 plus control state as
its footprint, emits pinned `41 0f 01 1a` bytes, and rejects on AArch64. User
spelling rejects before operand lowering; only a provider holding the sealed
record-before-reachability preparation proof may request compiler lowering.

Cathedral now authors the real 16-byte x86 interrupt-gate schema through this
same programmable-layout path. One logical 64-bit entry identity tiles the
low/middle/high hardware fields through three validated `Bits` placements;
selector, bounded IST, type attributes, and zero-reserved fields occupy their
dictated offsets. The policy is pure description data and grants no resolver,
root, materialization, publication, or `IdtControl` authority.

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
ceiling derived from the ABI volatile-register classes plus caller-volatile
condition flags, so honest body/dispatch evidence can include comparisons
without claiming protected interrupted state.

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
   and `max(a, b)` as exit 70. Dispatch-local initialization now consumes an
   bare assignment value-call's already-materialized result before considering
   the copied initializer expression, preserving single evaluation and
   preventing a bare callee body from being rebound against unrelated
   caller-frame slots. Compound initializers still lower the full expression;
   their call-result slots are operands rather than replacements for the
   surrounding arithmetic.
   Authored scalar helper calls in entry-terminal position already preserve the
   callee's native result register through entry termination; the former
   compile-only free-standing `add_i32(3, 4)` canary now executes and pins exit 7.
   Native aggregate/slice admission is settled: a policy classifies a public
   semantic shape only when that shape determines every ABI fact; otherwise the
   native leaf declares the foreign API's actual pointer/length/terminator/record
   structure. The compiler-side default-policy slice now recursively classifies
   direct fixed arrays and records under the bundled Microsoft x64, SysV AMD64,
   and AAPCS64 rules, including homogeneous-float/SSE cases; it no longer falls
   back to passing an arbitrary by-value place by address. Default-native leaves
   reject safe slice, text/bounded-text, and vector carriers (including carriers
   nested in public records), while an explicit custom `Calling<C>` relationship
   remains the canonical-representation escape hatch. Cross-target canaries pin
   `[u8; 16]` and `[f32; 4]` as direct aggregates and `&[u8; 16]` as the distinct
   pointer form. Authored policies now receive a bounded normalized
   `BoundarySignature` graph: parameter/result roots index recursive `ValueShape`
   nodes, fixed arrays name their element root and count, and records name
   `ValueField` ranges carrying child roots and exact byte offsets. Policy output
   uses the separate closed `AbiValueShape` vocabulary, and compiler validation
   checks its MS-x64, SysV AMD64, or AAPCS64 classification against the public
   graph before accepting the plan. The remaining retention check is
   **OWNER-BLOCKED (#14)**: checked adapters must not let a call-scoped borrowed-out
   pointer reach a retaining/asynchronous foreign contract without a pinned
   loan, ownership transfer, or registration protocol, but the governing
   extern-boundary brief still explicitly leaves callback registration,
   revocation, and long-lived foreign borrows open. Checked IR therefore has no
   normalized contract axis that distinguishes call-scoped borrowing from
   retention or identifies which protocol discharges it. Resume this check only
   after that foreign-lifetime contract is settled; do not infer retention from
  suspension, blocking, ABI shape, or an `addr` value. Canary evidence already
   pins the settled surrounding distinctions: under Microsoft x64, separate
   pointer and length scalars consume two argument positions while a 16-byte
   descriptor is one indirect argument backed by a caller copy; predicate-only
   `Utf8` may be forgotten at a raw-byte adapter and must be re-established by
   explicit validation on inbound bytes; and an ordinary process entry returns
   its selected requirement's scalar exit status (`runtime_entry_return_field_exit`
   executes as status 200 without `exit_process`).
   Byte size and the compiler's private slice carrier must never silently define
   public ABI.
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
   interpreter as exit 70. Machine statement calls now use the same evaluation
   order across dispatched body segments: nested call arguments execute in the
   pre-call segment, and authored call identity survives inline arm resolution
   so the outer parameter materializer reads the completed result slot. A
   string descriptor passed through a struct-field slice alias pins this path.
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
   Cathedral's first x86 policy is settled: dedicated ISTs for double fault,
   NMI, and machine check; one shared per-CPU IST stack class for mutually
   non-nesting maskable external roots; interrupt gates keep IF clear until
   deriver-owned exit; all GPRs are saved initially; final code is transitively
   SIMD/x87-free; and acknowledgement stays protocol-neutral over PIC/LAPIC
   providers. Implement that policy through the normalized plan rather than a
   source-level interrupt special case.
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
   result placement plus call/return control. Independently derived footprint
   fragments now compose through deterministic register/state union, validate
   once against the retained entry plan, and retain an order- and duplicate-
   insensitive implementation-evidence fingerprint. Validated inbound-storage
   fragments now survive instruction selection and abstract-operation lowering
   with explicit `entry_storage` provenance. The special bytes-handoff slice
   descriptor now contributes its own `entry_slice_descriptor` fragment from
   encoder-owned x86-64/AArch64 scratch declarations, closing the previously
   omitted half of that entry prologue. Direct exit-result materialization now
   contributes `exit_result_registers` evidence as well: immediate writes and
   runtime-storage loads use encoder-owned result/base/large-offset scratch sets
   and validate against the same complete entry plan. Structurally identified
   hidden-result copies now add `exit_indirect_result_copy` evidence without
   sweeping ordinary body `CopyPlaces` operations into the boundary slice;
   x86-64 shared-base and AArch64 pointee-copy scratch identities remain owned
   beside their encoders. The backend publishes these fragments and their
   composed fingerprint in
   `08_boundary_footprints.json`, whose
   `boundary_contract_fingerprint` binds every fragment to the exact canonical
   validated plan under which it was checked; retention rejects cross-contract
   composition, while the implementation evidence remains outside requirement
   identity. Fixed ordinary `CallReturn` mechanics now contribute a separate
   `call_return_mechanics` fragment: AArch64 records its fixed frame prologue
   plus x19-x30, SP, and control restoration. The x86-64 path now also has a
   fixed 64-byte frame that preserves the union of generated-code nonvolatile
   GPRs under SysV AMD64 and Microsoft x64; incoming stack unmarshalling
   includes that frame bias, and the mechanics evidence records the complete
   restoration.
   Provenance-aware validation admits those
   prescribed stack/control effects without widening the handler body's
   transitive-state ceiling. Runtime-dispatching entries now also retain a
   structurally scoped `dispatch_scaffold` fragment: the target encoders own
   the exact x86-64 R12/AArch64 X28 dispatch-state writes and case-entry flag
   effects. Storage-backed static guards add a separate
   `static_guard_comparison` fragment with exact encoder-owned GPR/vector
   scratch and flag effects; storage-free and other guard-lowering shapes are
   deliberately excluded. Dedicated runtime-text literal-buffer and
   descriptor-vs-literal comparisons now add a
   `runtime_text_guard_comparison` fragment with their encoder-owned base,
   pointer, length, loop, byte-scratch, large-offset, and flag effects. Exact
   x86-64/AArch64 artifact canaries exercise the literal-buffer path. The
   place-pair and place-vs-immediate guard families now add a
   `place_guard_comparison` fragment as well: x86-64 covers the complete place
   walk plus integer/vector compare scratch, while AArch64 covers the direct
   shapes it currently admits, including offset-dependent address scratch.
   Cross-target artifacts exercise the place-pair path. The
   recursive `CompareRuntimeValues` family now contributes
   `runtime_value_guard_comparison`: each ISA owns a closed may-write ceiling
   for its operand evaluator, and x86 derives balanced push/pop SP use only
   when the operand tree actually contains a nested binary. That stack scratch
   is admitted only for an ordinary call-return activation. Cross-target
   artifacts exercise text-equality value operands. The semantic boundary
   carrier is now canonical from abstract operations through target operations,
   assigned operations, machine instructions, and encoded machine bytes.
   `08_boundary_footprints.json` reads that post-emission carrier directly and
   records `evidence_stage: encoded_machine`, rather than republishing an
   earlier selection-plan copy. The artifact's `enumeration_complete: false`
   firewall prevents this partial slice from being mistaken for the final
   certificate. Final-image construction now also retains a typed executable-
   region inventory: object function spans and format-generated PE/Mach-O
   import thunks resolve to their placed addresses, invalid overlap/out-of-
   bounds regions reject, and placement retains any unclassified `.text` gaps
   explicitly for the checked-emission gate to reject. Whole-text and
   per-region/gap byte fingerprints bind the inventory to the exact
   post-relocation bytes. That inventory names
   the same boundary-contract and composed implementation-evidence fingerprints
   as the encoded carrier, plus a combined boundary/placement binding identity.
   Format-owned import thunks now validate their exact final opcode shapes after
   patching and relocation before placement: PE `jmp [rip+disp32]` records only
   instruction-pointer effects, while Mach-O `ADRP/LDR/BR X16` records X16 plus
   instruction-pointer effects. Mutated encodings reject, and the per-region
   evidence is published in the final inventory and covered by its fingerprint.
   The composed encoded-machine evidence now attaches to exactly one placed
   compiler entry-function region whenever a boundary contract is retained;
   final inventory emission rejects a missing or duplicate entry-symbol match,
   and the typed inventory fingerprint covers that exact association.
   Direct-image emission now validates the encoder-owned fixed entry prologue
   and return epilogue against the exact relocated compiler entry bytes on both
   architectures before publication; mutated final mechanics reject.
   It also proves the relocated compiler-authored `.text` prefix differs from
   encoded-machine bytes only within declared relocation bitfields, preserving
   AArch64 opcode and register bits rather than treating whole instructions as
   mutable; format-owned thunk tails retain their separate exact validators.
   The encoded, final-prefix, and canonical relocation-envelope identities now
   compose into published compiler-text derivation evidence, and that identity
   participates in the boundary/placement binding fingerprint.
   Checked-assembly instructions with closed encoder-owned encodings (`hlt`,
   x86 fences, and `cli`/`sti`) now retain typed instruction boundaries
   through encoded machine code. Immediate-port x86 `in`/`out` now retain the
   normalized port identity as well: direct-image emission independently
   validates the final DX/AL privileged-opcode envelope, and `in` additionally
   requires the exact compiler-owned destination relocation and store shape.
   Immediate-index `rdmsr`/`wrmsr` retain the normalized MSR identity and
   validate their final index setup, privileged opcode, result-combine or
   split-value envelope, and destination relocation where applicable.
   Structured CR0/CR2/CR3/CR4 reads and writes likewise retain the selected
   control-register identity, validate the exact ModRM/opcode envelope, and
   require the compiler-owned destination relocation for reads.
   The compiler-text derivation evidence binds the validated instruction count
   and final-byte fingerprint. This is a deliberately narrow first
   Stack-balanced RFLAGS snapshot/restore sequences now validate their exact
   flags opcodes and balanced register envelope; snapshots additionally bind
   the compiler-owned destination relocation. Runtime port and MSR-index forms
   now retain the exact encoded widths of their runtime operands, independently
   validate the fixed DX/AL or ECX/EDX:EAX privileged-opcode skeleton at those
   boundaries, and bind compiler-owned destination relocations for reads.
   Immediate-port and immediate-MSR writes likewise bind the exact value-loader
   boundary, while control-register writes and RFLAGS restore bind the exact
   source-loader boundary; none may hide extra unclassified bytes between its
   operand and privileged opcode. Machine emission now fails closed if any
   user-checked catalog instruction lacks that final-image validation evidence;
   an unsupported operand shape cannot silently fall back to ordinary bytes.
   Checked-assembly value operands now retain independent leaf-loader
   semantics for immediate and direct-storage forms: exact operand subspans,
   R10/R11 destination, constant value or storage displacement/width, and the
   source-storage relocation enter final-image validation and its fingerprint.
   Mutating an immediate, load opcode, displacement/register form, or exact
   relocation rejects even when the surrounding privileged opcode envelope
   remains intact. Pointee and constant frame-indexed loaders now retain their
   separate semantic identities, exact pointer/value displacements, widths,
   destination registers, and frame relocation; even byte-identical lowered
   shapes fingerprint differently when their source semantics differ.
   Frame-base-indexed loaders additionally bind the unsigned index load,
   element scale, address formation, and value projection. Descriptor-frame
   indexing also binds whether its index comes from the frame or machine
   storage, including the exact second relocation for cross-region access.
   Machine-indexed loaders now receive the symmetric treatment for machine
   bases and optional frame-resident indexes. Complete semantic decoding of
   text, conversion, and recursive binary value operands, other operand-bearing
   checked assembly, and general compiler-function body decoding remain
   incomplete.
   Checked direct-image emission now rejects any unclassified executable gap,
   so the current closed emitter has complete region enumeration. Relaxation
   products, veneers, and general generated stubs are recorded as absent by
   construction rather than falsely listed as emitted-but-unchecked classes;
   adding any unclassified bytes fails before publication.
   It names its currently covered and missing classes and likewise refuses to
   claim complete enumeration.
   StatePlan-driven nonordinary save/restore and return specialization, and
   final-byte footprint decoding for compiler-function bodies and handler
   regions, and admitted leaves remain; footprint enumeration is still incomplete even
   though final executable-region enumeration is now complete.
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
   publication, and never exposes the numeric address API. The writer boundary
   is settled: lower each normalized program as a compiler-generated checked
   Omega machine over one exclusive unpublished mapped/pinned/writable
   placement and a sealed exact-artifact resolver. It writes directly into
   that unpublished destination; failure leaves it unpublishable rather than
   requiring a transactional staging buffer. Validate the completed bytes and
   the software-fault-free bootstrap obligations before minting
   `MaterializedIdt`. The separate installer holds `IdtControl`, records
   prepared roots before checked `lidt` makes them reachable, and emits a
   distinct installation receipt. The normalized record-then-publish gate is
   now live: `MaterializedIdt` requires its symbolic writer entry targets to
   exactly equal its root bindings, publication requires the matching live
   handles and ledger records plus a content/ledger-bound success receipt, and
   `InstalledIdt` retains those liveness pins. The post-handoff executor now
   validates and resolves every write before mutating the exclusive unpublished
   destination directly (no transactional staging copy). IDT materialization
   consumes mapped/pinned/writable unpublished storage, executes only through
   the exact installed-artifact entry resolver, computes identity from the
   resulting bytes, and requires an exact code/artifact/destination/final-byte
   receipt plus the software-fault-free bootstrap verdict. Failure returns the
   still-unpublished destination and all consumed inputs. The checked
   publication-operation carrier is now live: `prepare_idt_load` first proves
   the exact live handles and ledger records, binds the unpublished descriptor
   destination/content and `IdtControl`, and only that sealed proof can enter
   compiler lowering. The generated target/machine operation retains those
   facts, carries the exact R10 + control-state footprint, emits only pinned
   x86 `lidt [r10]`, and rejects on AArch64; source spelling still rejects
   before operand lowering. Writer preparation and checked lowering are now
   live too: one sealed `PreparedIdtWriter` owns the exact unpublished
   destination, normalized plan, and root set after validating placement,
   writability/pinning/mapping, fragment geometry, and every source against the
   exact installed artifact. Its deterministic identity binds code, artifact,
   destination and its initial content, placement, plan, and roots. Compiler
   lowering retains those facts plus address-free fragment geometry and
   provider-private source-slot indices in generated-only target/machine
   carriers; foreign entries,
   phase/destination drift, root drift, and source construction all reject.
   The provider-private `IDTWRIT1` context ABI is now pinned: R10 addresses one
   packed destination pointer followed by dense u64 source slots. Exact x86
   emission and width are live with the derived RAX/RCX/RDX/R11 plus Flags
   footprint; AArch64, unknown ABI versions, sparse/out-of-range slots, and
   unrepresentable fragment geometry reject before emission. Context population
   is live too: a non-clonable opaque `PopulatedIdtWriter` consumes the
   preparation, resolves each distinct source once through the exact installed
   realization, binds the packed destination/source words by identity and
   fingerprint, and omits them from diagnostics and public accessors. Lowering
   and materialization now require that populated seal; plan/code/site/context
   drift rejects before mutation, and semantic completion uses those exact
   sealed values without re-resolution. Both generated helpers now consume a
   validated concrete one-private-pointer call/return plan, retain its exact
   GPR placement, and emit that register-to-R10 materialization before the
   writer or `lidt [r10]`; Microsoft x64 selects RCX and SysV AMD64 selects RDI,
   while extra parameters, stack/fragmented pointers, results, wrong control,
   architecture drift, and a footprint beyond the plan's state ceiling reject.
   `PreparedIdtLoad` also owns the exact private
   packed 10-byte x86 descriptor (table-byte limit plus placed base) and exposes
   only its content-bound fingerprint. Insertion into Cathedral's initial image
   is **OWNER-BLOCKED** on `OWNER_QUESTIONS.md` under "bootstrap provider
   helpers": the current lowering consumes runtime populated/prepared seals
   which cannot exist until after the artifact containing the helper has been
   admitted and placed. Once template-versus-invocation staging is settled,
   insert and execute both emitted helpers through opaque provider inputs and
   finalize their materialization/installation receipts.
5. **IDT2 — installed-root ledger.** The normalized `omega-external-roots`
   foundation is live. It admits only an entry present in the exact
   installed artifact; consumes owner-scoped slot authority; and records
   provider/effect/trust, nesting, acknowledgement, resources, and
   component-version pins without exposing a numeric code address. Its linear
   root handle borrows the installed code as a liveness pin. Removal returns
   the slot only after an exact receipt proves both entry unreachability and
   execution quiescence, while failed install/remove operations return every
   consumed authority. The live ledger now has a deterministic report
   fingerprint over exact root/realization/slot/admission bindings, and
   `omega-artifacts` emits `external_roots.json` directly from that ledger.
   The record/report now has three independently validated columns: stack
   ceiling plus sealed artifact-wide WCSU composition, structural-work ceiling
   plus canonical transitive fixed-work demand, and `StatePlan` ceiling plus
   realized final footprint. Each retains normalized validation receipts while
   excluding private ranking and codegen proofs. Fixed-work summaries fail
   closed on missing providers, cycles, zero multiplicities, arithmetic
   overflow, and a final demand above the root ceiling; they prove finite
   structural work, not WCET. Stack composition consumes provider-local demand,
   the exact normalized `EntryStack`, and one complete nesting relation;
   interrupted-stack entries add with alignment, distinct dedicated classes
   switch domains, sequential roots sharing a class combine by maximum, and
   cycles, missing endpoints, unknown nested provider-selected stacks,
   arithmetic overflow, or re-entry of an active dedicated class reject. The
   ledger requires every installed root to carry the same artifact-wide
   composition fingerprint. A sealed `ProviderExecution` now binds the selected
   normalized provider-plan identity, exact entry/boundary/effect set, and all
   three independent realizations into root admission; replay after entry or
   resource drift rejects, and the execution/plan identities enter the ledger
   fingerprint and manifest. The compiler now retains the exact validated
   selected `ProviderPlan` set through `CheckedTrees`, in canonical order with
   both per-plan and whole-selection identities. An external-root candidate now
   carries that exact plan identity before validation; it enters the normalized
   root fingerprint, `ProviderExecution` derives it from the validated root
   instead of accepting a late independent ID, and the compiler resolves the
   boundary slot only from its retained selected-plan set. Missing or ambiguous
   retained slots reject. A source-level timer acceptance test now authors the
   complete x86 `InterruptReturn` policy with dedicated stack class, masked
   preemption, and exact saved/restored state; the same plan fingerprint
   survives the boundary schema, selected provider plan, and external-root
   bridge rather than being reconstructed from a Rust fixture. Cathedral's
   acceptance canaries pin both the five-node fixed timer work graph and the
   shared-IRQ bound of maximum maskable-root demand plus the permitted
   current-stack fatal-fault term.
   The normalized IDT publication transition also enforces record-before-`lidt`
   ordering and retains every installed-root handle. A sealed prepared-load
   carrier now prevents the generated checked `lidt` operation from being
   constructed until that exact root/ledger/control gate succeeds. The sealed
   writer preparation and address-free generated machine carrier now likewise
   prevent a swapped plan/destination/resolver from reaching code generation.
   Concrete Cathedral PIC/LAPIC candidate construction may proceed, but
   inserting the plan-driven generated writer/load operations into the initial
   artifact is **OWNER-BLOCKED** on the bootstrap-helper staging question. R10
   materialization is emitted from the exact normalized private-pointer
   placement, the writer context is populated opaquely from the installed
   resolver, and the load preparation owns the exact private packed descriptor
   before invocation.
   The stack/IST policy must remain one fact consumed by both layout
   materialization and WCSU analysis.
6. **IDT3 — linear interrupt obligations.** The source contract is live in
   `omega::language::core::interrupt`: opaque linear `InterruptMaskGuard` and
   `InterruptAcknowledgement` values have explicit consuming `restore` and
   `complete` operations, while the distinct opaque `InterruptMaskControl`
   capability is the only source of a saved-mask guard. Their operations pin
   canonical `MachineControl` versus `PortIo` reach; construction, forgotten
   settlement, and double completion reject through ordinary opacity/effects/
   linearity rules. The normalized installed-root entry path now mints those
   opaque obligations only from an exact provider receipt bound to the root,
   slot, installed code, selected provider execution, invocation, and
   acknowledgement policy. Invocation and acknowledgement identities cannot be
   replayed. Nested mask guards retain exact prior-state identities and restore
   only in LIFO order; the deriver-owned exit rejects until the mask returns to
   its entry state and the exact acknowledgement receipt is present. Failed
   transitions return every consumed token/receipt, and a root with an active
   interrupt cannot retire. Connect the concrete Cathedral PIC/LAPIC entry
   implementation to these normalized transitions. Do not use drop cleanup or
   interrupt-specific linearity rules.
7. **Cathedral exception-IDT and timer acceptance.**
   - Materialize a diagnostic/fatal entry for every architecturally defined
     exception before enabling the timer. Provision distinct per-CPU ISTs for
     double fault, NMI, and machine check. Cathedral now carries a pure,
     zero-authority fact package for all 32 CPU-reserved vector slots, including
     profile-qualified AMD #HV/#VC/#SX identities and the three v1 IST vector
     selections. A second pure fact record binds the double-fault, NMI,
     machine-check, and shared maskable-IRQ analysis classes to hardware IST
     indices from one source of truth. Cathedral's core profile now composes
     the double-fault, NMI, machine-check, and remapped legacy-timer vectors
     with those exact records rather than restating stack numbers. Entry
     contracts, stubs, stack provisioning, and publication remain.
   - Provision one shared per-CPU IST stack class for all maskable external
     roots. Use interrupt gates, keep IF clear until deriver-owned exit, and
     include the maximum current-stack fatal-fault term in its WCSU.
   - Generate save-all-GPR entry stubs and reject SIMD/x87 anywhere in the final
     transitive handler footprint. Footprint-minimal GPR saves are a later
     optimization.
   - Program PIT plus remapped 8259 PIC as the first QEMU/PC provider. The hard
     root acknowledges exactly once, captures time, sets one preallocated
     coalescing wake state, and returns. An ordinary timer-service task drains
     due registrations and rearms the next deadline. Cathedral now carries
     pure, zero-authority 8259A and 8254 hardware-fact packages for the port
     map, initialization/control words, cascade wiring, masks, timer modes, and
     clock rate. Its pure x86 gate policy also pins the 16-byte descriptor and
     validated split-entry placement without acquiring installation authority.
     Cathedral also has checked `PortIo` helpers that remap both
     controllers while masked, unmask only the timer line, emit the master-PIC
     acknowledgement, and program PIT channel 0 in binary rate-generator mode
     from an explicitly supplied divisor. Invoking those helpers during
     bootstrap and connecting the acknowledgement to the installed root remain
     gated by the exception-IDT publication path; do not enable IF before that
     path is live.
   - Add LAPIC one-shot timing as the production multicore/tickless provider
     without changing the root requirement. Cathedral now carries one pure,
     zero-authority `local_apic` fact package for the architectural xAPIC MMIO
     offsets, x2APIC MSRs, EOI value, LVT timer fields, divider encodings, and
     optional TSC-deadline identities. The package intentionally asserts no
     universal LAPIC frequency. Cathedral's first checked x2APIC provider
     helpers now configure an admitted vector in one-shot/divide-by-16 mode,
     arm or stop the timer, and write EOI through parsed `wrmsr` contracts that
     retain `MachineControl` reach. They neither enable x2APIC/IF nor publish a
     root. Platform enumeration/calibration, admitted-mode establishment, and
     installed-root invocation/acknowledgement integration remain.
   - Report ticks over the owned serial line and `hlt` between ticks under QEMU.
     Negative rails: direct assembly cannot launder reach; user `iretq` rejects;
     incomplete fragment tiling rejects; forbidden final-artifact clobbers
     reject; omitted or double EOI rejects; a dynamic/recursive or unbounded
     provider leaf rejects the hard-root work profile; an open-authored
     `Layout` or `Calling<C>` policy cannot mint resolver/installation
     authority or publish a structurally valid but semantically inadmissible
     table; and a package that reaches the installer through either an
     abstraction or direct checked assembly retains the same normalized
     privileged reach in the artifact report.

### Provider plans and retirement of `provides`

Provider plans are derived from `satisfies` closure. Checked adapters have
Omega bodies; irreducible leaves use
`satisfies Requirement via <Binding>;`. Target packages provide defaults and a
slot owner may override by type. The migration order remains load-bearing.

1. **PRV4b — Console adapters.** Standard `Console::write` and
   `Console::write_line` now accept borrowed byte views directly and are checked
   Omega code: self-forwarding adapters walk that view with measured
   `Slice::Length` state transitions and reach only `write_byte`. Field-backed
   bounded carriers, literal-backed text, legacy owned-`String` compatibility,
   and empty-line cases run differentially; the checked-tree canary pins both
   calls to their adapters, and the lossless built-in plan oracle remains green.
   The obsolete adapter-internal `String -> &string -> bytes` chain is gone.
   The granted-build Console backstop and the inherited mutable-output
   host-call-plan unit now use borrowed byte views backed by explicit fixed
   carriers as well; neither test preserves a private `String` ABI fossil.
   Mutable `Console::read_line` now accepts a mutable byte view; a concrete
   `[u8; N] in D` carrier at the call site supplies the explicit bound, and
   backend planning derives `N` from that destination rather than reusing the
   legacy 256-byte String scratch limit. Carrier-input programs run through the
   same standard package in both engines and cross-compile on AArch64.
   More than 1,300 exact duplicate Console declarations now import that package.
   The compiler's dungeon lattice snapshot now shares the same standard import
   as the runnable sample instead of retaining a second String-based boundary.
   The carrier-focused `text_greeting`, `status_report`, and `text_padding` CLI
   samples now use the standard Console as well; their pause buffers are honest
   bounded Utf8 carriers, so the shared `read_line` adapter derives the concrete
   destination capacity instead of relying on a locally weakened signature.
   Eight ordinary call, collection, control-flow, and ownership pass fixtures
   now consume the same package and agree in native/interpreter execution; the
   previously unlisted fixtures are also pinned by the collect-all pass suite.
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
   and conflicting target defaults reject. Their selected full machine names
   survive the syntax-to-typed handoff exactly once (without duplicating the
   attached owner path), with default, override, and conflict canaries in the
   authoritative rosters. Test/component slot owners now use the same contract
   through an attached `Owner::build` declared at a `build.omg` root; the
   toolchain-provided `Build` vocabulary recognizes that scoped form, while the
   identical spelling in ordinary source remains unprivileged. Component, test,
   and file-authority canaries are in the authoritative rosters. Coverage,
   signature conformance, transitive
   effect refinement, normalized identity, and selected-target-only ambiguity
   are already enforced. The exact selected full plans now cross typed-to-
   checked lowering as one canonical `SelectedProviderPlanFacts` set. The
   carrier rejects missing, duplicate, partial, or identity-colliding
   selections, retains immutable normalized rows for later provider execution,
   and has a deterministic whole-selection identity; backend and generated-
   machine work no longer has to rediscover selection from authored
   `satisfies` declarations. External-root construction can resolve a boundary
   slot's `ProviderPlanId` only from that retained set and rejects absent or
   ambiguous matches.
3. **PRV4e — foreign format facts.** Move foreign offsets and bit constants
   from `Binding::Value` into programmable layout/format declarations and
   migrate filesystem leaves. Open-option flags have moved: portable code now
   supplies semantic `OpenOptions`, checked target-package machines own the
   Darwin/Linux/MSVCRT encodings, and no flag constant remains in a provider
   row. Target `open_with` implementations now copy the semantic record once
   and call their ordinary target encoder. Native lowering defers an outer
   value result until the complete nested statement-call splice has executed,
   and direct aggregate construction restores omitted fields and padding to
   their zero representation before applying named fields. Aggregate
   construction through reference and indexed destinations retains the
   composed pointee/index place while performing that whole-value zero fill,
   rather than overwriting a frame slot that carries an input reference. The
   remaining `struct stat` offsets have now moved as well: the four target filesystem
   packages author checked `StatLayout` policies, while portable copy and
   metadata decoding project semantic fields through one `StatView`. No
   `FilesystemHost::ST_*` value remains consumed. Shared record recasts may
   contain a plan-laid subrecord in both native and interpreter execution.
   Recast locals remain materialized type-bearing views instead of being
   flattened back to their byte-element initializer, including when their only
   uses are later local initializers or assignment values; straight-line nested
   value bodies also materialize the view address before projected reads.
   Stored widening from a projected plan-laid `u16` into `u64` lowers on both
   x86-64 and AArch64. The
   focused synthetic view canary pins exact non-native offsets, recast liveness,
   interpreter agreement, and cross-target conversion. PRV4e is complete; PRV4f
   has deleted the four now-unconsumed compatibility tables.
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
   Cathedral's boot image now consumes those same attached external leaves:
   `SimpleTextOutput` uses `Binding::VtableField`, while `BootServices` uses
   `Binding::TableFunction` so its dispatch-only table pointer stays off the
   foreign signature. The real seven-file UEFI package compiles without any
   source-level `provides` declaration.
   All callable canary source now uses the external-leaf surface as well:
   the closed Binding-form, ambiguity, duplicate-requirement, and unknown-case
   rails no longer depend on compatibility tables. Candidate validation
   rejects duplicate realizations inside one provider type before coverage or
   lowering, and qualified leaves cannot inherit the legacy bare-field
   shorthand. The obsolete standalone Value pass/wrong-target fixtures have
   been removed, and the qualified unresolved-path rail now depends only on
   ordinary const/case resolution. `Binding::Value`, its normalized provider
   representation, and the compiler substitution pass are now deleted; the
   remaining parser compatibility path emits a directed migration diagnostic
   for integer rows, and trust-report coverage derives plans from external
   leaves. The four target filesystem `struct stat`
   tables have now been deleted after foreign-record fact migration; a tracked-
   source invariant pins that no authored `provides` declaration remains.
   The obsolete standalone `host ... provides` syscall example is gone; the
   qualified external-leaf syscall canary already pins both x86-64 and AArch64
   admission, argument registers, syscall-number registers, and supervisor-call
   instructions.
   Compatibility deletion is complete: the `HostProvider` AST item and mapping
   arena, snapshot/copy/identity support, legacy provider-plan derivation,
   authored grammar, and `.provides.omg` loader fallback are gone. The parser
   keeps only a directed retirement diagnostic, and the valid backend carrier
   is now named `ExternalBindingRow` under an external-binding policy identity.
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
   calls, with no runtime callable, dictionary, or capture inference. Task-runtime
   `start<M>`/`try_start<M>` selection now has an authored core-import canary:
   both calls retain the concrete target, suspension reachability, and normalized
   activation identity in checked facts and the build manifest, while a target
   whose effects exceed the authored machine-parameter ceiling rejects. The
   build-time interpreter now evaluates the same specialized tree: a selected
   free helper computes PE subsystem metadata end to end, and a mismatched
   helper rejects before interpretation. Free selections retain their authored
   symbol leaf instead of leaking the internal `entry` state name. N7's first
   proof-schema consumer is live as well: recursive proof data can carry static
   machine parameters and concrete selections receive the same refinement
   judgment as calls. Higher-order machine-parameter signatures now carry
   nested authored requirements, refine binder-positionally, forward distinct
   schema parameters, and specialize nested direct invocations to a fixed point.

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
  preserves unpassed caller locals. Named boundary/operator statement calls now
  use the same exact operand-place mapping: mutable operands invalidate stale
  facts, then domain-membership `ensures` facts are instantiated back onto the
  caller place. Boolean operator postconditions now use the same general
  recursive formal-to-operand expression substitution as operator
  preconditions, retain canonical caller-term labels in semantic facts, and
  conservatively depend on every place-valued operand so mutation of either
  side invalidates the guarantee. Explicit state-arrival `requires` contracts
  are now live through syntax, resolution, typing, specialization identity,
  proof/semantic indexing, and checked flow: every named edge must prove the
  target contract, the target body assumes it, facts stay state-scoped, and a
  `self` back-edge must re-establish any arrival fact invalidated in the body.
  The monotone-counter Houdini seed now consumes the same recursive call-frame
  summaries: inferred loop bounds cross resolved sibling calls whose may-write
  paths are disjoint from the counter, while overlapping or opaque calls still
  fail closed; reserved pure value builtins have an explicit empty write frame.
  The first relational candidate is also live: when every entry and back edge
  into an increasing-counter head establishes `i < self.collection.len`, the
  checker carries that symbolic collection/index fact at the head across calls
  proven disjoint from both places. Direct collection writes and opaque or
  overlapping calls reject the candidate, and reassigning the index drops the
  fact before any later access. This is semantic guard matching across distinct
  expression handles, not a collapse of the dynamic length to a constant. The
  next stable-limit class is live as well: semantic entry/back-edge relations
  to `self.limit` compose with an authored machine relation from the limit to
  `self.collection.len`. Strictness is tracked rather than discarded, so
  `i < limit <= len` and `i <= limit < len` prove the index bound, while the
  unsound `i <= limit <= len` chain fails closed. Because the bridge originates
  at machine arrival, both limit and collection must be frame-stable across the
  whole machine (including preheaders), not only within the natural loop.
  Body-derived write frames now have one normalized checked representation:
  each state records a sorted/deduplicated complete path set or an explicit
  opaque result, normalizes non-`self` parameters positionally, and carries a
  deterministic implementation fingerprint. Call-frame path consumers derive
  through the same representation. `05_machine_contracts.json` publishes the
  frames under `implementation`; they are structurally excluded from the
  authored contract fingerprint and specialization identity, so body-only
  write changes cannot masquerade as public contract changes.
  Runtime-indexed record-field projections now retain their leaf type and
  store-enforced range through synthetic operand hoists; the same resolver
  rejects out-of-range writes through a runtime-indexed projection instead of
  losing the destination's constraint shell.
  **OWNER-BLOCKED (#15):** the boundary write-frame clause's semantics are
  settled, but its public keyword/spelling is still explicitly provisional
  (`stores` in the guide). Do not mint syntax until that spelling is frozen.
  Engineering can continue on additional relational candidate classes
  independently of the surface decision.

### Domain facets, effects, termination, and trust

- **DOM1 — facet kinds.** Resolved and typed domain declarations now carry the
  normalized predicate/semantic facet pair populated once at syntax lowering,
  copied downstream, and published in resolved/typed snapshots. Qualification
  casts, semantic commitment collection,
  introduction-authority checks, trust reports, and trust-lockfile grant
  matching consume the semantic facet; semantic-only domains no longer invent
  a predicate-proof obligation. Until authored facet syntax/policy lands, the
  compatibility projection treats factful declarations as hybrids and
  factless declarations as semantic-only. Typed `T in D` constraints now bind
  the authored short name to a
  carrier-compatible declaration after the complete typed program exists and
  retain its symbol, semantic identity, and facet pair. Nested generic
  arguments, type-table copies, checked field/contract facts, byte predicates,
  validation, and typed snapshots consume or preserve that normalized record;
  none re-resolve the short name globally. Domain conjunctions now compose per
  axis: every predicate facet synthesizes its own parameter obligation, write
  check, entry/read fact, and post-write re-establishment, while semantic-only
  facets never enter the proof lattice; implication consumers accept any
  declared predicate in the conjunction that entails the required domain.
  **OWNER-BLOCKED (#16):** authored facet declaration syntax and policy are
  still explicitly deferred in decision 19. Freeze that surface before
  retiring the factful=hybrid/factless=semantic-only compatibility projection.
- **DOM2 — binding-site operators.** Resolve operator theory from declarations,
  mints, and `requires`; never from flow facts. Resolve tuples deterministically
  and reject collisions. The checked selector now accepts no fact/flow plan and
  activates domain-owned meanings from normalized declared type constraints,
  direct/local explicit mints, and signature `requires`; call `ensures`, guards,
  and mutation-driven fact invalidation cannot reinterpret an operation.
  Checked evidence and diagnostics distinguish a selected domain meaning from
  builtin fallback. Spelled candidates now match the complete operand tuple:
  binary right operands, index operands, and both range bounds all participate,
  with one shared generic substitution across positions and no privileged
  receiver discriminator. The provisional global same-carrier collision fence
  is retired: inactive theories may coexist, candidate eligibility is anchored
  to the semantic domains selected by the participating operand bindings, and
  multiple participating meanings fail at that use site. This completes the
  closed-family coherence path. **OWNER-BLOCKED (#16):** the authored
  language has no open-operator-family declaration or designated
  dispatch-owner-position surface; decision 19 explicitly defers general
  open-family linking, so that extension cannot be lowered yet.
- **DOM3 — introduction authority.** Implement sealed-by-default domains,
  `introduction open`, and `MintAuthority<D>` with distinct missing-proof and
  missing-authority diagnostics. **OWNER-BLOCKED (#16):** decision 19
  explicitly defers the introduction-policy grammar and authority-passing cast
  spelling; the internal grant carrier exists, but authored policy cannot be
  lowered without that surface decision.
- **DOM5 — weakening.** Add `weakens_to` certificates and sealed-theory hashes
  that detect stale operator theories. **OWNER-BLOCKED (#16):** decision
  19 explicitly defers the `weakens_to` certificate-block syntax; theory
  hashing can continue once there is an authored certificate object to bind.
- **STR — semantic carrier cleanup.** Finish termination-plan integration,
  validation/resolution from normalized domain/machine/permission plans,
  lowering only from checked selections, and deletion of compatibility bools.
  Resolved and typed machines now carry no duplicate `terminates` boolean:
  `MachineTerminationPlan` is the sole downstream public-interface carrier,
  and their snapshots serialize that structured interface and guarantee.
  Resolved and typed snapshots now also serialize the complete normalized
  `MachineSupplyMode`, including the external-binding identity, instead of
  omitting supply or asking consumers to infer it from `boundary`.
  Static-machine refinement therefore accepts only an explicit
  `Published(EventualTerminal)` promise; a private ranking witness cannot
  masquerade as a public termination contract.
- **EFX — split service reach, suspension, and blocking.** Make `effects` a
  symbol-resolved row of boundary-service reach only. Add independent
  `suspends;` and `blocks;` public may-clauses; omission on a requirement or
  export is the corresponding negative guarantee, while private bodies infer
  both operational possibilities. Keep `terminates [by ...]` as the existing
  separate positive progress guarantee with premises, never an effect or may-
  clause. Carry `ServiceReachPlan`, `SuspensionPlan`, `BlockingPlan`, and
  `MachineTerminationPlan` independently through syntax, resolved, typed,
  checked, specialization, contract fingerprints, task/provider admission,
  manifests, and diagnostics. Compute recursive fixed points per may-axis;
  enforce checked behavior within public ceilings and providers within pinned
  slots. Reject retired `effects Suspend`, `effects Block`, and mixed service/
  operational rows with migration diagnostics. Migrate core, std, samples,
  canaries, and compiler/lattice fixtures atomically, then retire operational
  members from the legacy `EffectSet` semantic path. The durable core target is
  now established: `ServiceReachId`/`ServiceReachRowId` and its deterministic
  set normalizer are separate from `SuspensionPlan` and `BlockingPlan`; each
  plan distinguishes private inference from an explicit published `false`
  ceiling, so public omission cannot collapse into internal inference. Syntax,
  tree propagation, snapshots, checked machine plans, contract fingerprints,
  and task admission now carry the split axes. The parser rejects operational
  members in service rows with migration diagnostics, and the core/task/carry
  fixtures now use the independent clauses. Independent recursive boolean
  fixed points now feed checked contracts, carry checks, and task activation;
  local checked calls consume honest inferred summaries while requirements and
  boundaries consume pinned authored ceilings. Published omission and inferred
  static-provider widening reject independently on both axes. Specialization
  identity, machine-contract/task manifests, and task-plan identity carry both
  axes, and operational spellings no longer have legacy `EffectSet` bits or
  capability-audit projections. Boundary-trait declarations now mint
  canonical symbol-keyed service identities after name resolution; authored
  machine, requirement, and nested machine-parameter rows normalize through
  resolved trait symbols with boundary-parent closure. An independent
  recursive service fixed point feeds checked `ServiceReachPlan` values,
  published-ceiling diagnostics, static-machine and checked-provider
  admission, provider schema identity, contract fingerprints, tree snapshots,
  and machine/capability manifests. Ordinary policy traits cannot enter these
  rows, and artifact identity renders canonical service names rather than
  program-local row or numeric-bit IDs. Pure build-time clients now share one
  necessary admission gate over canonical effective service reach and the
  independent suspension/blocking summaries: fixed-array lengths,
  const-generic call leaves, machine-backed const-domain facts, layout plans,
  wire plans, and calling-policy plans no longer consult `EffectSet`. The
  complete build-time contract still needs its separately tracked authority,
  trust, resource, failure/control, termination, and escaping-mutation axes.
  Checked assembly now resolves compiler-known instructions to the canonical
  `MachineControl`/`PortIo` boundary-service rows, validates direct and known
  transitive asm reach without consulting `EffectSet`, and retains authority
  discharge as an independent instruction-contract obligation. The core
  assembly module, interrupt contracts, asm canaries, and Cathedral boot
  contracts author those identities; missing imports, missing direct rows, and
  helper laundering reject with normalized diagnostics and call paths. Pure
  discard classification now also consults normalized effective service reach.
  Lowercase asm bits are no longer an authored semantic input. Executable capability
  manifests now consume checked `ServiceReachPlan`, `SuspensionPlan`, and
  `BlockingPlan` values directly, publish canonical service names and the two
  independent operational booleans, and no longer expose legacy lowercase
  names or numeric effect bits. Normalized service inference now retains
  machine/state/call structure in grouped arenas backed by one shared row
  table. Checked-flow, state-graph, and control-flow nodes carry compact
  direct/transitive service-row identities plus independent suspension and
  blocking summaries; the persistent graph representations and their reports
  no longer depend on `omega-effects` or reconstruct numeric bits. The
  typed-tree report now joins the same normalized machine/state/call service
  rows with the independent operational fixed points, and the shared phase
  filter derives a deterministic canonical-service catalog from rendered
  nodes instead of the global lowercase effect-name table. Provider-plan
  method schemas and stable identities now carry canonical service names plus
  independent suspension/blocking ceilings; their redundant lowercase method
  rows and plan-wide `EffectSet` aggregate are deleted. Boundary-provider
  approval is now exact to the reached capability symbol, capability-flow
  acquisition derives from normalized call topology, primitive providers carry
  a categorical host-authority requirement, and boundary reports no longer
  render service names as authority. Static-machine refinement now compares
  exact normalized service rows, and checked trees own grouped
  `ServiceReachFacts` directly: the duplicate `EffectRowFacts` projection and
  legacy effect-row inputs/artifact field in machine-contract identity are
  deleted. The obsolete `EffectRowId`/`EffectRowTable` carrier is also gone
  from core, resolved trees, and typed trees; those stages retain only
  symbol-resolved `ServiceReachRowId` values. General validation, std, samples,
  canaries, and compiler fixtures now consume canonical service rows directly;
  the global lowercase service-name/u64 engine is deleted. Build-script admission
  now recognizes only the pinned canonical `FilesystemHost` and `Console`
  staging services; custom boundary wrappers cannot launder admission through
  a lowercase category.
  The direct-call acknowledgement keyword is a separate owner decision and
  must consume the normalized suspension/blocking plans rather than
  source-token presence.
- **TPR4/TPR6 — publication and progress profiles.** The normalized
  termination interface now distinguishes private `InternalDerived` bodies
  from public `Published(NoGuarantee | EventualTerminal)` contracts. Public
  omission on requirements/exports/providers is therefore explicit, satisfying
  implementations inherit that exact default, the distinction enters
  `MachineContractPlan` identity, and `04_machine_contract_manifest.json`
  serializes it without exposing private ranking witnesses. Resolved and typed
  compatibility booleans are retired, including static-machine selection and
  snapshot consumers. Continue with sealed profile domains, grant-backed
  admission and receipts, and pinned progress premises. Profiles are never
  flow-inferred ranking evidence.
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
  they remain live across a direct or transitively suspending call. Field-segment
  liveness also tracks attached-data fields and compatibility machine-owned
  cells through reachable state transitions without collapsing them into
  whole-`self`; effect, borrow, flow, and contract analyses join calls by the
  shared `(state, statement, ordinal)` identity. Intra-statement checking keeps
  that preorder identity while applying left-to-right evaluation: call
  arguments count as live during the call, and later operands cross an earlier
  nested suspending call. Call-carried generic parameters read the target
  declaration's normalized carry bounds rather than a same-spelled caller
  parameter. The parser-unreachable `Machine::contains` carrier is now retired
  end-to-end from resolved/typed trees and their semantic consumers. Runtime
  contained-machine topology is now canonical in checked `CarryFacts`: grouped
  machine/field/target arenas derive only from attached-data fields whose data
  type has one or more attached machines. State-graph lowering, capacity
  planning, branch-target discovery, and backend surface reports consume that
  fact instead of independently matching names. Safe-point activation demands
  join every descendant machine's suspension crossings, while asynchronous
  demands join every descendant all-instruction envelope with cycle-safe
  symbol deduplication. Continue with admitted and sealed per-mint facts,
  activation-demand joins against pessimistic admitted runtime behavior, and
  diagnostic and richer formal-model consumers. Checked builds now emit
  `05_carry_manifest.json`, keeping authored minimums separate from effective
  derived policies with all four axes structured; the artifact also exports
  each canonical safe-point crossing, its exact statement/call identity,
  target, joined policy, and typed live-value/storage set so downstream models
  never reconstruct liveness from source syntax.
- **CML4 — finish multiplicity migration.** Legacy move/drop compatibility
  arenas now terminate at control flow; abstract operations and every later
  backend representation carry only canonical permission events. A normalized,
  fail-closed realization ledger now joins runtime/direct selection sites to
  exact permission-event identity, merges folded provenance chains onto their
  selected instruction indices, and records checked no-code reasons only for
  explicit zero-code terminal consumes, events with no live debt, and trivial
  affine discard. An empty selection site no longer proves a live establish or
  transfer; missing materialization leaves the ledger incomplete. The
  backend report exposes complete versus `UNLINKED` ledgers. Every current
  ownership pass canary now has a complete ledger, including dispatch-edge
  and state-call argument materialization joined to the target state's entry
  establishment. Runtime and direct state-call sites preserve exact call
  ordinals, including statement-position host calls. Named transition targets
  reserve their canonical ordinal before nested argument calls, and edge joins
  filter by target symbol; a two-obligation nested-call canary pins the complete
  ten-event result. A live linear obligation now survives a dispatched state
  call's synthesized continuation and is consumed afterward; the runtime canary
  also pins implicit attached-`self` binding and distinct call-result/local
  storage. Two same-symbol nested transition calls also retain distinct
  canonical ordinals and jointly realize their shared target-state event.
  Coverage of entry establishments now includes normalized platform-boundary
  parameter writes; missing inbound code fails closed rather than borrowing
  proof from a later consume or from zero storage. Nontrivial automatic
  state-exit cleanup
  is OWNER-BLOCKED on the graph-edge timing, partial-value order, and
  proof/effect contract under "automatic cleanup's graph-edge and partial-value
  contract" in `OWNER_QUESTIONS.md`. Composite per-field
  debt is separately OWNER-BLOCKED on the nominal-versus-contained resource
  frontier and component-origin identity under "composite linear value's
  resource frontier." Continue with remaining
  whole-value ownership forms. A state-call result that carries one
  unambiguous locally-created obligation now joins the caller's receiving
  establishment to the callee origin instead of minting a second claim.
- **TR3 — activation plans.** The normalized `omega-task-plans` candidate and
  validator are live for contract/entry/calling-plan IDs, argument/outcome
  layouts, continuation size/alignment, cancellation, distinct-versus-inline
  execution, local suspension safety, and separate safe-point/asynchronous
  migration-demand envelopes. Core now exposes opaque `TaskRuntime::start` and
  `try_start` generic boundary signatures whose static target contracts admit
  suspension and blocking. Concrete machine specializations retain their instance
  symbols, and compiler elaboration emits validated target-specific plans in
  `05_task_activations.json`: normalized machine/entry/layout/calling IDs,
  continuation sizing from the resume word, persistent target layout, and
  canonical live values, independent `may_suspend`/`may_block` bits from the
  checked transitive `SuspensionPlan`/`BlockingPlan`, and safe-point migration
  demands from checked carry
  crossings. Missing
  crossing evidence fails closed. Checked carry facts now also contain one
  all-instruction envelope per machine, conservatively joining persistent
  storage, parameters, locals, call signatures, aggregate/cast temporaries,
  and reference formation. Unresolved value/type coverage marks the envelope
  incomplete; only a complete envelope becomes an asynchronous migration
  demand, so asynchronous provider admission still fails closed rather than
  guessing. `05_carry_manifest.json` publishes completeness and the joined
  policy. Every plan requires cancellation support because every `Task<T>`
  claim exposes cancellation-request authority. Real provider provenance and
  admission/dispatch integration are **OWNER-BLOCKED** on
  `OWNER_QUESTIONS.md` under "task-runtime provider publish checked behavior
  and own its slot": the current provider-plan surface selects boundary traits,
  while `TaskRuntime` is opaque boundary data, and no checked contract can yet
  express the runtime behavior/capacity statement that the existing receipt
  qualifier must bind.
- **TR4 — runtime requirement and admission.** The normalized demand/behavior
  join is live: provider storage/capacity, cancellation, inline behavior,
  preemption granularity, CPU/thread migration, and continuation movement fail
  closed against the activation plan; unknown runtimes are pessimistic. The
  nominal opaque `TaskRuntime` source surface is live. Activation demands now
  carry a normalized identity over every checked input, admission receipts
  derive their identity from the full demand plus runtime behavior contract.
  Runtime behavior is no longer self-authenticating normalized data: the
  freely constructible claim becomes admissible only through the shared
  grant/receipt spine, whose provider-plan statement hash covers the base plan
  identity plus every behavior promise. A changed pinning, capacity,
  preemption, cancellation, or storage claim therefore drifts the receipt;
  receipt provenance remains evidence and stays outside the normalizer-owned
  runtime identity. The activation artifact records `pending_provider` until a
  real selected provider supplies that exact receipt. Core lifecycle
  conservation is now pinned
  end to end: receiver types retain `&self` versus consuming `self`,
  `request_cancel` leaves the linear claim live, and `finish` transfers it into
  the conditional terminal outcome. Compiler provider-plan selection/receipt
  integration is OWNER-BLOCKED on the task-runtime provider-slot/behavior
  decision above; opaque runtime layout is independently OWNER-BLOCKED on
  `OWNER_QUESTIONS.md` under "opaque runtime `boundary data`." Once those
  surfaces are settled, add executable dispatch and ensure a rejected
  transactional start returns every moved argument and lease.
- **TR5 — custody and storage leases.** The normalized
  `omega-task-plans` lifecycle ledger now separates the admitted runtime
  contract, one concrete runtime instance, one activation instance, persistent
  storage owner/lease provenance, and the linear lifecycle claim. Accepting an
  activation records that complete dependency before issuing the non-clonable
  claim. Cancellation preserves it; exact terminal settlement releases the
  storage relationship; failed cross-instance settlement returns the claim
  unchanged. Provider close and storage reclaim reject while a matching child
  remains live, and activation or lease-era replay rejects even after
  settlement. Inline completion may omit persistent activation storage only
  when the exact admission permits it, and still returns a lifecycle claim
  requiring settlement. Connect this normalized ledger to the selected
  runtime value and source `Task<T>` once TR4's provider-slot and opaque-runtime
  representation owner questions are settled; do not infer provenance from
  the task result type.
- **TR6 — continuations and first provider.** Lower continuations; admit inline
  completion only when the pinned contract permits it.
- **TR7 — suspension-safe loans.** Enforce the conservative moved/shared-
  immutable/synchronized subset and integrate carry checking.
- **TR8 — reference packages.** Build `ArenaTaskPool`, bounded mailbox, and
  supervisor packages, then migrate samples. Package ergonomics do not justify
  new core syntax without a semantic impossibility.
- **Allocator migration.** Replace ambient legacy `alloc` with explicit
  `Arena`/`Allocation` contracts. Cathedral's bootstrap range-authority source
  now uses the settled `Extent`/`mint_extent` spelling; connect that temporary
  plain carrier to Omega's opaque linear Extent surface rather than
  reintroducing `Region`. **OWNER-BLOCKED:** `OWNER_QUESTIONS.md` under
  "opaque runtime `boundary data`" must decide the provider-backed runtime
  representation; the compiler deliberately gives all opaque boundary data no
  layout today, so pretending the temporary record already is the sealed
  carrier would expose forgeable authority. The source-visible
  `boundary data Extent [linear]`
  and debt-free `ExtentSlot` bridge are live in core; ordinary construction and
  scope loss reject. Core's stage-1 Arena now returns/reclaims that authority
  instead of a bare `addr`, while its trait/module/canary use `Arena`; the false
  ambient `Vec::with_capacity` surface is retired. Connect Cathedral's
  temporary carrier through provider minting and sealed extent-domain facts.
  Introduce `Allocation<T>` only once its borrow from the Arena can be expressed
  honestly. Structural multiplicity, not a permanent semantic ban, governs
  debt-bearing `Allocation<T>`.
- **Vec and slices.** Implement owned dynamic `Vec<T>` storage plus
  `as_slice`/`as_mut_slice` over real allocation/extents.

### Mathematical and float libraries

- **N6 — quotients (foundation landed).** The compiler now carries the settled
  bodyless `data Quotient = Carrier % relation;` declaration through syntax,
  resolution, and typed trees; admits it only for a proof-only carrier and a
  checked pure binary relation with structurally matching reflexivity,
  symmetry, and transitivity proof machines; restricts `as` minting to the
  carrier family (including any concrete machine-indexed instance); and
  transports a required relation fact into equality of quotient casts. The
  heterogeneous family boundary is pinned in both directions: distinct
  `CauchySeq<S>` generators share the quotient while a different data family
  cannot mint it. Relation and law contracts may now quantify over independent
  left/right generator symbols (`relation<A, B>(CauchySeq<A>, CauchySeq<B>)`):
  contract references resolve those lexical selections, and proof-only
  selections remain universal schema arguments instead of accidentally
  monomorphizing the relation to one concrete pair. Checked pure free and
  attached operations whose carrier operands and return share the quotient
  carrier now lift onto quotient arguments/receivers only when an ordinary
  proof machine structurally establishes the relation over paired outputs from
  pairwise-related inputs. For attached operations the receiver is the first
  operand, so both receiver-only and receiver-plus-argument forms require the
  corresponding premises. By-value operations attached directly to proof-only
  data are computed proof machines and emit no runtime dispatch; borrowed or
  mutable receivers remain forbidden runtime-consumption attempts, and an
  operation attached to runtime data remains runtime and cannot hide
  proof-only parameters.
  Missing certificates reject, and boundary axioms cannot masquerade as
  equivalence or respect proofs. The constructive replacement is
  OWNER-BLOCKED on `OWNER_QUESTIONS.md` under "quantified convergence as a
  quotient relation": the quotient needs one checked binary proposition, while
  Cauchy convergence needs an existential modulus packaging universally
  quantified point obligations. Do not substitute an always-true relation or a
  boundary axiom.
- **N7 — nested schemas (complete).** Proof-only data may declare `<machine S>`
  with its mandatory `where machine S(...)` contract. Recursive occurrences
  forward the family parameter, concrete arguments are checked through the full
  static-machine refinement judgment, finite-layout families reject, and a
  machine parameter cannot masquerade as a stored field type. Machine-parameter
  signatures may themselves take machine parameters: nested contracts resolve
  in recursive lexical scopes, refinement is binder-positional across the full
  callable contract, distinct schema parameters forward through proof-data
  families and calls, and fixed-point specialization removes both layers.
- **N8 — construction corpus.** The first honest metric rung is live in core
  `Rat`: `rat_gap` computes the division-free absolute cross-product gap and
  `rat_close(p, q, precision) == Zero` states the reciprocal-precision bound;
  checked reflexivity and symmetry lemmas ship with a false-twin rejection.
  Entry-state lemma citations now preserve preceding `let` meanings, which the
  gap proof needs for `sub_self(cross)` rather than injecting a fact about an
  unbound local name. `mk_rat` now declares denominator positivity explicitly.
  Resolved ordinary value-position calls now instantiate machine/state
  equality-style `requires` at their operands and reject facts the structural
  judge proves false: a zero-denominator `mk_rat` call and a false `!=` call
  reject, while valid construction compiles. This is deliberately the first
  engineering rung, not full discharge: obligations that are unknown or outside
  the structural language remain permissive until call-site flow facts are
  complete enough for fail-closed validation (citation sites already fail closed
  inside that language). The first quantified sequence-facing atoms now ship in
  core: `cauchy_at<Sequence, Modulus>` and heterogeneous
  `converges_together_at<Left, Right, Modulus>` take arbitrary symbolic
  precision/indices, enforce positive precision plus both modulus thresholds,
  and compute the `rat_close` residual without runtime callables. Valid concrete
  points compile and zero precision rejects. Structural application identity
  now retains static-machine selections (`f<A>` no longer aliases `f<B>`),
  generic body unfolding alpha-substitutes those selections, and a false
  cross-selection equality rejects. Machine-generic theorem contracts now
  validate once on the pristine typed graph after selection refinement and
  before concrete specialization mutates its first template; all non-entailment
  checks still run on the specialized graph. Consequently the generic
  `cauchy_at` reflexivity and heterogeneous `converges_together_at` symmetry
  laws are checked and remain citable after concrete selection. Core now also
  ships the Nat metric substrate used to prove the next rung: `nat_gap` plus
  checked reflexivity, symmetry, endpoint, successor, order-monotonicity, and
  triangle lemmas. Structural proof recognition follows nested named case
  states to every value leaf and refuses state cycles; recursive-measure
  provenance survives arbitrary forwarding through those proof states only
  when every incoming binding descends. Citation preconditions consume only
  facts established earlier in statement order, so transitivity and combined
  monotonicity proofs compose without implicit/global lemma application and a
  later fact cannot justify an earlier citation. Absolute Nat gaps now scale
  through a shared multiplier, and `rat_gap_triangle_scaled` lifts the Nat
  triangle to the three fractions' common denominator. Citation
  alpha-substitution also carries symbolic member places (`p.den`) into the
  caller frame, so that Rat theorem is reusable rather than merely checked in
  isolation. Nat multiplication now preserves monus order, and a positive
  common right factor reflects it. Requires-bearing induction hypotheses are
  available to per-arm citations only after every instantiated premise proves
  at that statement boundary; earlier citations may establish a conditional IH
  for later citations, while a swapped/unproved premise rejects. The checked
  `rat_close_triangle_split` theorem combines two doubled-precision bounds,
  scales the rational gap triangle, and cancels the positive middle denominator
  plus the concrete factor two. Its sequence-facing twin,
  `converges_together_at_triangle_split`, establishes pointwise heterogeneous
  transitivity with every precision, modulus threshold, and actual middle
  sample denominator explicit. Citation alpha-substitution now preserves
  member places rooted at selected static-machine applications, and a wrong
  middle generator cannot discharge that premise. Building the certified
  `CauchySeq` carrier and replacing boundary `Real` with
  `CauchySeq % converges_together` is OWNER-BLOCKED on the quantified
  proposition/certificate decision recorded in `OWNER_QUESTIONS.md`; after it
  is settled, prove operation well-definedness and order/completeness and retire
  axioms through the normal boundary-upgrade path.
- **F6 — total float order (complete 2026-07-23).**
  `omega::language::core::float_order` now exports the ordinary `Order::before`
  requirement plus named `F32::TotalOrder` and `F64::TotalOrder` satisfiers.
  Each recasts the landed primitive's honest bits and applies the branchless
  sign-magnitude-to-unsigned-key transform, so unsigned comparison implements
  IEEE totalOrder across negative/positive NaNs and payloads, infinities, and
  signed zero without changing arithmetic `<`. Static-machine
  monomorphization now carries an exact selected entry symbol through both the
  interpreter and native state-call planner, and the active differential
  canary runs repeated generic selections for both formats before exiting 70.
- **F7 — float format providers.** `FloatFormat::BINARY32` and
  `FloatFormat::BINARY64` now live in `omega::core` as ordinary semantic data.
  Replace the hardcoded IEEE lowering bootstrap with checked target
  conformances, derived provider plans, and checked assembly; there is no
  instruction-binding compatibility path. The exact source requirement family
  for primitive float spellings is **OWNER-BLOCKED** on
  `OWNER_QUESTIONS.md` under "primitive float operations": the current corpus
  does not decide whether concrete boundary operators, a format-policy family,
  or another existing requirement form owns those public contracts. Encoding
  work remains staged until that identity is settled.

## Layout, memory, and artifact foundation

- **L4/L5 — plan-laid views.** Derive projection over plan-laid byte views,
  complete non-scalar and mutable recast views, validate tiling beyond
  fact-free shapes, enforce validate/materialize mint exclusivity, and prove
  codec conformance through ordinary policy machines. Shared record recasts
  can now size and project a nested plan-laid record recursively, with a native
  and interpreter differential canary; widening and explicit narrowing
  conversions from projected plan-laid scalar places lower natively and
  cross-compile on both backend families. A plan-laid application now rewrites
  to one synthesized record identity across every type position instead of
  relying on field and parameter spellings sharing an arena handle; the
  policy's exact placement therefore survives by-value state parameters and
  further forwarding. Fact-free equal-width mutable scalar recasts now
  preserve address identity and bit-exact write-through in native and
  interpreter execution, cross-compile on x86-64 and AArch64, and reject a
  fact-bearing source unless the required bidirectional implication can be
  proved. Fact-free mutable scalar views now also preserve full-footprint
  little-endian write-through at a statically or dynamically bounded offset in
  a byte region, with native/interpreter execution and x86-64/AArch64 compile
  rails; fact-bearing targets reject. Recursively fact-free mutable record
  views now write through nested ordinary and plan-laid projections in native
  and interpreter execution, with x86-64/AArch64 compile rails; constrained
  fields reject at the bidirectional-fact gate. `struct stat` now uses target
  `StatLayout` policies rather than compatibility offsets. Fixed-array fields,
  including arrays of nested records, now participate in recursively fact-free
  mutable record views; runtime-indexed projections preserve live backing-byte
  identity in the interpreter and native x86-64/AArch64 lowering. Indexed value
  operands and the general indexed-`Place` materializer now carry the index
  slot's declared width through selection, assignment, reporting, relocation
  planning, and both ISA encoders. Exact 1/2/4/8-byte zero-extending loads cover
  ordinary indexed reads/writes and the independent runtime-text paths, so a
  narrow index cannot consume adjacent frame bytes and a wide index cannot lose
  its high half. Backend exact-encoding tests and a high-bit-capable `u64`
  out-of-bounds fail canary hold those rails. Fact-bearing array
  elements reject at the same bidirectional-fact gate. Mutable scalar
  views now admit normalized declared-domain conjunctions only when the shared
  domain graph proves implication in both directions; the implication relation
  is owned once by typed domains and reused by validation and flow checking.
  Fragment-source validation now uses the field's declared representation width
  rather than its rounded byte carrier: `bool` tiles as one bit and a
  non-negative constant range tiles through the highest representable bit.
  Exact source tiling still rejects gaps. This keeps bit-packed hardware fields
  on ordinary carriers and range facts instead of adding `u1`/`u3` primitives.
  Cathedral's first x86-64 page-table-entry policy now exercises the complete
  64-bit word, including nine one-bit flags, 3/7/4-bit policy fields, and the
  40-bit page-frame number for the architectural 52-bit physical envelope.
  The target-neutral ordinary-scalar materializer now consumes only named
  field values plus that same validated plan: it supplies no offset API,
  requires the complete field set exactly once, rejects width/fragment drift,
  zeroes padding/reserved bits, and commits to the destination only after all
  entries validate. Unit coverage pins an x86-64 page-table word, while the
  compiler integration rail materializes a build-time-evaluated compact-bit
  policy rather than a hand-authored Rust layout. The inverse scalar decoder
  now drives imported-table scans through the identical named geometry and
  compiler-materialized field widths; it reconstructs complete logical fields,
  rejects missing/extra/overlapping source fragments, and establishes no
  semantic or authority fact by itself.
  Constant integer ranges now normalize to exact two's-complement bit-pattern
  sets, including split intervals across signed zero. Adjacent and overlapping
  intervals canonicalize, so a signed range covering its complete carrier is
  representation-equivalent to the same-width unconstrained unsigned carrier.
  Equal sets admit mutable aliases across same-width signed/unsigned carriers;
  equal cardinality or shifted sets reject. Range-refined reference binding is no longer mistaken
  for a numeric store into the referee. Equal-looking predicates over different
  primitive carriers remain fenced until their bit-pattern sets can be proved
  equivalent. Same-carrier float ranges now compose by numeric interval
  inclusion for shared views and exact interval equality for mutable aliases;
  the same leaf judgment composes recursively through typed record views. They
  may weaken through a shared unconstrained equal-width bit carrier, but remain
  fenced from cross-carrier mutable equivalence because numeric intervals do
  not describe IEEE bit-pattern sets. Typed record-to-record mutable
  aliases now admit when total size/alignment, leaf offsets/sizes, and every leaf
  representation set are equivalent; this includes range-bearing signed/unsigned
  leaves and `bool`, with native/interpreter execution and x86-64/AArch64 compile
  rails. Scalar recasts now consume that same representation-set law: `bool`
  contributes exactly `{0,1}`, shared views may forget but never strengthen
  facts, and mutable aliases require equal sets. Consequently a typed `bool`
  may be shared as a byte, a typed `u8 [0..=1]` may be shared or mutably aliased
  as `bool`, while an unconstrained byte or raw byte region cannot mint the
  invariant. Raw bytes still cannot acquire record facts through `recast` and
  remain fenced until a validate/materialize mint supplies evidence. Continue
  general non-record tiling and richer representation-set reasoning. Cast/recast
  targets now carry full type references through syntax, resolution, typed and
  checked trees, while a separate spelling cache remains diagnostic-only.
  Parser coverage pins the settled `as &[T]` / `as &[T; N]` surface without
  encoding structural types as generated semantic names. Top-level
  literal-length fixed-array recasts now use that structural identity
  end-to-end: byte-region targets require recursively fact-free elements,
  typed shared aliases may weaken facts, typed mutable aliases require
  bidirectional representation equivalence, and mutable views preserve
  indexed read/write identity in native and interpreter execution. Focused
  canaries also cross-compile on x86-64 and AArch64 and reject attempts to mint
  `bool` element facts from raw bytes. Unsized-slice recasts now consume the
  complete source representation, reject non-divisible tilings, derive the fat
  descriptor length from the target element layout, preserve mutable indexed
  write-through across state forwarding in native and interpreter execution,
  and cross-compile on x86-64 and AArch64. Raw bytes remain unable to mint
  slice-element facts. Continue source-visible validate/materialize
  establishment, codec conformance, target page-table writer integration, and
  remaining representation-set work without adding a second structural-view
  mechanism or a public raw-offset writer.
- **L6a — Extent.** The normalized conservation foundation is live in
  `omega-extents`: admitted one-shot root grants mint nonempty ranges;
  move-split preserves exact geometry; only compatible siblings from one
  split lineage merge; attenuation cannot add open-set rights; failed
  consuming operations return their authority; and one borrow-carrying loan
  derives shared/exclusive polarity from its parent. Fixed-destination mapping
  now consumes virtual authority while independently owning, shared-borrowing,
  or exclusive-borrowing its source. Structural validation produces only a
  sealed pending map; no mapped loan/access exists until an exact provider
  receipt establishes translation installation plus every open activation
  fact. Unmap likewise returns reusable ranges only after an exact provider
  receipt releases stale translations and establishes its open completion
  facts. The hybrid page-table lifecycle is now normalized over that mapping
  state: an admitted grant validates concrete table storage; a draft rejects
  wrong-space, duplicate, or overlapping pending mappings; generated bytes or
  a one-time imported-table scan must bind the exact canonical plan and mapping
  set before `InstallablePageTable` exists; and installation exposes mappings
  only after one receipt binds the same grant, plan, content, construction
  evidence, and every exact activation receipt. The plan fingerprint now binds
  each mapping's actual source range, custody mode, space, provenance, era,
  lineage, and rights as well as the destination authority that teardown must
  restore; caller-chosen mapping IDs cannot hide physical-frame substitution
  or different returned authority. Removal is now symmetric:
  installed storage and mappings remain captive until the exact installation
  is inactive, every open table-retirement fact holds, and every mapping
  supplies its own valid translation-release receipt. A target provider can
  now borrow the draft's canonical pending-map sequence and inspect the exact
  source/destination geometry, custody, and normalized authority facts needed
  to derive entries, including the draft's exact table-storage destination;
  the projections expose inert data and cannot borrow, consume, split, complete
  a mapping, or release authority. Target-specific entry writers, imported-table
  entry writers and imported scanners can now share the same ordinary scalar
  materialize/decode consumer; target-specific field-policy mapping,
  page-table-control execution/effects, and further normalized mapping work
  remain ordinary engineering. Connecting
  this model to the source-visible opaque Omega `[linear]` carrier, sealed fact
  establishment, storage, calling conventions, and source APIs is
  **OWNER-BLOCKED (#8)** on the runtime representation plan for opaque
  `boundary data`; do not substitute a forgeable plain record or a universal
  pointer-sized handle.
- **L6b — AccessPlan and placed views.** The separate normalized validator is
  live: name-keyed entries pin exact transfer width, stable/external/atomic
  observation, ordinary and atomic permissions, exported versus
  provider-private access, and static service reach. Authored entry order is
  erased: validation sorts by field identity and assigns a deterministic
  normalizer-owned plan identity covering every operation, observation, width,
  exposure, and reach fact, so equivalent policy machines cannot fracture
  artifact identity. Validation checks fixed
  layout geometry, rejects multi-container one-access laundering and public
  external RMW, and enforces borrow polarity at operation authorization. Add
  the Omega-authored policy surface and source-level borrow-carrying access
  values, then connect target-specific external/atomic emission. The normalized
  lowering seam is live: consuming one sealed field authorization produces an
  exact request bound to plan identity, admitted grant, field, address, width,
  observation, loan-derived borrow polarity/lifetime, operation-specific atomic
  ordering, and static reach; malformed load/store/compare-exchange ordering
  rejects before target lowering.
  Provider-admitted placed-view
  grants now check an actual Extent loan's space, provenance, open-set rights,
  size, and permitted static reaches; field authorization derives polarity
  from that loan and mints the only token primitive lowering may accept. Never
  expose arbitrary-offset access or per-access revocation probes.
  **OWNER-BLOCKED (#17):** the normalized plan and sealed lowering seam
  are settled, but the concrete Omega `AccessPlan` record/policy spelling is
  still explicitly open in the programmable-layouts brief. Do not invent that
  source contract while implementing the independent target-emission work.
- **L6c — symbolic materializer.** The normalized source/action plan and
  loader-versus-post-handoff validation are live. Range/alignment/phase/regime/
  installation-scope constraints are normalized, concrete-site validated, and
  bound through decoded artifact construction, admission evidence, placement,
  and materialization without permitting constraint substitution. Entry-source
  integration now reaches canonical executable entry-set decoding and
  admission-bound sealed entry targets. Exact installed code now resolves those
  targets privately while executing the atomic writer. A sealed preparation
  now consumes the exact destination/plan/root set, validates every entry
  against that installed realization, and lowers to an address-free generated
  target/machine carrier with deterministic plan/placement/initial-content/
  root fingerprints and private source-slot indices. Its packed private
  `IDTWRIT1` context ABI, exact x86 encoding/width, and RAX/RCX/RDX/R11 plus
  Flags footprint are pinned; unsupported architecture, ABI, slot, and geometry
  combinations reject before emission. A non-clonable populated seal now owns
  the exact once-resolved private destination/source words, and only that seal
  can lower or materialize; its values never enter diagnostics or public
  accessors. Materialize its address into R10 and execute the emitted
  instructions in the concrete provider. The settled writer
  consumes one exclusive unpublished
  mapped/pinned/writable placement plus a sealed exact-artifact resolver and
  writes the destination directly. A partial or failed fill remains
  unpublished and cannot produce the established result; no public address
  resolver, arbitrary-offset write, callback ABI, or staging allocator is
  introduced. Validate the complete bytes and bootstrap fault-freedom
  conjunction before minting the content-bound linear materialization claim.
  Installation remains a separate provider operation holding the
  hardware-table capability, with its own receipt and record-before-publish
  ordering. Native whole-pointer actions already lower into section-qualified
  object relocations with materialization provenance.
- **External loans.** The normalized `omega-extents` model is live: a token
  borrow-carries the real Extent loan; device-read requires shared polarity;
  device-write requires exclusive polarity; admitted grants pin borrower,
  space, provenance, open-set rights, and an open set of completion facts; an
  exact provider receipt must establish borrower release plus every required
  fence/cache/provider fact. Completion evidence is now derived from the exact
  live loan and binds its confinement receipt, direction, space, provenance,
  mapping era, and lent range; reusing a loan ID cannot replay a stale
  completion after authority drift. Provider execution and the normalized DMA
  slice may proceed independently. Connecting the token and its borrowed Extent to
  Omega storage, calling conventions, linearity, and permission contexts is
  **OWNER-BLOCKED (#8)** on opaque runtime `boundary data` representation.
  Bidirectional sharing remains an explicit atomic/coherence protocol, not
  ordinary lending.
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
  schema/layout byte decoding. The post-decode closed relocation validator is
  now live for absolute-64, x86 relative-32, and AArch64 page/page-offset/
  branch relocations: it applies a configured count bound, canonicalizes
  destination order, checks exact relocation widths against the code section,
  and rejects overlapping or overflowing fields while retaining only symbolic
  entry/data targets. The object-layer adapter now consumes only that validated
  carrier, maps it to the existing target relocation vocabulary, resolves
  symbols through a sealed compiler/provider callback, and rejects
  architecture mismatch, missing symbols, or section-offset overflow before
  appending any record. Signed semantic addends now survive the object carrier,
  direct-image application, reports, and identity fingerprints instead of
  being silently discarded or rejected. The decoded carrier and resulting
  immutable artifact now retain the exact code bytes, so a later materializer
  needs no unmodeled byte side channel. Validation derives rather than trusts
  the normalizer-owned content identity over code, contract/footprint/
  placement promises, instruction-set architecture, canonical entries, and
  canonical relocations. Architecture-specific relocation meanings now reject
  during candidate validation rather than surviving until object emission;
  the object adapter independently rejects cross-architecture substitution
  even for shared relocation forms. Canonical AArch64 entry offsets must also
  satisfy the ISA's four-byte instruction alignment; x86 entry offsets retain
  their byte-granular rule. The reusable immutable artifact now retains the
  canonical relocation set alongside its exact code bytes, so admission cannot
  strand the later materializer or force an unmodeled relocation side channel.
  A canonical provider helper now resolves only sealed entry/data targets,
  applies exact x86-relative, AArch64 page/branch, and absolute relocations to
  a private copy, validates AArch64 instruction shapes, and derives a
  placement/content-bound final-byte identity. Its inert output can bind the
  materialization receipt but grants neither destination writes nor execution.
  The write/freeze transition now consumes that exact canonical output rather
  than accepting only a restated hash: artifact, admission, placement, base,
  plan, byte length, and final identity must all match. `FrozenPlacement`
  retains the immutable final-byte snapshot so the later footprint/PCC
  validator can inspect precisely the bytes whose write authority was frozen.
  Entry/section order, proof evidence, and informational sections remain identity-invisible.
  Implement
  admission/PCC and final-footprint validators, destination write/freeze and
  installer providers, Omega linear integration, and provider-backed
  quiescence/replacement execution. Code-placement claims already validate the
  actual Extent base/length against normalized range, alignment, phase, regime,
  and installation-scope constraints before materialization. The normalized
  retirement path already distinguishes visibility from quiescence, requires
  X removal and write-authority restoration, and returns the exact placement
  for reuse only after an exact scoped receipt. PE/COFF remains only a firmware
  envelope; no arbitrary byte-to-code path exists.
- **Wire runtime.** Scalar decode now establishes destination range facts
  instead of rejecting ranged fields: each call binds the normalized inclusive
  interval from the actual value declaration across plain, nested, and repeated
  scalar destinations. Native x86-64/AArch64 and the interpreter reject hostile
  out-of-range values before writing the field or array element, and the sticky
  `WireVerdict` becomes `Invalid` while the prior valid value stays intact.
  The same path establishes `bool`'s intrinsic `{0, 1}` representation, so a
  hostile noncanonical boolean varint cannot be normalized into a valid value;
  plain and repeated runtime canaries pin rejection and write preservation.
  Finite `i32`/`u32` carrier bounds are likewise checked before truncation, so
  wider hostile varints cannot wrap into an apparently valid destination.
  Varint readers now require canonical minimal LEB128, reject payload bits
  beyond `u64`, and retain the legal ten-group encoding of `u64::MAX`.
  Compatibility reports are complete: `04_wire_protocols.txt` records the
  current schema and adjacent-era compatible, reserved, and migration verdicts.
  **Design-blocked (OWNER #11):** remaining wire values, additional encoding
  families, unknown-field policy, and version negotiation depend on unsettled
  presence/default/migration and publication rules. Do not invent runtime
  encodings before that contract is settled.

## Remaining language surfaces

- **Lifetimes.** Decision 15's reference-side arc is live: explicit lifetime
  tags and declaration binders survive every semantic tree phase.
  Lifetime binders are stored separately from type/const/machine parameters,
  so erased regions do not alter runtime generic arity or monomorphization;
  duplicate binder names and undeclared lifetime tags reject. Single-reference
  and `self` elision link returned views to their input loans, and ambiguous
  multi-reference results reject.
  Borrow-carrying detection is structural and cycle-safe through nested
  records, active sum payloads, fixed arrays, constraints, and concrete
  generic arguments. Literal construction records every carried source,
  return escape requires all sources to outlive the call, and named-field
  projection rebases through only that field, so a valid first field cannot
  hide a dangling sibling. Fixed-array literal positions likewise retain exact
  element ordinals: constant indexing keeps only the selected element's loans,
  while dynamic indexing conservatively retains every candidate element.
  Explicit applications such as `View<'call, T>` now preserve their lifetime
  arguments separately from runtime generic arguments, validate lexical scope
  and arity, survive generic-data specialization without entering runtime
  identity/layout, and let a single-lifetime aggregate result retain precisely
  the named input through a call-produced local. Moving a borrow-carrying local
  or one of its fields now transfers its exact nested loan paths and polarity
  instead of laundering the source through ordinary data assignment.
  Multi-lifetime aggregate results now derive a field-specific result contract
  from the data declaration and instantiated lifetime arguments, retaining
  each field's named input independently through nested records, payloads,
  fixed arrays, and concrete generic arguments. Canonical last-use accounting
  now compares the actual field/index path, so using one result field does not
  artificially retain an unrelated sibling's loan. Assignment into an existing
  local or its nested fields now evaluates the right-hand side under the old
  loans, retires only the overwritten owner path, and activates the
  replacement's reference/call/literal/transferred-aggregate loans; fixed
  indexes remain exact and dynamic indexes conservative. Published loan-owner
  projections now live in one grouped arena with handle spans rather than a
  `Vec` per loan. Because state-local loans still expire at state exit,
  borrow-carrying assignments into attached or machine-owned persistent storage
  now fail closed with an explicit diagnostic instead of silently losing the
  source loan. Finish general outlives constraints, propagate persistent-owner
  loans through graph edges with state-parameter root rebasing, then admit those
  writes, and cover the remaining aggregate expression forms. These are settled
  implementation work needed by placed views and task storage, not owner-design
  blockers.
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
  fixed-array const evaluator's normalized service/operational gate,
  interpreter fuel,
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
  domain-body fact list directly, retaining normalized build-time-contract and
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
- **Dynamic traits (OWNER-BLOCKED: `OWNER_QUESTIONS.md` under "runtime and
  object-safety contract for `dyn Trait`").**
  Closed-world parameter calls currently specialize per concrete call site.
  Runtime-varying construction/storage, descriptors carrying satisfier
  identity, vtable emission, true indirect dispatch, and object-safety await
  the runtime representation, ownership, ABI, and admissible-signature
  decisions recorded there.
- **Equatable synthesis.** A declared conformance now emits a callable
  compiler-owned `Type::equals` wrapper over the same structural expansion as
  `==`/`!=`; a written implementation still wins. Generalize this closed core
  privilege through build-time trait generators below.
- **Build-time evaluation.** Extend the landed build-time evaluator from fixed
  arrays, const-generic calls, machine-backed domain facts, and compiler-owned
  policy plans into richer value/refinement positions; complete the remaining
  authority/trust/resource/failure/termination/mutation admission axes, then
  add reflection-driven trait generators.
- **Separate compilation and component artifacts.** Normalize imports, pinned
  contracts, provider selections, artifact identities, and replacement
  certificates without hashing private implementation witnesses into public
  identity.
- **Hot swap.** Implement liveness pins, quiescence proofs, and borrows as swap
  barriers through packages and admitted runtime operations; add no `replace`
  syntax.
- **Serialized capabilities.** Implement attenuation and revocation across
  boundaries.
- **Growable text storage after String retirement.** Implement the
  allocator-backed `Vec<u8> in Utf8` owner and its capacity-preserving append
  surface through the general Arena/Vec work; do not restore a text-specific
  primitive. The fixed carrier/domain migration and builtin retirement are
  complete. Generic domain-fact forwarding is complete across immutable
  parameters, guarded-transition fallthrough, declared fields/nested fields,
  indexed reads, and destructured case payloads; the non-text
  `Blob::Scanned` canary keeps this out of the text special-case bucket.
  Carrier migration has begun: a two-runtime-source concat now lowers without
  a literal anchor by initializing a distinct bounded destination from the
  first carrier and appending later segments. Bounded-carrier content equality
  now covers literal and carrier peers in guard, local, forwarded-local,
  machine-field, nested-boolean, and `!=` value positions on both ISAs; the
  migrated canaries retain native/interpreter differential oracles. The old
  frame-slot comparison writer now consumes the same representation-aware
  value operand as ordinary writes instead of assuming a `{ptr,len}` String
  descriptor. ZII carrier defaults, nested case-payload equality, mutable
  carrier aliases, and local-field copies through mutable carrier parameters
  have also moved off builtin `String`; the interpreter now constructs a
  domain-qualified fixed byte carrier as empty `{len, bytes}` rather than an
  always-full zero array. Literal and value-call terminal results can now
  construct `[u8; N] in D` directly, with an exact compile-time rejection when
  the literal exceeds `N`; bounded-carrier calls contribute their declared
  return capacity to assignment proofs. Guard-selected literal returns build
  the owned `{len, bytes}` carrier in the call-result slot before the caller
  copies it; they are not mistaken for legacy `{ptr, len}` text descriptors.
  Direct carrier-to-`&[u8]` projection now builds `{ptr, runtime_len}` rather
  than raw-copying the inline carrier prefix, including when the carrier is
  reached through a mutable-reference parameter. Named boundary/operator calls
  now substitute domain-membership
  `ensures` facts back onto the exact mutable caller place; the migrated
  `utf8_boundary_established` / `no_nul_boundary_established` canaries pin the
  positive route, while a negative canary proves mutation without such an
  `ensures` invalidates the old fact. Direct, parameter-pointee, and nested-field
  carrier writes now share Place-shaped lowering on x86-64 and AArch64. Immutable
  data parameters seed their nested declared-domain facts; writes through mutable
  parameters must establish those facts at the write, with a negative canary
  pinning rejection of an unqualified source. A nested declared-domain field
  whose domain admits the carrier's ZII value also remains a valid write source
  after its enclosing record crosses a mutable out-parameter call; the
  compile canary is authoritative, while empty-violating domains still require
  an established flow fact and retain their rejection rail. By-value bounded
  carriers are also distinguished from equally-sized fat descriptors during
  argument materialization, so `{len, bytes}` is copied inline instead of being
  rewritten as `{ptr, len}`.
  Lookup records, their large-record variants, the clear/carve/render dungeon,
  and the full-level wrapper path now use bounded UTF-8 carriers in both engines.
  Their mutable-output lookup contract explicitly returns the
  nested field's `Utf8` fact, so the caller never relies on a stale fact across
  mutation. Repeated fixed-capacity declarations such as `[u8; 16]::Utf8` and
  `[u8; 64]::Utf8` resolve as one short-name domain only when their normalized
  fact sets agree; divergent same-name declarations reject before flow checking.
  Standard Console input/output, provider forwarding, ZII host output, and
  borrowed view typing now operate directly on byte views or bounded carriers.
  Input lowering derives each owned carrier's inline capacity from its concrete
  destination, so a short carrier can never inherit the legacy 256-byte read
  limit. No pass-canary source now declares builtin `String`/`string`: the final
  two in-place-append regressions use bounded UTF-8 carriers. Their
  straight-line length proof tracks reaching writes, invalidates overlapping
  places, and drops its knowledge across calls or opaque effects; proven chains
  that fit compile, while the first append whose bound exceeds `N` rejects.
  The numeric-output path now converts a runtime integer directly into a
  runtime-indexed carrier element without an intermediate scalar field; native
  AArch64 execution, x86-64 emission/relocation, and the interpreter oracle pin
  that Place-shaped converted write.
  The 120 sample-only `pause: String` fields now use explicit
  `[u8; 256]` storage, preserving the former native read ceiling, and the three
  matching local Console declarations accept mutable byte views. The complete
  sample compile and documented-exit runtime sweeps stay green. The standalone
  `text/string_catalog` sample and its lattice mirror now use bounded UTF-8
  fields as well, with capacity proofs covering their concatenated label.
  The eleven-file dungeon text workload and its lattice mirror now use bounded
  UTF-8 carriers too: fixed room/player/enemy fields carry honest capacities,
  scratch writers preserve their carrier qualification with explicit `Utf8`
  postconditions, and the mirror incorporates the already-settled in-machine inventory
  scan. No sample or lattice-corpus source now declares builtin `String` or
  `string`. Proof discharge now follows the normalized semantic identity across
  carrier-specific declarations: equal-fact `[u8; N]::Utf8` specializations
  retain distinct operator symbols but imply one another at writes, calls, and
  operator contracts. The concat and dungeon carrier canary families pin that
  capacity changes do not fracture the semantic domain. Calling-policy
  rejection data now uses a bounded UTF-8 carrier, and
  the unimported `omega/host` scaffold was deleted rather than cosmetically
  migrating its retired `capability`/`entry` architecture; live portable and
  target-provider homes are recorded in `omega/host/README.md`. The two
  standalone nested-command run fixtures now use bounded UTF-8 input and retain
  their native `look` result. The failure corpus no longer declares builtin
  `String`/`string`: semantic negatives now use bounded carriers, borrowed text,
  or carrier-independent payloads, while obsolete primitive-only restrictions
  and unlisted compatibility fossils were deleted. Wire diagnostics and fixture
  names now describe runtime-sized text rather than advertising the
  compatibility type. The core surface, compiler-injected build vocabulary,
  builtin registrations, `PrimitiveType::String`, `str.omg`, and all compiler
  compatibility branches are retired. The remaining open work is the honest
  allocator-backed `Vec<u8> in Utf8` surface; it must use the general Arena/Vec
  work rather than restoring a text-specific primitive. The completed migration
  and its verification recipe are recorded in
  `wiki/architecture/string_retirement_execution.md`.
- **Atomics remainder.** The closed ordering vocabulary and operation-specific
  legality rules now reject release-bearing loads, acquire-bearing stores,
  unknown names, and compare-exchange failure orderings that release or exceed
  the success ordering.
  Load/store/fetch_add/fetch_sub/fetch_xor/fetch_or/fetch_and/swap/
  compare_exchange now preserve their normalized order through target
  lowering on x86_64 and aarch64; RMW
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
  Fetch-and uses complement-plus-ordering-selected `LDCLR` on ARM64 and the
  shared locked retry loop on x86_64, with equivalent coverage.
  The conventional integer fetch family is complete. Standalone portable
  atomic fences are **OWNER-BLOCKED (#13)** on their source contract, legal
  orderings, and separation from checked ISA/device/compiler barriers; do not
  lower a portable fence by treating x86 `lfence`/`sfence`/`mfence` as
  synonymous language operations. The cross-activation proof model beyond the
  implemented operations also remains.
- **Proof engine.** Continue induction and proof-data support required by
  layouts, quotients, and Real.

## Vertical acceptance slices

- **Termination firewall.** Pin one public `terminates` requirement inherited
  by acyclic and cyclic providers; swap descending and bounded-increasing
  witnesses without changing caller/import-slot identity; reject runtime
  non-tail lowering and ungranted progress profiles. Proof-only cross-machine
  SCCs now admit non-tail calls only when every member carries one structural
  witness and every edge passes a strict case-payload subterm into the callee's
  ranked parameter; unmeasured and nondecreasing cycles reject, and all three
  cases are authoritative canaries. Frozen decision 23 resolves the previously
  inconsistent runtime notes in the strict direction: every edge inside a
  cyclic component must strictly decrease the selected joint rank. The
  state-cycle and cross-machine forwarding-plus-later-decrease fixtures are now
  authoritative rejection canaries rather than pass-shaped pending examples;
  the checker no longer admits a merely non-increasing edge by analyzing a
  separate acyclic forwarding subgraph.
- **Service and operational contract split.** Demonstrate service-reach rows,
  independent inferred/published `suspends` and `blocks` ceilings, and the
  separate `terminates` guarantee; pin recursive inference, omission as a
  negative public guarantee, provider subset admission on every axis, retired
  mixed-row diagnostics, and stable normalized IDs independent of prover
  strength. The authoritative compiler integration rail now exercises the
  accepted composition, independent blocking-axis widening rejection, retired
  operational names in service rows, and witness-insensitive public contract
  identity. Recursive operational fixed-point, published-omission, and
  service-parent/subset cases remain pinned by the focused validation suite.
- **Units.** Before broad generic work, implement two units in one dimension
  and pin: explicit conversion, scaled dimensionless results, distinct
  Energy/Torque kinds, generic preservation, no silent forgetting, and package
  coherence for operator tuples. **OWNER-BLOCKED (#16):** the honest slice
  depends on the still-deferred authored domain-facet/introduction policy and
  open operator-family/dispatch-owner surface (DOM1/DOM2). A closed local
  encoding would not satisfy this acceptance contract; resume after those
  surfaces are frozen.
- **OS gauntlet.** Validate the foundation against UART/MMIO, page tables,
  DMA, shared-page IPC, IDT/timer entry, and SMP AP bringup. A customer that
  needs a new keyword or customer-shaped primitive returns to design review.
- **Control-state negative rails.** Backward-edge return integrity is derived
  from existing semantics and is not owner-blocked. Add four explicit
  acceptance families:
  1. **Live:** the freestanding `pushfq; cli; popfq` canary pins the catalog's
     exact balanced-stack/RFLAGS flow and emitted bytes, the same sequence
     rejects without machine-owner discharge in a hosted context, and
     `iretq`/`lidt` canaries keep entry/exit-only transfers deriver-only;
  2. **Live:** provider-exit realization is checked separately from body
     footprint evidence against the admitted `CallPlan` control and
     `StatePlan` restore set; opaque execution requires either that exact
     claim under a root-reported trust receipt or root-reported adequate
     hardware isolation, and missing, unreported, or drifted evidence rejects;
  3. **Live:** external-loan start requires per-transfer borrower-reach
     evidence bound to the exact loan, borrower, direction, address space,
     provenance/era, and lent range; missing, stale, or overbroad admitted
     borrower-contract/hardware-isolation evidence rejects before the agent
     receives the loan, so unrelated task/control storage remains unreachable;
     and
  4. **Live:** `Task<T>` remains only a lifecycle claim; compiler/provider-owned
     parked continuation storage is absent from its source-visible fields.
     Explicit fail canaries reject projection, recast, address-taking, and
     mutation through shared or mutable task claims. The shared place resolver
     now diagnoses missing members on typed locals and parameters, including
     applied generic carriers, rather than letting a two-segment access become
     a zero/default backend value.
  Keep forward-edge indirect targeting on sealed entry references and the
  runtime descriptor/object-safety work; do not mark all CFI as derived.

## Platform-gated verification

- **Differential baseline (2026-07-22, macOS).** The complete 861-entry RUN
  roster has 852 native/interpreter matches, eight intentional interpreter
  skips for out-of-range shifts in the Trapping domain, one native host gate,
  and zero mismatches. The host gate is the Windows-only GDI memory-DC canary
  (`CreateCompatibleDC`/`StretchDIBits`): the compiler suite already excludes
  it on non-Windows hosts because Darwin substitutes `Gui` with `MacosGui`,
  which intentionally has no `dc_create` operation.
- **Linux hosts.** Run filesystem/time structural rows on real x86_64 and
  AArch64 Linux. `clock_gettime` additionally needs composite `timespec`
  lowering before it can be verified.
- **macOS/x86 and other unavailable hosts.** Keep target emission structurally
  pinned; do not claim runtime verification without the host.
- **Windows GUI callback entry — OWNER-BLOCKED (#12).** Ordinary Microsoft-x64
  inbound argument/result lowering already exists, but `RegisterClassEx`
  requires a machine to become a stored, sealed callback reference. Implement
  the WndProc stub and real title-bar close path after the source-visible entry
  reference and registration/root-lifetime contract is settled. Do not pass a
  raw code address or add a Win32-only callback escape hatch.

## Deferred until a real customer

- Richer measured-recursion guards and multi-subject lexicographic cycles.
- Reduced-Rat divisibility theory beyond what N5/N6 demands.
- Async extent revocation beyond provider quiescence.
- Non-blocking executable-visibility tokens.
- Runtime-generated host code/JIT and arbitrary self-modifying code remain
  intentionally unsupported, not backlog items.
- Independent final-byte control-transfer certificates and CET, PAC, or
  shadow-stack hardening are PCC/TCB-reduction assurance work. They do not
  block checked-Omega returns, the IDT, or the Cathedral timer.
- Universe levels wait for a full-mathlib replay goal.
- A serious SSA/register-allocation/SIMD backend is post-1.0; correctness of
  current native output remains the active bar.
