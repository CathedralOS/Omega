# Counts, indices, and addresses

Source: `wiki/spec/language/counts_and_addresses.md` lines 1–277 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| COUNTS_AND_ADDRESSES1 | "General lengths, sizes, and compiler-facing policy geometry use `u64`." | Separate roles | Count roles must use `u64` for lengths, sizes, and compiler-facing policy geometry | not stated | 5 |
| COUNTS_AND_ADDRESSES2 | "Index — Any integer operand whose use proves the collection's bounds; no privileged index carrier." | Separate roles | Index uses must prove collection bounds; no privileged index carrier type may be assumed | at the operation | 6 |
| COUNTS_AND_ADDRESSES3 | "Address data grants no storage or access authority." | Separate roles | `addr` values must not confer storage or access authority by themselves | not stated | 2 |
| COUNTS_AND_ADDRESSES4 | "`usize` and `isize` are not Omega carriers." | Separate roles | Programs must not use `usize` or `isize` as language carriers | not stated | 5 |
| COUNTS_AND_ADDRESSES5 | "No global width equality or ordering relates counts to addresses." | Separate roles | The type system must not relate count and address widths by global equality or ordering | not stated | 5 |
| COUNTS_AND_ADDRESSES6 | "Interpreting a count as address geometry requires an occurrence-specific fit proof." | Separate roles | Each count-to-address-geometry use must have a fit proof at that occurrence | at the operation | 8 |
| COUNTS_AND_ADDRESSES7 | "Compiler-facing nonnegative schema sizes, lengths, counts, and indices likewise use `u64`, not signed integers or `addr`." | Separate roles | Compiler-facing nonnegative sizes, lengths, counts, and indices must be `u64`, not signed integers or `addr` | not stated | 5 |
| COUNTS_AND_ADDRESSES8 | "Signed offsets/addends remain signed." | Separate roles | Signed offsets and addends must stay on signed integer carriers | not stated | 5 |
| COUNTS_AND_ADDRESSES9 | "A future explicitly admitted integer-width family would not change `.len`, `Extent.length`, indexing, or conversion contracts through a package alias." | Separate roles | Package aliases must not alter `.len`, `Extent.length`, indexing, or conversion contracts via a future width family | not stated | 6 |
| COUNTS_AND_ADDRESSES10 | "The storage operand identifies the referent, not the temporary reference descriptor." | Address exposure | `address_of` on storage must target the referent, not the reference descriptor | at the operation | 2 |
| COUNTS_AND_ADDRESSES11 | "It must satisfy ordinary borrowing and addressable-storage rules;" | Address exposure | `address_of` storage operand must meet borrowing and addressable-storage rules | at the operation | 2 |
| COUNTS_AND_ADDRESSES12 | "the query cannot establish a loan to otherwise inaccessible storage." | Address exposure | `address_of` must not create access to storage that borrowing rules forbid | at the operation | 2 |
| COUNTS_AND_ADDRESSES13 | "Compatible views of the same storage location expose the same address while that location remains in place." | Address exposure | Compatible views of one in-place location must yield the same address | at the operation | 2 |
| COUNTS_AND_ADDRESSES14 | "The result retains neither a loan nor access authority." | Address exposure | `address_of` results must carry no loan and no access authority | at the operation | 2 |
| COUNTS_AND_ADDRESSES15 | "Nonaddressable projections cannot acquire an address by this query." | Address exposure | Nonaddressable projections must reject `address_of` | at the operation | 2 |
| COUNTS_AND_ADDRESSES16 | "Overloaded or unresolved application families reject." | Address exposure | Static `address_of` on machines must reject overloaded or unresolved application families | at the operation | 3 |
| COUNTS_AND_ADDRESSES17 | "No ordinary arguments are supplied or evaluated, no body is invoked, and no context is captured." | Address exposure | Static machine `address_of` must not supply, evaluate, invoke, or capture context for ordinary arguments | at the operation | 3 |
| COUNTS_AND_ADDRESSES18 | "A carrier-qualified machine retains its ordinary explicit parameter contract; qualification supplies no context argument or storage." | Address exposure | Carrier qualification must not add context arguments or storage beyond the machine's explicit parameter contract | at the operation | 10 |
| COUNTS_AND_ADDRESSES19 | "This is a representation query, not a value conversion or size-compatible representation recast." | Address exposure | `address_of` must be treated as representation query only, not conversion or layout recast | at the operation | 5 |
| COUNTS_AND_ADDRESSES20 | "Its static machine form does not make a declaration a runtime value or retain context arguments." | Address exposure | Static machine `address_of` must not turn the declaration into a runtime value or retain context arguments | at the operation | 3 |
| COUNTS_AND_ADDRESSES21 | "Ordinary machine calls still require their complete argument lists." | Address exposure | Machine calls other than `address_of` must still require full argument lists | at a call | 3 |
| COUNTS_AND_ADDRESSES22 | "Machine/reference `as addr` is not an alternate spelling;" | Address exposure | `as addr` on machines or references must not be accepted as spelling of `address_of` | at the operation | 5 |
| COUNTS_AND_ADDRESSES23 | "neither arbitrary value-to-address conversions nor the inverse address-to-borrow/callable conversion follow from these queries." | Address exposure | Arbitrary value↔address conversions must not be derived from `address_of` alone | at the operation | 2 |
| COUNTS_AND_ADDRESSES24 | "`size_of<T>()` and `align_of<T>()`, both returning `u64`." | Address exposure | `size_of` and type-form `align_of` must return `u64` | at the operation | 5 |
| COUNTS_AND_ADDRESSES25 | "A numeric result type does not choose a property to inspect: sizes and alignments are not casts to a count or a distinct alignment carrier." | Address exposure | Result numeric type must not select which size/alignment property is inspected | at the operation | 5 |
| COUNTS_AND_ADDRESSES26 | "A retained machine-address expression creates an executable-entry dependency on its exact selected application even when no direct call exists." | Executable demand and native realization | Retained machine `address_of` must register executable-entry dependency on the exact selected application | not stated | 9 |
| COUNTS_AND_ADDRESSES27 | "Native realization retains an addressable entry and resolves its relocation at linking/loading." | Executable demand and native realization | Native realization must retain addressable entries and resolve relocations at link/load | not stated | 9 |
| COUNTS_AND_ADDRESSES28 | "Observably equivalent optimizations must preserve the exposed address observations." | Executable demand and native realization | Optimizations must preserve observably equivalent exposed addresses | not stated | 9 |
| COUNTS_AND_ADDRESSES29 | "A proof-only entity with no executable realization rejects in both native and interpreted execution." | Executable demand and native realization | Proof-only entities without executable realization must reject when address is demanded | at the operation | 8 |
| COUNTS_AND_ADDRESSES30 | "Do not fabricate an empty body, a zero result, or an entry ID for it." | Executable demand and native realization | Compiler must not invent bodies, zero results, or entry IDs for proof-only address targets | at the operation | 9 |
| COUNTS_AND_ADDRESSES31 | "Use by a proof alone does not classify an otherwise executable machine as proof-only." | Executable demand and native realization | Proof-only use must not reclassify an executable machine as proof-only by itself | not stated | 8 |
| COUNTS_AND_ADDRESSES32 | "Dead or erased address expressions need not force emission; erasure does not excuse an ill-formed query." | Executable demand and native realization | Erased addresses may skip emission but ill-formed queries must still reject | not stated | 9 |
| COUNTS_AND_ADDRESSES33 | "The address belongs to the selected realization, not to a permanent declaration identity." | Executable demand and native realization | Exposed addresses must bind to realization identity, not permanent declaration identity | not stated | 9 |
| COUNTS_AND_ADDRESSES34 | "Unequal addresses establish unequal exposed addresses, not different signatures or mathematical functions." | Executable demand and native realization | Address inequality must not be treated as signature or function difference | not stated | 3 |
| COUNTS_AND_ADDRESSES35 | "An address does not describe a contiguous body-sized byte range, require every call to pass through that entry, or make code patchable." | Executable demand and native realization | Addresses must not be interpreted as full body ranges, mandatory entry routing, or patchability | not stated | 9 |
| COUNTS_AND_ADDRESSES36 | "It supplies no calling convention, checked indirect-call permission, installation authority, or code lifetime extension." | Executable demand and native realization | Addresses alone must not imply calling convention, indirect-call permission, installation, or lifetime extension | not stated | 3 |
| COUNTS_AND_ADDRESSES37 | "Address exposure does not make those bits canonical inputs to semantic evaluation, turn interpreter IDs into portable constants, or give a `const` declaration its own storage." | Executable demand and native realization | Address bits must not become semantic-evaluation constants, portable interpreter IDs, or `const` storage | not stated | 9 |
| COUNTS_AND_ADDRESSES38 | "For interpreted execution, address exposure returns a stable numeric identity for the retained machine entry or runtime storage location, not a native host pointer." | Interpreted address identities | Interpreted `address_of` must return stable numeric identity, not host pointers | at the operation | 10 |
| COUNTS_AND_ADDRESSES39 | "A valid interpreted machine does not return zero merely because it has no native body." | Interpreted address identities | Interpreted machine addresses must not be zero solely for lack of native body | at the operation | 10 |
| COUNTS_AND_ADDRESSES40 | "Code-entry and storage-location encodings occupy disjoint namespaces." | Interpreted address identities | Entry and storage interpreted IDs must use disjoint encoding namespaces | not stated | 10 |
| COUNTS_AND_ADDRESSES41 | "Tag by address kind, not source type: changing a compatible view's type cannot change the referent's address." | Interpreted address identities | Address tagging must follow address kind; compatible view type changes must not change referent address | at the operation | 2 |
| COUNTS_AND_ADDRESSES42 | "Repeated exposure of the same live entry/location returns the same identity." | Interpreted address identities | Repeated exposure of the same live entry or location must yield identical identity | at the operation | 10 |
| COUNTS_AND_ADDRESSES43 | "Distinct live locations must not collide through accidental reuse of a compiler-local variable number, an entry number, or a number from another loaded program whose addresses can meet." | Interpreted address identities | Distinct live locations must not collide via reused numbers across variables, entries, or programs | not stated | 10 |
| COUNTS_AND_ADDRESSES44 | "Storage identity includes the actual runtime occurrence and selected subplace, not just the containing record's or declaring machine's ID." | Interpreted address identities | Storage interpreted identity must include runtime occurrence and subplace, not container ID alone | not stated | 2 |
| COUNTS_AND_ADDRESSES45 | "Complete identities must fit the selected encoding." | Interpreted address identities | Interpreted identities must fit the encoding without truncation ambiguity | not stated | 10 |
| COUNTS_AND_ADDRESSES46 | "Never truncate, hash without collision handling, or mask away significant bits to make room for tags; an exhausted encoding rejects before publishing an ambiguous value." | Interpreted address identities | Exhausted encoding must reject; truncation, unhandled hashing, or masking significant bits is forbidden | not stated | 10 |
| COUNTS_AND_ADDRESSES47 | "Zero remains the absence encoding, not a successful exposure of a live interpreted entry/location." | Interpreted address identities | Zero must mean absence, not a successful live interpreted address | at the operation | 10 |
| COUNTS_AND_ADDRESSES48 | "Their numeric order or difference does not establish byte adjacency or an offset within Psi instructions." | Interpreted address identities | Interpreted address order or difference must not prove byte adjacency or Psi instruction offsets | not stated | 6 |
| COUNTS_AND_ADDRESSES49 | "any operation needing physical memory geometry requires an explicit admitted mapping, not arithmetic on these identities." | Interpreted address identities | Physical memory geometry must use explicit admitted mapping, not arithmetic on interpreted IDs | at the operation | 2 |
| COUNTS_AND_ADDRESSES50 | "`align_of(value)` is the value form of the existing compiler-known core query, not a keyword or a cast." | Location alignment | Value `align_of` must be core query only, not keyword or cast syntax | at the operation | 5 |
| COUNTS_AND_ADDRESSES51 | "It returns a positive power-of-two `u64` alignment guaranteed for the observed storage location." | Location alignment | Value `align_of` must return positive power-of-two `u64` for observed location | at the operation | 5 |
| COUNTS_AND_ADDRESSES52 | "For `&T` or `&mut T`, it observes the referent, not the reference descriptor." | Location alignment | Reference operands to value `align_of` must observe referent, not descriptor | at the operation | 2 |
| COUNTS_AND_ADDRESSES53 | "For a borrowed slice, it observes the first element's location, not the slice descriptor; this form requires a nonempty slice." | Location alignment | Slice `align_of` must use first element and reject empty slices | at the operation | 6 |
| COUNTS_AND_ADDRESSES54 | "It does not claim that every element shares that alignment: element offsets and stride still matter." | Location alignment | Compiler must not infer all slice elements share first-element alignment without offset/stride reasoning | at the operation | 6 |
| COUNTS_AND_ADDRESSES55 | "Querying a numeric `addr` place measures that place, not storage at the numeric address." | Location alignment | `align_of` on `addr` place measures the place, not memory at that numeric value | at the operation | 2 |
| COUNTS_AND_ADDRESSES56 | "An inaccessible or nonaddressable subject does not acquire a location through the query." | Location alignment | Inaccessible or nonaddressable subjects must not gain a location via `align_of` | at the operation | 2 |
| COUNTS_AND_ADDRESSES57 | "The operand is observed once without moving its value or reading element bytes." | Location alignment | Value `align_of` must not move operand or read element bytes | at the operation | 2 |
| COUNTS_AND_ADDRESSES58 | "The query neither creates nor retains an access loan, initializes storage, strengthens permissions or authorizes a memory operation." | Location alignment | `align_of` must not create loans, init storage, strengthen permissions, or authorize memory ops | at the operation | 2 |
| COUNTS_AND_ADDRESSES59 | "An empty slice cannot justify an implicit `&slice[0]`; guard or use an independently justified backing view first." | Location alignment | Empty slices must not drive implicit `&slice[0]`; nonempty guard or backing view required first | at the operation | 6 |
| COUNTS_AND_ADDRESSES60 | "Mutating data bytes does not itself move the location, but replacing the view, changing its offset, moving its backing or ending its lifetime cannot preserve a stale relationship." | Location alignment | After view/offset/backing/lifetime changes, stale alignment relationships must not be reused | at the operation | 2 |
| COUNTS_AND_ADDRESSES61 | "Pointer adjustments must reestablish the resulting location's alignment from the original guarantee and the offset." | Location alignment | Pointer adjustments must re-prove resulting location alignment from original guarantee and offset | at the operation | 2 |
| COUNTS_AND_ADDRESSES62 | "A copied scalar alignment value alone does not identify the storage it once described." | Location alignment | Stored alignment scalars must not identify storage without live location reasoning | not stated | 2 |
| COUNTS_AND_ADDRESSES63 | "Missing required geometry rejects rather than fabricating a result." | Location alignment | Missing alignment geometry must reject, not synthesize a value | at the operation | 8 |
| COUNTS_AND_ADDRESSES64 | "`code_layout<SelectedMachine<Arguments>>()` describes the exact selected native realization." | Native code layout queries | `code_layout` must describe only the exact selected native realization | at the operation | 9 |
| COUNTS_AND_ADDRESSES65 | "Like `address_of`, it is a compiler-known core query, not a keyword or an invocation of that machine." | Native code layout queries | `code_layout` must be core query, not keyword or machine call | at the operation | 3 |
| COUNTS_AND_ADDRESSES66 | "Its retained use creates the same executable demand." | Native code layout queries | Retained `code_layout` must create same executable-entry demand as `address_of` | not stated | 9 |
| COUNTS_AND_ADDRESSES67 | "The description contains an entry `addr` and a finite sequence of code regions, each with `base: addr` and `length: u64`." | Native code layout queries | Layout description must include entry `addr` and regions with `base`/`length: u64` | not stated | 9 |
| COUNTS_AND_ADDRESSES68 | "It grants no authority or storage lifetime extension." | Native code layout queries | `code_layout` results must grant no authority or lifetime extension | not stated | 2 |
| COUNTS_AND_ADDRESSES69 | "They do not include unrelated intervening bytes, padding, data pools or transitively called machines merely because those lie nearby or are needed for execution." | Native code layout queries | Regions must exclude unrelated bytes, padding, pools, and transitive callees not attributed to the realization | not stated | 9 |
| COUNTS_AND_ADDRESSES70 | "Unsupported or unavailable realization metadata rejects rather than inventing a region." | Native code layout queries | Missing realization metadata must reject, not invent regions | at the operation | 9 |
| COUNTS_AND_ADDRESSES71 | "Neither its sizes nor its addresses may feed that same program's semantic constants, type arguments, layout policies or optimization decisions through a circular query." | Native code layout queries | Same-program semantic constants, type args, layout policy, and optimizations must not circularly depend on `code_layout` sizes/addresses | not stated | 9 |
| COUNTS_AND_ADDRESSES72 | "`size_of<Machine>()` and `align_of<Machine>()` reject: a machine declaration is not a data carrier." | Native code layout queries | `size_of`/`align_of` on machine types must reject | at the operation | 10 |
| COUNTS_AND_ADDRESSES73 | "A sum of code-region lengths does not make the interval `[entry, entry + sum)` valid." | Native code layout queries | Summed region lengths must not validate a contiguous `[entry, entry+sum)` interval by themselves | not stated | 6 |
| COUNTS_AND_ADDRESSES74 | "A purely interpreted body has no native regions; its entry ID is not a bytecode address, and this query supplies neither a fake native size nor an implicit Psi representation-size API." | Native code layout queries | Interpreted-only bodies must yield no native regions and no fake sizes from `code_layout` | at the operation | 10 |
| COUNTS_AND_ADDRESSES75 | "This describes a 2 KiB window, not the machine's actual size and not access authority." | Describing an address window | Extent windows describe geometry only, not machine size or access rights | not stated | 6 |
| COUNTS_AND_ADDRESSES76 | "Establishing valid geometry proves `embed(region.base) + embed(region.length) <= addr::Bound` in unbounded proof integers." | Describing an address window | Valid extents must prove non-wrapping bound in unbounded integers | at the operation | 8 |
| COUNTS_AND_ADDRESSES77 | "It does not compute a wrapping one-past address." | Describing an address window | Extent geometry must not use wrapping one-past-end address arithmetic | at the operation | 5 |
| COUNTS_AND_ADDRESSES78 | "Backing, read/write rights, lifetime and custody are separate extent obligations." | Describing an address window | Extent byte geometry does not discharge backing, rights, lifetime, or custody | not stated | 2 |
| COUNTS_AND_ADDRESSES79 | "Interpreted address identities do not supply this byte geometry." | Describing an address window | Byte extent geometry must not be derived from interpreted address identities alone | at the operation | 6 |
| COUNTS_AND_ADDRESSES80 | "Scalar indexing requires `0 <= index < live_length`." | Bounds and lowering | Scalar index operations must prove `0 <= index < live_length` | at the operation | 6 |
| COUNTS_AND_ADDRESSES81 | "Unsigned type range proves the lower bound; a signed index needs evidence of nonnegativity." | Bounds and lowering | Signed indices need nonnegativity evidence; unsigned types may prove lower bound | at the operation | 5 |
| COUNTS_AND_ADDRESSES82 | "The upper bound must refer to the current collection, through enforced types, constants, live guards, or a selected accessor's checked contract." | Bounds and lowering | Upper bound must tie to current collection via types, constants, guards, or accessor contract | at the operation | 6 |
| COUNTS_AND_ADDRESSES83 | "Mutation invalidates stale facts under invocation contracts." | Bounds and lowering | After mutation, stale bound facts from invocation contracts must be invalidated | at the operation | 2 |
| COUNTS_AND_ADDRESSES84 | "A fixed array `[T; N]` has length exactly `N`." | Bounds and lowering | Fixed arrays must have compile-time length exactly `N` | not stated | 6 |
| COUNTS_AND_ADDRESSES85 | "A growable collection's live length is nonnegative and at most its capacity; indexing uses that live length, not unused capacity." | Bounds and lowering | Growable indexing must use live length ≤ capacity, not spare capacity | at the operation | 6 |
| COUNTS_AND_ADDRESSES86 | "`[T; N] in D` still has length exactly `N`, while `FixedVec<T, N> in D` and `Vec<T> in D` retain their explicitly declared live-length semantics." | Bounds and lowering | Domain qualification must not change fixed-array length `N` or declared growable live-length rules | not stated | 6 |
| COUNTS_AND_ADDRESSES87 | "Index proofs do not create a modular index value or change the operand's arithmetic semantics." | Bounds and lowering | Bounds checking must not rewrite index to modular arithmetic or alter operand semantics | at the operation | 6 |
| COUNTS_AND_ADDRESSES88 | "Once checked, they require no runtime proof object." | Bounds and lowering | Proved index bounds must not require a runtime proof object after check | at the operation | 8 |
| COUNTS_AND_ADDRESSES89 | "Physical lowering may select the narrowest representation justified by the complete bound, retaining the source carrier's meaning and observations; `u64` is the general count representation when no narrower bound is established." | Bounds and lowering | Lowering may narrow count representation only with full bound evidence while preserving meaning; default count carrier is `u64` | not stated | 5 |
| COUNTS_AND_ADDRESSES90 | "On a target with exclusive address bound `2^32`, `no_wrap(base, length)` establishes at most `length <= 2^32`, not that the length fits `u32`." | Bounds and lowering | `no_wrap` with exclusive `2^32` bound must establish `length <= 2^32`, not automatic `u32` fit | at the operation | 5 |
| COUNTS_AND_ADDRESSES91 | "Lowering must not replace `<=` with `<`." | Bounds and lowering | Lowering must preserve inclusive `<=` where the spec states inclusive bounds | not stated | 5 |
| COUNTS_AND_ADDRESSES92 | "Exact narrowing requires positive evidence that the value fits." | Bounds and lowering | Narrowing conversions need positive fit evidence | at the operation | 5 |
| COUNTS_AND_ADDRESSES93 | "Failure to prove overflow is not that evidence." | Bounds and lowering | Absence of overflow proof must not count as narrowing evidence | at the operation | 8 |
| COUNTS_AND_ADDRESSES94 | "Otherwise reject or use an explicitly selected conversion with its declared Wrapping, Saturating, or Trapping policy." | Bounds and lowering | Without fit evidence, reject or require explicit conversion policy | at the operation | 5 |
| COUNTS_AND_ADDRESSES95 | "Erasing a refinement can require a later use to prove its bound again." | Bounds and lowering | After refinement erasure, later uses may need to re-prove bounds | at the operation | 8 |
| COUNTS_AND_ADDRESSES96 | "Its introduction follows the domain's declared proof and/or sealed route;" | Nominal index qualifications | Domain-qualified indices must be introduced only via domain proof or sealed route | at the operation | 8 |
| COUNTS_AND_ADDRESSES97 | "absence of a violated predicate does not establish membership." | Nominal index qualifications | Lack of violated domain predicate must not prove domain membership | not stated | 8 |
| COUNTS_AND_ADDRESSES98 | "Nominal identity and a live collection bound are separate facts." | Nominal index qualifications | Nominal index tags and live bounds must be proved separately | at the operation | 8 |
| COUNTS_AND_ADDRESSES99 | "An accessor must reestablish the current bound, or an ordinary generational-handle abstraction must validate its generation and bounds." | Nominal index qualifications | After storage shrink, accessors or generational handles must revalidate bounds | at the operation | 6 |
| COUNTS_AND_ADDRESSES100 | "A nominal tag alone never licenses a stale access." | Nominal index qualifications | Nominal tags must not authorize access on stale bounds | at the operation | 8 |
| COUNTS_AND_ADDRESSES101 | "Domain qualification does not itself introduce a new indexing primitive or an implicit runtime check." | Nominal index qualifications | Domain qualification must not add new indexing primitives or implicit runtime bounds checks | not stated | 6 |
| COUNTS_AND_ADDRESSES102 | "Count — A nonnegative magnitude." | Separate roles | Count-typed magnitudes must be nonnegative | not stated | 5 |
| COUNTS_AND_ADDRESSES103 | "`address_of` is a core name, not a reserved keyword." | Address exposure | `address_of` must be recognized as core name, not as reserved keyword spelling | not stated | SHARED |
| COUNTS_AND_ADDRESSES104 | "No native trampoline is created by the query." | Interpreted address identities | `address_of` must not create a native trampoline | at the operation | 10 |
| COUNTS_AND_ADDRESSES105 | "For an ordinary addressable place, the query observes that place's storage." | Location alignment | Value `align_of` on addressable places must observe that place's storage | at the operation | 2 |
| COUNTS_AND_ADDRESSES106 | "Ordinary storage, borrow and lifetime obligations remain in force." | Location alignment | `align_of` must not discharge ordinary storage, borrow, or lifetime obligations | at the operation | 2 |
| COUNTS_AND_ADDRESSES107 | "Shared code can appear in more than one realization's description; a region is not exclusive ownership." | Native code layout queries | Shared code may appear in multiple layout descriptions; regions must not imply exclusive ownership | not stated | 9 |
| COUNTS_AND_ADDRESSES108 | "The queries do not force other calls to avoid inlining, prevent code sharing, make pages writable, or guarantee that editing an exposed entry edits every invocation." | Native code layout queries | `address_of`/`code_layout` must not mandate anti-inlining, anti-sharing, writable pages, or global entry patch semantics | not stated | 9 |
| COUNTS_AND_ADDRESSES109 | "Bounds are inclusive where stated." | Bounds and lowering | Where the spec states inclusive bounds, checks and lowering must treat bounds as inclusive | not stated | 6 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Index — Any integer operand whose use proves the collection's bounds; no privileged index carrier." | Any integer type may index if bounds are proved at use sites. | "No privileged index carrier" forbids a dedicated index type even when bounds are proved elsewhere. |
| "A count can exceed one target's address space; an address representation can be wider than the count carrier." | These are independent facts programs must reason about separately. | The compiler must allow count values that cannot map to addresses on some targets without rejecting the program globally. |
| "Unrelated magnitudes require no address interpretation." | Counts with no address role need no fit proof. | Counts unrelated to a particular address expression need no fit proof for that expression only. |
| "The static machine argument selects a declaration and its required arguments, including selected conformances and machine arguments, not merely a signature." | Resolution must close conformances and machine arguments before `address_of` is well-formed. | Selection is by declaration identity; conformances may be checked later at realization. |
| "A static machine binder may supply the selected application under the same rule." | Binder syntax can stand in for explicit machine argument in `address_of`. | Only certain binder forms qualify; which forms is not enumerated in this chapter. |
| "The ordinary specialization, retention, and realization pipeline follows that dependency; no separate registration is required." | Address dependency is carried automatically by the existing pipeline. | Programs need not call an explicit registration API; whether implicit registration has ordering constraints is not stated. |
| "An executable machine used only during compilation so far is still eligible: the address request creates the runtime demand." | Compile-only machines become runtime entries when first addressed. | Eligibility is conditional on machine kind; "so far" might exclude machines never emitted for other reasons. |
| "Dead or erased address expressions need not force emission; erasure does not excuse an ill-formed query." | Well-formed erased queries skip emission; ill-formed queries reject even if unreachable. | Erasure is an optimization hint; ill-formed queries might still reject only when the expression is retained. |
| "Code sharing may give distinct applications the same native entry; different realizations need not preserve addresses." | Same native address does not imply same source application identity. | Distinct applications may compare equal on address observably; whether source types must remain distinguishable is not stated. |
| "Reserved high-order kind bits are a valid implementation; their exact allocation is not source ABI." | Implementations may tag kinds in high bits without stabilizing bit positions in source. | Kind bits are mandatory for correctness but their layout is implementation-defined at runtime only. |
| "It may establish stronger alignment than the type's minimum; it need not expose the greatest divisor of a physical numeric address." | Value `align_of` may report alignment stronger than type minimum but weaker than physical maximum. | Stronger alignment is optional reporting; programs cannot rely on receiving the tightest physically possible alignment. |
| "Validated layout, placement and allocator contracts can discharge alignment goals statically." | Some alignment goals may be proved without runtime `align_of`. | "Discharge" means proof obligations only; runtime queries may still be required for other goals. |
| "A location-dependent query may require runtime computation; it is not automatically a semantic constant." | Value `align_of` is runtime unless layout/placement proofs make it constant. | "Semantic constant" is a specific evaluation tier; constant folding might still apply outside semantic evaluation. |
| "The final layout producer owns the complete description, not source-order subtraction or a guessed next-symbol address." | Only the layout stage may author region lists; front-end must not infer regions from symbols. | Layout producer may delegate internally; "owns" means authoritative output, not exclusivity of algorithms. |
| "A separately completed artifact may be inspected as an ordinary explicit input." | Finished binaries may supply `code_layout`-like facts as external inputs. | Only build-graph artifacts completed before the current module may be queried; cross-build inspection rules are elsewhere. |
| "The whole-space case `base == 0, length == 2^32` is valid count geometry but cannot narrow to `u32`." | Such extents may type-check for geometry but block `u32` narrowing without extra premises. | Valid geometry always permits some representation; whether lowering must use `u64` only is not stated here. |
| "A separate premise such as `base > 0` can establish the stricter bound." | Additional hypotheses may enable `u32` narrowing when geometry alone cannot. | Only `base > 0` is named; other premises that establish stricter bounds are not listed. |
| "A domain qualification may distinguish, for example, a room index from an item index." | Domains may mark indices nominally without changing bounds rules. | Distinction is purely type-level; runtime representation may still be identical. |
| "Shrinking storage can invalidate a formerly established bound." | Capacity or length shrink invalidates prior index proofs. | Only explicit shrink operations invalidate; implicit invalidation on mutation is covered only via accessor/rehandle rules. |

## Terms used without definition in this chapter

- Count (table role vs general magnitude)
- Index (as role vs operand)
- Address / `addr`
- compiler-facing policy geometry
- privileged index carrier
- selected target's address model
- occurrence-specific fit proof
- `address_of`
- referent / reference descriptor
- addressable-storage rules
- compatible views
- nonaddressable projections
- static machine argument / static machine binder
- carrier-qualified machine
- representation recast
- core name (vs reserved keyword)
- `size_of` / type-form `align_of`
- companion data queries
- executable-entry dependency
- specialization, retention, and realization pipeline
- observably equivalent optimizations
- proof-only entity
- declaration identity vs realization identity
- callback / binding / installation contracts (linked)
- semantic evaluation
- interpreter IDs / portable constants
- interpreted address identities
- kind bits / address kind
- compiler-local variable number / entry number
- Psi instructions
- interpreted assembly (linked)
- admitted storage mapping
- value-location-alignment-query
- value form `align_of`
- observed storage location
- backing view
- layout, placement and allocator contracts
- semantic constant
- `code_layout`
- SelectedMachine
- code regions
- final layout producer
- circular query
- data carrier (machines)
- `Extent` / embed / `addr::Bound`
- extent obligations (linked)
- live_length
- invocation contracts (linked)
- range windows (numeric values, linked)
- capacity vs live length
- `FixedVec` / domain qualification `in D`
- modular index value
- `no_wrap`
- exclusive address bound
- Wrapping, Saturating, or Trapping policy
- refinement (bounds)
- nominal index qualifications / room index / item index
- sealed route
- generational-handle abstraction
- accessor (reestablish bound)

CHAPTER COMPLETE
