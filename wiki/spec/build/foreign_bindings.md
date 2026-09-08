# Foreign boundary bindings

Application code calls abstract boundary requirements. Target/toolchain packages
provide explicit checked satisfiers or bodyless native leaves; a foreign library,
syscall number, or firmware table is not an ordinary imported Omega module.
[Provider selection](provider_selection.md) chooses the derived candidate.
These contracts do not imply implementation support for every native route.

## Binding values

A bodyless leaf declares `satisfies Requirement` and uses `via expression` only
for an irreducible payload not determined by declaration, signature, and target.
The expression is compile-time evaluable to an ordinary closed
`Binding<ObjectLength, SymbolLength, VersionLength>` value. Each const argument
is a `u64` byte-array length and part of type identity; unused coordinates are
zero.

| Binding case | Payload |
| --- | --- |
| `DllImport` | One `DllImport<ObjectLength, SymbolLength, VersionLength>` locator. |
| `Syscall` | `number: u64`, validated against the selected target's syscall mechanism. |
| `VtableField` | `field: NativeFieldIdentity` from one validated native layout. |

| DllImport case | Coordinates |
| --- | --- |
| `PeByName` | `library: [u8; ObjectLength]`, `export: [u8; SymbolLength]`. |
| `PeByOrdinal` | `library: [u8; ObjectLength]`, `ordinal: u16`. |
| `ElfVersioned` | `object: [u8; ObjectLength]`, `symbol: [u8; SymbolLength]`, `version: [u8; VersionLength]`. |
| `MachODylibSymbol` | `install_name: [u8; ObjectLength]`, `symbol: [u8; SymbolLength]`. |

The locator case gives raw bytes their physical interpretation. They are not
Omega names, requirement keys, or independently replaceable fields. Quoted
literals in fixed-byte-array positions copy exactly their source bytes; width
mismatch rejects. No evaluator reference or dynamically sized byte primitive
crosses this boundary.

The sum extends only for a genuinely distinct irreducible binding mechanism.
It has no generic `Value`, `CompilerIntrinsic`, or `Instruction` case and no
`host:` mini-language. Foreign constants and offsets belong to checked target
code and [layout plans](../layouts/plans.md). A table call names a validated
field, not an authored numeric slot ordinal. Privileged instructions use parsed,
contract-emitting `asm`, not a second binding instruction language.

Compiler intrinsics use the exact realization declaration, signature, selected
target, and sealed catalog entry with accepted package/toolchain custody.
An empty `via` value, lookalike symbol, or targetless plan supplies no catalog
identity. Physical nonreturning behavior does not establish semantic successful
termination; [boundary realization](../terminal-psi/boundary_calls.md#consumer-owned-settlement)
requires the distinct terminal-effect identity.

## Validation and identity

Before a value enters a provider plan, variant- and target-specific validation
checks required nonempty coordinates, forbidden terminators/bytes, ordinal
ranges, object-format encoding and versioning rules, target applicability, and
unused-coordinate zeros. The whole locator is normalized atomically. A consumer
cannot reconstruct binding identity by looking up its text.

Retain the normalized value, producer closure, evaluation evidence, and selected
target through provider planning, object relocations, image emission, and audit.
The requirement independently owns its evaluated
[calling-policy application](calling_plans.md#identity-and-selection). The
binding refines that application; it neither carries a separately selectable
copy nor reselects policy from a library or symbol name.

Constructing a binding grants no trust or selection authority. Structural
validation checks its declaration; admission supplies trust classification and
receipt. Build may choose a provider but cannot replace its evaluated locator
fields. Changing raw coordinates changes binding identity, forces reachable
final artifacts to relink, and requires fresh admission. Audit reports show the
actual coordinates, not a nominal name that may resolve elsewhere. There is no
parallel endpoint registry or self-authored trust metadata language.

## Operational coherence

The satisfied requirement supplies the public reach, suspension, blocking,
guarded-crash, and synchronous-invocation ceilings. A checked realization derives
its behavior; an opaque realization supplies admitted facts and must refine
every ceiling. The `via` producer does not repeat those clauses. A wrapper can
refine behavior or reduce trust expenditure but cannot erase abstract service
reach or the opaque premise on which its guarantee depends.

Blocking requires the caller's permitted ceiling and source acknowledgement.
Without selected finite-wait evidence, response analysis reports
`NoFiniteGuarantee(Edge(edge), UnboundedWait)`. The selected executor must satisfy
thread/apartment affinity. Synchronous callbacks obey the direct acyclic
[`invokes` graph](private_callbacks.md#registration-and-lifetime), not a graph
inferred from the provider's binary. An unconstrained invocation ceiling cannot
establish acyclicity. Over-approximation may reject uses; under-approximation of
an opaque provider is an unsound admitted claim.

Execution placement is ordinary provider/executor selection, not a foreign-call
disposition keyword. A dedicated pinned UI executor may perform a blocking
native call; a blocking-executor package may move custody through bounded queues
and completion claims. A detached in-process call retains its worker, storage,
and provider era until native return. Safely bounded recovery from a genuine hang
requires process isolation; an in-process worker cannot simply be killed.

Hosted direct calls use host-managed backing under the calling plan and provider
containment contract. Fixed-stack or freestanding calls need admitted foreign
demand or separately provisioned provider storage. A requirement ceiling is not
evidence that opaque code fits. A provider's enforced guarded capacity can supply
containment with abnormal overflow, not proof of successful completion. This is
not a source crash cause for compiler-owned spills or a replacement for admitted
Omega stack backing. Exact contributions follow
[stack accounting](../resources/storage.md#stack-demand-and-backing); callback
preflight, arrival, switching, and underestimation detection follow
[entry stacks](../resources/entry_stacks.md#foreign-callback-entry).

## Floating control state

Native boundaries preserve Omega's canonical semantic floating controls for
`f32` and `f64`. A preserving binding proves the relevant masked MXCSR/FPCR
controls unchanged; otherwise its trampoline saves and restores them. Inbound
callbacks establish canonical controls before checked code and restore foreign
controls on exit. Sticky status flags are not part of the semantic invariant.
Directed rounding does not change ambient control state, and `Trapping` does
not unmask hardware exceptions. A target's flush-to-zero controls cannot remain
enabled when they violate Omega's gradual-underflow semantics.

The conservative outbound policy wraps every returning import or indirect
table call in an aligned save/restore envelope around the complete call sequence:
save before outbound allocation/staging, release outbound stack, restore, then
normalize a scalar result. Direct syscalls have no returning user-space
counterparty and need no such envelope. An admitted preservation proof may
justify omitting it. An inbound policy predicate alone is not evidence that a
callback envelope was emitted.
