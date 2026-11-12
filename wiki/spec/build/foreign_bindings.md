# Foreign boundary bindings

Application code calls abstract boundary-trait requirements. Packages supply
ordinary checked implementations or Build-owned native bindings; a foreign library,
syscall number, or firmware table is not an ordinary imported Omega module.
[Provider selection](provider_selection.md) chooses the derived candidate.
These contracts do not imply implementation support for every native route.

## Build-owned native binding

The owner-ratified `build-owned-native-bindings` decision (2026-10-27) binds an
exact product requirement directly to a typed native locator in the owning
package's Build:

```omega
Build::bind<WindowsAudioPlaybackIo::c_play_sound>(
	builder,
	windows_x86_64,
	ForeignBinding::DllImport {
		import: DllImport::PeByName {
			library: "winmm.dll",
			export: "PlaySoundW",
		},
	}
);
```

The source-level `via` clause and the
bodyless per-symbol realization machine that only forwards to a locator are
retired. A separate machine returning `ForeignBinding` is optional ordinary
value computation, never mandatory import boilerplate. Checked adaptation still
uses ordinary named implementations and real machine bodies.

`builder` is the current activation's original mutable Build value. `target` is
an ordinary exact validated target-profile reference, not a declaration keyword
or an observation of the selected realization target. One Build evaluation
records explicit target-keyed binding intent. This overload does not change the
existing entry-binding form or turn arbitrary values into requirements.

The requirement operand is an exact designated product-selection reference,
including package/declaration ownership and its static application. Build may
retain it until product discovery and checking resolve it; this does not import,
execute, or make unresolved product source generally nameable during Build.
Missing, inaccessible, mismatched, or ambiguous references reject. Product rows
never install a service into the running Build activation; Build-host readiness
remains the separate [explicit provider route](provider_selection.md#build-provider-readiness).

Bindings are package-local. A service package records the raw native bindings
needed by its checked provider closure; an application selects the portable
provider without repeating the dependency's DLL symbols or naming private raw
requirements. Defaults and explicit overrides retain the ordinary ownership,
visibility, and precedence rules. Conflicting rows at the same precedence reject
rather than adopting the last declaration or silently replacing installed supply.

Build authors intent, not trusted evidence or executable ProviderPlan rows. The
compiler derives the native candidate and calling plan, validates target/ABI
compatibility, and requires admission of the foreign behavioral promise under
the complete requirement contract. Neither locator construction nor selection
proves behavior or grants execution authority. Representation or protocol
adaptation requires a checked adapter, not an asserted locator conversion.

## Locator values

`ForeignBinding<ObjectLength, SymbolLength, VersionLength>`
(`source/library/core/external_binding.omg`) is the compile-time
native-locator carrier. It is ordinary closed data, not a runtime provider slot;
a validated locator neither selects a source call nor grants runtime authority.

The locator argument is an ordinary compile-time expression producing a closed
`ForeignBinding<ObjectLength, SymbolLength, VersionLength>` value. Each const
argument is a `u64` byte-array length and part of type identity. Ordinary
constructor/call inference determines lengths from the supplied fixed-byte
values; coordinates unused by the selected case are zero. Authors need not
write byte counts on the constructor or `Build::bind`, and need not introduce
a helper with an explicitly counted return type. In the example above the
inferred type is `ForeignBinding<9, 10, 0>`; these are locator byte lengths,
not syscall numbers, argument counts, or calling conventions. Incorrect explicit
lengths still reject.

| Binding case | Payload |
| --- | --- |
| `DllImport` | One `DllImport<ObjectLength, SymbolLength, VersionLength>` locator. |
| `Syscall` | `number: u64`, validated against the selected target's syscall mechanism. |

Those two are the whole sum. Function-table bindings use the separate typed
`FunctionFieldBinding` operand described below, not a `VtableField` case, a
numeric slot, or an encoded field name inside this locator sum.

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
It has no generic `Value` or `Instruction` case and no `host:` mini-language.

The wider legacy source-level vocabulary (`ExternalBinding`) resolved through
`via` — `Syscall`, `CompilerIntrinsic`, `VtableField`, `TableFunction`, and
`VtableSlot` — is a migration input, not a supported alternative source binding
form.
Retiring `via` does not add table or instruction variants to `ForeignBinding`:
validated typed table/field mechanisms and the sealed intrinsic catalog retain
their own obligations and must migrate without a compatibility parser.
Foreign constants and offsets belong to checked target
code and [layout plans](../layouts/plans.md). A table call names a validated
field, not an authored numeric slot ordinal. Privileged instructions use parsed,
contract-emitting `asm`, not a second binding instruction language.

## Function-table bindings

Named decision: `native-table-field-binding`, settled 2026-11-11.

A foreign API may supply its functions through a record of addresses rather
than exported symbols. The package's Build binds an exact boundary requirement
to one field of the table supplied at each call. The binding identifies where
to read the function address; the requirement's calling plan independently
describes which arguments the foreign function receives.

`Build::bind<Requirement>(builder, target, binding)` accepts an ordinary
`FunctionFieldBinding` description with two operands:

| Operand | Meaning |
| --- | --- |
| `table: ParameterInfo` | An exact formal of the bound requirement, selected with `reflect::parameter(Requirement::formal)`. |
| `field: FieldInfo` | An exact field of the borrowed table application, selected with `reflect::field(Table::field)`. |

These are [typed declaration selectors](../language/reflection.md#typed-declaration-selectors),
not strings, ordinals, runtime values, or executable machine selections. In a
Build binding they use the existing designated product-reference mechanism:
resolution retains exact declaration, application and signature/schema identity
without importing or executing product code in the Build activation. The formal
must belong to the bound requirement application. It may occur anywhere in the
signature; its position is never inferred from its type or from a first-parameter
convention. The field must be a live, non-atomic `addr` field of the exact table
application borrowed by that formal. Layout validation derives its offset.
Wrong owners, applications, revisions, field kinds and inaccessible selectors
reject, including in declared bindings that are not reached by an entry.

The selected formal must permit reading the dispatch field. Its calling-plan
role is explicit: either dispatch-only, with no native argument position, or
passed normally as well as used for dispatch. Every other parameter retains
its ordinary mapping, including declared private callback destinations. All
semantic parameters are accounted for and all native arguments placed exactly
once. The Build row must fit the requirement's commitment; it cannot override
the calling policy or silently remove an argument. See
[table dispatch argument accounting](calling_plans.md#table-dispatch-argument-accounting).

For a Boot Services-style operation, the table is used only to locate the
function; the native callee receives the remaining arguments. A receiver-style
operation may also pass that table, or pass a separate explicitly supplied
receiver. These are argument mappings of one binding mechanism, not separate
foreign binding kinds. No implicit `self` or receiver adjustment follows.

At runtime, evaluate the arguments under ordinary evaluation rules, read the
selected field from the supplied table, and invoke the resulting address under
the validated calling plan. Build does not read firmware memory or freeze a
function address. Different table instances may supply different addresses under
the same admitted contract. Dispatch is an actual read: its access, lifetime and
memory-observation obligations survive even when the formal is dispatch-only;
it cannot be discarded as an unused argument.

Reading an `addr` proves neither a callable signature nor permission to invoke
it. The existing admitted boundary and entry/handoff contracts must justify the
table's provenance, field-to-signature correspondence, foreign behavior and
applicable authority/protocol lifetime. Constructing a matching record or a
binding description supplies none of those facts. Firmware-entry adapters may
establish the declared entry environment; table dispatch does not recognize
UEFI type names or give operation names special lifecycle effects.

Resolve the formal and field once and retain their exact identities and the
normalized argument correspondence before ABI placement. Provider planning,
native realization, package/artifact validation and supported interpreter routes
consume that same correspondence, never a rediscovered dispatch prefix. Retain
enough declaration/signature/layout evidence to validate it without source;
offsets and argument counts alone are insufficient. An unsupported interpreter
binding is refused explicitly, with no fallback to native execution.

This mechanism selects one field on one explicitly supplied table borrow.
Checked code obtains a nested table view before the call. Arbitrary pointer
chasing, argument permutations/conversions, C++ object-model adaptation and
turning a raw address into an ordinary Omega callable are not added here.
Genuine adaptation belongs in checked adapters. `VtableField` and `TableFunction`
migrate to this one mechanism; their legacy parser forms and positional
handling are retired together.

### Migration acceptance

`BUILD-OWNED-NATIVE-BINDINGS` carries this decision through the existing table
fixtures and library callers without weaker contracts. Native acceptance covers
a non-first table parameter, dispatch-only and also-passed tables, a separate
receiver, and a non-UEFI table with a different layout on supported hosts. Reject
wrong formal/field owners or applications, unreadable tables, non-address fields,
incompatible argument mappings and tampered retained correspondence. Consume an
artifact without source to establish that the retained identities and mapping
suffice; a fresh source compilation alone does not exercise that contract.

Supported interpreter routes must agree under `omega run --both`. Unsupported
bindings must refuse explicitly without native fallback; implementing a new
interpreter foreign-call route is not a prerequisite for retiring the legacy
surface. The migration removes both legacy table forms and their positional
handling from downstream consumers, not just their parser spelling.

## Sealed compiler intrinsics

Compiler intrinsics use the exact realization declaration, signature, selected
target, and sealed catalog entry with accepted package/toolchain custody.
An empty locator, lookalike symbol, or targetless plan supplies no catalog
identity. Physical nonreturning behavior does not establish semantic successful
termination; [boundary realization](../terminal-psi/boundary_calls.md#consumer-owned-settlement)
must preserve the canonical [ProcessExit requirement](../language/process_exit.md)
and its explicit terminal completion and exact arguments. The syscall, provider
name, or physical status encoding cannot supply or change that identity.

## Exact requirement lifetime application

A checked implementation writes the complete trait application in `satisfies`,
with lifetimes first: `satisfies Reads<'scope, Item>::read`.
Its lifetime argument count equals the trait telescope, and each argument names
an active binder in the realizer's lifetime telescope. Repeated arguments are
valid. The declaration-order ordinal vector substitutes through the requirement
signature, inherited requirements, contracts, and evidence; it is not inferred
from incidental occurrences in the machine's signature.

A direct native binding has no extra realization declaration or fresh lifetime
telescope. Its designated requirement reference retains the requirement's full
binder schema and exact static application; concrete uses substitute their own
actuals under that contract, never the Build activation's regions. Derivation
retains binder identities and equality partitions in the native candidate,
calling plan, review, and artifact. Removing the forwarding declaration does not
erase or guess lifetime obligations from a native signature.

The exact requirement edge's public identity numbers distinct realizer binders
by first occurrence in trait-parameter order. Thus `[1,1]` becomes `[0,0]`,
`[4,2]` becomes `[0,1]`, and `[4,2,4]` becomes `[0,1,0]`. This equality partition
does not change under private binder renaming, reordering, or unused additions.
It applies regardless of machine visibility. A public machine's direct callable
identity and a whole conformance's public telescope separately retain their
declaration-order meaning; neither is the exact satisfaction edge.

Checked and opaque external rows use the same edge partition. Distinct borrow
contracts remain distinct even if physical signatures and bindings coincide.
This names the foreign promise, not proof that the implementation satisfies it.
The partition cannot reconstruct the actual signature substitution: retain the
full lifetime arguments separately wherever policy/signature checking needs them.

There is no lifetime constant such as `'static`; application arguments
are active binders. Adding constants would require a closed binder-or-constant
identity while keeping every telescope slot explicit. A lifetime fixed directly
inside a requirement, rather than declared as a trait parameter, has no slot to
supply. Runtime erasure is never permission to omit a declared application.

## Validation and identity

Before a value enters a provider plan, variant- and target-specific validation
checks required nonempty coordinates, forbidden terminators/bytes, ordinal
ranges, object-format encoding and versioning rules, target applicability, and
unused-coordinate zeros. The whole locator is normalized atomically. A consumer
cannot reconstruct binding identity by looking up its text.

Retain an auditable native-provider identity derived from the exact requirement
application, target, supplying immutable package, normalized locator, pinned
calling policy, and authored selection/evaluation provenance. Distinct owners or
contracts do not collapse because symbol bytes or signatures coincide. Retain
that identity, producer closure, and evaluation evidence through provider
planning, object relocations, image emission, replacement, and audit; the removed
forwarding machine is not a required identity anchor.
The requirement independently owns its evaluated
[calling-policy application](calling_plans.md#identity-and-selection). The
binding refines that application; it neither carries a separately selectable
copy nor reselects policy from a library or symbol name.

Constructing a binding grants no trust or selection authority. Structural
validation checks its declaration; admission supplies trust classification and
receipt. Build evaluates and records the complete locator atomically; it cannot
patch an admitted candidate's fields or manufacture derived plans. Changing raw
coordinates changes binding identity, forces reachable
final artifacts to relink, and requires fresh admission. Audit reports show the
actual coordinates, not a nominal name that may resolve elsewhere. There is no
parallel endpoint registry or self-authored trust metadata language.

## Operational coherence

The satisfied requirement supplies the public reach, suspension, blocking,
guarded-crash, and synchronous-entry ceilings. A checked realization derives
its behavior; an opaque realization supplies admitted facts and must refine
every ceiling. A Build locator expression does not repeat or weaken those clauses. A wrapper can
refine behavior or reduce trust expenditure but cannot erase abstract service
reach or the opaque premise on which its guarantee depends.

Blocking requires the caller's permitted ceiling and source acknowledgement.
Without selected finite-wait evidence, response analysis reports
`NoFiniteGuarantee(Edge(edge), UnboundedWait)`. The selected executor must satisfy
thread/apartment affinity. Synchronous callbacks obey the direct acyclic
[`invokes` graph](private_callbacks.md#registration-and-lifetime), not a graph
inferred from the provider's binary. A native binding's `invokes` clause, or
its omission, is its admitted claim about synchronous entry; its host calls
need no row. An unconstrained invocation ceiling cannot establish acyclicity. Over-approximation may reject uses; under-approximation of
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
containment with abnormal overflow, not proof of successful completion. Admitted
demand and provisioned capacity are distinct: the binding admits that the leaf
uses at most its declared bytes, while a provider may provision more as
headroom, and headroom is never evidence that the code fits. A guard region
alone is defense; containment requires evidence that the boundary cannot be
skipped, such as a stack-probing discipline on the native code, together with a
declared failure contract for reaching it. A false demand premise obliges no
recovery; surviving one is the isolated provider's contract. This is not a
source crash cause for compiler-owned spills or a replacement for admitted
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

The envelope's restore writes the floating control unit, so it counts as use of
control state. Every hosted `ProgramEntry` ceiling and the ordinary boundary
ceiling therefore include control state, as the UEFI entry's already does; the
hosted ABIs make these controls callee-preserved, like the callee-saved general
registers the ceilings list.
