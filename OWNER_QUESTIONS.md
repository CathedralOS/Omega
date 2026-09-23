# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### Q1

**Which declared-default field rows of a `&mut self` receiver cross a
method call — the machine-storage ZII set or the caller's checked
incoming set?** (named decision:
`mutable-self-receiver-declared-field-rows`).
[Default domains and zero
initialization](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
states both halves without joining them for receivers: "machine-owned
storage may begin zeroed while gated fields remain inaccessible until
established" governs a callee's `self` entry, while the same section
makes calls a consumption point where "the domain must be proved again"
— without saying whether the receiver place at a method call is the
callee's machine storage or a caller-obligated place like a `&mut`
actual. [Receiver state](
wiki/language_guide/chapter_8_domains.md#machines-and-states-can-require-or-guarantee-domains)
supplies the authored route (`requires self in D` / `ensures self in D`)
and stays silent on unannotated declared-field rows, and
[data_and_literals](wiki/spec/language/data_and_literals.md) confines
zero-initialization to construction, not call contracts. The
implementation currently picks the machine-storage route: a callee's
`self` entry assumption seeds only rows whose ZII value satisfies the
domain (`typed-trees-to-checked-trees/src/semantic/field_domains.rs`),
`checks/contracts/exits/result_domains.rs` re-proves exactly the assumed
rows at return, and `flow/call_phases/referents.rs` hands exactly those
back — so a caller's established non-ZII receiver facts survive a method
call only when the callee's write frame provably spares the field. Every
`&mut` parameter already rides the other route: "nominal input storage
instead relies on the checked incoming argument" (same file).
Motivating requirement: receiver field invariants are the common shape
of authored data invariants — a caller that proves `player.health`
inside its declared bound and then invokes `player.update()` permanently
loses the row under the current reading, while no authored
`requires`/`ensures` is needed for declared fields on plain arguments,
making receivers the one surface where declared defaults drop silently.
Options:

- (a) A `&mut self` receiver is a readable `&mut` referent like any
  other: the caller owes the receiver's declared-default field rows at
  the call, the callee assumes them on entry and re-proves them at
  return. Method calls preserve receiver invariants the way argument
  calls do; in exchange the receiver's declared surface becomes
  caller-facing contract.
- (b) Receiver declared-default rows stay machine-internal: calls on
  `self` carry only ZII-satisfiable rows plus authored
  `requires`/`ensures`, established non-ZII facts retire at the
  boundary, and frame precision remains the only preservation route —
  the current implementation is the contract.

Until answered, NOMINAL-FIELD-FLOW's receiver-handback widening stays
open: the flow machinery already transports whatever the entry
assumption carries, so the missing piece is the ruling, not the
plumbing.

### Q2

**How does the Delta compiler's cumulative pair allocation receive a
fail-closed profile under the selected Gamma evaluator?** (named
decision: `delta-compiler-pair-arena-profile`). The product requirement
is the Delta edge's observation contract, independent of this study:
every admitted `DCREQ` must end in a defined compiler observation — the
Epsilon evaluator closure already compiles through it, and every later
chain edge inherits it. The measured
[whole-producer pair study](bootstrap/3_delta/implementation/boundary/execution_storage.md#whole-producer-pair-study-measured)
drove every admitted extent to its selected boundary on the unchanged
chain. The real Epsilon closure customer compiles at 1,864,697
cumulative pairs, but the pair-maximizing admitted corner — ledger-capped
checked arithmetic (240 pairs per `+` node, at most about 129,000 nodes)
composed with ledger-free identifier bytes (three pairs per source byte)
— exceeded the retired 40,265,318-pair immutable arena: a
4,194,288-byte source
(SHA-256 `0b588a5376cefaecd8b7556d61464f857f0326df248607f8b6bc0285d605d89a`)
inside every authored provision halted 252 with empty stdout at exactly
40,265,318 pairs. That exhaustion is the evaluator's
`application_heap_failure`, not a compiler-owned DCOUT row. The
contracts checked and found silent:
[LANGUAGE.md](bootstrap/3_delta/LANGUAGE.md) fixes the DCOUT coordinate
vocabulary (none, Delta source, emitted payload, internal row, DCREQ,
bound support section) with no cumulative-allocation identity, and
states that evaluator failures do not substitute for the open
compiler-owned resource/internal outcomes;
[EVALUATOR_PROFILE.md](bootstrap/2_gamma/EVALUATOR_PROFILE.md) owns
status 252 as the pair-exhaustion evaluator observation, which exposes
no stdout; and the
[selected producer's resource ownership](bootstrap/3_delta/implementation/boundary/README.md#resource-ownership-in-the-selected-producer)
carries no resource row for cumulative pairs, while its
[arithmetic probe](bootstrap/3_delta/implementation/boundary/README.md#arithmetic-allocation-probe)
bars inventing a general DCOUT heap code. Decision needed: (a) enlarge
the selected evaluator's pair arena and re-derive its containment
argument — the 20,132,659-to-40,265,318 increase already set that
precedent for the complete-D parser customer, and the profile has since
adopted a 3,422,453,760-pair extent under which the
[measured worst-shape study](tests/delta/resource-boundary/README.md#measured-worst-shape-pair-containment)
projects 417,063,339 pairs at full admitted extents; (b) add a
compiler-owned allocation ledger with its own DCOUT resource, making
the bound fail-closed at a threshold chosen below the arena; or (c)
accept the raw status-252 observation as the contract for
cumulative-pair overflow and record it explicitly at the request
boundary. The arena enlargement was taken without the named decision
being recorded, so until it is answered **DELTA-COMPILER**'s
containment obligation stays open and a sufficiently large admitted
compile can still end without a compiler-owned observation.


### Q3

**How does Terminal Psi key and observe the crash site of an executable
Trapping arithmetic operation — one that is neither an edge nor a
`BoundaryCall`?** (named decision:
`terminal-operation-level-trap-crash-site`).
[Structural predicates](wiki/spec/terminal-psi/structural_predicates.md)
line 71 requires that "Executable Trapping operations instead carry their
primitive denotation and path-conditioned crash site, checked against the
published same-cause ceiling", and
[effects](wiki/spec/language/effects.md) line 261 states that "The body
operation creates the crash site under its compiler-defined denotation".
The language side is therefore settled: a trapping body operation owns a
crash site. What is not settled is the Terminal form of that site.
[Observations](wiki/spec/terminal-psi/observations.md) enumerates the
reconstructed profile as a closed, ordered row list under
`omega.terminal.observation-profile.v1`, and only two of its groups carry a
crash: group 3, "Crash sites ordered by machine, block, and edge", and
group 4, "Boundary crash sites ordered by machine, block, operation, and
cause", which is scoped to "every declared route of every `BoundaryCall`"
and keyed by the boundary's exact public identity and route bucket. The
implementation matches exactly — `terminal_trace_v1.rs:232` keys an
ordinary crash site by `(MachineId, BlockId, EdgeId)` and
`terminal_trace_v1.rs:236` keys a boundary crash site by
`(MachineId, BlockId, OperationId, CrashCause)` with a boundary identity.
A trapping `a + b` has no edge and no boundary identity, so it fits
neither group, and the same section forbids the obvious workaround: "A
boundary crash is observed at its calling operation, not on a fabricated
terminator edge." A producer may not expand a trapping operation into a
guard plus a `Crash` terminator, so the repair cannot stay inside Omega.
The choice is (a) widen group 3's key from edge to an edge-or-operation
coordinate; (b) add a third crash-site row group for operation-level
non-boundary traps, with its own tag, ordering and cause encoding; or (c)
generalize group 4 from `BoundaryCall` to any crash-bearing operation,
with the boundary identity becoming optional. Each changes a versioned
wire format whose spec says "Unknown schemas, vocabularies, tags,
classifications, malformed ordering, duplicate coordinates, missing/extra
sites ... reject", so the profile version and the verifier's independent
derivation move with it. Until this is answered, **ARITHMETIC-POLICY-
REALIZATION**'s first bullet — a Terminal Trapping operation family with
its `terminal-verifier` rule, `terminal-interpreter` case and Omega
realization — cannot be implemented without inventing the encoding, and
every `source/library/core/numeric_conversion.omg` machine ending in a
Trapping conversion stays unable to reach a native artifact.

### Q4

**May a lowering be refused for the aggregate amount of proof work it
requires, and if so what fixes that ceiling?** (named decision:
`compile-time-proof-work-ceiling`).
[Kernel metatheory](wiki/spec/proofs/kernel_metatheory.md) settles the
per-conversion case and only that case: `Budget` "makes the procedure
total on arbitrary input: exhaustion is `CoreError::StepCeiling`, a typed
error — never `Ok(false)` and never a hang", and `DEFAULT_CONVERSION_STEPS`
"is a policy default, not part of the calculus" (`:232-240`). Stack depth
is handled the same way, as a resource outside the calculus (`:242-250`).
Both guarantees are per call, and both are honoured today.

What is unspecified is the aggregate. `mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
in `checked-trees-to-lowered-psi` makes roughly 20,000 individually
bounded, individually successful kernel certificate acceptances and never
returns a verdict — measured at `de5798c306`: 192 of 192 traced producer
calls over 200ms returned a proof, the relaxed fallback was never reached,
and no `StepCeiling` fired. So the spec's promise that exhaustion is
"never a hang" holds for every call while the compilation hangs anyway.
Cost grows about cubically in the number of top-level `&&` conjuncts in a
machine body, with a cliff between 73 conjuncts (37s) and 74 (over 400s).

The near-term repair is algorithmic and needs no decision — the cost is
two redundant whole-module reconstructions and a per-condition certificate
that cites the entire equality roster, both named on
**C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT**. The question is what the compiler
owes when an algorithmically-reasonable program still exceeds any
practical budget. Three candidate answers: (a) nothing — compile time is
unbounded by design, and a program that is too large simply takes too
long, which keeps the calculus clean and leaves the hang; (b) a ceiling on
aggregate proof work per lowering, refused as a typed error in the same
register as `StepCeiling`, which makes the compiler total but fixes a
**language-visible limit on how large a `requires`-carrying machine body
may be** — a program's admissibility would then depend on a policy
default; (c) a ceiling that is diagnostic only, reported without refusing,
which preserves admissibility but does not make the compiler total.

This matters beyond one fixture: the member is the `+ 1 SIGTERM` in every
recorded run of that crate, and it blocks **RC-PCC-REPLAY**, whose gate
command runs `-p checked-trees-to-lowered-psi --no-fail-fast` and
therefore cannot reach a verdict. Until this is answered, no row should
add a bound here — 192 of 192 obligations are provable, so a bound chosen
without this decision would silently abandon obligations the compiler can
discharge, which is worse than the hang.

### Q5

**On AArch64, where the target-authored semantic `ProgramStorageEntry::enter`
plan passes both Extents by value in x0–x3, what does the compiler-private
semantic ProgramStorage wrapper owe?** (named decision:
`aarch64-semantic-wrapper-arrival-shape`).
[Target slots and program entry](wiki/spec/build/entry_roots.md) settles who
authors the surface and says nothing about this shape. It requires a
"target-authored bootstrap adapter and physical result map" and gives the
validation to the target — "The exact target adapter validates them,
installs scoped providers, establishes semantic arrival, and supplies only
schema-declared source arguments" — while its Slots and Entry-shape sections
name no wrapper, no frame geometry, and no compiler-authored copy area.
[Storage](wiki/spec/resources/storage.md#stack-demand-and-backing), cited
there for stack supply, governs demand against target StackPlan supply, not
the layout of one compiler-private frame.

The x86-64 wrapper is not an arbitrary encoding: every number in it is read
off a target declaration. `source/library/std/targets/uefi_x86_64/entry.omg`
places each semantic Extent as `ValueLocation::Indirect { pointer:
Register(X86Rcx/X86Rdx), has_copy: true, copy_stack_byte_offset: 32 / 48 }`
and sets `output.call.shadow_bytes = 32`, and
`program-entry-plan/src/optimized_semantic_wrapper/recipe.rs` reproduces
exactly those numbers — shadow 32, copies landing at 32/40/48/56, outgoing
address binds at 32 and 48. Microsoft-x64 makes the *caller* own the
by-reference copy, so materializing a fresh copy and passing its address is
the wrapper's whole purpose.

Both AArch64 targets author the opposite arrival. `MacosArm64::extent_value`
and `LinuxArm64::extent_value` place each Extent as two `ValueLocation::
Register { Aarch64X }` fragments — `parameters[0] = extent_value(0, 1)`,
`parameters[1] = extent_value(2, 3)` — with no copy offset, and neither file
sets `shadow_bytes`; `linux_arm64/entry.omg` states it plainly: "The
generated bridge passes the image and initial-storage roots in the first
four AAPCS64 integer registers." The wrapper's `CallPrivateTerminalContinuation`
step calls the continuation under the same plan fingerprint the wrapper
arrived on, so on AArch64 the four words are already in the registers the
continuation reads. Three candidate answers, and they are not encoding
variants of one template: (a) the wrapper degenerates — inbound and outbound
placement coincide, so the body is a single `b` to the private Terminal
continuation, or the lane declines to interpose at all, which keeps the
compiler from authoring a layout no target declared but leaves the recipe's
fixed eleven-step shape, its four copies, and its two address binds empty on
AArch64; (b) the wrapper still materializes a compiler-owned 16-byte copy
per root and passes its address, which requires the compiler to author frame
offsets and a shadow-equivalent that no AArch64 target declares and hands
the continuation pointers in x0/x1 instead of the four value words the
authored plan specifies — a second, undeclared calling contract on the same
fingerprint; (c) the AArch64 semantic crossing belongs to the hosted bridge
and this lane is UEFI-only by construction.

(c) is not obviously wrong, which is why this is a decision rather than an
omission: `TargetProfile::program_entry_slot` in
`omega-rust/omega/representations/target/src/lib.rs` gives
`ProgramEntrySchema::ProgramStorageApplication` with
`ImageAndInitialStorage` to `UefiX64` alone, while `MacosArm64` and
`LinuxArm64` are `HostedApplication` with `ProgramEntryVisibleParameters::
None`. The lane keys off two *visible* Extent parameters, so there is no
AArch64 instance of its own input contract to write a test against today —
an AArch64 acceptance test would have to invent a profile.

The requirement behind the question is the arm64 targets themselves, not a
test: `linux_arm64` and `macos_arm64` are shipping profiles that must
eventually realize program entry, and NATIVE-WRAPPER-ENCODING-AARCH64 on
`TASKS.md` cannot choose an encoder before the shape is fixed. The ISA half
needs no ruling and is recorded on that row; answering (a), (b), or (c)
fixes the step vocabulary, the frame geometry, and whether the relocation is
a `b` or a `bl`. Whichever is chosen, the relocation vocabulary is x86-shaped
and must gain a peer: the single variant
`OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1`
carries `byte_width: 4` and resolves by overwriting four bytes with
`i32::to_le_bytes`, whereas an AArch64 branch carries `imm26` in bits [25:0]
of the instruction word, scaled by 4, resolved as a masked merge into the
retained opcode.

### Q6

**How does target-specific checked inline assembly execute inside an
interpreted component?** (named decision: `interpreted-inline-assembly`).
The owner ratified [embedding](wiki/spec/build/embedding.md) with interpreted
Cathedral startup and guest-owned component replacement as the integration
goal. Shared OS algorithms should differ by provider/build composition rather
than execution-mode conditionals. Existing [checked assembly](wiki/spec/language/assembly.md)
remains legitimate OS source; boot/device code already uses port I/O, idle,
register clobbers, and target-state operations. It has not been decided that
these must all move behind new boundary interfaces.

Specify the admitted execution route and its scope: interpretation of selected
instruction contracts against an explicit target-state/environment model;
checked alternate realizations bound to those same contracts; or an explicitly
limited interpreter profile that rejects specified assembly-bearing components.
These may cover different operations, but each admitted route must preserve
memory/authority, registers/flags, ordering, control/entry, and fault behavior.
Establish where live host hardware versus modeled device state is selected,
and what correspondence evidence is required. Do not let a provider named
`MachineControl` silently redefine an instruction's meaning or treat a paused
interpreter as equivalent to a hardware idle/interrupt transition.

Acceptance for the decision: one Cathedral port-I/O/device sequence and one
idle/external-entry sequence run with their actual admitted effects and state;
unsupported instructions reject explicitly. No OS policy is moved into the
compiler/host to manufacture a pass. The instruction/target-state bridge is
undetermined, not a ban on inline assembly and not a promise of a universal
emulator. Only the assembly-dependent portion of **INTERPRETED-CATHEDRAL** is
owner-blocked; ordinary scripting, provider adapters, and independent Psi
component work proceed under their settled contracts.

### Q10

**What portable identity does a root compiled with no package declaration
carry, and may two such owners declare the same carrier-qualified domain
path?** (named decision: `unmanaged-root-package-identity`).
[Build declarations and package
identity](wiki/spec/build/declarations.md) defines `PackageKey` as
"Declared name and canonical source lineage" and `PackageInstance` as
"Key at an exact immutable source resolution" -- both presuppose a
DECLARED package, and neither that file nor
[package sources](wiki/spec/packages/sources.md) says what identity a
root without a package declaration carries. The word "unmanaged" does not
appear anywhere under `wiki/spec/`.
[Domains](wiki/spec/language/domains.md) states the collision rule in
terms of an owner it assumes exists: "distinct visible declarations
competing for the same carrier-qualified case, domain, or machine name
reject without inherent-declaration or import-order priority", and
"the exact domain owner and carrier owner remain independent".

The consequence is user-facing, not a corpus artifact. An ordinary
program compiled without a package declaration cannot declare its own
`pub domain [u8; N]::Utf8`: it and the toolchain-injected
`source/library/std/calling.omg` declaration both carry
`package_identity: None`, so `lowering/domain.rs` mints the same legacy
key `[u8; N]::Utf8` for both owners and `domains.rs` rejects the share as
a cross-owner collision. Every un-packaged program that wants a byte-array
Utf8 domain of its own meets this, whether or not any fixture does.

The implementation currently keeps the conservative refusal, which is the
right default while the rule is unstated. Two obvious identities are
already ruled out as non-portable -- host paths and source-order numbers --
and no admissible third is named, which is what makes this an owner
decision rather than an engineering one.

Answering it settles whether an unmanaged root receives a derived portable
identity (and from what: declared root name, content digest, or an
explicit authored key), or whether equal carrier-qualified domain paths
across unmanaged owners stay forbidden and the toolchain library's
declarations are exempted instead. Until then MODULE-NAMESPACE-RESOLUTION
keeps `domains.rs`'s independent collision rejection.

### Q7

**Which half owns the step from a materialization plan to the post-handoff
writer program?** (named decision: `post-handoff-writer-ownership`).
The [Psi/Omega firewall](AGENTS.md#the-psiomega-ownership-firewall) gives
Omega "provider selection, ... target realization, ABI, native emission,
and execution machinery" and keeps Psi target-neutral. The post-handoff
writer carriers (`PostHandoffWriterPlan`,
`GeneratedPostHandoffWriterFragmentPlan`, `PostHandoffWriterInvocationPlan`,
the step rows) live in `psi/foundation/layout-plans/src/post_handoff_writer/`,
whose own doc calls them "the step/plan carriers a provider replays after
control leaves the loader", and that crate both derives them
(`SymbolicMaterializationPlan::derive_post_handoff_writer` maps
`MaterializationAction::{ResolvedWrite, RuntimeWriter}` into writer steps)
and executes them (`PostHandoffWriterPlan::{lower_reusable_fragment,
validate, execute}` and `apply_post_handoff_writes_atomically` over the
crate-private stored-integer write and fit validators). Five Omega
consumers then each re-own a same-named module (`program-entry-plan`,
`isa-x86_64`, `isa-aarch64`, `executable-installation`, plus the
`external-roots`/`provider-planning` readers). No other Psi crate reads
the carriers. An architecture sweep (2026-09-21) stopped here because the
move is not mechanical: either the writer program is a Psi layout artifact
(a target-neutral description of writes a loader-independent replayer
performs, in which case only the *execution* and the ISA-specific fragment
encodings belong to Omega), or it is Omega execution machinery (in which
case Psi's materialization plan stops at `MaterializationAction` rows and
Omega derives, validates, and executes the writer, taking the
stored-integer write/fit validation with it). Options:

- (a) Psi owns the writer *plan* (derivation and validation stay in
  `layout-plans`, as a layout artifact keyed on placement vocabulary);
  Omega owns its *execution* and fragment encoding: `execute`,
  `apply_post_handoff_writes_atomically` and the reusable-fragment ABI move
  to `backend/plans/program-entry-plan` as the root concept, and the ISA
  and installation modules are renamed to what they contribute. Keeps
  Psi target-neutral without duplicating validation. Recommended default.
- (b) Omega owns the whole step: `derive_post_handoff_writer` and the
  carriers move to Omega, Psi's materialization plan ends at
  `MaterializationAction`, and the stored-integer validators become a
  public `layout-plans` query API that Omega calls. Simplest crate graph,
  but Psi then exports validation for a program it never sees.
- (c) Status quo, documented: the writer program is declared a Psi layout
  artifact end to end, the five Omega same-named modules are renamed to
  their contributions, and the firewall text gains the sentence that
  loader-replayed layout writes are Psi-owned. No move.

Until answered, the sweep leaves the carriers where they are and renames
nothing; the crate-root roster keeps `post_handoff_writer` as a
`layout-plans` area.

### Q8

**Is a routed `Binding<R>` carrier moved or copied when a machine passes
it by value out of its own storage?** (named decision:
`service-carrier-argument-multiplicity`). Since 32f5182254 the only
service value spelling is the intrinsic `Binding<R>` carrier, and the
value-custody check treats it like any non-copy data value: the pass
canary `capabilities/uses_caller_folder`
(`self.librarian.archive(self.folder)`, where `archive` takes
`folder: Binding<Folder>` by value) is refused with "cannot make a
boundary or service call at statement 0 while `self.folder` is absent:
restore the value moved out of borrowed storage" and "cannot transfer a
non-copy value out of borrowed storage without replacing its owner". The
fixture was written when the field was a bare boundary trait, which the
checker treated as a capability handle rather than an owned value, and it
is the corpus's only by-value hand-off of a `Binding<R>` field from a
`&mut self` receiver. Options:

- (a) A routed carrier is a capability handle: the checker classifies
  `Binding<R>` as copy in argument position (the callee receives the
  same routed slot; nothing is vacated), and the fixture stays as
  written. `type_multiplicity` learns the carrier the way it learns
  integers.
- (b) A routed carrier is an affine value: by-value hand-off out of
  receiver storage is a move, the fixture is rewritten to lend
  (`archive(&self, folder: &Binding<Folder>)` and
  `self.librarian.archive(&self.folder)`), and a fail canary pins the
  refusal.
- (c) Status quo, documented: the refusal stands and the canary is
  re-rostered as a fail control until (a) or (b) is chosen.

Until answered, `checked_only_capability_canaries_compile_in_isolation`
stays red on that one fixture and the architecture sweep does not touch
the multiplicity classifier.

### Q9

**Which three Terminal operations does the Unit body composition need
before the remaining pass canaries can lower?** (named decision:
`terminal-vocabulary-for-unit-bodies`). The Unit body producer now
composes every single-state body through one statement-sequence
producer (c3383bfcbe, a601f34f7a, 4ffc6e387b, e95fd0382d, 9de580c45d);
the corpus fixtures it still omits stop at shapes Terminal has no
operation for, so no per-statement emitter can close them:

- (a) *Cleanup-free structural field replacement.*
  `self.event.kind = EventKind::Treasure;` and the depth-1
  `self.facing = Direction::South;` store a payloadless case into a sum
  field. `StructuralScalarFieldStore` cannot carry a `Structural` field
  and `StoreStructuralField` is spec- and verifier-gated
  (`validation/borrowed_windows.rs`, `BorrowedStorageRepairMismatch`) to
  reseating an open `MoveStructuralField` window. Missing: a store into a
  never-vacated, disposal-free structural field. Blocks
  `control_flow/composite_field_guard_dispatch`,
  `control_flow/runtime_case_member_dispatch_exit`,
  `runtime_string_literal_dispatch_exit`.
- (c) *Recast views.* `let d: &Desc = &self.buf[8] as &Desc;` (the eleven
  `recast/*` fixtures): validation admits the reinterpreting view only in
  `let` position, and neither the checked Unit plan nor Terminal has an
  operation that yields a `&T` referent over a byte offset of a region.
- (d) *Fresh linear claim establishment.* Every `ownership/linear_*`
  fixture (the nine `reports_and_capabilities` reds) builds its
  `Receipt [linear]` by literal. `EstablishRecord` is claim-free by spec
  (`operations.rs`) and by the verifier (`validation/record.rs` rejects a
  Linear result or a nonempty `result.claims`); Terminal's only claim
  sources are entry claims, call `returned_claim_transfers`, and
  boundary-route minting. `checked-trees-to-lowered-psi` refuses "fresh
  structural value cannot create linear custody". Missing: an
  establishment that mints one fresh root claim for a `[linear]` record,
  retired by its terminal consumer or transferred like an entry claim.
  Pinned at plan level by
  `tests::flow::terminal_unit::linear_local_consumers::a_fresh_linear_literal_still_stops_at_its_local_binding`.

Each is a vocabulary addition with a verifier rule, not a producer
change. Until answered, the producer keeps refusing these shapes by name
(`structural field store: record literal field`, `local data:
structural call binding`, `structural call binding`) and the affected
fixtures stay on their measured-stage rosters.

### Q11

**Does a bare case name denote its case in value position?** (named
decision: `bare-case-value-names`). `tests/omega/pass/arithmetic/bare_name_scopes`
asserts `let signal: Light = On;` "must be accepted" for
`data Light { case Off; case On; }`, and resolution already folds the
qualified `Light::On`. The contracts checked are silent on the bare
spelling: [modules](wiki/spec/language/modules.md) lists bare-name
resolution as locals, state parameters, machine parameters, receiver
fields, imports, then qualified paths, with no case step;
[data and literals](wiki/spec/language/data_and_literals.md) says a case
"implicitly declares its same-named domain" so `Type::Case` denotes it,
and that cases share their carrier's member namespace;
[expressions](wiki/spec/language/expressions.md#operator-families) lets an
unqualified name establish a semantic home only for operator families.
Validation's `exact_case_reference_owner` refuses one-segment references,
so the Unit producer never reaches its `Case` route and the fixture stops
at `local data: structural call binding`.

Options: (a) a bare case name resolves against the expected type's
carrier only (contextual, like the declared type of the `let`), after
every lexical binding, and rejects without an expected carrier; (b) a
bare case name resolves like any imported declaration, visible when its
carrier is visible, with the modules collision rule; (c) bare case names
are not values; the fixture is rewritten to `Light::On`. (a) and (b)
change what a one-segment name means to proof narrowing and interpreter
equality, which today treat it as a binding only.

### Q12

**Does an owned `self` receiver retire implicitly at a successful exit
edge, and does that depend on its multiplicity?** (named decision:
`owned-self-receiver-implicit-retirement`).
[Terminal ownership](wiki/spec/terminal-psi/ownership.md) is the contract
checked and found silent: it says every incoming owned obligation on a
returning edge "occurs exactly once in the edge's transfer map, explicit
terminal consumption, eligible automatic cleanup, or validated no-code
affine discard" -- four accounted routes. Implicit retirement by reaching
normal completion is a fifth, and the clause neither names nor forbids it,
so the spelling that decides two live interpreter tests is not written
down anywhere.

The implementation has already chosen, without the clause saying so.
`6e8cb1f85c` added `consume_terminal_self_receiver` to
`terminal-verifier/src/validation/frontier/terminators.rs`, guarded on
`parameter.is_self && parameter.access == Owned` and NOT consulting
`multiplicity`, so an Affine owned receiver leaves `frontier.owned_places`
before `validate_scalar_cleanup_actions` runs. An authored `DiscardRoot`
on that receiver is then rejected as `ScalarReturnAffineDiscardsMismatch`
because the place it names is already gone.

Why multiplicity is the crux rather than a detail: for a Linear receiver,
"the machine consumes it by completing" reads coherently against the
clause, because a linear value must be used exactly once and completion is
that use. For an Affine one the discard is precisely the observable,
charged disposition the clause's third and fourth routes name, so removing
the place silently deletes the accounting those routes exist to require.
A `git bisect` over roughly four days names `6e8cb1f85c` as first-bad for
`terminal-interpreter::unit affine_cleanups::scalar_return_performs_affine_discard_only_after_edge_charge`
and `::conditional_commits_only_the_selected_affine_cleanup_after_edge_charge`,
which exist to observe edge-charge ORDERING and have been red on main
since; they are the decision's witnesses, not stale fixtures.

Options: (a) the implicit route is Linear-only, and an Affine owned
receiver keeps its authored discard as a charged disposition -- the two
tests then pass unchanged and the guard gains a multiplicity test;
(b) the route stays multiplicity blind, and the spec gains a fifth
accounted disposition naming what an owned receiver occurs in at a
returning edge -- the two tests are then rewritten to state that, not
merely repinned. Either way the clause above must end up saying which,
because an implementation that silently adds a route the ownership
contract enumerates is the thing this queue exists to prevent.
**OWNED-SELF-RECEIVER-AFFINE-DISCARD** on `TASKS.md` holds the engineering
once the ruling lands.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
