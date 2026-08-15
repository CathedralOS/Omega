# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-08.

## Q1 — Fixed-operator surface-binding syntax

Named `operator` declarations are the semantic identities behind fixed surface
tokens such as `+`, `[]`, and `[..]`. The language guide previously wrote this
association with a `spelling` clause, but that keyword and clause shape were
never approved and are not part of the settled language.

Choose the source form that binds a fixed operator token to a named declaration.
The decision must settle where the binding appears relative to the signature and
contract, how punctuation-shaped tokens such as `[]` and `[..]` are named, and
whether one declaration may bind more than one fixed token. It must preserve the
settled semantics: the named path remains canonical, resolution is static and
operand-directed, the public signature and proof contract remain visible, and a
`boundary operator` differs only in how its implementation is supplied.

## Q2 — UEFI physical handoff versus semantic program-storage entry

`ProgramStorageEntry::enter` canonically introduces image and initial-storage
roots, and the UEFI target slot currently requires its selected source machine
to expose exactly those two qualified `Extent` parameters. The Cathedral boot
contract separately records that firmware actually invokes the PE entry with
`ImageHandle` and `SystemTable`, that the semantic roots are not additional
firmware arguments, and that a generated bridge must preserve that real
invocation while installing the roots. The current target entry-shape carrier
cannot state both surfaces.

Choose how a target entry schema composes platform-private physical inputs with
portable semantic arrival requirements. In particular, decide whether the
selected source continuation receives both sets of values, receives only the
semantic roots while platform inputs install separately selected providers, or
binds a second target-owned slot for the platform handoff. The decision must
keep `ProgramStorageEntry::enter` as the sole root-introduction requirement,
avoid treating firmware handles as `Extent` values, and leave the generated
bridge with one exact auditable physical ABI and source-visible shape.

## Q3 — Sealed local-capacity declaration form

Compiler provisioning may originate a program-local content root only from an
owner-authored sealed declaration with declared capacity. The semantic model
settles that this is a compile-time root origin, not a runtime establishment or
provider issuance, but no approved source declaration form identifies the
owner, capacity algebra/value, qualification, and authorized establishment
route.

Choose that declaration form and where it may appear. The decision must keep
the declaration owner-unique and sealed, make its finite capacity explicit,
define whether it provisions one root or a declared family of roots, and bind
the resulting account to an exact qualification and establishment route.
Ordinary construction, proof terms, and firing a checked runtime route must not
be able to reproduce the provisioning evidence.

## Q4 — Write-only memory view

Foreign providers sometimes receive storage they may initialize or overwrite
but must not read. Omega's settled reference surface has only shared read and
exclusive read/write borrows, while placed-field accessors model individual
write-only operations rather than a contiguous memory view. Treating a
write-only foreign parameter as `&mut T` would therefore grant more authority
than the binding declares and may expose preexisting or uninitialized bytes.

Choose the core representation and source form for a write-only memory view.
The decision must settle whether it is a third borrow kind, a nominal core view,
or an operation-capability value; how it projects and subdivides; whether and
when it may cover uninitialized storage; and what evidence turns a complete or
partial provider write into readable initialized content. It must preserve
ordinary lifetime and nonaliasing checks without implying provider read
authority, and it must remain distinct from `Placed<P, T>` field accessors and
from durable custody transfer.

## Q5 — Static callback-parameter requirement form

Registered-callback lowering requires a foreign operation to bind one static
machine-parameter position to one exact boundary callback requirement. The
implemented `where machine Selected(...)` clause carries only a structural
callable contract; signature coincidence or a selected machine's unique
conformance cannot establish the nominal relationship. `invokes` is also not
that relationship: it declares synchronous entry before return and therefore
cannot type a deferred durable registration.

Choose the source form and checked representation for this binding. The
decision must settle whether it belongs on the machine-parameter declaration,
its `where machine` contract, or the foreign operation parameter; how trait
requirements and satisfying implementations retain the same exact relation;
and how overload identity is named. It must produce a per-use checked fact
containing the call site, machine-argument ordinal, selected machine, and exact
callback trait requirement so native lowering can place one private relocation
without creating a runtime machine value or exposing a code address.

## Q6 — Native logical-fuel meter ABI and continuation

Terminal Psi settles sponsor-owned logical fuel: trusted native lowering charges
before each semantic operation or taken edge, exhaustion is not program-visible,
and replenishment resumes at the unpaid site without replaying completed work.
Native artifacts and installation records now retain the exact schedule, site,
units, and byte interval, but no approved target/runtime contract says where the
mutable budget lives or how exhaustion transfers control and later resumes.

Choose the native meter ABI and continuation contract. The decision must settle
whether the budget is passed explicitly, held in a reserved register, or reached
through sponsor-owned execution context; which layer owns and preserves that
state across ordinary and provider calls; how the slow path reports the exact
unpaid `OperationId` or `EdgeId`; and what non-forgeable continuation authorizes
resume without exposing fuel to the program. It must apply coherently across
x86-64 and AArch64 calling policies, preserve stack/alignment and machine-state
contracts, and keep fixed-fuel meter elision a separately admitted installation
decision.

## Q7 — Terminal-Psi artifact-verifier trust closure

The Rust `psi-terminal-verifier` independently reconstructs the exact obligation
set and invokes `psi-proof-kernel`, but proof-kernel acceptance does not prove
that the verifier reconstructed the right obligations. The canonical
architecture permits three final assurance routes and has not selected one:
a low-rung reference artifact verifier, a reconstruction derivation checked by
the low-rung kernel, or an explicit trust-ledger entry for the Psi-aware verifier.

Choose the deployment-authoritative closure. The decision must name the trusted
implementation or checked derivation format, bind it to the exact terminal-Psi
vocabulary and obligation-reconstruction rules, state how independent agreement
is audited across releases, and ensure that a Psi-hosted port of the proof kernel
cannot be mistaken for reconstruction assurance. If the Rust verifier is made
explicitly trusted, its enumerable primitive judgments and artifact-decoding
dependencies must enter the executable trust ledger rather than being implied by
successful certificate checks.

## Q8 — Erased-evidence establishment for placed views

`Placed<P, T>` may interpret backing as a semantic `T` whose declaration
contains `[erased]` fields. The settled representation law gives those bindings
no offset, size, or transfer, and admission proves only demand/supply
compatibility; it does not establish that `T` or its erased terms inhabit the
place. The current design does not say how Stable adopt/initialize/validate or
External adopt supplies those exact terms.

Choose the source contract and checked representation that establishes each
erased binding on every placement-establishment route. The decision must bind
evidence to the exact nominal `T`, normalized placement,
extent/content/revision, and source or provider derivation; define projection,
multiplicity, lifetime, provenance, retirement, and invalidation under permitted
writes; and prevent raw bytes, admission alone, or a layout/access policy from
manufacturing proof. Physical `LayoutPlan`, `AccessPlan`, offsets, and transfers
remain erased-stripped.

## Q9 — Provider-neutral interrupt acknowledgement settlement

`InterruptAcknowledgement in Pending` is the semantic debt carried from one
hard-interrupt arrival to its exact completion. The core
`InterruptAcknowledgement::complete` boundary currently publishes `PortIo`,
which fits the legacy PIC realization but falsely requires port-I/O authority
from LAPIC/x2APIC providers whose acknowledgement is a `MachineControl`
operation. Cathedral must not widen an x2APIC implementation to `PortIo` merely
to satisfy that hardcoded effect.

Choose how the provider-neutral acknowledgement requirement selects and
publishes its realization-specific effect without leaking PIC or APIC mechanism
into the semantic acknowledgement type. The decision must preserve the exact
pending-to-completed linear transition, reject forgotten and duplicate
completion, retain the selected provider and operation in checked/terminal
evidence, and expose only the authority actually used by that realization. It
must compose with both port-I/O PIC EOI and machine-control LAPIC/x2APIC EOI,
without treating either mechanism as universally reachable.

## Q10 — Trapping arithmetic inside contract predicates

`Trapping` arithmetic has settled runtime behavior: invalid counts, overflow,
and the other policy-defined failures trap instead of producing a value. A
`requires`, `ensures`, or `crashes` predicate is specification evidence rather
than an ordinary runtime expression, however, and the language does not define
what happens when evaluating a trapping arithmetic subterm would trap.

Choose whether trapping arithmetic is permitted in contract predicates and, if
it is, what proposition it denotes on a trapping input. In particular, decide
whether the trap contributes an exact `Trap` crash route for the governed
machine, makes the proposition partial or false, or requires separate
nontrapping evidence (collapsing that occurrence to Exact behavior). The ruling
must define how body execution and specification evaluation relate, how callers
reason about the trap edge, and what explicit term/effect and proof obligations
terminal Psi carries. The compiler must not silently treat a potentially
trapping contract term as a total mathematical operation.

## Q11 — External-entry stack-domain accounting

Terminal-Psi stack evidence derives the exact closure below a selected machine
entry, but an external root also consumes provider-specific adapter and hardware
arrival state. The provider must size that state; the current `EntryStack`
contract does not say which stack domain owns it when entry interrupts an active
stack and may switch to a dedicated or provider-selected stack. Adding all bytes
to the terminal closure would silently assume one domain and can misstate both
nesting peaks and dedicated-stack provisioning.

Choose the resource evidence and composition rule for external-entry overhead.
The decision must distinguish adapter frames from hardware arrival state when
they occupy different domains, identify the interrupted and post-switch stack
for each portion, define alignment and maximum nesting behavior, and bind every
provider-authored size to validated entry-stub/arrival evidence. It must compose
with `EntryStack::{Interrupted, Dedicated, ProviderSelected}` without placing
OS-specific interrupt-frame vocabulary in the language or treating a numeric
provider assertion as compiler-derived terminal evidence.

## Q12 — Generic conformance application syntax

A named conformance may own lifetime, type, const, and static-machine binders,
and its subject and trait application may depend on them. The language guide
settles declaration syntax such as
`SequenceEncoding<Element, Message>: Vec<Element> satisfies
WireEncodable<Message>`, but it does not settle how one concrete instantiation
is passed in an evidence-argument position. The current flat call telescope can
name `SequenceEncoding` but cannot delimit that conformance's own argument pack,
and silently inferring every omitted argument would make const and
static-machine evidence selection implicit.

Choose the call-site form for applying a generic conformance name. The decision
must delimit the conformance's own telescope from the enclosing machine's type,
const, static-machine, and evidence arguments; state which arguments, if any,
may be inferred from the expected subject and trait application; and define how
lifetime arguments erase while remaining part of semantic identity. The result
must select one exact package-scoped conformance instance and normalized row
map without visibility search, priority, or ambient uniqueness.

## Q13 — Erased evidence-term multiplicity

Named `requires` and `ensures` clauses expose erased evidence terms, and an
output-package pattern may eventually write `_` for a field it explicitly
discards. The settled guide says evidence retains ordinary multiplicity and
that linear evidence may not be discarded, but no approved source or
derivation rule assigns a multiplicity to an evidence term. The current
checked and terminal carriers retain proposition, interface, lane, and term
identity without a multiplicity field.

Choose where an evidence term's multiplicity originates: the proposition
declaration, its carrierless evidence interface, each named contract binding,
the selected producer conformance, or another explicit declaration. The
decision must state the default, require compatible multiplicity when terms
are forwarded through `requires`/`ensures` lanes and generated output
packages, and define exactly when `_` is a legal discharge. Erasure must remain
independent of multiplicity, and neither producer choice nor runtime layout
may silently weaken exact-use evidence into discardable evidence.

## Q14 — Generated evidence-output package identity and projection

The immediate output-package rung destructures one complete unconditional
package at its call site, but the retained and outcome-guarded package model is
not settled. The guide promises an inferred, source-unnameable nominal type
derived from the producer machine, runtime result, named evidence fields,
propositions, and outcome guards without defining its canonical identity,
binding lifetime, projection ownership, or proof-artifact representation.

Choose whether nominal identity belongs to one normalized machine application,
one call site, or another exact origin, including substitutions and guarded
outcome variants. Define how `let package = call()` binds a zero-layout or
runtime-bearing unnameable value; whether projecting `value` or an evidence
field borrows, copies, moves, or partially consumes it; what remains valid after
each projection; and which origin, field, outcome, and exact evidence-term rows
Terminal Psi must retain and verify. Q12 continues to own generic conformance
application syntax. Q13 continues to own evidence multiplicity, use counts,
residual-field discharge, and `_` legality.
