# Per-value type packages with some

Source: `wiki/spec/language/some_packages.md` lines 1–197 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
SOME_PACKAGES1 | "Unlike an ordinary generic argument, that choice is not part of the external data type." | Per-value type packages with some | The concrete witness chosen per value must not appear as an argument of the external data type name | not stated | 7
SOME_PACKAGES2 | "The spelling `some` is a data binder, not visibility, an inference hole, a universal type, or a runtime type object." | Per-value type packages with some | `some` must be classified only as a data binder and must not be treated as visibility, inference hole, universal type, or runtime type object | not stated | 7
SOME_PACKAGES3 | "`Reader<'a>` is the external type; callers supply ordinary type, lifetime and const arguments, not the `some` arguments." | Declaration and construction | Call sites must supply only ordinary type, lifetime, and const arguments to the external type, not `some` binders | at a call | 7
SOME_PACKAGES4 | "`Reader<File>` is not the spelling for choosing this declaration's packaged `S`." | Declaration and construction | Using an ordinary type argument in place of per-value witness selection for a `some` declaration must be rejected | not stated | 7
SOME_PACKAGES5 | "An ordinary `Reader<S>` declaration without `some` still produces different types for different `S` arguments." | Declaration and construction | Declarations without `some` must keep ordinary per-`S` type separation | not stated | 7
SOME_PACKAGES6 | "Aliases preserve this distinction rather than introducing another binder." | Declaration and construction | Type aliases must preserve external-versus-witness distinction and must not introduce another witness binder | not stated | 7
SOME_PACKAGES7 | "Ordinary lifetime elision remains available where already permitted; packaging does not invent a lifetime bound." | Declaration and construction | Lifetime elision applies only where already permitted; packaging must not add new lifetime bounds | not stated | 2
SOME_PACKAGES8 | "Multiple `some` binders choose separate witnesses, with any relationships stated and checked in the declaration." | Declaration and construction | Each `some` binder gets its own witness; declared relationships between witnesses must be checked in the declaration | not stated | 7
SOME_PACKAGES9 | "Construction infers one concrete witness for each `some` binder from the field initializers and checks all related fields against that same choice, including callable contracts and the data invariant." | Declaration and construction | Record literal construction must infer one witness per `some` binder from initializers and check all related fields, callables, and the data invariant against those witnesses | at the operation | 1
SOME_PACKAGES10 | "Unresolved or inconsistent witnesses reject." | Declaration and construction | Construction must reject when witness inference is unresolved or fields disagree on witness choice | at the operation | 7
SOME_PACKAGES11 | "A machine accepting `Socket` cannot accompany `&mut File`." | Declaration and construction | Callable witness types and reference witness types in one package must be mutually consistent | at the operation | 3
SOME_PACKAGES12 | "Witnesses remain checking evidence, not runtime source-level type values." | Declaration and construction | Witness choices must exist only in checking evidence, not as runtime source-level type values | not stated | SHARED
SOME_PACKAGES13 | "A generic helper may construct a package using its ordinary static type argument as the witness," | Declaration and construction | Generic construction using a static type parameter as witness must be permitted when other construction checks pass | at the operation | 7
SOME_PACKAGES14 | "and an [opened abstract witness](#owned-state-through-uniform-carriers) may do so within its scope." | Declaration and construction | Construction using an opened abstract witness must be permitted only within that opening's scope | at the operation | 2
SOME_PACKAGES15 | "V1 permits sized concrete witnesses." | Declaration and construction | Witness types must be sized concrete types under V1 | not stated | 5
SOME_PACKAGES16 | "A field depending on `S` must have a uniform representation independent of the witness: references to sized referents, callable descriptors over those references, and [uniform carriers](#owned-state-through-uniform-carriers) whose custody index names the witness are the basic forms; ordinary compositions of admitted uniform fields are allowed." | Declaration and construction | Each field depending on witness `S` must use only admitted uniform forms or their compositions | not stated | 1
SOME_PACKAGES17 | "Inline owned `S`, witness-sized arrays, witness-dependent field offsets and unsized referents are not admitted by this rule." | Declaration and construction | Inline owned witness, witness-sized arrays, witness-dependent offsets, and unsized referents must be rejected | not stated | 1
SOME_PACKAGES18 | "A callable cannot require moving an unknown-layout `S` by value." | Declaration and construction | Callables must not require moving unknown-layout witness `S` by value | not stated | 3
SOME_PACKAGES19 | "Unrelated concrete owned fields remain ordinary data with their own cleanup." | Declaration and construction | Owned fields not tied to the witness keep ordinary ownership and cleanup rules | not stated | 2
SOME_PACKAGES20 | "Projecting or decomposing a package opens its witness to a fresh abstract type tied to that exact subject and storage version." | Opening by stable subject | Package projection or decomposition must introduce a fresh abstract witness type for that subject and storage version | at the operation | 7
SOME_PACKAGES21 | "All dependent projections in the same stable opening share that type." | Opening by stable subject | Dependent projections in one stable opening must share the same abstract witness type | not stated | 7
SOME_PACKAGES22 | "The rule is independent of expression spelling; it does not recognize a special `reader.read(reader.state)` idiom." | Opening by stable subject | Opening must not depend on special expression spellings such as paired field access idioms | not stated | 1
SOME_PACKAGES23 | "Here the callable and state have matching abstract types under the same opening." | Opening by stable subject | Under one opening, correlated callable and state fields must use matching abstract witness types | at the operation | 3
SOME_PACKAGES24 | "Invocation obtains an ordinary fresh reborrow of the stored mutable reference; it does not duplicate or consume that reference just to make a reusable call." | Opening by stable subject | Callable invocation must reborrow the stored mutable reference without duplicating or consuming it merely for callability | at a call | 2
SOME_PACKAGES25 | "The callable's complete contract governs preconditions, results, effects, suspension and custody." | Opening by stable subject | Invoked stored callables remain subject to their full contracts including suspension and custody | at a call | 3
SOME_PACKAGES26 | "Opening alone supplies neither those premises nor private facts about the concrete witness." | Opening by stable subject | Opening must not by itself establish callable premises or private concrete-witness facts | not stated | 8
SOME_PACKAGES27 | "Use canonical places, storage versions and captured index values to identify openings." | Opening by stable subject | Openings must be identified by canonical places, storage versions, and captured index values | not stated | 2
SOME_PACKAGES28 | "Repeated indexing shares an opening only when the checker establishes the same stable element and relevant index value." | Opening by stable subject | Repeated indexing shares one opening only with proof of same stable element and index value | not stated | 6
SOME_PACKAGES29 | "It must not equate two textual `items[i]` occurrences across a change to `i`, a collection relocation, or an overlapping write." | Opening by stable subject | Two syntactically identical index expressions must not share an opening across index change, relocation, or overlapping write | not stated | 6
SOME_PACKAGES30 | "Borrowing one element into a local gives an ordinary way to make the stable subject explicit." | Opening by stable subject | Element borrows into locals must be available as the ordinary explicit stable-subject mechanism | not stated | 2
SOME_PACKAGES31 | "Reassignment of the whole package establishes a new witness association." | Opening by stable subject | Whole-package reassignment must establish a new witness association | at the operation | 2
SOME_PACKAGES32 | "Live dependent projections prevent invalid replacement under the ordinary loan rules;" | Opening by stable subject | Active dependent projections must block invalid whole-package replacement under ordinary loan rules | on every edge | 2
SOME_PACKAGES33 | "otherwise the old opening and its facts expire." | Opening by stable subject | When replacement is valid, prior opening and its facts must expire | not stated | 8
SOME_PACKAGES34 | "Updating a field in an open package must preserve the existing witness and invariant." | Opening by stable subject | In-place field updates in an open package must keep the same witness and data invariant | at the operation | 1
SOME_PACKAGES35 | "Changing the witness requires constructing and replacing the complete package coherently, not mixing fields from old and new packages inside an invariant window." | Opening by stable subject | Witness change must go through full package reconstruction; mixed old/new fields inside an invariant window must reject | at the operation | 1
SOME_PACKAGES36 | "Moves transfer the package's witness association and custody." | Opening by stable subject | Moving a package must transfer witness association and custody with the value | at the operation | 2
SOME_PACKAGES37 | "Permitted copies retain evidence of their shared witness origin;" | Opening by stable subject | Allowed copies must retain checking evidence linking them to a shared witness origin | not stated | 8
SOME_PACKAGES38 | "unrelated packages do not gain type equality because they happen to contain the same concrete type." | Opening by stable subject | Distinct packages must not become witness-equal solely because concrete types match | not stated | 7
SOME_PACKAGES39 | "Joins keep correlated fields together while forgetting unproved concrete choices." | Opening by stable subject | Joins must preserve correlation of witness-dependent fields and drop unproved concrete witness facts | at the operation | 1
SOME_PACKAGES40 | "An explicit checked type equality may justify transport;" | Opening by stable subject | Transport of witness facts across packages is allowed only with explicit checked type equality | not stated | 8
SOME_PACKAGES41 | "equal sizes, erased addresses or equal runtime tags cannot manufacture that evidence." | Opening by stable subject | Size, erased address, or runtime tag equality must not substitute for checked witness transport evidence | not stated | 8
SOME_PACKAGES42 | "No executable cross-package type comparison or synthesized structural comparison is introduced merely by having a `some` binder." | Opening by stable subject | `some` must not add runtime cross-package witness comparison or synthesized structural comparison | not stated | 3
SOME_PACKAGES43 | "An abstract witness is usable within its stable opening, including by local bindings, checks and calls whose published operations support it." | Scope, escape and borrowing | Abstract witness use must stay within stable opening and supported operations | not stated | 7
SOME_PACKAGES44 | "It supplies no concrete field namespace, size-based allocation or arbitrary executable generic specialization." | Scope, escape and borrowing | Abstract witness must not provide concrete field namespace, witness-sized allocation, or arbitrary runtime generic specialization | not stated | 7
SOME_PACKAGES45 | "Generic code needs its ordinary operations/evidence; erasure does not synthesize missing implementations or runtime dictionaries." | Scope, escape and borrowing | Generic code must still have ordinary operations and evidence; erasure must not fabricate implementations or runtime dictionaries | not stated | 7
SOME_PACKAGES46 | "A result or retained field whose type contains the opened witness cannot escape its scope." | Scope, escape and borrowing | Types containing the opened abstract witness must not escape the opening scope in results or stored fields | at normal return | 2
SOME_PACKAGES47 | "A helper may return a complete `Reader` package, or another properly constructed package," | Scope, escape and borrowing | Returning a complete properly constructed package must be allowed | at normal return | 1
SOME_PACKAGES48 | "but cannot expose `reader.state` as an unbound `&S`." | Scope, escape and borrowing | Exposing an unbound reference to witness state must reject | at normal return | 2
SOME_PACKAGES49 | "A concrete result such as `&[u8]` may escape under its checked input-relative lifetime and custody contract." | Scope, escape and borrowing | Concrete results may escape only under their checked input-relative lifetime and custody | at normal return | 2
SOME_PACKAGES50 | "Borrowing, packing and opening never extend that lifetime." | Scope, escape and borrowing | Borrow, pack, and open operations must not extend result lifetimes beyond ordinary rules | not stated | 2
SOME_PACKAGES51 | "Existing access policies and reflection authority still apply; `some` adds no private fields, privileged consumer or dynamic downcast route." | Scope, escape and borrowing | Access policies and reflection limits remain; `some` must not add private fields, privileged consumers, or dynamic downcast | not stated | SHARED
SOME_PACKAGES52 | "Shared/exclusive access, suspension, movement, linear obligations and cleanup follow ordinary data rules." | Scope, escape and borrowing | Loans, suspension, movement, linearity, and cleanup use ordinary data rules for packages | not stated | 2
SOME_PACKAGES53 | "Dropping a borrowed package releases its loans when permitted; it does not destroy the borrowed state." | Scope, escape and borrowing | Drop of a borrowed package releases package loans only; borrowed inner state must survive when rules permit | at the operation | 2
SOME_PACKAGES54 | "Moving it into a container retains those loans." | Scope, escape and borrowing | Moving a borrowed package into a container must retain outstanding loans | at the operation | 2
SOME_PACKAGES55 | "Removing it only transfers them to the removed value;" | Scope, escape and borrowing | Removing a package from a container must transfer loans to the removed value | at the operation | 2
SOME_PACKAGES56 | "consumption releases access only when no dependent copy or result loan remains." | Scope, escape and borrowing | Consumption releases access only after dependent copies and result loans are gone | at the operation | 2
SOME_PACKAGES57 | "Runtime branches and generic helpers may append packages with different witness types to the same `Vec<Reader>`." | Scope, escape and borrowing | Heterogeneous per-value witnesses in one homogeneous external container type must be allowed when each element type-checks | at the operation | 6
SOME_PACKAGES58 | "Neither the number, order nor type choices of runtime entries require compile-time prediction." | Scope, escape and borrowing | Compiler must not require static prediction of container length, order, or witness choices | not stated | 6
SOME_PACKAGES59 | "Each construction and every use must be well-typed and obey the container's capacity/allocation contract." | Scope, escape and borrowing | Every construct and use in such containers must type-check and satisfy capacity/allocation contracts | at the operation; at a call | 6
SOME_PACKAGES60 | "A reference to a helper's local cannot survive that local through packaging." | Scope, escape and borrowing | Packaging must not let references to a helper local outlive that local | not stated | 2
SOME_PACKAGES61 | "A field depending on a `some` witness may own that state when its type is uniform: the witness occurs only in custody or evidence indices, such as `Extent in Granted & Resident<P, S>`, or behind admitted references and callable descriptors, never in a position that determines layout." | Owned state through uniform carriers | Owning witness state requires uniform field type with witness only in custody/evidence indices or behind admitted references or callables, never in layout-determining position | not stated | 2
SOME_PACKAGES62 | "The judgment is structural over the field's type and over every parameter and result of every stored callable." | Owned state through uniform carriers | Uniformity is a structural judgment on the field type and every stored callable parameter and result | not stated | 3
SOME_PACKAGES63 | "A parameter or result of type `Owner<S>` is admitted where `Owner<S>` is uniform; bare `S` by value stays rejected." | Owned state through uniform carriers | Uniform `Owner<S>` by value may appear; bare witness `S` by value must reject | not stated | 2
SOME_PACKAGES64 | "Equal sizes and authored assertions of uniformity establish nothing." | Owned state through uniform carriers | Size equality and author assertions must not establish uniformity without structural judgment | not stated | 8
SOME_PACKAGES65 | "The carrier is ordinary library data, not a privileged name." | Owned state through uniform carriers | Uniform carriers must be ordinary library types without privileged compiler names | not stated | 9
SOME_PACKAGES66 | "The qualified extent already owns one complete live `S` under [resident custody](../resources/access.md#establishment-and-retirement) and returns to its provider;" | Owned state through uniform carriers | Qualified extent must model one live owned witness under resident custody with return to provider | not stated | 2
SOME_PACKAGES67 | "an allocator family wraps it with its return handle where one is needed, and two owners of the same payload from different providers keep distinct return paths." | Owned state through uniform carriers | Allocators must preserve distinct return paths for owners from different providers | not stated | 2
SOME_PACKAGES68 | "Packing allocates nothing: it moves an established owner into the record." | Owned state through uniform carriers | Package construction must move an existing owner without new allocation | at the operation | 2
SOME_PACKAGES69 | "Stored operations take the owner, by reference or by value, rather than `&mut S`, so the consumer never specializes over the hidden type." | Owned state through uniform carriers | Stored callables must take uniform owner by ref or value, not `&mut S` that exposes witness for specialization | not stated | 3
SOME_PACKAGES70 | "Opening pairs `state` with its operations as above, while `&mut package.state` remains an ordinary field borrow." | Owned state through uniform carriers | Opening pairs state with operations; field borrow of state stays an ordinary borrow | at the operation | 2
SOME_PACKAGES71 | "Each stored operation keeps its own [callable contract](machine_values.md#callable-types-and-construction), and construction checks the selected machine against it;" | Owned state through uniform carriers | Each stored machine keeps its own contract; construction must check the chosen machine against that contract | at the operation | 3
SOME_PACKAGES72 | "one operation's reach or suspension does not attach to another because they share a record." | Owned state through uniform carriers | Reach and suspension obligations must not leak between stored callables solely because they share a record | not stated | 3
SOME_PACKAGES73 | "Consumption is any stored operation taking the owner by value." | Owned state through uniform carriers | Consumption is defined as any stored operation taking the uniform owner by value | not stated | 2
SOME_PACKAGES74 | "There is no terminal-consumer spelling, and a package may offer several, such as `run` and `cancel`." | Owned state through uniform carriers | No single terminal consumer form is required; multiple consuming operations may coexist | not stated | 2
SOME_PACKAGES75 | "Owning packages are `[linear]`." | Owned state through uniform carriers | Packages that own witness state through uniform carriers must be linear | not stated | 2
SOME_PACKAGES76 | "Packing preserves the hidden state's resident custody, contained loans, zero-sized obligations, storage claims, carry demands and code dependencies," | Owned state through uniform carriers | Packing must preserve resident custody, loans, ZST obligations, storage claims, carry demands, and code dependencies | at the operation | 2
SOME_PACKAGES77 | "and storing a consuming operation admits no automatic cleanup." | Owned state through uniform carriers | Storing a consuming operation must not trigger automatic cleanup of the hidden owner | not stated | 2
SOME_PACKAGES78 | "A pending or failure outcome returns the remaining ownership as a package exposing only the operations valid in that state," | Owned state through uniform carriers | Pending or failure paths must return remaining ownership as a package with only state-valid operations | at normal return | 2
SOME_PACKAGES79 | "An abstract witness introduced by a valid opening may construct another package within its scope under the ordinary construction, lifetime, custody and contract checks." | Owned state through uniform carriers | Repacking from an abstract witness must be allowed inside the opening under ordinary construction, lifetime, custody, and contract checks | at the operation | 7
SOME_PACKAGES80 | "This permits repacking after an incomplete settlement." | Owned state through uniform carriers | Repacking after incomplete settlement must be permitted when checks pass | not stated | 2
SOME_PACKAGES81 | "It grants no equality with another package's witness and extends no contained loan." | Owned state through uniform carriers | Witness repacking must not equate witnesses across packages or extend contained loans beyond ordinary rules | not stated | 2
SOME_PACKAGES82 | "Uniform borrowed or owning fields plus indirect callable descriptors give one external layout without a generated sum of all possible witnesses." | Representation, costs and replacement | External layout must be uniform without a generated sum type over all witnesses | not stated | 1
SOME_PACKAGES83 | "No global roster, tag assignment, or type/layout identity changes when another construction uses a new sized witness." | Representation, costs and replacement | Adding a new sized witness at construction must not require global roster, tag, or layout identity updates | not stated | 9
SOME_PACKAGES84 | "There is no implicit box or runtime witness equality test for a source-checked pairing." | Representation, costs and replacement | Must not introduce implicit boxing or runtime witness equality for source-checked pairings | not stated | 3
SOME_PACKAGES85 | "Descriptor representation and dispatch still have costs; loan/effect checking and external artifact verification remain required." | Representation, costs and replacement | Descriptor costs remain; loan/effect checking and external artifact verification must still run | not stated | SHARED
SOME_PACKAGES86 | "Type openings and pairings must be represented explicitly in checked evidence and carried through [Terminal dispatch](../terminal-psi/dynamic_dispatch.md)." | Representation, costs and replacement | Openings and witness pairings must appear in checked evidence and survive to Terminal dispatch | not stated | 8
SOME_PACKAGES87 | "Lowering must consume the established plan rather than re-deriving witness identity from source expressions." | Representation, costs and replacement | Lowering must use the checked witness plan, not re-derive identity from source syntax | not stated | 3
SOME_PACKAGES88 | "External structural checks preserve these associations under [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption)." | Representation, costs and replacement | Psi admission structural checks must preserve witness opening associations | not stated | 9
SOME_PACKAGES89 | "Trusted producer judgments may be reused; interpretation still checks actual pairings, custody and call access." | Representation, costs and replacement | Reused trusted judgments must still be verified against actual pairings, custody, and call access at interpretation | not stated | 9
SOME_PACKAGES90 | "An untrusted asserted pairing grants no authority." | Representation, costs and replacement | Untrusted asserted witness pairings must confer no checking authority | not stated | 8
SOME_PACKAGES91 | "This does not authorize arbitrary new code to enter a closed product." | Representation, costs and replacement | `some` must not waive closed-product restrictions on admitting new code | not stated | 9
SOME_PACKAGES92 | "Build admission, reachability, code retention and replacement contracts still apply." | Representation, costs and replacement | Build admission, reachability, retention, and replacement rules remain in force | not stated | 9
SOME_PACKAGES93 | "No Fused-only global-roster mechanism is required, but a package containing local machine descriptors, borrows or owned state whose layout a component defines cannot cross an Independent boundary unchanged." | Representation, costs and replacement | Independent-boundary crossing must not move unchanged packages with local descriptors, borrows, or component-defined owned layout | not stated | 9
SOME_PACKAGES94 | "Use its admitted boundary representation or a local proxy." | Representation, costs and replacement | Cross-boundary transport must use admitted boundary representation or a local proxy | not stated | 9
SOME_PACKAGES95 | "Quiescence does not convert old state or release stored code dependencies: retain the old era, drain the packages, or consume and reconstruct them around explicit state migration." | Representation, costs and replacement | Quiescence must not silently migrate state or drop code dependencies; listed migration strategies must be enforced | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Multiple `some` binders choose separate witnesses, with any relationships stated and checked in the declaration." | Every cross-binder relationship must be written and checked in the declaration. | Separate witnesses are mandatory; cross-binder relationships are optional and checked only when stated. |
"Construction infers one concrete witness for each `some` binder from the field initializers and checks all related fields against that same choice, including callable contracts and the data invariant." | Inference runs only from field initializers in a record literal. | Other construction forms might infer witnesses from additional contexts not named in this sentence. |
"A generic helper may construct a package using its ordinary static type argument as the witness," | Generic helpers are always allowed to use a type parameter as witness when consistent. | Permission applies only in helpers that satisfy unstated conditions beyond this sentence. |
"Witnesses remain checking evidence, not runtime source-level type values." | The compiler must emit no runtime representation of witness types. | Witnesses are invisible to source-level reflection but a distinct runtime witness carrier might still exist outside source types. |
"Opening alone supplies neither those premises nor private facts about the concrete witness." | No facts from opening except abstract witness typing. | Callable facts listed as absent might still be obtained from other fields of the same package without counting as "opening alone." |
"Repeated indexing shares an opening only when the checker establishes the same stable element and relevant index value." | Textually identical index expressions share an opening only with a stability proof. | Other proof paths not involving "relevant index value" might still justify sharing an opening. |
"Permitted copies retain evidence of their shared witness origin;" | Only copies explicitly allowed by the checker retain shared-origin evidence. | Any value-copy operation automatically carries shared-origin evidence when witnesses match. |
"Updating a field in an open package must preserve the existing witness and invariant." | Any in-place field update in an opened package must keep the same witness type. | Updates that replace the field with a value of the same abstract witness under the opening satisfy "preserve" even if concrete witness changes. |
"Equal sizes and authored assertions of uniformity establish nothing." | The checker must ignore size and author assertions for uniformity. | Assertions may affect diagnostics or review but not the structural uniformity judgment. |
"Packing allocates nothing: it moves an established owner into the record." | Packing never performs heap allocation. | Packing never allocates a new owner cell but might still allocate container or descriptor storage elsewhere. |
"storing a consuming operation admits no automatic cleanup." | Storing a consumer in the record does not trigger drop of the hidden owner. | No automatic cleanup hook runs for the package as a whole when a consumer is stored. |
"Trusted producer judgments may be reused; interpretation still checks actual pairings, custody and call access." | Reuse is optional; interpretation always re-verifies pairings. | Reuse might skip some checks when trust level is high enough, with "still checks" naming only a subset. |
"No Fused-only global-roster mechanism is required, but a package containing local machine descriptors, borrows or owned state whose layout a component defines cannot cross an Independent boundary unchanged." | Independent crossing is forbidden whenever any listed content is present. | Crossing is forbidden only when layout is component-defined among the listed contents. |
"Quiescence does not convert old state or release stored code dependencies: retain the old era, drain the packages, or consume and reconstruct them around explicit state migration." | Quiescence requires choosing exactly one of the three listed strategies. | Quiescence forbids silent conversion; any explicit strategy meeting migration contracts suffices and combinations are not ruled out here. |
"Different choices can therefore inhabit one array or growable container." | Heterogeneous witnesses in one homogeneous external container must be supported. | The sentence is explanatory motivation; mandatory support is established only where later sections say "may" or "must." |

## Terms used without definition in this chapter

- `some` / `some S` / data binder
- witness / concrete witness / abstract witness / packaged `S`
- external data type / external type
- ordinary generic argument
- captured `dyn` / compiler-derived global roster
- inference hole / universal type / runtime type object (negated categories)
- data declaration / data invariant
- callable contract / callable descriptor / machine (stored)
- record literal / package
- checking evidence / source-level type values
- generic helper / opened abstract witness
- sized concrete witnesses (V1)
- uniform representation / uniform fields / uniform carriers
- custody index
- sized referents / unsized referents
- witness-dependent field offsets
- stable opening / stable subject
- storage version
- canonical places / captured index values
- dependent projections
- fresh reborrow
- preconditions / suspension / custody (callable contract parts)
- private facts (concrete witness)
- invariant window
- witness association
- permitted copies / shared witness origin
- joins (correlated fields)
- explicit checked type equality / transport
- erased addresses / runtime tags
- published operations
- erasure / runtime dictionaries
- input-relative lifetime
- access policies / reflection authority
- privileged consumer / dynamic downcast route
- linear obligations / `[linear]`
- resident custody / provider / return handle / return paths
- qualified extent / Granted / Resident
- `Owner<S>`
- terminal-consumer spelling
- zero-sized obligations / storage claims / carry demands / code dependencies
- pending or failure outcome
- incomplete settlement / repacking
- global roster / tag assignment
- implicit box
- Terminal dispatch
- checked evidence / established plan
- Psi admission / external structural checks
- trusted producer judgments
- closed product
- Build admission / reachability / code retention / replacement contracts
- Fused / Independent boundary
- admitted boundary representation / local proxy
- quiescence / old era / explicit state migration
- runtime machine values (cross-reference category)

CHAPTER COMPLETE
