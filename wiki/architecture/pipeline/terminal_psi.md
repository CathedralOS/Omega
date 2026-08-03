# Terminal Psi Architecture

[Pipeline](pipeline.md)

Status: target architecture settled 2026-08-02. This document records the
implementation cut and migration from the current bootstrap pipeline. The
semantic and evidence contract is owned by
[`canonical_ir_fuel_and_resource_provisioning.md`](../../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

Implementation status (2026-08-02): `compiler/psi-rs` is the Psi-owned
workspace root. The first source-facing ownership slice is live:
`psi-source` owns loaded-source records/maps, identities, byte spans, and
source-backed text;
`psi-tokens` owns token streams; and
`psi-source-files-to-tokens` owns the Omega lexer without depending on any
Omega crate. The old Omega-named frontend crates are thin compatibility
exports for remaining legacy consumers. `omega-compiler` invokes the
Psi-owned lexer, parser, resolver, typer, checker, and source representations
directly, although its legacy backend still consumes checked semantics until
general terminal production replaces that early cut. `psi-core` provides nonzero stable semantic identities, the
typed scalar proposition vocabulary, and a module-owned value-typing context.
`psi-proof-kernel` provides total primitive judgments, structural proof
checking (including semantic-axiom citation and typed equality transitivity),
versioned certificate envelopes, and exact profile-authorized admission
validation. Admission cannot replace a primitive derivation, and architecture
tests reject any Psi dependency on Omega.

The first in-memory executable slice is also live. `psi-terminal` defines a
versioned module with stable machines, blocks, values, operations, edges, and
bodyful contracts. Frozen semantic v1 contains representable integer constants;
v2 adds Boolean constants; v3 adds exact-width wrapping integer addition; v4
adds exact-width saturating integer addition; v5 adds exact-width
wrapping integer subtraction; v6 adds exact-width saturating integer
subtraction; v7 adds exact-width wrapping integer multiplication; v8 adds
exact-width saturating integer multiplication; v9 adds proof-only
structural-place declarations and content-conservation propositions; v10 adds
canonical identity-preserving claim reshuffles; v11 adds distinct stable
sum-case segments to structural content paths; and current v12 adds exact
authored-partition substitution witnesses. None of v9-v12 adds an executable
operation. Every executable slice uses unconditional jump and return edges.
`psi-terminal-verifier` rejects malformed identities, types, contract scopes,
cycles, unreachable fact sources, and missing/extra evidence, reconstructs the
exact operation/edge/return axioms, and checks every `ensures` from a separate
proof bundle. `omega-interpreter` executes only a `VerifiedTerminalModule` on
this path.

The checked-frontend migration also keeps the ownership firewall explicit:
`psi-checked-trees` now owns the target-neutral checked representation and
`omega-checked-trees` is a compatibility export. Target-neutral facts and
effect summaries are likewise Psi-owned, while concrete selected provider
plans and target/layout-specific task activation plans are Omega-owned and
travel as orchestration sidecars. `CheckedTrees` does not embed that
target/provider realization state. `psi-typed-trees-to-checked-trees` now owns
semantic checking and checked-fact construction. Its validation and proof
dependencies live in `psi-validation` and `psi-proof`; the corresponding Omega
packages are compatibility exports. Provider installation and approval remain
Omega concerns, and Omega orchestration runs that admission explicitly after
the Psi check.

The first Psi-owned terminal source producer is live as
`psi-checked-trees-to-terminal`. It accepts one exact free-machine slice:
typed integer constants, one unconditional literal-carrying state jump, one
literal return, and a matching closed `requires`/`ensures` pair. It rejects all
other checked-tree shapes. The source canary discards `CheckedTrees` before it
verifies and executes the produced semantic module, proving the artifact has no
frontend lifetime dependency. This is the correct ownership direction, but its
accepted vocabulary remains the original integer/control/contract canary. An
architecture test keeps one fail-closed `lower_machine` entry. General terminal
production must extend this Psi stage rather than reintroduce an Omega-to-Psi
bridge. The stage now independently revalidates and lowers checked content
conservation, identity reshuffles, and direct partition compositions into the
existing v9-v12 terminal vocabulary. Those evidence translators retain stable
semantic paths, dense claim identities, source theorem fingerprints, and exact
place substitutions. The current executable source canary itself remains
content-free.
The current legacy exit prover also cannot establish an ordinary
`result == literal` contract, so the bootstrap canary carries the closed typed
fact `7i32 == 7i32` and asserts the executed result separately. An Omega
source-independent consumer is also live:
`omega-terminal-psi-to-abstract-operations` accepts only a
`VerifiedTerminalModule` and produces an owned stream of scalar materialization,
wrapping-add, saturating-add, wrapping-subtract, saturating-subtract,
wrapping-multiply, saturating-multiply,
jump-binding, and return requirements with stable Psi provenance. Its function records also retain declared runtime parameters
and the result pseudo-value with exact scalar types. Neither it nor
`omega-terminal-abstract-operations` depends on
checked/typed trees, `ExpressionHandle`, or the legacy source-shaped abstract
operation plan.

The first target/native realization is live on the same clean lane.
`omega-terminal-abstract-operations-to-target-operations` resolves the verified
compile-known scalar operations and jump bindings into a target immediate
return while retaining every contributing Psi operation and edge identity. It
also uses the established native call planner to select AAPCS64, System V
AMD64, or Microsoft x64 register/incoming-stack locations for runtime scalar
parameters. Direct parameter returns stay explicit; parameter-fed wrapping and
saturating addition plus wrapping and saturating subtraction and wrapping and
saturating multiplication lower to recursive, exact-width
target expressions.
`omega-terminal-machine-emission` emits ordinary scalar-return code for
AArch64 and x86-64 and rejects non-native integer widths.
`omega-terminal-image-emission` then constructs an owned, canonical-order
object artifact whose function spans retain terminal-Psi provenance and exact
semantic identity. It emits the compatibility Omega object container and
standalone ELF/AArch64, ELF/x86-64, Mach-O/AArch64, and PE/x86-64 images through
the shared image model and writers. Parameter-return emission supports Boolean
and 8/16/32/64-bit integers in selected native registers or incoming stack
slots on both architectures. Runtime wrapping/saturating addition,
wrapping/saturating subtraction, and wrapping/saturating multiplication support
signed and unsigned 8/16/32/64-bit operands, recursive expressions, and mixed
immediate/register/stack leaves. The AArch64 emitter preserves every referenced
argument register in an aligned local spill frame before evaluating into `x0`;
both emitters compensate incoming-stack addresses for their expression stack.
The relocation-free slice requires exact
final text, complete provenance-bearing compiler regions, and no unclassified
executable gaps. The source, Boolean, wrapping-add, and saturating-add canaries
drop all producing semantic and lowering state before artifact emission; the
host linker harness executes the retained entry bytes, and the macOS host
canary also executes the emitted Mach-O image directly. General register
assignment and migration of the legacy backend remain outside this checkpoint.

Canonical semantic serialization and identity are now live for this initial
vocabulary in `psi-terminal-codec`. The real-source canary encodes the semantic
module, records its identity, discards the source and producing module, decodes
a fresh module and proof bundle, validates their section manifest, and then
drives verification, interpretation, and native realization. The v3 wrapping
canary independently round-trips, verifies, meters, lowers, emits, and executes
`u8` 200+100 as 44 after producer state is discarded; the v4 saturating
canary follows the same path and clamps that sum to 255. A frozen-v1
nine-parameter canary forces its returned `u8` through the host incoming-stack
ABI and matches interpretation at 77. A v4 nested runtime canary wraps
a register and ninth-argument stack `u8`, then saturates with another register
to 255; a signed `i64` canary exercises both saturation bounds. Both agree with
interpretation through real C ABI calls. Branching, the remaining
arithmetic-policy variants, general register assignment, typed debug payloads,
general safe-point/branch fixed-work checking, build-time fuel migration, and
native fuel metering remain next.
The v5 wrapping-subtract canary independently round-trips, verifies,
costs one operation plus one return edge, lowers, and executes parameter-fed
`u8` 5-10 as 251 through a real C ABI call.
The v6 saturating-subtract canary follows the same path and exercises
both signed `i64` bounds through real C ABI calls.
The v7 wrapping-multiply canary round-trips, verifies, costs one
operation plus one return edge, and executes parameter-fed `u8` 20*13 as 4
through a real C ABI call.
The v8 saturating-multiply canary follows the same two-unit path and
executes parameter-fed signed `i64` multiplication through real C ABI calls,
covering positive overflow, negative overflow, `MIN * -1`, and an ordinary
negative product.

## Boundary

Psi operates on Omega-branded source files and owns every target-neutral stage
through one canonical terminal representation. Omega consumes terminal Psi; it
does not feed source-shaped data back into Psi.

```text
Psi
    source files
    -> tokens -> syntax -> resolved -> typed -> checked
    -> lowered expressions, predicates, places, blocks, and edges
    -> terminal Psi

Omega
    terminal Psi
    -> abstract operations -> target operations
    -> assigned instructions -> bytes -> installed image
```

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

## Why no existing stage is the cut

The current implementation has no expression-lowering pass before instruction
selection:

- `CheckedTrees` embeds `TypedTrees` plus checked fact tables;
- `StateGraphCode` copies the typed expression table, and operations and
  transitions retain `ExpressionHandle`;
- `ControlFlowCode` clones the same expression table and mostly remaps the
  graph topology and semantic arenas; and
- abstract-operation construction and instruction selection still inspect and
  substitute tree expressions directly.

`StateGraph` and `ControlFlowPlan` are therefore useful topology and evidence
scaffolds, not self-contained executable representations. Conversely,
`AbstractOperations` already owns runtime storage regions, calling-convention
classes, ABI aggregate distinctions, and other Omega realization concerns.
Removing those fields would not reveal a hidden portable IR.

The missing pass is the boundary: merge the useful state-graph/control-flow
shape and fill it with lowered semantic content. This is not serialization of
today's `StateGraph`, purification of `AbstractOperations`, or a second similar
block IR placed beside `ControlFlowPlan`.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, suspension, and
  boundary actions on edges;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

Author-declared hardware geometry is semantic and may contain offsets, widths,
and alignment. Omega begins where the target chooses native layout, stack and
register placement, ABI classes, concrete storage regions, instructions, and
relocations.

## Psi operation definition

Every operation enters the vocabulary as one reviewed vertical slice:

```text
operation identity and canonical encoding
execution transition
generated obligations and authorized admissions
proof rule / logical interpretation
soundness proof of that rule against the transition
interpreter realization
Omega lowering requirement
fuel identity
```

Operations are statically distinct when execution semantics or generated
obligations differ. Obligation-affecting policy is a closed instruction variant,
not an ordinary value that requires constant folding before verification.
Additional sound proof lemmas may be published without changing operation or
program identity.

The proof kernel, proposition representation, total primitive judgments,
certificate envelope, and admission taxonomy land before an operation depends
on them. Concrete proposition and operation vocabularies are then co-designed
in vertical slices; the proof language is not speculated in isolation.

### Content-conservation proposition slice

The content slice extends structural-place terms with an entry/current version;
it does not add a general historical-expression modality. It carries the exact
owner-unique content-projection identity, canonical
`IntervalSet<CoordinateSpace>` and `CountedQuantity<Unit>` terms, variadic
partial `separate(...)`, containment and equality, and canonical interval-set
residual difference. Sealed claim-frontier rows record content introduced into
or transferred out of checked custody.

The verifier infers identity-preserving reshuffles. A primitive that changes a
partition carries an authored theorem, and checked wrappers compose those
theorems. At a bodyless partial boundary, Psi derives the kept content and
residual and permits the provider to admit only acceptance of custody for that
exact residual—not the partition arithmetic. External root correspondence and
fresh issuance remain scoped admitted hypotheses with provenance; downstream
conservation remains derived.

Implementation checkpoint (2026-08-02): the source-to-checked precursor and
the first terminal proposition slice are live. Exact owner-projection calls,
entry/current structural-place versions, and flattened canonical
`separate(...)` equations retain one schema-stable fingerprint per callable /
algebra in checked facts and proof/debug artifacts. Terminal semantic v9
declares proof-visible parameter/result roots and carries the exact algebra,
semantic domain, projection fingerprint, versioned stable place path, and
canonical equation without any Omega arena identity. Canonical semantic bytes
and minimal proof format v8 are golden-pinned; verifier checks restrict content
propositions to `ensures`, reject invalid roots and `entry(result)`, and accept
replaceable certificates. Identity-preserving reshuffle inference has a
Psi-checked producer: exact input-relative outcome maps derive one fingerprinted
entry/current equality per preserved claim, retaining its claim identity and
both structural paths. The derivation requires the same terminal projection
identity and algebra on both places, accepts type or ordinary contract
qualification, and never synthesizes separated composition across independent
claims. Fresh establishments, mismatched projections, and runtime indices infer
nothing. Terminal semantic v10 carries field/fixed-index rows in canonical
machine-local claim order; semantic v11 adds distinct stable sum-case path
segments, and proof format v9 carries those segments in certificates. The
verifier revalidates one-to-one, non-overlapping
parameter-entry/result-current paths, exact projection and algebra identity,
and reconstructs one content-equality semantic axiom per projection for
replaceable certificates. Terminal semantic v12 adds exact direct-wrapper
partition composition: canonical rows retain the source theorem and
fingerprint, dense participating claims, total structural-place substitution,
and derived theorem. Validation requires an authored separation tree, checks
source and wrapper place roles, binds every derived entry projection to one
listed identity row, and mechanically replays the substitution before exposing
the derived proposition as a semantic axiom. Existing proof format v9 already
represents that proposition. Composition through surrounding non-direct
rewrites, sealed introduction and custody-exit frontier rows, and the general
frontier theorem remain to land.

Design block discovered by the first real-source integration attempt
(2026-08-03): a direct partition wrapper has checked entry claim identities and
an exact partition substitution, but correctly has no one-to-one identity
reshuffles. Aggregate conservation does not establish that either input equals
a particular output. Terminal v12 currently declares `input_claims` only by
referencing `ContentIdentityReshuffle` rows and requires each derived entry
projection to match one, which would force that stronger and potentially false
equality. The producer must continue to fail closed. Before a content-bearing
source canary lands, terminal Psi needs a versioned entry-claim binding that
records claim, projection, algebra, and structural place without asserting an
output equality, or an equivalent reviewed redesign of partition input claims.

Correction checkpoint (2026-08-02): checked-to-terminal content production now
lives in `psi-checked-trees-to-terminal`. It consumes Psi-owned checked facts;
the v9-v12 terminal vocabulary, canonical codec, and verifier remain Psi-owned
and source-independent. The deleted Omega-to-Psi translator must not return.

These normalized obligations are semantic and fingerprinted. Their proof
derivations remain replaceable proof-bundle material.

## Verification boundary

The verifier derives structural obligations from terminal Psi and checks the
fingerprinted author contracts. Every accepted fact is:

- re-decided by a specified total kernel judgment;
- discharged by a checked certificate, carried or reconstructed by a total
  certifying procedure; or
- admitted at a sealed site and accepted by the consuming profile.

Admission cannot replace a derivable obligation. Search that may time out or
return unknown must carry its certificate for portable verification. Primitive
trusted judgments are minimized and each joins the enumerable language
soundness audit.

The semantic module, proof bundle, installation record, and debug/source maps
remain separate. Proof improvements do not change semantic identity; provider
selection and attached evidence do change their own section and container
identities. One execution verifies and runs one complete Psi semantic version.

## Canonical semantic bytes (format v1)

`psi-terminal-codec` owns the canonical encoding of the supported in-memory
vocabularies. Wire format v1 begins with `PSITERM\0`, a little-endian `u16`
format version, and the terminal semantic version. Counts are fixed-width
little-endian `u32`, stable identities are nonzero little-endian `u64`, integer
payloads occupy the full signed or unsigned 128-bit field, and every sum type
uses a closed one-byte tag. This intentionally favors one simple auditable
encoding over density.

Machines, blocks, and v9 structural-place declarations are strictly ordered by
their stable identities; ensures are strictly ordered by obligation identity. Requirements and flattened
conjunction members are strictly ordered by their canonical encoded bytes,
duplicates are rejected, and symmetric equality operands use that same wire
ordering. Content equations order their symmetric sides canonically;
`separate(...)` is flat, sorted, duplicate-free, and exact projection/domain,
entry/current place, field, fixed-index, and v11 sum-case identities are encoded. Nested
conjunctions, proposition nesting, recursive scalar terms, and content terms
deeper than 256 edges are rejected. Execution-significant vectors—parameters, operations, and
jump arguments—retain their declared order.

Decoding fails on unknown versions or tags, zero identities, invalid booleans,
noncanonical ordering/forms, malformed or verifier-invalid modules, truncated
input, and trailing bytes. A successfully decoded module is re-encoded and the
bytes must match exactly; the decoder never normalizes an alternate encoding.
The semantic fingerprint is SHA-256 over a v1 domain separator, the canonical
byte length, and those exact bytes. `TerminalPsiIdentity` contains only the
semantic version and this fingerprint: proof bundles, installation records,
and debug maps are deliberately absent and remain replaceable.

Semantic version 1 is frozen with `IntegerConstant`; version 2 adds
`BooleanConstant`; version 3 adds `WrappingIntegerAdd`; version 4 adds
`SaturatingIntegerAdd`; version 5 adds `WrappingIntegerSubtract`; version 6
adds `SaturatingIntegerSubtract`; version 7 adds `WrappingIntegerMultiply`;
version 8 adds `SaturatingIntegerMultiply`; version 9 adds proof-only
structural places and content-conservation propositions; version 10 adds
canonical identity-preserving claim reshuffles; version 11 adds stable sum-case
content-path segments; current version 12 adds exact authored-partition
substitution rows.
The arithmetic operations require two already defined operands of the exact
result integer type and have distinct canonical recursive proposition terms for
their exact logical results. Validation and execution continue to accept valid
v1/v2/v3/v4/v5/v6/v7/v8/v9/v10 modules under their original meaning, while an older
module cannot claim a later operation or proposition tag.
`migrate_module_to_current` is an explicit validated older-to-v12 translation:
it preserves the graph and obligations, changes the version field, and therefore
creates new canonical bytes and a new semantic fingerprint. An unchanged proof
bundle retains its separate bytes and identity but is verified again against the
migrated module. Golden tests retain the archived v1 through v11 fingerprints
and independently freeze the current v12 fingerprint, v10 identity-reshuffle
fixture, v11 sum-case fixture, and v12 partition-composition fixture.

The same codec gives proof bundles their own canonical `PSIPRF` bytes and golden
fingerprint. Proof format v1 remains the minimal frozen encoding for the
original proposition vocabulary. Format v2 adds the recursive wrapping-add
scalar term; format v3 adds the recursive saturating-add scalar term; format v4
adds the recursive wrapping-subtract scalar term; format v5 adds the recursive
saturating-subtract scalar term; format v6 adds the recursive wrapping-multiply
scalar term; format v7 adds the recursive saturating-multiply scalar term;
format v8 adds content-conservation propositions and field/fixed-index
structural-place terms; format v9 adds sum-case path segments. The encoder
selects the minimal format needed by a carried proof tree, and the
decoder rejects a v2, v3, v4, v5, v6, v7, or v8 bundle representable in an earlier format.
Evidence entries are strictly ordered by `ObligationId`; the
closed encoding covers kernel judgments, separately versioned recursive proof
trees, and exact admission site/authority/evidence/profile identities. Unknown
tags, zero identities or proof versions, alternate evidence ordering,
truncation/trailing data, malformed propositions, and proof/proposition nesting
beyond the v1 bounds reject. Proof-tree propositions retain their exact rule
direction rather than being normalized as semantic contracts, because a proof
section is replaceable evidence and its cited axiom direction is significant.

`TerminalArtifactManifest` binds the canonical semantic and proof identities
plus optional installation and debug section hashes. Each role has a separate
SHA-256 domain, and absent differs from a present empty section. Replacing a
valid proof, installation record, or debug map changes that section and the
container identity while preserving `TerminalPsiIdentity`; validation
recomputes the complete manifest from attached bytes.

The first typed installation payload is live in
`omega-terminal-image-emission`. Wire format v1 begins with `PSIINST\0` and
binds the terminal semantic identity, architecture, object format, pointer
size/alignment, PE subsystem when present, exact profile-decision identity,
strictly ordered selected-provider-plan identities, a domain-separated SHA-256
of the complete emitted image, and the compiler text-validation evidence. Its
decoder rejects unknown versions/tags, zero identities, invalid target facts,
alternate provider order, nonzero reserved fields, truncation, and trailing
bytes, then reproduces the canonical bytes. Validation recomputes the image
binding from the sealed `TerminalExecutableImage`. The scalar canaries carry an
empty provider set because they contain no calls or boundaries; later vertical
slices populate that set from actual selected plans. The record is manifest
metadata, not executable authority and not a replacement for the separate
`omega-executable-installation` admission/placement ladder. Typed debug/source
maps remain a later artifact slice.

## Logical-fuel v1

`psi-terminal-fuel` owns the accounting identity independently from terminal
semantic versioning. Schedule v1 charges one logical unit for each executed
`IntegerConstant`, `BooleanConstant`, `WrappingIntegerAdd`,
`SaturatingIntegerAdd`, `WrappingIntegerSubtract`,
`SaturatingIntegerSubtract`, `WrappingIntegerMultiply`, or
`SaturatingIntegerMultiply` and one for each
taken `Jump` or `Return` edge. The cost table
matches the closed operation/terminator enums exhaustively, so a new vocabulary
variant cannot compile without making its schedule treatment explicit. A
schedule revision changes accounting identity, never terminal semantic bytes or
the program fingerprint.

The interpreter charges before executing each semantic site and returns a
deterministic `TerminalFuelUsage`: total units plus execution count and units
aggregated under stable `OperationId`/`EdgeId` attribution. Its sponsor-owned
meter may be unbounded or carry a finite allowance. Insufficient allowance is a
host result before the unpaid site, leaves usage unchanged, and is not visible
or catchable as a terminal-Psi machine result. The serialized real-source
canary costs four v1 units—two constants and two edges—and retains the same
semantic identity before and after accounting. `TerminalExecution` retains the
exact block/operation cursor and values across that sponsor event; checked
replenishment resumes at the unpaid site without replaying or double-charging
earlier work, including in the serialized real-source/native canary. Build-time
migration, general fixed-work/segment certificates, attributed response
outcomes, and trusted native block metering remain later IRFUEL slices.

The v3 wrapping canary also costs four v1 units: two constants, one addition,
and one return edge. Semantic-version migration therefore does not imply a fuel
schedule change. The v4 saturating canary has the same four-unit shape;
each newly admitted operation is reviewed against the closed schedule table.
The v5 parameter-fed wrapping-subtract canary costs two units: one
subtraction and one return edge. It retains schedule v1 because the existing
per-operation rule already determines that cost.
The v6 parameter-fed saturating-subtract canary has the same two-unit
shape and independently reaches both signed `i64` bounds.
The v7 parameter-fed wrapping-multiply canary also costs two units and
computes `u8` 20*13 as 4.
The v8 parameter-fed saturating-multiply canary costs two units and
reaches both signed `i64` bounds.

`psi-terminal-fixed-fuel` provides the first restricted checker over this same
schedule. Because the supported v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11 control vocabulary currently
permits one acyclic straight-line path, it derives an exact entry-to-return
ceiling with no additional precondition assumptions. The certificate keys the
canonical terminal-Psi identity, entry machine, reached return edge, schedule
identity, and ceiling.
Validation recomputes every field from the verified decoded module; changing
program semantics invalidates an old certificate even when the numeric cost is
unchanged, and a verified but noncanonical module cannot acquire semantic
identity. The source canary's exact four-unit certificate equals measured
execution after source and producer state are discarded. Branch/loop outcomes,
safe-point segments, relevant-precondition subsets, and provider-summary
migration remain later slices.

## Migration plan

1. Continue the established workspace boundary: move or rename the current
   target-neutral parsing-through-lowering crates under Psi ownership while
   retaining temporary compatibility adapters. No parser or semantic checker
   remains on an Omega-to-Psi path.
2. Extend the live stable Psi value, proposition, proof, and place identities
   into the first terminal semantic module without changing the current backend.
   **Initial scalar subsets complete:** frozen v1 integer constants, v2 Boolean
   constants, v3 wrapping integer addition, v4 saturating integer addition, v5
   wrapping integer subtraction, v6 saturating integer subtraction, v7
   wrapping integer multiplication, and v8 saturating integer multiplication
   have verifier, direct-interpreter, canonical-codec, fuel, Omega-lowering,
   and native-return coverage. The runtime-parameter slice covers direct returns plus recursive
   wrapping/saturating addition, subtraction, and multiplication
   expressions over native register and incoming-stack ABI locations. The v9
   content slice has canonical semantic/proof bytes, checked-plan translation,
   and certificate verification; v10 carries and revalidates checked
   identity-reshuffle rows as semantic axioms, while sealed frontier rows remain.
   Executable storage places, general register assignment,
   and the other arithmetic variants remain later slices.
3. Lower the live integer/control/contract slice from Psi checked semantics
   into terminal Psi, add its Omega abstract-operation consumer, and
   compare interpreted/native behavior before broadening the vocabulary.
   **Initial vertical slice complete through native comparison:** the
   fail-closed Psi terminal producer and a real source canary now verify and
   execute after checked trees are dropped, then lower the verified module into
   an owned, source-independent Omega requirement stream, a target
   return-immediate, host machine code, an owned object artifact, and a direct
   host image whose execution matches interpretation. The same exact-text image
   boundary is structurally exercised for all four currently supported
   architecture/format pairs.
4. Add the remaining arithmetic variants, calls, continuations, cleanup,
   conservation inference/frontiers, boundary operations, suspension, and scoped ordering as
   reviewed vertical slices.
5. Move binding substitution and concrete instantiation above terminal Psi so
   no Omega pass consumes source expressions.
6. Re-root the reference interpreter, rebuilding differential-oracle evidence
   during the transition.
7. Re-root abstract-operation construction on terminal Psi, then retire the
   redundant state-graph/control-flow representation and adapters.
8. Freeze canonical serialization and semantic fingerprints only after the
   in-memory vocabulary has passed interpreter and lowering canaries.
   **Initial vocabulary complete:** canonical semantic bytes and identity now
   round-trip through the real-source interpreter/native canary. Canonical
   proof bytes and role-separated semantic/proof/install/debug manifest hashes
   are also live. Semantic migration is exercised: archived v1 and v2 bytes
   retain their identities and migrate explicitly into separately fingerprinted
   current-v9 modules; archived v3 wrapping-add, v4 saturating-add, v5
   wrapping-subtract, v6 saturating-subtract, and v7 wrapping-multiply
   identities plus the v8 saturating-multiply identity are frozen as well. Typed
   installation records are live; typed debug/source maps
   remain a later artifact slice.

The migration may keep old and new paths temporarily for comparison. That is a
testing bridge, not a permanent two-semantics architecture.
