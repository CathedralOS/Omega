# Terminal Psi product

This is the current portable-product contract, independent of the implementation
language. The [remaining vocabulary reference](../../pre_migration/architecture/pipeline/terminal_psi.md)
still carries operation-specific details not yet consolidated here.
[Boundary calls](boundary_calls.md), [byte views](byte_views.md), and
[observations](observations.md), and [verification](verification.md) have their
own references. [Mathematical proof values](mathematical_values.md) and
[integer certificates](integer_certificates.md) define their proof vocabulary.
Implementation coverage is not implied by a specified
contract; unfinished integration belongs on the [execution board](../../../TASKS.md).

## Semantic boundary

Psi owns source parsing, resolution, typing, checking, proof construction,
expression lowering, selected target-neutral optimization, and publication of
Terminal Psi. Omega consumes that product and owns provider selection and
installation, target realization, its own optimization phases, ABI lowering,
native emission, and execution machinery. Parsing is not an Omega backend step.

Terminal Psi is immutable, self-contained, concrete, and post-instantiation.
Its meaning requires no source syntax, typed-tree arena, producer process,
instruction selector, or generic execution machinery. Execution, proof
questions, evidence, diagnostics, fuel, and lowering refer to the same stable
values, places, operations, and edges. Each executable choice affecting behavior
or obligations has a closed static identity. Author-declared hardware geometry
remains semantic; target-selected layout, ABI classes, registers, storage regions,
and instructions are later realization facts.

The reference interpreter executes the portable semantics directly. Native
code realizes the same semantics. Differential agreement is evidence about
implementations, not a definition of the language.

## Publication and separate consumption

Compilation may stop at Terminal Psi. A later interpreter or native lowerer
may consume it in a different process, on another machine, under separately
supplied authority. The first-party source-to-native path crosses this same
boundary; checked frontend state is not a privileged alternative input.

The portable envelope contains canonical semantic, proof, and pre-Terminal
optimization-execution sections, plus an optional canonical debug section.
Program meaning, replaceable proof evidence, installation decisions, and debug
information have distinct identities. The manifest is reconstructed from the
sections, not accepted as redundant authoritative input. Proof improvement
does not change program identity.

Optimization-execution evidence retains the exact Psi-local selection and
the input and output semantic/proof identities. The manifest binds its strong
identity; decoding rejoins the recorded output to the decoded semantic and
proof sections. Producers and consumers use the current pre-release vocabulary;
stale encodings reject rather than being guessed compatible.

A consumer decodes and verifies the canonical sections before creating
resumable execution state or native realization requirements. A public
in-memory module cannot bypass artifact admission. The verifier reconstructs
the full obligation set from semantics and fingerprinted contracts; the proof
bundle cannot select a smaller set. The kernel checks that evidence. An accepted
fact is decided by a total kernel judgment, proved by checked evidence, or
explicitly admitted at a sealed site allowed by the active profile.
Unsupported entailment rejects.

An authoritative vocabulary extension must define encoding, execution,
reconstructed obligations and authorized admissions, proof rules and their
soundness argument, interpretation, lowering requirements, and fuel identity.
An incomplete consumer rejects unsupported forms; a metadata-only producer
does not establish complete executable support.

## Build-owned companion

Target/profile, ProgramEntry, target-constrained provider plans, external-binding
requirements, compiler-builtin proposals, and pending Omega optimization
selections travel beside the target-neutral module in an exact owned companion.
They are not executable semantics inserted into Terminal, and possession does
not grant realization authority.

The companion binds the pending selection to the complete build-selection
identity. Terminal independently retains the Psi selection already executed.
A receiving consumer must admit the exact proposed pending selection using its
own target catalog and local authority, or reject realization. Rejection does
not invalidate the portable semantics. With no target companion, subsequent
physical selection belongs to the receiving authority.

Source-declaration receipts may separately establish author intent, entry
identity, provider selection, and target closure. They must rejoin the canonical
artifact; they cannot supply executable choices absent from Terminal.

## Optimization ordering

The stage order is lowering, selected Psi optimization, then Terminal
publication. An empty selection is the identity transformation of the same
phase: validate input and output without constructing rule candidates,
transformations, or pass manifests. Selection does not choose an alternate
pipeline.

Later Omega phases optimize their own representations while retaining the
canonical Terminal identity. They cannot mutate the published module or
reselect a Psi/checked-tree pass. Rollback may subtract a selection only at a
phase the invocation actually executes; the report distinguishes effective
Psi selection from pending native selection.

A schema-derived semantic plan belongs in Terminal only when its carrier and
validator are representation-level or lower, contain no backend-owned types,
and are independently reconstructible before assignment or emission.
Assigned homes, final call placement, relocations, and emitted bytes remain
later facts.

## Accounting and subsequent products

Semantic/program identity and fuel-schedule identity are independent.
Changing a fuel schedule changes accounting, not program meaning. Semantic
caches bind semantics and program identity; cost evidence additionally binds
the schedule. [Logical work](../resources/logical_work.md) and
[spatial resources](../resources/storage.md) own those separate contracts.

Executable publication stages and replays retained image bytes. Installation
is a further authority-bearing operation. Neither native lowering nor the
portable companion silently performs deployment admission.
