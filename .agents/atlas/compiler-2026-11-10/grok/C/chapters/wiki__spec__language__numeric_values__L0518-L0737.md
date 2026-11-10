# Floating formats and operations + Collection bounds + Policy realization

Source: `wiki/spec/language/numeric_values.md` lines 518–737 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| NUMERIC_VALUES1 | "`f32` permanently means IEEE binary32 and `f64` IEEE binary64 on every target providing them." | Floating formats and operations | `f32` and `f64` must retain IEEE binary32 and binary64 semantics on every target that provides them | not stated | 5 |
| NUMERIC_VALUES2 | "A new format requires a distinct carrier identity, never a reinterpretation of an existing name." | Floating formats and operations | A new floating format must get a new carrier identity; existing names must not be reinterpreted as another format | not stated | 5 |
| NUMERIC_VALUES3 | "Both records include signed zeros, subnormals, infinities, NaNs, and ordinary round-to-nearest-ties-to-even." | Floating formats and operations | Core format records must include signed zeros, subnormals, infinities, NaNs, and round-to-nearest-ties-to-even | not stated | 5 |
| NUMERIC_VALUES4 | "Format records name the exponent of the leading radix digit for normal values and the least subnormal exponent." | Floating formats and operations | Format records must name normal leading-digit exponent and least subnormal exponent | not stated | 5 |
| NUMERIC_VALUES5 | "Finite nonzero values denote exact signed rationals; operations compute their exact meaning and round through the selected format, with defined special-value rules." | Floating formats and operations | Finite nonzero floats denote exact rationals; operations must compute exact meaning then round per format and special-value rules | at the operation | 5 |
| NUMERIC_VALUES6 | "Core `FloatSemantics` is the common executable definition for proof, semantic evaluation, folding, interpretation, software realization, and target validation." | Floating formats and operations | Proof, folding, interpretation, software, and target validation must use the same `FloatSemantics` definition | not stated | 5 |
| NUMERIC_VALUES7 | "Host `f64` arithmetic is not an alternative definition for `f32`." | Floating formats and operations | `f32` checking and semantics must not be defined via host `f64` arithmetic | not stated | 5 |
| NUMERIC_VALUES8 | "Primitive spellings use ordinary operand-driven resolution over the complete static carrier/domain tuple, selecting the exact requirement and the selected target's declared realization." | Floating formats and operations | Float primitive spellings must resolve operands to the full static carrier/domain tuple and select the exact requirement and target realization | at the operation | 5 |
| NUMERIC_VALUES9 | "There is no float-only resolver, runtime operation tag, `FloatOperations<F>` bundle, dynamic dispatch table, or ambient mode." | Floating formats and operations | Float operations must not be routed through a float-only resolver, runtime tags, `FloatOperations<F>`, dynamic dispatch, or ambient mode | not stated | 5 |
| NUMERIC_VALUES10 | "Arithmetic, comparison, classification, conversion, directed rounding, and fused/unfused operations have separate normalized requirement identities." | Floating formats and operations | Each float operation class must have its own normalized requirement identity | at the operation | 5 |
| NUMERIC_VALUES11 | "The compiler never contracts the former into the latter." | Floating formats and operations | Must not rewrite unfused multiply-then-add into fused multiply-add | at the operation | 5 |
| NUMERIC_VALUES12 | "Ordinary operations round nearest-even; directed modes are separately named requirements." | Floating formats and operations | Ordinary float operations use nearest-even; directed rounding requires separately named requirements | at the operation | 5 |
| NUMERIC_VALUES13 | "There is no ambient fast-math flag or build option." | Floating formats and operations | No ambient fast-math flag or build option may alter float semantics | not stated | 5 |
| NUMERIC_VALUES14 | "A future relaxed operation would need an explicit per-operation contract;" | Floating formats and operations | Any relaxed float operation must carry an explicit per-operation contract before admission | not stated | 5 |
| NUMERIC_VALUES15 | "an either-rounding `muladd` remains deferred without a customer." | Floating formats and operations | Either-rounding `muladd` must not be admitted without an explicit customer requirement | not stated | 5 |
| NUMERIC_VALUES16 | "Hardware or checked software must implement the same selected contract." | Floating formats and operations | Hardware and checked software realizations must implement the same selected float contract | not stated | 9 |
| NUMERIC_VALUES17 | "Selecting an optimization in `build.omg` cannot weaken the arithmetic facts used by checking;" | Floating formats and operations | Build optimization selection must not weaken arithmetic facts used during checking | not stated | 9 |
| NUMERIC_VALUES18 | "relaxed arithmetic would need a separately accepted contract visible to checking as well as realization." | Floating formats and operations | Relaxed arithmetic requires a contract visible to both checking and realization | not stated | 5 |
| NUMERIC_VALUES19 | "Formats are semantic data, independent of their hardware/software realization." | Floating formats and operations | Format semantics must be checked independently of particular hardware or software realization | not stated | 5 |
| NUMERIC_VALUES20 | "These are result guarantees, not a requirement to execute operations through runtime `Rat` values." | Floating formats and operations | Correct float results must not be interpreted as requiring runtime `Rat` execution paths | not stated | 5 |
| NUMERIC_VALUES21 | "`Float::meaning32` and `Float::meaning64` project to proof-only `FloatMeaning`: finite nonzero `Rat::NonZero`, signed zero, signed infinity, or payloadless NaN." | Meaning, comparisons, and representation | Meaning projections must map to the listed `FloatMeaning` cases only | not stated | 8 |
| NUMERIC_VALUES22 | "Subnormals are ordinary nonzero rationals." | Meaning, comparisons, and representation | Subnormals in proof meaning must be treated as ordinary nonzero rationals | not stated | 8 |
| NUMERIC_VALUES23 | "Rational zero is excluded from the finite case; infinities/NaNs have no rational embedding." | Meaning, comparisons, and representation | Finite proof meaning must exclude rational zero; infinities and NaNs must have no rational embedding | not stated | 8 |
| NUMERIC_VALUES24 | "The sum has no runtime ABI." | Meaning, comparisons, and representation | `FloatMeaning` sum must not be given a runtime ABI | not stated | 8 |
| NUMERIC_VALUES25 | "[FloatMeaning identity and equality](../terminal-psi/mathematical_values.md#floatmeaning) are structural proof equality, distinct from IEEE runtime comparison." | Meaning, comparisons, and representation | Proof equality on `FloatMeaning` must be structural and must not be confused with IEEE runtime comparison | not stated | 8 |
| NUMERIC_VALUES26 | "Under IEEE comparison, ordered relations are false on NaN, `==` is false, and `!=` is true." | Meaning, comparisons, and representation | IEEE ordered comparisons and equality must follow NaN rules as stated | at the operation | 5 |
| NUMERIC_VALUES27 | "Signed zeros compare equal." | Meaning, comparisons, and representation | IEEE comparison must treat signed zeros as equal | at the operation | 5 |
| NUMERIC_VALUES28 | "Integer order complements and unconditional self-comparison folding therefore do not apply." | Meaning, comparisons, and representation | Integer order-complement and unconditional self-comparison folding must not apply to floats | not stated | 5 |
| NUMERIC_VALUES29 | "`is_finite` is the portable wellness query;" | Meaning, comparisons, and representation | Finiteness wellness must use `is_finite`, not ad hoc NaN tests alone | not stated | 5 |
| NUMERIC_VALUES30 | "`x != x` is an IEEE-specific NaN test." | Meaning, comparisons, and representation | `x != x` NaN testing is IEEE-specific and must not be treated as the portable wellness query | not stated | 5 |
| NUMERIC_VALUES31 | "`minimum(a, b)` returns `a` only when `a < b`;" | Meaning, comparisons, and representation | `minimum` must return the first operand only when `a < b` under IEEE comparison | at the operation | 5 |
| NUMERIC_VALUES32 | "`maximum(a, b)` returns `a` only when `a > b`." | Meaning, comparisons, and representation | `maximum` must return the first operand only when `a > b` under IEEE comparison | at the operation | 5 |
| NUMERIC_VALUES33 | "Both return the second operand on unordered or equal inputs, including equal signed zeros." | Meaning, comparisons, and representation | `minimum` and `maximum` must return the second operand on unordered or equal inputs including equal signed zeros | at the operation | 5 |
| NUMERIC_VALUES34 | "NaN behavior is operand-order-dependent, not the IEEE-2019 NaN-propagating `minimum` or a non-NaN-wins rule." | Meaning, comparisons, and representation | `minimum`/`maximum` NaN behavior must follow operand-order rule, not IEEE-2019 NaN propagation or NaN-wins | at the operation | 5 |
| NUMERIC_VALUES35 | "Sorting/keying selects the named `F32::TotalOrder` or `F64::TotalOrder` in `omega::language::core::float_order`." | Meaning, comparisons, and representation | Sorting and keying must select the named total-order requirements | at the operation | 5 |
| NUMERIC_VALUES36 | "These implement IEEE totalOrder over honest representation bits, including NaN signs/payload ordering and `-0.0 < +0.0`, without changing arithmetic comparison." | Meaning, comparisons, and representation | Total order must implement IEEE totalOrder on representation bits without altering arithmetic comparison | at the operation | 5 |
| NUMERIC_VALUES37 | "NaN payloads are absent only from proof meaning, not from runtime storage." | Meaning, comparisons, and representation | Proof meaning must be payloadless NaN; runtime may retain payloads | not stated | 8 |
| NUMERIC_VALUES38 | "A runtime [recast](../layouts/recasts.md) reads actual bits but promises no cross-build/target identity." | Meaning, comparisons, and representation | Runtime recast of float bits must not promise cross-build or cross-target identity | at the operation | 6 |
| NUMERIC_VALUES39 | "A compile-time raw-bit observation of a computed possibly-NaN value requires non-NaN proof, canonicalization, or a selected exact NaN-bit refinement." | Meaning, comparisons, and representation | Compile-time raw-bit observation of possibly-NaN values requires non-NaN proof, canonicalization, or exact NaN-bit refinement | not stated | 8 |
| NUMERIC_VALUES40 | "[Constant materialization](constants.md#materialization) enforces this separately from proof-only use." | Meaning, comparisons, and representation | Constant materialization must enforce raw-bit rules separately from proof-only use | not stated | 1 |
| NUMERIC_VALUES41 | "Value qualifications such as `Finite` and float ranges are invariant-window facts, not runtime metadata or arithmetic modes." | Value qualifications and policy adapters | Float qualifications and ranges must be invariant-window facts, not runtime metadata or modes | not stated | 8 |
| NUMERIC_VALUES42 | "`Finite` excludes NaN and both infinities;" | Value qualifications and policy adapters | `Finite` must exclude NaN and both infinities | not stated | 8 |
| NUMERIC_VALUES43 | "a finite float range implies it." | Value qualifications and policy adapters | A finite float range qualification must imply `Finite` | not stated | 8 |
| NUMERIC_VALUES44 | "Value facts may compose with one policy;" | Value qualifications and policy adapters | A float value fact may compose with at most one policy | not stated | 5 |
| NUMERIC_VALUES45 | "range windows are not restricted to the quiet default." | Value qualifications and policy adapters | Range windows may combine with policies other than the quiet default | not stated | 5 |
| NUMERIC_VALUES46 | "Unqualified | Quiet format semantics, including non-finite results." | Value qualifications and policy adapters | Unqualified float policy must permit quiet format semantics including non-finite results | at the operation | 5 |
| NUMERIC_VALUES47 | "Trapping | Trap when the semantic result is NaN or infinity, including propagation from an operand." | Value qualifications and policy adapters | Trapping policy must trap on semantic NaN or infinity results including operand propagation | at the operation | 5 |
| NUMERIC_VALUES48 | "Saturating | Clamp finite-operand magnitude overflow to the signed largest finite value; do not repair invalid operations or division by signed zero." | Value qualifications and policy adapters | Saturating policy must clamp finite-operand magnitude overflow to largest finite; must not repair invalid ops or division by signed zero | at the operation | 5 |
| NUMERIC_VALUES49 | "Wrapping | No candidate; floats have no modular overflow meaning." | Value qualifications and policy adapters | Float wrapping policy has no candidate; modular overflow must not be admitted for floats | not stated | 5 |
| NUMERIC_VALUES50 | "Finite operands alone do not prove finite division: both signed zero divisors must be excluded and the rounded quotient must not overflow." | Value qualifications and policy adapters | Finite division proof must exclude both signed zero divisors and require rounded quotient not overflow finite range | not stated | 8 |
| NUMERIC_VALUES51 | "Underflow to signed zero is finite." | Value qualifications and policy adapters | Underflow to signed zero must count as finite for qualification purposes | not stated | 5 |
| NUMERIC_VALUES52 | "`Finite & Saturating` removes magnitude-overflow work, not the nonzero-divisor obligation." | Value qualifications and policy adapters | `Finite & Saturating` must not remove nonzero-divisor proof obligations | not stated | 8 |
| NUMERIC_VALUES53 | "`0/0`, infinity subtraction, and negative square root retain their non-finite behavior under saturation." | Value qualifications and policy adapters | Listed invalid operations must remain non-finite under saturating policy | at the operation | 5 |
| NUMERIC_VALUES54 | "Trapping is a result adapter, not unmasked hardware floating exceptions;" | Value qualifications and policy adapters | Trapping float policy must be implemented as result adapter, not as unmasked hardware exceptions | not stated | 10 |
| NUMERIC_VALUES55 | "a fused adapter needs proof of the same contract." | Value qualifications and policy adapters | A fused trapping adapter must prove the same trapping contract as non-fused | not stated | 8 |
| NUMERIC_VALUES56 | "Named requirements belong to the destination: for example `F32::from_f64`, `F32::from_i64`, and `I32::from_f64`." | Destination-owned conversion | Conversion named requirements must belong to the destination type | at a call | 5 |
| NUMERIC_VALUES57 | "Generic clients name the exact machine requirement, not a universal `Convert<From, To>` bundle." | Destination-owned conversion | Generic conversion clients must name exact machine requirements, not a universal convert bundle | at a call | 7 |
| NUMERIC_VALUES58 | "Float-format and integer-to-float conversions normally round nearest-even." | Destination-owned conversion | Float-format and integer-to-float conversions must normally round nearest-even | at the operation | 5 |
| NUMERIC_VALUES59 | "Float-to-integer conversion rounds toward zero;" | Destination-owned conversion | Float-to-integer conversion must round toward zero | at the operation | 5 |
| NUMERIC_VALUES60 | "a fractional part is lossy, not itself failure." | Destination-owned conversion | Loss of fractional part in float-to-integer must not be treated as conversion failure | at the operation | 5 |
| NUMERIC_VALUES61 | "Its unqualified result requires finite input and a representable truncated value." | Destination-owned conversion | Unqualified float-to-integer requires finite input and representable truncated value | at the operation | 5 |
| NUMERIC_VALUES62 | "`Trapping` traps on failed conversion;" | Destination-owned conversion | Trapping float-to-integer must trap on failed conversion | at the operation | 5 |
| NUMERIC_VALUES63 | "`Saturating` clamps to integer bounds and maps NaN to zero." | Destination-owned conversion | Saturating float-to-integer must clamp to bounds and map NaN to zero | at the operation | 5 |
| NUMERIC_VALUES64 | "`Wrapping` has no float-to-integer candidate." | Destination-owned conversion | Wrapping policy must not be offered for float-to-integer conversion | not stated | 5 |
| NUMERIC_VALUES65 | "The returned value keeps its policy domain until a cast such as `as -Trapping` removes it;" | Destination-owned conversion | Converted values retain policy domain until an explicit cast removes it | not stated | 5 |
| NUMERIC_VALUES66 | "later arithmetic on the bare carrier is Exact." | Destination-owned conversion | Arithmetic on bare carrier after policy-removing cast is Exact policy | at the operation | 5 |
| NUMERIC_VALUES67 | "Selection follows the general [result-domain overload rule](domains.md#result-domain-overloads)." | Destination-owned conversion | Conversion overload selection must follow the result-domain overload rule | not stated | 5 |
| NUMERIC_VALUES68 | "Predicate-only facts do not dispatch; they remain obligations after selecting the exact result-domain projection." | Destination-owned conversion | Predicate-only facts must not dispatch overloads; they remain obligations after result-domain selection | not stated | 8 |
| NUMERIC_VALUES69 | "Operator syntax remains operand-directed." | Destination-owned conversion | Conversion operator syntax must stay operand-directed | at the operation | 0 |
| NUMERIC_VALUES70 | "Without an expected result type, the unqualified overload is selected;" | Destination-owned conversion | Missing expected result type must select the unqualified overload | not stated | 5 |
| NUMERIC_VALUES71 | "a failed finite/range proof diagnostic lists the available qualified overloads." | Destination-owned conversion | Failed finite or range proof diagnostics must list available qualified overloads | not stated | 5 |
| NUMERIC_VALUES72 | "Directed one-step conversions are separately named and cannot consult an ambient rounding mode." | Destination-owned conversion | Directed one-step conversions must be separately named and must not use ambient rounding mode | at the operation | 5 |
| NUMERIC_VALUES73 | "A wrapper composing rounding and conversion must prove equivalent meaning;" | Destination-owned conversion | Wrappers combining rounding and conversion must prove equivalent meaning to one-step conversion | not stated | 8 |
| NUMERIC_VALUES74 | "it cannot substitute two roundings for a required one-step conversion." | Destination-owned conversion | Must reject two-rounding substitutes for required one-step conversion | not stated | 5 |
| NUMERIC_VALUES75 | "Bare format conversion is total, including infinity." | Destination-owned conversion | Bare format conversion must be total including infinity | at the operation | 5 |
| NUMERIC_VALUES76 | "Refinements such as `Finite` are later predicates, not extra conversion variants." | Destination-owned conversion | `Finite` and similar refinements are predicates after conversion, not separate conversion variants | not stated | 8 |
| NUMERIC_VALUES77 | "Same-format qualification belongs to `as`." | Destination-owned conversion | Same-format qualification must use `as`, not conversion variants | not stated | 5 |
| NUMERIC_VALUES78 | "Failure-returning conversions change result shape; their public name/carrier remain unsettled with checked-result arithmetic." | Destination-owned conversion | Failure-returning conversions change result shape; public name/carrier for checked-result arithmetic remain unsettled and must not be assumed admitted | not stated | 5 |
| NUMERIC_VALUES79 | "Every checked activation uses canonical floating controls: nearest-even, gradual underflow, and masked exceptions." | Target realization and control state | Every checked activation must use nearest-even, gradual underflow, and masked exceptions | at entry | 10 |
| NUMERIC_VALUES80 | "On x86 this excludes FTZ/DAZ;" | Target realization and control state | x86 realization must exclude flush-to-zero and denormals-are-zero for checked activation | at entry | 10 |
| NUMERIC_VALUES81 | "AArch64 has corresponding FPCR requirements." | Target realization and control state | AArch64 must meet corresponding FPCR requirements for canonical controls | at entry | 10 |
| NUMERIC_VALUES82 | "Sticky status flags are not part of that semantic invariant." | Target realization and control state | Sticky status flags must not be treated as part of the semantic floating-control invariant | not stated | 10 |
| NUMERIC_VALUES83 | "Directed operations may internally change controls only with restoration;" | Target realization and control state | Directed operations may change controls only if restored afterward | at the operation | 10 |
| NUMERIC_VALUES84 | "they never introduce a source-visible ambient mode." | Target realization and control state | Directed operations must not introduce source-visible ambient floating mode | not stated | 10 |
| NUMERIC_VALUES85 | "Schedulers need not switch semantic modes between Omega activations." | Target realization and control state | Schedulers are not required to switch semantic float modes between activations | not stated | 10 |
| NUMERIC_VALUES86 | "Foreign bindings prove preservation or save/restore relevant controls." | Target realization and control state | Foreign bindings must prove control preservation or save/restore | not stated | 9 |
| NUMERIC_VALUES87 | "Callback entry establishes Omega controls and exit restores foreign controls." | Target realization and control state | Callback entry must establish Omega controls; exit must restore foreign controls | at entry; at normal return | 9 |
| NUMERIC_VALUES88 | "A stated calling-policy predicate alone does not establish execution of those steps." | Target realization and control state | Calling-policy predicates alone must not satisfy control save/restore without proof of execution | not stated | 8 |
| NUMERIC_VALUES89 | "canonical `f32` and `f64` operations keep their declaration identities and `FloatSemantics` contracts and are realized by the selected target's declared realization table, not by Build provider selection." | Target realization and control state | Canonical float ops keep declaration identities and `FloatSemantics`; realization comes from target table, not Build provider selection | not stated | 9 |
| NUMERIC_VALUES90 | "For each canonical operation a target package declares one entry: a catalog intrinsic, a checked Omega machine satisfying the operation's contract, or unsupported." | Target realization and control state | Each canonical float op must have exactly one target-table entry: intrinsic, checked machine, or unsupported | not stated | 9 |
| NUMERIC_VALUES91 | "The realization plan retains the chosen entry with its ISA-feature admission and floating-control obligations." | Target realization and control state | Realization plan must retain chosen entry with ISA-feature admission and floating-control obligations | not stated | 9 |
| NUMERIC_VALUES92 | "A program using an unsupported operation on its target rejects, naming the operation and the target constraint." | Target realization and control state | Unsupported canonical float op on target must reject with operation and target constraint named | not stated | 9 |
| NUMERIC_VALUES93 | "Build programs neither select nor override canonical float realizations;" | Target realization and control state | Build programs must not select or override canonical float realizations | not stated | 9 |
| NUMERIC_VALUES94 | "a program needing a particular algorithm calls a named machine." | Target realization and control state | Non-canonical float algorithms must be invoked only via named machines | at a call | 9 |
| NUMERIC_VALUES95 | "Format records do not teach machine encodings;" | Target realization and control state | Format records must not encode machine instruction encodings | not stated | 9 |
| NUMERIC_VALUES96 | "new instructions need the checked instruction catalog." | Target realization and control state | New float instructions must be admitted through the checked instruction catalog | not stated | 9 |
| NUMERIC_VALUES97 | "Hardware meaning remains admitted vendor evidence, while checked software can be derived against the same executable contract." | Target realization and control state | Hardware float meaning requires vendor evidence; checked software must derive against the same executable contract | not stated | 9 |
| NUMERIC_VALUES98 | "Differential evidence retains selected identities and covers normal/subnormal boundaries, ties, overflow/underflow, signed zeros, infinities, NaNs, and policy edges." | Target realization and control state | Differential evidence must retain selected identities and cover listed float edge cases | not stated | 9 |
| NUMERIC_VALUES99 | "Exact NaN bit comparisons are required only for claimed raw-bit refinements." | Target realization and control state | Exact NaN bit comparison obligations apply only to claimed raw-bit refinements | not stated | 8 |
| NUMERIC_VALUES100 | "Compilation or image reproducibility is not hardware execution evidence." | Target realization and control state | Compilation or image reproducibility must not substitute for hardware execution evidence | not stated | 9 |
| NUMERIC_VALUES101 | "Optional ISA features, including x86 AVX+FMA3, are admitted by the target's realization table and retained in the realization plan." | Target realization and control state | Optional ISA features including AVX+FMA3 must be admitted via target realization table and retained in plan | not stated | 9 |
| NUMERIC_VALUES102 | "FMA uses the ordinary operation and instruction pipeline." | Target realization and control state | FMA must use the ordinary operation and instruction pipeline, not a separate custody route | not stated | 9 |
| NUMERIC_VALUES103 | "Native images require no separate FMA fragment, occurrence, digest or provider-custody record, and emission does not decode or replay those records." | Target realization and control state | Native images must not require separate FMA fragment, occurrence, digest, or provider-custody records | not stated | 9 |
| NUMERIC_VALUES104 | "The superseded constant-operand FMA custody route is not an alternative lowering path." | Target realization and control state | Superseded constant-operand FMA custody route must not be used as lowering | not stated | 9 |
| NUMERIC_VALUES105 | "Instruction encoding and execution tests still check the selected realization, including its feature admission and floating controls;" | Target realization and control state | Encoding and execution tests must check selected realization, feature admission, and floating controls | not stated | 9 |
| NUMERIC_VALUES106 | "this ruling removes no arithmetic guarantee." | Target realization and control state | FMA pipeline ruling must not remove any arithmetic guarantee | not stated | 5 |
| NUMERIC_VALUES107 | "It does not introduce a loader-facing ISA-feature manifest." | Target realization and control state | Must not introduce loader-facing ISA-feature manifest | not stated | 9 |
| NUMERIC_VALUES108 | "A library can define an encoded float carrier and checked software operations without new primitives." | Target realization and control state | Libraries may define encoded float carriers and software ops without new language primitives | not stated | 5 |
| NUMERIC_VALUES109 | "First-class literal/evaluation integration requires an admitted format record;" | Target realization and control state | First-class literal/evaluation for new float formats requires admitted format record | not stated | 5 |
| NUMERIC_VALUES110 | "fixed-precision radix-2 records do not already describe tapered precision." | Target realization and control state | Fixed-precision radix-2 format records must not be assumed to describe tapered precision | not stated | 5 |
| NUMERIC_VALUES111 | "Hardware integration additionally needs selected supported lowerings." | Target realization and control state | Hardware integration of new float formats requires selected supported lowerings | not stated | 9 |
| NUMERIC_VALUES112 | "Posit carriers and their extended format vocabulary remain future work, not permission to rebind `f32`." | Target realization and control state | Posit work must not rebind `f32` | not stated | 5 |
| NUMERIC_VALUES113 | "Scalar indexing and range windows have separate bounds obligations." | Collection bounds | Scalar indexing and range windows must carry separate bounds obligations | at the operation | 6 |
| NUMERIC_VALUES114 | "Each nested array indexing occurrence uses the selected element type's own extent." | Collection bounds | Nested array indexing must use the selected element type's extent | at the operation | 6 |
| NUMERIC_VALUES115 | "An unsupported collection shape supplies no proof." | Collection bounds | Unsupported collection shapes must supply no bounds proof | not stated | 6 |
| NUMERIC_VALUES116 | "Collection-relative upper bounds or endpoint ordering alone do not establish that signed operands are nonnegative." | Collection bounds | Upper bounds or endpoint order alone must not prove signed operands nonnegative | not stated | 8 |
| NUMERIC_VALUES117 | "Unknown-length scalar accesses and all range windows also require lower-bound evidence from enforced types, constants, or live facts." | Collection bounds | Unknown-length scalar access and all range windows require lower-bound evidence from types, constants, or live facts | at the operation | 6 |
| NUMERIC_VALUES118 | "A nonnegative start and endpoint order may establish a nonnegative end." | Collection bounds | Nonnegative start plus endpoint order may establish nonnegative end | not stated | 8 |
| NUMERIC_VALUES119 | "Omitted endpoints retain zero/length defaults." | Collection bounds | Omitted range endpoints must default to zero and length | not stated | 6 |
| NUMERIC_VALUES120 | "All value-dependent bounds obey [live-fact invalidation](state_contracts.md#mutation-and-subject-identity)." | Collection bounds | Value-dependent bounds must obey live-fact invalidation on mutation | at the operation | 8 |
| NUMERIC_VALUES121 | "Indexed operator syntax may adapt a fixed-array or slice collection shell to a shared slice parameter." | Collection bounds | Indexed operator syntax may adapt fixed-array or slice shell to shared slice parameter | at the operation | 6 |
| NUMERIC_VALUES122 | "Element bindings, other operands, domains, ambiguity, and additional requirements remain exact;" | Collection bounds | After shell adaptation, element bindings, operands, domains, ambiguity, and requirements stay exact | at the operation | 6 |
| NUMERIC_VALUES123 | "named calls do not gain this conversion." | Collection bounds | Named calls must not gain indexed-operator shell-to-slice adaptation | at a call | 6 |
| NUMERIC_VALUES124 | "Bounds must follow that selected operator's actual contract, not another declaration sharing its token." | Collection bounds | Bounds proof must follow the selected operator's contract, not a homonymous declaration | at the operation | 6 |
| NUMERIC_VALUES125 | "Bounds evidence grants no element-domain, borrow, mutation, or transfer authority." | Collection bounds | Bounds evidence alone must not grant element-domain, borrow, mutation, or transfer authority | not stated | 2 |
| NUMERIC_VALUES126 | "The [core indexing contract](expressions.md#contract-gated-core-operations) requires ordinary call-contract discharge for every selected requirement." | Collection bounds | Core indexing requires ordinary call-contract discharge for each selected requirement | at a call | 3 |
| NUMERIC_VALUES127 | "Bounds strategies establish reusable propositions over live operand identities;" | Collection bounds | Bounds strategies establish reusable propositions over live operand identities | not stated | 8 |
| NUMERIC_VALUES128 | "they do not admit indexing independently of its declaration." | Collection bounds | Bounds strategies must not admit indexing without the indexing declaration's contract | not stated | 6 |
| NUMERIC_VALUES129 | "Fixed and generic extents, saved selectors, assignment-length facts and normalized range endpoints use this same proof interface." | Collection bounds | Listed bounds evidence kinds must use the same proof interface as core indexing | not stated | 6 |
| NUMERIC_VALUES130 | "Successful discharge retains the evidence for each emitted access occurrence, separately from its loan and access authority." | Collection bounds | Successful bounds discharge must retain evidence per emitted access, separate from loan and access authority | at the operation | 8 |
| NUMERIC_VALUES131 | "Integer `as` coercions preserve denotation under every target policy and require representability." | Policy realization | Integer `as` coercions must preserve denotation under every target policy and require representability | at the operation | 5 |
| NUMERIC_VALUES132 | "The named machines of `core/numeric_conversion.omg` provide explicit wrapping, saturating, and trapping conversions across signed and unsigned fixed carriers." | Policy realization | Wrapping, saturating, and trapping integer conversions must use named machines from `core/numeric_conversion.omg` | at a call | 5 |
| NUMERIC_VALUES133 | "Boolean-to-integer conversion and Trapping arithmetic and shifts lower through Terminal Psi." | Policy realization | Boolean-to-integer and Trapping arithmetic and shifts must lower through Terminal Psi | not stated | 9 |
| NUMERIC_VALUES134 | "Trapping integer operations retain their operation-level `Trap` sites;" | Policy realization | Trapping integer operations must retain operation-level `Trap` sites | at the operation | 5 |
| NUMERIC_VALUES135 | "native realization checks each predicate in place and stops through the same architectural trap as a `Crash` terminator on x86-64 and AArch64 at every fixed 8/16/32/64-bit carrier." | Policy realization | Native realization must check trapping predicates in place and trap like `Crash` on x86-64 and AArch64 for fixed-width carriers | at the operation | 10 |
| NUMERIC_VALUES136 | "Trapping float result adapters are checked against the published `Trap` ceiling at their exact operation site, including nonentry states." | Policy realization | Trapping float adapters must be checked against published `Trap` ceiling at exact operation site including nonentry states | at the operation | 5 |
| NUMERIC_VALUES137 | "Raw-result finiteness proofs retain the selected operation and original operands." | Policy realization | Raw-result finiteness proofs must retain selected operation and original operands | not stated | 8 |

## Ambiguous sentences

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "A future relaxed operation would need an explicit per-operation contract;" | No relaxed float operation may be admitted until a per-operation contract is ratified and wired to checking. | The sentence states intent for hypothetical future work only and imposes no current rejection rule on compilers. |
| "These are result guarantees, not a requirement to execute operations through runtime `Rat` values." | Checkers and realizers may use any implementation path as long as results match; `Rat` execution is never mandatory. | Some phases (e.g. proof or folding) might still require `Rat` paths even though runtime execution need not use them; "these" might refer only to hardware/software realization. |
| "Value facts may compose with one policy;" | Each qualified float value has at most one policy adapter at a time. | Multiple policies might stack if they are classified as value facts rather than policies; "one policy" might mean one adapter layer per composition site. |
| "range windows are not restricted to the quiet default." | Range-qualified floats may pair with Trapping or Saturating, not only Unqualified. | Range windows may exist without any policy qualifier; "quiet default" might refer only to default overload selection, not policy pairing. |
| "Bare format conversion is total, including infinity." | Format conversions never fail for non-finite inputs at the conversion operation itself. | Totality applies only to bare format-to-format conversion before later `Finite` predicates; infinity might still fail later predicates without failing the conversion call. |
| "Failure-returning conversions change result shape; their public name/carrier remain unsettled with checked-result arithmetic." | Compilers must reject or treat as non-public any failure-returning conversion pending a settled name/carrier. | Ordinary programs may use failure-returning conversions experimentally; "unsettled" is documentation status, not a hard gate today. |
| "Schedulers need not switch semantic modes between Omega activations." | Implementations never have to change float mode between activations. | Schedulers are allowed but not required to switch; another subsystem might still switch modes for directed operations with restoration. |
| "A stated calling-policy predicate alone does not establish execution of those steps." | Foreign-binding proofs must show actual save/restore or preservation, not merely state a policy predicate. | The predicate is necessary but insufficient; whether additional evidence types suffice besides execution proof is not stated. |
| "Hardware meaning remains admitted vendor evidence, while checked software can be derived against the same executable contract." | Hardware float paths require vendor evidence admission before use; software paths require derivation only. | Vendor evidence and derivation are alternative admissions of equal standing once either is recorded. |
| "A library can define an encoded float carrier and checked software operations without new primitives." | Such library types compile without language primitive extension. | Library definitions are permitted in principle but might still fail checking unless other unstated admission rules pass. |
| "A nonnegative start and endpoint order may establish a nonnegative end." | Compiler may derive nonnegative end from nonnegative start plus ordered endpoints without extra facts. | Derivation applies only when additional unstated range-window or length facts also hold. |
| "Indexed operator syntax may adapt a fixed-array or slice collection shell to a shared slice parameter." | Adaptation is optional sugar that rewrites the call shape when operator syntax is used. | Adaptation is mandatory whenever indexed operator syntax is chosen and the shell is fixed-array or slice. |

## Terms used without definition in this chapter

- IEEE binary32 / IEEE binary64
- carrier identity
- hardware/software realization
- `FloatFormat::BINARY32` / `FloatFormat::BINARY64`
- normal exponent range / minimum subnormal exponent
- signed rationals
- special-value rules
- `FloatSemantics`
- semantic evaluation / folding / interpretation
- complete static carrier/domain tuple
- normalized requirement identity
- multiply_then_add / fused_multiply_add
- directed modes / directed rounding
- relaxed operation / either-rounding `muladd`
- runtime `Rat` values
- selected contract
- `Float::meaning32` / `Float::meaning64`
- `FloatMeaning` / `Rat::NonZero`
- structural proof equality
- integer order complements
- unconditional self-comparison folding
- wellness query
- `F32::TotalOrder` / `F64::TotalOrder`
- honest representation bits
- NaN-bit refinement
- invariant-window facts
- float ranges
- quiet format semantics
- magnitude-overflow work
- nonzero-divisor obligation
- result adapter / fused adapter
- `Convert<From, To>` bundle
- representable truncated value
- result-domain overload rule
- predicate-only facts
- expected result type
- one-step conversion
- ambient rounding mode
- bare format conversion
- checked-result arithmetic
- checked activation
- gradual underflow / masked exceptions
- FTZ / DAZ / FPCR
- sticky status flags
- foreign bindings / callback entry
- calling-policy predicate
- realization table / realization plan
- catalog intrinsic
- ISA-feature admission
- differential evidence
- raw-bit refinements
- AVX+FMA3
- provider-custody record
- tapered precision
- posit carriers
- range windows
- live facts / live-fact invalidation
- collection shell / shared slice parameter
- bounds strategies
- saved selectors / assignment-length facts
- normalized range endpoints
- denotation
- `core/numeric_conversion.omg`
- `Trap` ceiling / `Trap` sites
- `Crash` terminator
- raw-result finiteness proofs

CHAPTER COMPLETE
