# Foreign boundary bindings

Source: `wiki/spec/build/foreign_bindings.md` lines 1–255 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| FOREIGN_BINDINGS1 | "a foreign library, syscall number, or firmware table is not an ordinary imported Omega module." | Foreign boundary bindings | Foreign libraries, syscall numbers, and firmware tables must not be treated as ordinary imported Omega modules | not stated | 9 |
| FOREIGN_BINDINGS2 | "These contracts do not imply implementation support for every native route." | Foreign boundary bindings | Compiler and tools must not infer that every native route named by these contracts is implemented | not stated | 9 |
| FOREIGN_BINDINGS3 | "binds an exact product requirement directly to a typed native locator in the owning package's Build:" | Build-owned native binding | `Build::bind` must tie an exact product requirement to a typed native locator in the owning package's Build | at a call | 9 |
| FOREIGN_BINDINGS4 | "The source-level `via` clause and the bodyless per-symbol realization machine that only forwards to a locator are retired." | Build-owned native binding | `via` clauses and bodyless per-symbol forwarding realization machines must be rejected | not stated | 9 |
| FOREIGN_BINDINGS5 | "A separate machine returning `ForeignBinding` is optional ordinary value computation, never mandatory import boilerplate." | Build-owned native binding | No program may be required to author a machine that only returns `ForeignBinding` as import boilerplate | not stated | 9 |
| FOREIGN_BINDINGS6 | "Checked adaptation still uses ordinary named implementations and real machine bodies." | Build-owned native binding | Checked adaptation must use ordinary named implementations and real machine bodies, not locator-only stubs | not stated | 9 |
| FOREIGN_BINDINGS7 | "`builder` is the current activation's original mutable Build value." | Build-owned native binding | The `builder` operand must be the current activation's original mutable Build value | at a call | 9 |
| FOREIGN_BINDINGS8 | "`target` is an ordinary exact validated target-profile reference, not a declaration keyword or an observation of the selected realization target." | Build-owned native binding | The `target` operand must be an exact validated target-profile reference and must not be a declaration keyword or realization-target observation | at a call | 9 |
| FOREIGN_BINDINGS9 | "One Build evaluation records explicit target-keyed binding intent." | Build-owned native binding | Each Build evaluation must record explicit target-keyed binding intent | not stated | 9 |
| FOREIGN_BINDINGS10 | "This overload does not change the existing entry-binding form or turn arbitrary values into requirements." | Build-owned native binding | The `Build::bind` overload must not alter entry-binding form or accept arbitrary values as requirements | not stated | 9 |
| FOREIGN_BINDINGS11 | "The requirement operand is an exact designated product-selection reference, including package/declaration ownership and its static application." | Build-owned native binding | The requirement operand must be an exact product-selection reference with correct ownership and static application | at a call | 9 |
| FOREIGN_BINDINGS12 | "Build may retain it until product discovery and checking resolve it; this does not import, execute, or make unresolved product source generally nameable during Build." | Build-owned native binding | Build may defer resolution but must not import, execute, or make unresolved product source generally nameable during Build | not stated | 9 |
| FOREIGN_BINDINGS13 | "Missing, inaccessible, mismatched, or ambiguous references reject." | Build-owned native binding | Missing, inaccessible, mismatched, or ambiguous requirement references must reject compilation | not stated | 9 |
| FOREIGN_BINDINGS14 | "Product rows never install a service into the running Build activation;" | Build-owned native binding | Product binding rows must not install a service into the running Build activation | not stated | 9 |
| FOREIGN_BINDINGS15 | "Bindings are package-local." | Build-owned native binding | Native binding rows must remain package-local | not stated | 9 |
| FOREIGN_BINDINGS16 | "Conflicting rows at the same precedence reject rather than adopting the last declaration or silently replacing installed supply." | Build-owned native binding | Conflicting binding rows at equal precedence must reject; last-wins and silent replacement are forbidden | not stated | 9 |
| FOREIGN_BINDINGS17 | "Build authors intent, not trusted evidence or executable ProviderPlan rows." | Build-owned native binding | Build must record author intent only, not trusted evidence or executable ProviderPlan rows | not stated | 9 |
| FOREIGN_BINDINGS18 | "The compiler derives the native candidate and calling plan, validates target/ABI compatibility, and requires admission of the foreign behavioral promise under the complete requirement contract." | Build-owned native binding | Compiler must derive candidate and calling plan, validate target/ABI fit, and require admission of the foreign behavioral promise under the full requirement contract | not stated | SHARED |
| FOREIGN_BINDINGS19 | "Neither locator construction nor selection proves behavior or grants execution authority." | Build-owned native binding | Locator construction and selection must not count as behavior proof or execution authority | not stated | 9 |
| FOREIGN_BINDINGS20 | "Representation or protocol adaptation requires a checked adapter, not an asserted locator conversion." | Build-owned native binding | Representation or protocol adaptation must use a checked adapter; asserted locator conversion alone must not suffice | not stated | 9 |
| FOREIGN_BINDINGS21 | "It is ordinary closed data, not a runtime provider slot;" | Locator values | `ForeignBinding` must be ordinary closed compile-time data, not a runtime provider slot | not stated | 9 |
| FOREIGN_BINDINGS22 | "a validated locator neither selects a source call nor grants runtime authority." | Locator values | A validated locator must not select a source call or grant runtime authority | not stated | 9 |
| FOREIGN_BINDINGS23 | "The locator argument is an ordinary compile-time expression producing a closed `ForeignBinding<ObjectLength, SymbolLength, VersionLength>` value." | Locator values | Locator arguments must be compile-time closed `ForeignBinding` values | at a call | 9 |
| FOREIGN_BINDINGS24 | "Each const argument is a `u64` byte-array length and part of type identity." | Locator values | Each `ForeignBinding` const length parameter must be `u64` and must participate in type identity | not stated | 9 |
| FOREIGN_BINDINGS25 | "Ordinary constructor/call inference determines lengths from the supplied fixed-byte values; coordinates unused by the selected case are zero." | Locator values | Length inference must follow fixed-byte values; unused coordinates for the selected case must be zero | not stated | 9 |
| FOREIGN_BINDINGS26 | "Incorrect explicit lengths still reject." | Locator values | Wrong explicit length parameters must reject compilation | not stated | 9 |
| FOREIGN_BINDINGS27 | "`DllImport` \| One `DllImport<ObjectLength, SymbolLength, VersionLength>` locator." | Locator values | `DllImport` case payload must be a single typed `DllImport` locator with the stated length parameters | not stated | 9 |
| FOREIGN_BINDINGS28 | "`Syscall` \| `number: u64`, validated against the selected target's syscall mechanism." | Locator values | `Syscall` must carry `number: u64` validated against the selected target's syscall mechanism | not stated | 9 |
| FOREIGN_BINDINGS29 | "Those two are the whole sum." | Locator values | `ForeignBinding` must contain only `DllImport` and `Syscall` cases | not stated | 9 |
| FOREIGN_BINDINGS30 | "`VtableField` is deliberately *not* a case of it: the declaration's own comment says it \"remains on its explicit legacy carrier until NativeFieldIdentity has an ordinary typed source form; it must not be smuggled into this sum\"." | Locator values | `VtableField` must not appear as a `ForeignBinding` case | not stated | 9 |
| FOREIGN_BINDINGS31 | "Provider evaluation enforces that, admitting only \"the exact Binding::DllImport or Binding::Syscall payload\"." | Locator values | Provider evaluation must admit only exact `DllImport` or `Syscall` payloads | not stated | 9 |
| FOREIGN_BINDINGS32 | "`PeByName` \| `library: [u8; ObjectLength]`, `export: [u8; SymbolLength]`." | Locator values | `PeByName` must supply library and export fixed byte arrays of the stated lengths | not stated | 9 |
| FOREIGN_BINDINGS33 | "`PeByOrdinal` \| `library: [u8; ObjectLength]`, `ordinal: u16`." | Locator values | `PeByOrdinal` must supply library bytes and a `u16` ordinal | not stated | 9 |
| FOREIGN_BINDINGS34 | "`ElfVersioned` \| `object: [u8; ObjectLength]`, `symbol: [u8; SymbolLength]`, `version: [u8; VersionLength]`." | Locator values | `ElfVersioned` must supply object, symbol, and version fixed byte arrays | not stated | 9 |
| FOREIGN_BINDINGS35 | "`MachODylibSymbol` \| `install_name: [u8; ObjectLength]`, `symbol: [u8; SymbolLength]`." | Locator values | `MachODylibSymbol` must supply install name and symbol fixed byte arrays | not stated | 9 |
| FOREIGN_BINDINGS36 | "The locator case gives raw bytes their physical interpretation. They are not Omega names, requirement keys, or independently replaceable fields." | Locator values | Raw bytes must be interpreted only via the locator case; they must not be treated as Omega names, requirement keys, or independently replaceable fields | not stated | 9 |
| FOREIGN_BINDINGS37 | "Quoted literals in fixed-byte-array positions copy exactly their source bytes; width mismatch rejects." | Locator values | Quoted literals in fixed-byte positions must copy exact source bytes; width mismatch must reject | not stated | 9 |
| FOREIGN_BINDINGS38 | "No evaluator reference or dynamically sized byte primitive crosses this boundary." | Locator values | Evaluator references and dynamically sized byte primitives must not cross the foreign-binding boundary | not stated | 9 |
| FOREIGN_BINDINGS39 | "The sum extends only for a genuinely distinct irreducible binding mechanism." | Locator values | New `ForeignBinding` cases may be added only for genuinely distinct irreducible mechanisms | not stated | 9 |
| FOREIGN_BINDINGS40 | "It has no generic `Value` or `Instruction` case and no `host:` mini-language." | Locator values | `ForeignBinding` must not include generic `Value` or `Instruction` cases or a `host:` mini-language | not stated | 9 |
| FOREIGN_BINDINGS41 | "The wider legacy source-level vocabulary (`ExternalBinding`) resolved through `via` — `Syscall`, `CompilerIntrinsic`, `VtableField`, `TableFunction`, and `VtableSlot` — is a migration input, not a supported alternative source binding form." | Locator values | Legacy `ExternalBinding`/`via` forms must not be accepted as supported binding source | not stated | 9 |
| FOREIGN_BINDINGS42 | "Retiring `via` does not add table or instruction variants to `ForeignBinding`:" | Locator values | Retiring `via` must not add table or instruction variants to `ForeignBinding` | not stated | 9 |
| FOREIGN_BINDINGS43 | "validated typed table/field mechanisms and the sealed intrinsic catalog retain their own obligations and must migrate without a compatibility parser." | Locator values | Table/field mechanisms and sealed intrinsics must keep separate obligations and migrate without a compatibility parser | not stated | 9 |
| FOREIGN_BINDINGS44 | "Foreign constants and offsets belong to checked target code and [layout plans](../layouts/plans.md)." | Locator values | Foreign constants and offsets must live in checked target code and layout plans, not in binding locators alone | not stated | 9 |
| FOREIGN_BINDINGS45 | "A table call names a validated field, not an authored numeric slot ordinal." | Locator values | Table calls must name validated fields, not author numeric slot ordinals | at a call | 9 |
| FOREIGN_BINDINGS46 | "Privileged instructions use parsed, contract-emitting `asm`, not a second binding instruction language." | Locator values | Privileged instructions must use contract-emitting `asm`, not a parallel binding instruction language | not stated | 10 |
| FOREIGN_BINDINGS47 | "Compiler intrinsics use the exact realization declaration, signature, selected target, and sealed catalog entry with accepted package/toolchain custody." | Locator values | Compiler intrinsics must bind realization declaration, signature, target, and sealed catalog entry with accepted custody | not stated | 9 |
| FOREIGN_BINDINGS48 | "An empty locator, lookalike symbol, or targetless plan supplies no catalog identity." | Locator values | Empty locator, lookalike symbol, or targetless plan must not supply catalog identity | not stated | 9 |
| FOREIGN_BINDINGS49 | "Physical nonreturning behavior does not establish semantic successful termination;" | Locator values | Physical nonreturning behavior must not establish semantic successful termination | at normal return | 3 |
| FOREIGN_BINDINGS50 | "[boundary realization](../terminal-psi/boundary_calls.md#consumer-owned-settlement) must preserve the canonical [ProcessExit requirement](../language/process_exit.md) and its explicit terminal completion and exact arguments." | Locator values | Boundary realization must preserve `ProcessExit` with explicit terminal completion and exact arguments | not stated | 3 |
| FOREIGN_BINDINGS51 | "The syscall, provider name, or physical status encoding cannot supply or change that identity." | Locator values | Syscall numbers, provider names, and physical status encodings must not supply or alter `ProcessExit` identity | not stated | 3 |
| FOREIGN_BINDINGS52 | "A checked implementation writes the complete trait application in `satisfies`, with lifetimes first: `satisfies Reads<'scope, Item>::read`." | Exact requirement lifetime application | Checked implementations must write the complete trait application in `satisfies` with lifetime arguments first | not stated | 7 |
| FOREIGN_BINDINGS53 | "Its lifetime argument count equals the trait telescope, and each argument names an active binder in the realizer's lifetime telescope." | Exact requirement lifetime application | Lifetime argument count must match the trait telescope and each argument must name an active realizer binder | not stated | 7 |
| FOREIGN_BINDINGS54 | "Repeated arguments are valid." | Exact requirement lifetime application | Repeated lifetime arguments in the application must be accepted when valid | not stated | 7 |
| FOREIGN_BINDINGS55 | "The declaration-order ordinal vector substitutes through the requirement signature, inherited requirements, contracts, and evidence; it is not inferred from incidental occurrences in the machine's signature." | Exact requirement lifetime application | Declaration-order ordinal vector must drive substitution everywhere required and must not be inferred from incidental machine signature occurrences | not stated | 7 |
| FOREIGN_BINDINGS56 | "A direct native binding has no extra realization declaration or fresh lifetime telescope." | Exact requirement lifetime application | Direct native bindings must not add realization declarations or fresh lifetime telescopes | not stated | 9 |
| FOREIGN_BINDINGS57 | "Its designated requirement reference retains the requirement's full binder schema and exact static application; concrete uses substitute their own actuals under that contract, never the Build activation's regions." | Exact requirement lifetime application | Designated requirement references must keep full binder schema and static application; uses substitute own actuals, never Build activation regions | not stated | 9 |
| FOREIGN_BINDINGS58 | "Derivation retains binder identities and equality partitions in the native candidate, calling plan, review, and artifact." | Exact requirement lifetime application | Derivation must retain binder identities and equality partitions through candidate, plan, review, and artifact | not stated | 9 |
| FOREIGN_BINDINGS59 | "Removing the forwarding declaration does not erase or guess lifetime obligations from a native signature." | Exact requirement lifetime application | Lifetime obligations must not be erased or guessed from native signatures when forwarding declarations are removed | not stated | 9 |
| FOREIGN_BINDINGS60 | "The exact requirement edge's public identity numbers distinct realizer binders by first occurrence in trait-parameter order." | Exact requirement lifetime application | Exact-requirement-edge public identity must number distinct realizer binders by first occurrence in trait-parameter order | not stated | 7 |
| FOREIGN_BINDINGS61 | "Thus `[1,1]` becomes `[0,0]`, `[4,2]` becomes `[0,1]`, and `[4,2,4]` becomes `[0,1,0]`." | Exact requirement lifetime application | Lifetime ordinal vectors must normalize to the stated equality partition examples | not stated | 7 |
| FOREIGN_BINDINGS62 | "This equality partition does not change under private binder renaming, reordering, or unused additions." | Exact requirement lifetime application | Equality partition must be invariant under private binder renaming, reordering, or unused additions | not stated | 7 |
| FOREIGN_BINDINGS63 | "It applies regardless of machine visibility." | Exact requirement lifetime application | Edge partition rules apply regardless of machine visibility | not stated | 7 |
| FOREIGN_BINDINGS64 | "A public machine's direct callable identity and a whole conformance's public telescope separately retain their declaration-order meaning; neither is the exact satisfaction edge." | Exact requirement lifetime application | Public callable identity and conformance telescope must keep declaration-order meaning and must not be confused with the exact satisfaction edge | not stated | 7 |
| FOREIGN_BINDINGS65 | "Checked and opaque external rows use the same edge partition." | Exact requirement lifetime application | Checked and opaque external binding rows must share the same edge partition | not stated | 9 |
| FOREIGN_BINDINGS66 | "Distinct borrow contracts remain distinct even if physical signatures and bindings coincide." | Exact requirement lifetime application | Distinct borrow contracts must stay distinct even when physical signatures and bindings match | not stated | 2 |
| FOREIGN_BINDINGS67 | "The partition cannot reconstruct the actual signature substitution: retain the full lifetime arguments separately wherever policy/signature checking needs them." | Exact requirement lifetime application | Full lifetime arguments must be retained for policy/signature checking wherever the partition is insufficient | not stated | 7 |
| FOREIGN_BINDINGS68 | "There is no lifetime constant such as `'static`; application arguments are active binders." | Exact requirement lifetime application | Lifetime applications must use active binders only; no `'static` lifetime constant | not stated | 7 |
| FOREIGN_BINDINGS69 | "Adding constants would require a closed binder-or-constant identity while keeping every telescope slot explicit." | Exact requirement lifetime application | Lifetime constants in applications are not supported unless a closed binder-or-constant identity keeps every telescope slot explicit | not stated | 7 |
| FOREIGN_BINDINGS70 | "A lifetime fixed directly inside a requirement, rather than declared as a trait parameter, has no slot to supply." | Exact requirement lifetime application | Lifetimes fixed inside a requirement without trait parameters must reject or fail for lack of supply slot | not stated | 7 |
| FOREIGN_BINDINGS71 | "Runtime erasure is never permission to omit a declared application." | Exact requirement lifetime application | Runtime erasure must not allow omitting declared lifetime applications | not stated | 7 |
| FOREIGN_BINDINGS72 | "Before a value enters a provider plan, variant- and target-specific validation checks required nonempty coordinates, forbidden terminators/bytes, ordinal ranges, object-format encoding and versioning rules, target applicability, and unused-coordinate zeros." | Validation and identity | Pre–provider-plan validation must check listed coordinate, format, target, and zeroing rules | not stated | 9 |
| FOREIGN_BINDINGS73 | "The whole locator is normalized atomically." | Validation and identity | Locator normalization must be atomic over the whole value | not stated | 9 |
| FOREIGN_BINDINGS74 | "A consumer cannot reconstruct binding identity by looking up its text." | Validation and identity | Binding identity must not be recoverable from textual lookup alone | not stated | 9 |
| FOREIGN_BINDINGS75 | "Retain an auditable native-provider identity derived from the exact requirement application, target, supplying immutable package, normalized locator, pinned calling policy, and authored selection/evaluation provenance." | Validation and identity | Native-provider identity must be derived from and retain the listed components | not stated | 9 |
| FOREIGN_BINDINGS76 | "Distinct owners or contracts do not collapse because symbol bytes or signatures coincide." | Validation and identity | Distinct owners or contracts must remain distinct when symbol bytes or signatures match | not stated | 9 |
| FOREIGN_BINDINGS77 | "Retain that identity, producer closure, and evaluation evidence through provider planning, object relocations, image emission, replacement, and audit;" | Validation and identity | Identity, producer closure, and evaluation evidence must persist through listed pipeline stages | not stated | 9 |
| FOREIGN_BINDINGS78 | "the removed forwarding machine is not a required identity anchor." | Validation and identity | Removed forwarding machines must not be required as identity anchors | not stated | 9 |
| FOREIGN_BINDINGS79 | "The requirement independently owns its evaluated [calling-policy application](calling_plans.md#identity-and-selection)." | Validation and identity | Each requirement must own its evaluated calling-policy application independently of the binding | not stated | 3 |
| FOREIGN_BINDINGS80 | "The binding refines that application; it neither carries a separately selectable copy nor reselects policy from a library or symbol name." | Validation and identity | Bindings must refine the requirement's policy application and must not carry a separately selectable copy or reselect policy from library/symbol names | not stated | 9 |
| FOREIGN_BINDINGS81 | "Constructing a binding grants no trust or selection authority." | Validation and identity | Binding construction must not grant trust or selection authority | not stated | 9 |
| FOREIGN_BINDINGS82 | "Structural validation checks its declaration; admission supplies trust classification and receipt." | Validation and identity | Declaration structural validation and separate admission trust classification must be distinguished | not stated | 9 |
| FOREIGN_BINDINGS83 | "Build evaluates and records the complete locator atomically; it cannot patch an admitted candidate's fields or manufacture derived plans." | Validation and identity | Build must record whole locators atomically and must not patch admitted candidates or manufacture derived plans | not stated | 9 |
| FOREIGN_BINDINGS84 | "Changing raw coordinates changes binding identity, forces reachable final artifacts to relink, and requires fresh admission." | Validation and identity | Coordinate changes must change identity, force relink of reachable artifacts, and require fresh admission | not stated | 9 |
| FOREIGN_BINDINGS85 | "Audit reports show the actual coordinates, not a nominal name that may resolve elsewhere." | Validation and identity | Audit output must show actual coordinates, not nominal names | not stated | SHARED |
| FOREIGN_BINDINGS86 | "There is no parallel endpoint registry or self-authored trust metadata language." | Validation and identity | Parallel endpoint registries and self-authored trust metadata languages must not be used | not stated | 9 |
| FOREIGN_BINDINGS87 | "The satisfied requirement supplies the public reach, suspension, blocking, guarded-crash, and synchronous-invocation ceilings." | Operational coherence | Satisfied requirements must supply the listed public ceilings | not stated | 3 |
| FOREIGN_BINDINGS88 | "A checked realization derives its behavior; an opaque realization supplies admitted facts and must refine every ceiling." | Operational coherence | Checked realizations must derive behavior; opaque realizations must refine every ceiling with admitted facts | not stated | 3 |
| FOREIGN_BINDINGS89 | "A Build locator expression does not repeat or weaken those clauses." | Operational coherence | Build locator expressions must not repeat or weaken requirement ceiling clauses | not stated | 9 |
| FOREIGN_BINDINGS90 | "A wrapper can refine behavior or reduce trust expenditure but cannot erase abstract service reach or the opaque premise on which its guarantee depends." | Operational coherence | Wrappers may refine behavior or reduce trust spend but must not erase abstract service reach or required opaque premises | not stated | 3 |
| FOREIGN_BINDINGS91 | "Blocking requires the caller's permitted ceiling and source acknowledgement." | Operational coherence | Blocking calls require caller permitted ceiling and source acknowledgement | at a call | 3 |
| FOREIGN_BINDINGS92 | "Without selected finite-wait evidence, response analysis reports `NoFiniteGuarantee(Edge(edge), UnboundedWait)`." | Operational coherence | Absent finite-wait evidence must yield `NoFiniteGuarantee` with unbounded wait | not stated | 3 |
| FOREIGN_BINDINGS93 | "The selected executor must satisfy thread/apartment affinity." | Operational coherence | Selected executor must meet thread/apartment affinity obligations | not stated | 3 |
| FOREIGN_BINDINGS94 | "Synchronous callbacks obey the direct acyclic [`invokes` graph](private_callbacks.md#registration-and-lifetime), not a graph inferred from the provider's binary." | Operational coherence | Synchronous callbacks must obey the direct acyclic `invokes` graph, not provider-binary inference | at a call | 3 |
| FOREIGN_BINDINGS95 | "An unconstrained invocation ceiling cannot establish acyclicity." | Operational coherence | Unconstrained invocation ceilings must not be used to prove callback acyclicity | not stated | 3 |
| FOREIGN_BINDINGS96 | "Over-approximation may reject uses; under-approximation of an opaque provider is an unsound admitted claim." | Operational coherence | Over-approximation may reject; under-approximation of opaque providers must be treated as unsound admission | not stated | 3 |
| FOREIGN_BINDINGS97 | "Execution placement is ordinary provider/executor selection, not a foreign-call disposition keyword." | Operational coherence | Execution placement must use provider/executor selection only, not a foreign-call disposition keyword | not stated | 9 |
| FOREIGN_BINDINGS98 | "Hosted direct calls use host-managed backing under the calling plan and provider containment contract." | Operational coherence | Hosted direct calls must use host-managed backing per calling plan and containment contract | at a call | 9 |
| FOREIGN_BINDINGS99 | "Fixed-stack or freestanding calls need admitted foreign demand or separately provisioned provider storage." | Operational coherence | Fixed-stack or freestanding calls must have admitted foreign demand or provisioned provider storage | at a call | 9 |
| FOREIGN_BINDINGS100 | "A requirement ceiling is not evidence that opaque code fits." | Operational coherence | Requirement ceilings must not count as evidence that opaque code fits declared limits | not stated | 3 |
| FOREIGN_BINDINGS101 | "A provider's enforced guarded capacity can supply containment with abnormal overflow, not proof of successful completion." | Operational coherence | Guarded capacity may model containment overflow handling but must not prove successful completion | not stated | 3 |
| FOREIGN_BINDINGS102 | "Admitted demand and provisioned capacity are distinct: the binding admits that the leaf uses at most its declared bytes, while a provider may provision more as headroom, and headroom is never evidence that the code fits." | Operational coherence | Demand admission and provisioned capacity must stay distinct; headroom must not prove fit | not stated | 9 |
| FOREIGN_BINDINGS103 | "A guard region alone is defense; containment requires evidence that the boundary cannot be skipped, such as a stack-probing discipline on the native code, together with a declared failure contract for reaching it." | Operational coherence | Containment must include skip-prevention evidence and failure contract, not guard region alone | not stated | 9 |
| FOREIGN_BINDINGS104 | "A false demand premise obliges no recovery; surviving one is the isolated provider's contract." | Operational coherence | False demand premises must not obligate recovery; survival obligations belong to isolated provider contract | not stated | 3 |
| FOREIGN_BINDINGS105 | "This is not a source crash cause for compiler-owned spills or a replacement for admitted Omega stack backing." | Operational coherence | Native demand premises must not replace admitted Omega stack backing or compiler spill crash causes | not stated | 9 |
| FOREIGN_BINDINGS106 | "Exact contributions follow [stack accounting](../resources/storage.md#stack-demand-and-backing); callback preflight, arrival, switching, and underestimation detection follow [entry stacks](../resources/entry_stacks.md#foreign-callback-entry)." | Operational coherence | Stack contributions and callback entry obligations must follow linked stack accounting and entry-stack rules | not stated | 9 |
| FOREIGN_BINDINGS107 | "Native boundaries preserve Omega's canonical semantic floating controls for `f32` and `f64`." | Floating control state | Native boundaries must preserve Omega canonical semantic floating controls for `f32` and `f64` | not stated | 5 |
| FOREIGN_BINDINGS108 | "A preserving binding proves the relevant masked MXCSR/FPCR controls unchanged; otherwise its trampoline saves and restores them." | Floating control state | Preserving bindings must prove controls unchanged or emit save/restore trampoline | at a call | 10 |
| FOREIGN_BINDINGS109 | "Inbound callbacks establish canonical controls before checked code and restore foreign controls on exit." | Floating control state | Inbound callbacks must set canonical controls before checked code and restore foreign controls on exit | at entry; at normal return | 10 |
| FOREIGN_BINDINGS110 | "Sticky status flags are not part of the semantic invariant." | Floating control state | Sticky FP status flags must not be treated as part of the semantic invariant | not stated | 5 |
| FOREIGN_BINDINGS111 | "Directed rounding does not change ambient control state, and `Trapping` does not unmask hardware exceptions." | Floating control state | Directed rounding must not alter ambient control state; `Trapping` must not unmask hardware exceptions | at the operation | 5 |
| FOREIGN_BINDINGS112 | "A target's flush-to-zero controls cannot remain enabled when they violate Omega's gradual-underflow semantics." | Floating control state | Flush-to-zero controls that violate gradual-underflow semantics must not stay enabled | not stated | 5 |
| FOREIGN_BINDINGS113 | "The conservative outbound policy wraps every returning import or indirect table call in an aligned save/restore envelope around the complete call sequence: save before outbound allocation/staging, release outbound stack, restore, then normalize a scalar result." | Floating control state | Conservative outbound policy must wrap returning imports and indirect table calls in the stated save/restore envelope sequence | at a call | 10 |
| FOREIGN_BINDINGS114 | "Direct syscalls have no returning user-space counterparty and need no such envelope." | Floating control state | Direct syscalls must not require the outbound FP save/restore envelope | at a call | 10 |
| FOREIGN_BINDINGS115 | "An admitted preservation proof may justify omitting it." | Floating control state | Outbound FP envelope may be omitted only with admitted preservation proof | at a call | 8 |
| FOREIGN_BINDINGS116 | "An inbound policy predicate alone is not evidence that a callback envelope was emitted." | Floating control state | Inbound policy predicates must not prove callback envelope emission by themselves | not stated | 8 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Packages supply ordinary checked implementations or Build-owned native bindings;" | Every foreign call site must resolve to exactly one of those two supply kinds. | Other supply paths named elsewhere (e.g. provider selection) may still exist; this sentence lists only the two primary package-authored forms. |
| "Build may retain it until product discovery and checking resolve it;" | Build evaluation may defer checking the requirement reference until after discovery. | Retention is optional; the compiler may require immediate resolution during the same Build pass. |
| "this does not import, execute, or make unresolved product source generally nameable during Build." | Unresolved requirements never become importable or nameable in Build even if retained. | Retained references may become nameable after discovery completes in the same Build session. |
| "A service package records the raw native bindings needed by its checked provider closure; an application selects the portable provider without repeating the dependency's DLL symbols or naming private raw requirements." | Applications must not re-declare dependency DLL symbols when selecting a portable provider. | This is architectural guidance only; the compiler does not enforce symbol hiding at application sites. |
| "Defaults and explicit overrides retain the ordinary ownership, visibility, and precedence rules." | Binding rows follow the same precedence rules as other Build defaults/overrides. | "Ordinary" rules are defined outside this chapter; this sentence does not state which conflicts beyond same-precedence are possible. |
| "Neither locator construction nor selection proves behavior or grants execution authority." | Admission and execution authority require separate evidence beyond locator construction. | Locator construction may still be necessary for admission even though it is not sufficient proof of behavior. |
| "Ordinary constructor/call inference determines lengths from the supplied fixed-byte values; coordinates unused by the selected case are zero." | Unused length type parameters must be zero for unselected coordinates at validation time. | Authors may supply nonzero unused coordinates if the case does not read them; only selected-case coordinates are checked. |
| "these are locator byte lengths, not syscall numbers, argument counts, or calling conventions." | Type parameters always denote byte lengths for locator arrays. | Inferred numeric type parameters might be misread as syscall metadata in diagnostics only; runtime meaning is still byte lengths. |
| "Provider evaluation enforces that, admitting only \"the exact Binding::DllImport or Binding::Syscall payload\"." | Provider evaluation rejects any other binding payload shape before planning. | Provider evaluation filters at plan construction; earlier Build validation might still accept other shapes for non-provider paths. |
| "Retiring `via` does not add table or instruction variants to `ForeignBinding`:" | Table and instruction bindings must use separate mechanisms forever. | Future compiler versions could extend `ForeignBinding` only via new irreducible mechanisms, not via `via` compatibility. |
| "Physical nonreturning behavior does not establish semantic successful termination;" | Native nonreturn paths do not satisfy Omega return/ProcessExit obligations without boundary settlement. | Nonreturning natives may still terminate the process physically while semantic termination remains unproved. |
| "Removing the forwarding declaration does not erase or guess lifetime obligations from a native signature." | Lifetime obligations always come from the designated requirement reference, never from native signatures alone. | When forwarding is removed, native signatures may still inform diagnostics but must not override requirement binders. |
| "This names the foreign promise, not proof that the implementation satisfies it." | Edge partition identifies the promised contract only; satisfaction proof is separate. | Physical binding coincidence does not merge borrow contracts but might still affect runtime checking elsewhere. |
| "Constructing a binding grants no trust or selection authority." | Trust and provider selection require admission flows separate from locator construction in Build. | Build recording of a locator may still be required before admission even though it grants no authority by itself. |
| "Build evaluates and records the complete locator atomically; it cannot patch an admitted candidate's fields or manufacture derived plans." | After admission, locator fields are immutable without fresh admission. | Build may record draft locators before admission that are not yet "admitted candidates" and could be rewritten pre-admission. |
| "A checked realization derives its behavior; an opaque realization supplies admitted facts and must refine every ceiling." | Opaque realizations must explicitly refine each ceiling with admitted facts or reject. | "Refine" may mean inherit ceilings unchanged when facts are empty; whether empty admission is allowed is not stated here. |
| "A wrapper can refine behavior or reduce trust expenditure but cannot erase abstract service reach or the opaque premise on which its guarantee depends." | Wrappers may add checks but must preserve declared reach and opaque premises. | Wrappers may narrow reach if the outer requirement still carries the abstract service reach separately. |
| "Over-approximation may reject uses; under-approximation of an opaque provider is an unsound admitted claim." | Conservative analysis may reject safe programs; unsound under-approximation is forbidden at admission. | Over-approximation is optional for rejection only; whether under-approximation is detected at compile time or audit time is not stated. |
| "A dedicated pinned UI executor may perform a blocking native call;" | Blocking on a UI executor is permitted when that executor is selected. | UI executor blocking is an example of placement freedom, not a guarantee that all UI executors allow blocking natives. |
| "Safely bounded recovery from a genuine hang requires process isolation; an in-process worker cannot simply be killed." | Hang recovery guarantees require process isolation; compilers must not claim in-process kill recovery. | In-process workers may still be cancelled via cooperative mechanisms not described here. |
| "headroom is never evidence that the code fits." | Extra provisioned capacity must not be used to prove declared demand is satisfied. | Headroom may be used at runtime for safety margins while compile-time fit still keys only on admitted demand. |
| "A false demand premise obliges no recovery; surviving one is the isolated provider's contract." | Callers need not recover from false demand; only isolated providers contract for survival after false premises. | False demand may still trap at the boundary without obligating caller recovery paths. |
| "A preserving binding proves the relevant masked MXCSR/FPCR controls unchanged; otherwise its trampoline saves and restores them." | Compiler must emit proof or trampoline for outbound calls based on preservation status. | "Preserving binding" is a author assertion checked at admission; without it, trampoline is mandatory. |
| "An admitted preservation proof may justify omitting it." | Outbound envelope may be omitted only when a checked preservation proof is admitted. | Preservation proof may omit envelope for some call kinds but not others; scope of "it" is the outbound envelope only. |
| "An inbound policy predicate alone is not evidence that a callback envelope was emitted." | Inbound policy flags do not certify emission of callback FP envelopes. | Policy predicates may be necessary inputs to envelope emission decisions but insufficient as standalone evidence. |

## Terms used without definition in this chapter

- abstract boundary-trait requirements
- application code (vs packages)
- ordinary checked implementations
- Build-owned native bindings
- foreign library / syscall number / firmware table
- ordinary imported Omega module
- provider selection / derived candidate
- native route
- `build-owned-native-bindings` decision
- exact product requirement
- typed native locator
- owning package's Build
- `Build::bind`
- `WindowsAudioPlaybackIo::c_play_sound` (example)
- `ForeignBinding::DllImport` / `DllImport::PeByName`
- source-level `via` clause
- bodyless per-symbol realization machine
- forwarding to a locator
- machine returning `ForeignBinding`
- import boilerplate
- checked adaptation
- current activation's original mutable Build value
- exact validated target-profile reference
- declaration keyword
- observation of the selected realization target
- target-keyed binding intent
- entry-binding form
- exact designated product-selection reference
- package/declaration ownership
- static application
- product discovery and checking
- unresolved product source
- generally nameable during Build
- Product rows
- running Build activation
- Build-host readiness
- explicit provider route
- package-local bindings
- service package
- checked provider closure
- portable provider
- DLL symbols
- private raw requirements
- defaults and explicit overrides
- ownership, visibility, and precedence rules
- installed supply
- trusted evidence
- executable ProviderPlan rows
- native candidate
- calling plan
- target/ABI compatibility
- admission of the foreign behavioral promise
- complete requirement contract
- execution authority
- asserted locator conversion
- `ForeignBinding<ObjectLength, SymbolLength, VersionLength>`
- compile-time native-locator carrier
- runtime provider slot
- closed data
- validated locator
- `u64` byte-array length
- constructor/call inference
- fixed-byte values
- unused coordinates
- `ForeignBinding<9, 10, 0>` (example)
- locator byte lengths
- `Binding::DllImport` / `Binding::Syscall`
- `VtableField`
- legacy carrier
- `NativeFieldIdentity`
- Provider evaluation
- `PeByName` / `PeByOrdinal` / `ElfVersioned` / `MachODylibSymbol`
- physical interpretation of raw bytes
- Omega names / requirement keys
- quoted literals / fixed-byte-array positions
- width mismatch
- evaluator reference
- dynamically sized byte primitive
- genuinely distinct irreducible binding mechanism
- generic `Value` / `Instruction` case
- `host:` mini-language
- `ExternalBinding`
- `CompilerIntrinsic` / `TableFunction` / `VtableSlot`
- migration input
- compatibility parser
- layout plans
- validated field
- authored numeric slot ordinal
- contract-emitting `asm`
- sealed intrinsic catalog
- exact realization declaration
- accepted package/toolchain custody
- empty locator / lookalike symbol / targetless plan
- catalog identity
- physical nonreturning behavior
- semantic successful termination
- boundary realization / consumer-owned settlement
- `ProcessExit` requirement
- terminal completion and exact arguments
- physical status encoding
- complete trait application in `satisfies`
- trait telescope
- realizer's lifetime telescope
- active binder
- declaration-order ordinal vector
- inherited requirements
- direct native binding
- fresh lifetime telescope
- binder schema
- Build activation's regions
- binder identities and equality partitions
- native candidate / review / artifact
- forwarding declaration
- native signature
- exact requirement edge's public identity
- trait-parameter order
- equality partition
- private binder renaming
- machine visibility
- public machine's direct callable identity
- whole conformance's public telescope
- exact satisfaction edge
- checked and opaque external rows
- borrow contracts
- foreign promise
- actual signature substitution
- policy/signature checking
- lifetime constant `'static`
- binder-or-constant identity
- telescope slot
- runtime erasure
- provider plan
- variant- and target-specific validation
- forbidden terminators/bytes
- ordinal ranges
- object-format encoding and versioning rules
- target applicability
- normalized locator (atomic)
- binding identity by text lookup
- auditable native-provider identity
- supplying immutable package
- pinned calling policy
- selection/evaluation provenance
- producer closure
- evaluation evidence
- object relocations / image emission / replacement / audit
- calling-policy application
- separately selectable copy
- trust classification and receipt
- admitted candidate
- derived plans
- reachable final artifacts
- relink
- fresh admission
- nominal name
- parallel endpoint registry
- self-authored trust metadata language
- public reach / suspension / blocking / guarded-crash / synchronous-invocation ceilings
- checked realization / opaque realization
- admitted facts
- Build locator expression
- wrapper
- abstract service reach
- opaque premise
- trust expenditure
- caller's permitted ceiling
- source acknowledgement
- finite-wait evidence
- `NoFiniteGuarantee` / `UnboundedWait`
- selected executor
- thread/apartment affinity
- direct acyclic `invokes` graph
- provider's binary
- unconstrained invocation ceiling
- over-approximation / under-approximation
- unsound admitted claim
- execution placement
- foreign-call disposition keyword
- pinned UI executor
- blocking-executor package
- bounded queues / completion claims
- detached in-process call
- worker, storage, and provider era
- process isolation
- host-managed backing
- provider containment contract
- fixed-stack or freestanding calls
- admitted foreign demand
- separately provisioned provider storage
- requirement ceiling
- enforced guarded capacity
- abnormal overflow
- admitted demand vs provisioned capacity
- headroom
- guard region
- stack-probing discipline
- declared failure contract
- false demand premise
- isolated provider's contract
- compiler-owned spills
- Omega stack backing
- stack accounting
- callback preflight / arrival / switching / underestimation detection
- entry stacks / foreign-callback-entry
- canonical semantic floating controls
- `f32` / `f64`
- masked MXCSR/FPCR
- preserving binding
- trampoline
- inbound callbacks
- sticky status flags
- semantic invariant
- directed rounding
- ambient control state
- `Trapping`
- hardware exceptions
- flush-to-zero controls
- gradual-underflow semantics
- conservative outbound policy
- returning import / indirect table call
- aligned save/restore envelope
- outbound allocation/staging
- outbound stack
- normalize a scalar result
- direct syscalls
- returning user-space counterparty
- admitted preservation proof
- inbound policy predicate
- callback envelope

CHAPTER COMPLETE
