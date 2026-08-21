# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-21.

## Q1 — UEFI physical handoff versus semantic program-storage entry

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

## Q2 — Native logical-fuel meter ABI and continuation

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

## Q3 — External-entry stack-domain accounting

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

## Q4 — Progress-profile classification and premise attachment

Termination guarantees can retain sealed `ProgressProfileId` premises, but the
ordinary domain and routed-requirement surface does not distinguish a progress
profile from another predicate-free qualification. Choose the explicit
classification or exact closed inference rule, and choose whether the premise
attaches to the machine guarantee or to a selected operation/provider edge.

The decision must bind establishment to the profile owner or explicit acceptance
authority and to an admitted grant/receipt, while keeping private ranking
witnesses outside public contract identity. Generic routed/domain requirements
must not become progress premises merely because they are predicate-free,
provider-backed, or mentioned by a terminating machine.

## Q5 — Registered-callback private placement contract

The settled callback surface selects a named static machine through
`where machine Selected satisfies Trait::requirement`. The requirement's
evaluated `BoundaryEntryPlan` completely describes the *inbound callback* ABI,
but it does not identify where an outbound registrar expects the generated
private entry pointer. The current external-binding schema describes only the
registrar mechanism and its ordinary runtime parameters; a static-machine
ordinal is not a native argument ordinal, and the guides permit either a direct
native argument or a nested field.

Choose the authored binding form that maps each nominal callback binder to one
exact private placement destination. In particular, decide whether the row is
part of the registrar's calling plan, its external-binding declaration, or a
separate normalized materialization plan; how it denotes a direct argument
versus a nested field; and how that destination participates in compatibility
identity. The compiler must reject missing, duplicate, or shape-incompatible
destinations and materialize only a private relocation. It must not infer a
slot from static-machine order, append an undeclared ABI parameter, or expose
the generated symbol as source-visible `addr` data.
