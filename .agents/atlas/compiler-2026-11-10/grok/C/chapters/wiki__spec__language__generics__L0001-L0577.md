# Generics (wiki/spec/language/generics.md L0001–L0577)

Source: `wiki/spec/language/generics.md` lines 1–577 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---

GENERICS1 | "Data declarations additionally admit `some` binders: their witness is chosen per value and omitted from the external type application." | Generic declarations and static machine parameters | Data declarations additionally admit some binders: their witness is chosen per value and omitted ... | not stated | 7
GENERICS2 | "This is not a machine generic binder and does not replace `machine F`." | Generic declarations and static machine parameters | This is not a machine generic binder and does not replace machine F | not stated | 7
GENERICS3 | "Bodies are checked against their declared requirements; a concrete selection cannot silently strengthen the generic API." | Generic declarations and static machine parameters | Bodies are checked against their declared requirements; a concrete selection cannot silently stre... | not stated | 7
GENERICS4 | "Runtime indices follow dependent-value rules, not const-generic substitution." | Generic declarations and static machine parameters | Runtime indices follow dependent-value rules, not const-generic substitution | not stated | 7
GENERICS5 | "Lifetime binders follow lifetimes." | Generic declarations and static machine parameters | Lifetime binders follow lifetimes | not stated | 7
GENERICS6 | "Requirement spelling follows unified requirement clauses; the spelling does not merge static admission with runtime entry obligations or flatten nested callable contracts." | Generic declarations and static machine parameters | Requirement spelling follows unified requirement clauses; the spelling does not merge static admi... | not stated | 3
GENERICS7 | "An explicit `requires machine Step(...)` or `requires machine Step satisfies Trait::requirement` clause can determine the kind of Step introduced without a kind marker in the generic list." | Generic declarations and static machine parameters | An explicit requires machine Step(...) or requires machine Step satisfies Trait::requirement clau... | not stated | 7
GENERICS8 | "The clause declares an originating contract, not a contract guessed from an invocation's shape or the current consumer set." | Generic declarations and static machine parameters | The clause declares an originating contract, not a contract guessed from an invocation's shape or... | not stated | 7
GENERICS9 | "Missing or conflicting kinds/contracts reject." | Generic declarations and static machine parameters | Missing or conflicting kinds/contracts reject | not stated | 7
GENERICS10 | "This applies to members of ordinary and boundary traits and to static callable requirements; declaring a requirement does not remove a binder kind that its checked implementation may use." | Generic machine requirements | This applies to members of ordinary and boundary traits and to static callable requirements; decl... | not stated | 3
GENERICS11 | "Binder names need not match; identities and substitution must." | Generic machine requirements | Binder names need not match; identities and substitution must | not stated | 7
GENERICS12 | "Fixing an arbitrary concrete implementation inside the body cannot implement a requirement universally quantified over `Order`." | Generic machine requirements | Fixing an arbitrary concrete implementation inside the body cannot implement a requirement univer... | not stated | 7
GENERICS13 | "It does not choose or fill `Order`, enumerate all possible generic arguments, or strengthen the caller's requirements." | Generic machine requirements | It does not choose or fill Order, enumerate all possible generic arguments, or strengthen the cal... | not stated | 7
GENERICS14 | "Ordinary specialization retains the caller's exact static selections; runtime-capable value binders retain their ordinary value-argument meaning." | Generic machine requirements | Ordinary specialization retains the caller's exact static selections; runtime-capable value binde... | not stated | 7
GENERICS15 | "Executable applications follow ordinary demand and realization rather than a new runtime generic engine." | Generic machine requirements | Executable applications follow ordinary demand and realization rather than a new runtime generic ... | not stated | 7
GENERICS16 | "Static generic eligibility does not confer unrestricted runtime polymorphism." | Generic machine requirements | Static generic eligibility does not confer unrestricted runtime polymorphism | not stated | 7
GENERICS17 | "Machine values require closed callable signatures; bind ordinary type, machine and evidence arguments before constructing them." | Generic machine requirements | Machine values require closed callable signatures; bind ordinary type, machine and evidence argum... | at the operation | 3
GENERICS18 | "Explicit finite value families retain their declared tuple contracts under callable dispatch." | Generic machine requirements | Explicit finite value families retain their declared tuple contracts under callable dispatch | not stated | 3
GENERICS19 | "No global roster of helper instantiations or hidden types is needed for the package's uniform borrowed representation." | Generic machine requirements | No global roster of helper instantiations or hidden types is needed for the package's uniform bor... | not stated | 2
GENERICS20 | "An `asm machine` uses these same binders and machine-satisfaction rules; its body language does not require an asm-specific generic or iterator facility." | Applications and specialization | An asm machine uses these same binders and machine-satisfaction rules; its body language does not... | not stated | 10
GENERICS21 | "Every admitted specialization must justify its exact instruction widths, layouts, copying rights and target obligations." | Applications and specialization | Every admitted specialization must justify its exact instruction widths, layouts, copying rights ... | not stated | 8
GENERICS22 | "Selecting an assembly callback does not imply textual expansion, unrolling or inlining." | Applications and specialization | Selecting an assembly callback does not imply textual expansion, unrolling or inlining | not stated | 7
GENERICS23 | "Elaboration retains that context in the nested machine's application identity; flattening must not bind by spelling or merge distinct outer applications." | Applications and specialization | Elaboration retains that context in the nested machine's application identity; flattening must no... | not stated | 7
GENERICS24 | "This is not a runtime closure." | Applications and specialization | This is not a runtime closure | not stated | 7
GENERICS25 | "Runtime-value generic subjects, like ordinary runtime parameters, must be passed explicitly when a nested machine needs their values." | Applications and specialization | Runtime-value generic subjects, like ordinary runtime parameters, must be passed explicitly when ... | not stated | 7
GENERICS26 | "Lifetime context constrains explicit references and does not supply an enclosing activation or borrowed storage." | Applications and specialization | Lifetime context constrains explicit references and does not supply an enclosing activation or bo... | not stated | 2
GENERICS27 | "Every application validates the complete argument tuple, including kinds, ranges, and constraints of unused parameters." | Applications and specialization | Every application validates the complete argument tuple, including kinds, ranges, and constraints... | at a call | 7
GENERICS28 | "Conflicting, excessive, or incomplete selections reject." | Applications and specialization | Conflicting, excessive, or incomplete selections reject | not stated | 7
GENERICS29 | "Inference does not waive a binder's declared carrier or obligations." | Applications and specialization | Inference does not waive a binder's declared carrier or obligations | not stated | 8
GENERICS30 | "Named constants and forwarded binders must match that carrier before canonical identity erases their source names." | Applications and specialization | Named constants and forwarded binders must match that carrier before canonical identity erases th... | not stated | 5
GENERICS31 | "Authored local annotations remain independent store obligations; inferred call-result types follow the selected application's substituted result." | Applications and specialization | Authored local annotations remain independent store obligations; inferred call-result types follo... | not stated | 8
GENERICS32 | "A data declaration with `=` and a type expression is a transparent alias, not a new nominal data definition." | Transparent type aliases | Transparent alias is not a new nominal data definition | not stated | 7
GENERICS33 | "Buffer and the policy implementation above are selected ordinary declarations, not names that grant alias-specific compiler behavior." | Transparent type aliases | Buffer and the policy implementation above are selected ordinary declarations, not names that gra... | not stated | 7
GENERICS34 | "General aliases are not the domain-conjunction aliases described under domains." | Transparent type aliases | General aliases are not the domain-conjunction aliases described under domains | not stated | 5
GENERICS35 | "There is no wrapper field, constructor, conversion, additional nominal case owner or new ownership identity." | Transparent type aliases | There is no wrapper field, constructor, conversion, additional nominal case owner or new ownershi... | not stated | 2
GENERICS36 | "The alias cannot weaken requirements, hide an incompatible representation or turn a conversion into type equality." | Transparent type aliases | The alias cannot weaken requirements, hide an incompatible representation or turn a conversion in... | not stated | 7
GENERICS37 | "Nominal distinction requires an ordinary data declaration with a body." | Transparent type aliases | Nominal distinction requires an ordinary data declaration with a body | not stated | 7
GENERICS38 | "Runtime-capable indices retain exact runtime subjects and cannot be silently promoted to compile-time constants." | Transparent type aliases | Runtime-capable indices retain exact runtime subjects and cannot be silently promoted to compile-... | not stated | 7
GENERICS39 | "Alias expansion supplies no host grant or exception to evaluation admission." | Transparent type aliases | Alias expansion supplies no host grant or exception to evaluation admission | not stated | 7
GENERICS40 | "Name resolution, visibility, public-signature exposure and retained private implementation identities follow their existing rules." | Transparent type aliases | Name resolution, visibility, public-signature exposure and retained private implementation identi... | not stated | 7
GENERICS41 | "Exporting an alias does not grant permission to inspect or name private constituents." | Transparent type aliases | Exporting an alias does not grant permission to inspect or name private constituents | not stated | 7
GENERICS42 | "This facility does not introduce first-class higher-kinded arguments or ambient conformance search." | Transparent type aliases | This facility does not introduce first-class higher-kinded arguments or ambient conformance search | not stated | 7
GENERICS43 | "Target-semantic observations may be arguments and retain their exact target dependencies." | Closed applications and canonical identity | Target-semantic observations may be arguments and retain their exact target dependencies | not stated | 10
GENERICS44 | "A local shadowing declaration does not rename another binder." | Closed applications and canonical identity | A local shadowing declaration does not rename another binder | not stated | 7
GENERICS45 | "Const values retain their declared width and arithmetic policy." | Closed applications and canonical identity | Const values retain their declared width and arithmetic policy | not stated | 5
GENERICS46 | "Static data applications retain their generic base and exact tuple; substituted fields determine layout, not rendered names or provenance encodings." | Closed applications and canonical identity | Static data applications retain their generic base and exact tuple; substituted fields determine ... | not stated | 7
GENERICS47 | "Runtime indices retain the same declaration identity plus exact value-subject bindings; their future observed values are not static specialization/cache keys." | Closed applications and canonical identity | Runtime indices retain the same declaration identity plus exact value-subject bindings; their fut... | not stated | 7
GENERICS48 | "Concrete runtime applications need valid concrete layout; proof-only applications do not acquire runtime layout merely by being closed." | Closed applications and canonical identity | Concrete runtime applications need valid concrete layout; proof-only applications do not acquire ... | not stated | 8
GENERICS49 | "Static dispatch and monomorphization remain the baseline for static applications." | Closed applications and canonical identity | Static dispatch and monomorphization remain the baseline for static applications | not stated | 7
GENERICS50 | "Code sharing or specialization must preserve checked application identities, contracts, ownership, and observable behavior." | Closed applications and canonical identity | Code sharing or specialization must preserve checked application identities, contracts, ownership... | not stated | 2
GENERICS51 | "`const` is a staging requirement, not an optimization hint." | Value binders and const requirements | const is a staging requirement, not an optimization hint | not stated | 7
GENERICS52 | "Omitting it does not forbid specialization when a value is known." | Value binders and const requirements | Omitting it does not forbid specialization when a value is known | not stated | 7
GENERICS53 | "It does not describe source-variable mutability or a permission to use a mutable variable's later contents as an earlier type index." | Value binders and const requirements | It does not describe source-variable mutability or a permission to use a mutable variable's later... | not stated | 7
GENERICS54 | "It does not compile a new machine for each input value." | Value binders and const requirements | It does not compile a new machine for each input value | not stated | 7
GENERICS55 | "Constructing an instance owes the coupling; the declaration does not establish an inhabitant when Limit is zero." | Value binders and const requirements | Constructing an instance owes the coupling; the declaration does not establish an inhabitant when... | at the operation | 7
GENERICS56 | "The index can remain proof-only when its runtime value is not needed by any operation." | Value binders and const requirements | The index can remain proof-only when its runtime value is not needed by any operation | not stated | 8
GENERICS57 | "Bodies must have a valid realization for their admitted arguments." | Value binders and const requirements | Bodies must have a valid realization for their admitted arguments | not stated | 7
GENERICS58 | "A static-only use of a dynamic binder rejects unless an explicit, checked dispatch route or other supported realization supplies the required static value." | Value binders and const requirements | A static-only use of a dynamic binder rejects unless an explicit, checked dispatch route or other... | not stated | 7
GENERICS59 | "The compiler does not silently strengthen a public binder to `const`, add a fallback body, or convert a runtime type choice into an unrelated statically selected type." | Value binders and const requirements | The compiler does not silently strengthen a public binder to const, add a fallback body, or conve... | not stated | 7
GENERICS60 | "Type equations and domain-application matching follow the rules below; knowing the type of a runtime argument does not make that argument's value static." | Runtime index identity and storage | Type equations and domain-application matching follow the rules below; knowing the type of a runt... | not stated | 5
GENERICS61 | "Later assignment to the source variable does not retag existing indexed values." | Runtime index identity and storage | Later assignment to the source variable does not retag existing indexed values | not stated | 7
GENERICS62 | "Compatibility between applications with different runtime subjects requires the relevant checked relationship, such as equality established by a guard; matching variable spellings is not evidence." | Runtime index identity and storage | Compatibility between applications with different runtime subjects requires the relevant checked ... | not stated | 7
GENERICS63 | "Mutation and dependent loans retain their ordinary invalidation and witness preservation rules." | Runtime index identity and storage | Mutation and dependent loans retain their ordinary invalidation and witness preservation rules | not stated | 2
GENERICS64 | "Indexing a linear resource cannot duplicate or erase custody." | Runtime index identity and storage | Indexing a linear resource cannot duplicate or erase custody | not stated | 2
GENERICS65 | "If an index is needed by executable operations, retain it as ordinary data, an argument, or an already present descriptor field." | Runtime index identity and storage | If an index is needed by executable operations, retain it as ordinary data, an argument, or an al... | not stated | 7
GENERICS66 | "No runtime proof object, JIT, hidden allocation, or global type factory follows from value indexing." | Runtime index identity and storage | No runtime proof object, JIT, hidden allocation, or global type factory follows from value indexing | not stated | 6
GENERICS67 | "An index escaping with a result must remain bound to that result through an explicit parameter/result or carried-data contract; this rule does not invent existential packaging syntax or a dynamic ABI." | Runtime index identity and storage | An index escaping with a result must remain bound to that result through an explicit parameter/re... | not stated | 7
GENERICS68 | "A runtime capacity may remain fixed for the owner's lifetime without implying growth or automatic reallocation." | Runtime index identity and storage | A runtime capacity may remain fixed for the owner's lifetime without implying growth or automatic... | not stated | 6
GENERICS69 | "Their protected custody and cell transformations supply the initialized prefix contract needed by a library vector; a count or byte slice alone does not." | Runtime index identity and storage | Their protected custody and cell transformations supply the initialized prefix contract needed by... | not stated | 2
GENERICS70 | "A range such as `0..=65536` or a full `u32` carrier is not an instruction to reserve its maximum on the stack." | Runtime index identity and storage | A range such as 0..=65536 or a full u32 carrier is not an instruction to reserve its maximum on t... | not stated | 6
GENERICS71 | "Fixed placement must fit the actual proved stack plan and supply; dynamic stack layout is not granted by a value binder." | Runtime index identity and storage | Fixed placement must fit the actual proved stack plan and supply; dynamic stack layout is not gra... | not stated | 6
GENERICS72 | "Insufficient static placement rejects; explicit runtime storage acquisition handles its own failure." | Runtime index identity and storage | Insufficient static placement rejects; explicit runtime storage acquisition handles its own failure | not stated | 6
GENERICS73 | "The compiler cannot silently box a value to make an otherwise unsupported application work." | Runtime index identity and storage | The compiler cannot silently box a value to make an otherwise unsupported application work | not stated | 6
GENERICS74 | "Each operand must resolve in its type role." | Static type equality | Each operand must resolve in its type role | not stated | 7
GENERICS75 | "A type/value mixture rejects rather than converting a type to a runtime identifier." | Static type equality | A type/value mixture rejects rather than converting a type to a runtime identifier | not stated | 7
GENERICS76 | "On leaving the branch, only facts common to its surviving predecessors remain." | Static type equality | On leaving the branch, only facts common to its surviving predecessors remain | on every edge | 7
GENERICS77 | "An unspecialized body checks every admitted alternative; one consumer cannot authorize an otherwise invalid generic body." | Static type equality | An unspecialized body checks every admitted alternative; one consumer cannot authorize an otherwi... | not stated | 7
GENERICS78 | "Type parameters remain static even when their admissible set has several alternatives." | Static type equality | Type parameters remain static even when their admissible set has several alternatives | not stated | 7
GENERICS79 | "No runtime type objects or RTTI are introduced." | Static type equality | No runtime type objects or RTTI are introduced | not stated | 7
GENERICS80 | "The generic parameters remain fixed." | Static type equality | The generic parameters remain fixed | not stated | 7
GENERICS81 | "This does not introduce constructor-local hidden types or change ordinary specialization and layout requirements." | Static type equality | This does not introduce constructor-local hidden types or change ordinary specialization and layo... | not stated | 7
GENERICS82 | "Types containing runtime indices retain the corresponding exact subject/equality obligations; the static comparison facility cannot turn an unknown runtime endpoint into a constant or silently generate a runtime type test." | Static type equality | Types containing runtime indices retain the corresponding exact subject/equality obligations; the... | not stated | 8
GENERICS83 | "Equal layout or equal bytes do not identify different nominal types." | Static type equality | Equal layout or equal bytes do not identify different nominal types | not stated | 7
GENERICS84 | "Transparent aliases use their specified expansion; independently named domains are not merged because a proof suggests that their predicates have the same inhabitants." | Static type equality | Transparent aliases use their specified expansion; independently named domains are not merged bec... | not stated | 8
GENERICS85 | "Type equality does not perform ambient conformance selection or permit changing the meaning of `==` on values." | Static type equality | Type equality does not perform ambient conformance selection or permit changing the meaning of ==... | not stated | 7
GENERICS86 | "Ordinary initialization and invariant checking remain required." | Structural type equations and inference | Ordinary initialization and invariant checking remain required | not stated | 7
GENERICS87 | "A general element-owning vector also needs its initialized prefix and element-disposition protocol; the equation does not supply those." | Structural type equations and inference | A general element-owning vector also needs its initialized prefix and element-disposition protoco... | not stated | 7
GENERICS88 | "Repeated occurrences must agree under defined normalization." | Structural type equations and inference | Repeated occurrences must agree under defined normalization | not stated | 7
GENERICS89 | "An explicitly supplied argument is fixed; it cannot be overwritten by inference." | Structural type equations and inference | An explicitly supplied argument is fixed; it cannot be overwritten by inference | not stated | 7
GENERICS90 | "Missing, cyclic, conflicting, or underdetermined bindings reject with a request for an explicit argument." | Structural type equations and inference | Missing, cyclic, conflicting, or underdetermined bindings reject with a request for an explicit a... | not stated | 7
GENERICS91 | "An occurs check prevents a parameter being defined through itself." | Structural type equations and inference | An occurs check prevents a parameter being defined through itself | not stated | 7
GENERICS92 | "Bracketed scalar range shells are not type structure and cannot infer generic arguments." | Structural type equations and inference | Bracketed scalar range shells are not type structure and cannot infer generic arguments | not stated | 7
GENERICS93 | "Bounds proved by `requires`, `ensures`, or flow establish obligations after selection; they never choose specialization or layout identity." | Structural type equations and inference | Bounds proved by requires, ensures, or flow establish obligations after selection; they never cho... | not stated | 8
GENERICS94 | "For ordinary data and machine applications, omitted arguments can be recovered only when the combined structural equations and existing argument/result inference determine one substitution." | Structural type equations and inference | For ordinary data and machine applications, omitted arguments can be recovered only when the comb... | not stated | 7
GENERICS95 | "Unanchored disjunctions do not choose a type or integer by declaration order." | Structural type equations and inference | Unanchored disjunctions do not choose a type or integer by declaration order | not stated | 5
GENERICS96 | "All applicable occurrences must be consistent, not just the first one visited." | Structural type equations and inference | All applicable occurrences must be consistent, not just the first one visited | not stated | 7
GENERICS97 | "Named conformance selection retains its explicit telescope rule; this does not introduce ambient conformance search or omit its required arguments." | Structural type equations and inference | Named conformance selection retains its explicit telescope rule; this does not introduce ambient ... | not stated | 7
GENERICS98 | "Calling this with a value declared `u64::AtMost<256>` and omitting N selects 256 from its explicit domain argument, not a tighter bound on that particular value." | Structural type equations and inference | Calling this with a value declared u64::AtMost<256> and omitting N selects 256 from its explicit ... | not stated | 5
GENERICS99 | "The applications `AtMost<256>` and `AtMost<512>` remain distinct; containment of their predicates supplies no implicit variance." | Structural type equations and inference | The applications AtMost<256> and AtMost<512> remain distinct; containment of their predicates sup... | not stated | 7
GENERICS100 | "The TinyBytes equation requires exact qualified type equality." | Structural type equations and inference | The TinyBytes equation requires exact qualified type equality | not stated | 7
GENERICS101 | "Temporary branch facts, a literal's observed value, or a satisfier's stronger private contract cannot redefine declared type structure for this inference." | Structural type equations and inference | Temporary branch facts, a literal's observed value, or a satisfier's stronger private contract ca... | not stated | 7
GENERICS102 | "If an actual lacks the required domain-application structure, supply the argument explicitly and establish the required qualification rather than deriving a type from a value's proved bounds." | Structural type equations and inference | If an actual lacks the required domain-application structure, supply the argument explicitly and ... | not stated | 5
GENERICS103 | "Index expressions may be bound as a whole; matching `AtMost<N>` can bind the argument `Limit + 1`, but solving `N * 2 == 256` is not structural inference." | Structural type equations and inference | Index expressions may be bound as a whole; matching AtMost<N> can bind the argument Limit + 1, bu... | not stated | 7
GENERICS104 | "`upper_bound<N>()` needs no reflection, and `upper_bound()` cannot discover an unconstrained N from nothing." | Structural type equations and inference | upper_bound<N>() needs no reflection, and upper_bound() cannot discover an unconstrained N from n... | not stated | 7
GENERICS105 | "Argument formation and evaluation retain their own obligations; normalization cannot repair an ill-formed expression." | Canonical domain-index matching | Argument formation and evaluation retain their own obligations; normalization cannot repair an il... | not stated | 8
GENERICS106 | "Bounds inferred from contracts are proof facts, not an additional scalar type constructor or a source of generic arguments." | Canonical domain-index matching | Bounds inferred from contracts are proof facts, not an additional scalar type constructor or a so... | not stated | 8
GENERICS107 | "Symbolic indices retain their exact bindings and permitted normalized expressions." | Canonical domain-index matching | Symbolic indices retain their exact bindings and permitted normalized expressions | not stated | 7
GENERICS108 | "A repeated inferred parameter must match consistently; stronger proof automation must not change an application's inferred constants, storage layout, or public identity." | Canonical domain-index matching | A repeated inferred parameter must match consistently; stronger proof automation must not change ... | not stated | 8
GENERICS109 | "Separately proved relationships may establish compatibility without renaming either index." | Canonical domain-index matching | Separately proved relationships may establish compatibility without renaming either index | not stated | 7
GENERICS110 | "User algebra laws and distributive expansion establish compatibility without renaming indices." | Canonical domain-index matching | User algebra laws and distributive expansion establish compatibility without renaming indices | not stated | 7
GENERICS111 | "A caller's proof budget cannot change inferred arguments, declaration matching, or specialization keys." | Canonical domain-index matching | A caller's proof budget cannot change inferred arguments, declaration matching, or specialization... | not stated | 8
GENERICS112 | "A conformance for one exact form does not automatically match the other." | Canonical domain-index matching | A conformance for one exact form does not automatically match the other | not stated | 7
GENERICS113 | "An unresolved obligation may need a larger search allowance or an explicit theorem, but an unequal key is not evidence of mathematical inequality." | Canonical domain-index matching | An unresolved obligation may need a larger search allowance or an explicit theorem, but an unequa... | not stated | 8
GENERICS114 | "Ordinary formation, custody and trust obligations remain in force." | Canonical domain-index matching | Ordinary formation, custody and trust obligations remain in force | not stated | 2
GENERICS115 | "Explicitly uninhabited domains remain legal." | Canonical domain-index matching | Explicitly uninhabited domains remain legal | not stated | 5
GENERICS116 | "Matching that lacks a unique index rejects; emptiness cannot choose an arbitrary capacity." | Canonical domain-index matching | Matching that lacks a unique index rejects; emptiness cannot choose an arbitrary capacity | not stated | 7
GENERICS117 | "An opaque domain or an unbounded classification does not implicitly expose a finite bound." | Canonical domain-index matching | An opaque domain or an unbounded classification does not implicitly expose a finite bound | not stated | 5
GENERICS118 | "Domain predicates are not searched for a maximum, and equal inhabitant sets do not collapse distinct domain identities." | Canonical domain-index matching | Domain predicates are not searched for a maximum, and equal inhabitant sets do not collapse disti... | not stated | 5
GENERICS119 | "This is explicit application matching, not a general greatest-member operation or satisfiability solver." | Canonical domain-index matching | This is explicit application matching, not a general greatest-member operation or satisfiability ... | not stated | 7
GENERICS120 | "Placement and total stack supply must fit independently before optional optimization." | Explicitly derived bounded storage | Placement and total stack supply must fit independently before optional optimization | not stated | 6
GENERICS121 | "Runtime live length does not change fixed backing extent; growth and spill allocation need the container's explicit contract." | Explicitly derived bounded storage | Runtime live length does not change fixed backing extent; growth and spill allocation need the co... | not stated | 6
GENERICS122 | "No separate roster keyword or reflection API is required." | Finite specialization boundary | No separate roster keyword or reflection API is required | not stated | 7
GENERICS123 | "For multiple parameters, each alternative supplies a complete tuple and retains its correlations; do not silently specialize a cross-product of unrelated runtime settings." | Finite specialization boundary | For multiple parameters, each alternative supplies a complete tuple and retains its correlations;... | not stated | 7
GENERICS124 | "Opaque predicates, arbitrary inequality ranges, and mere finiteness of an integer carrier do not request enumeration." | Finite specialization boundary | Opaque predicates, arbitrary inequality ranges, and mere finiteness of an integer carrier do not ... | not stated | 5
GENERICS125 | "A runtime-capable family may lower to generated dispatch among those closed bodies when static-only operations require specialization." | Finite specialization boundary | A runtime-capable family may lower to generated dispatch among those closed bodies when static-on... | not stated | 7
GENERICS126 | "A `const` binder remains strict: runtime values enter const-only kernels through a runtime-capable family or explicit branch, not a silent relaxation of the const API." | Finite specialization boundary | A const binder remains strict: runtime values enter const-only kernels through a runtime-capable ... | not stated | 7
GENERICS127 | "A general body outside the listed alternatives must be authored under a contract admitting those inputs; the compiler invents neither a fallback nor a failure outcome." | Finite specialization boundary | A general body outside the listed alternatives must be authored under a contract admitting those ... | not stated | 7
GENERICS128 | "Every specialization must satisfy the public contract and be valid for its selected target." | Finite specialization boundary | Every specialization must satisfy the public contract and be valid for its selected target | not stated | 10
GENERICS129 | "SIMD availability, legal lane counts, instruction immediates, and exact mask conversion remain checked independently." | Finite specialization boundary | SIMD availability, legal lane counts, instruction immediates, and exact mask conversion remain ch... | not stated | 10
GENERICS130 | "Compilation resource exhaustion reports incomplete production, not permission to drop cases, change behavior, or emit unbounded JIT code." | Finite specialization boundary | Compilation resource exhaustion reports incomplete production, not permission to drop cases, chan... | not stated | 7
GENERICS131 | "Width selection need not recur per SIMD instruction; per-region scan methods make that boundary explicit." | Finite specialization boundary | Width selection need not recur per SIMD instruction; per-region scan methods make that boundary e... | not stated | 10
GENERICS132 | "Repeated public invocations may still dispatch, and unknown plugin implementations remain dynamic." | Finite specialization boundary | Repeated public invocations may still dispatch, and unknown plugin implementations remain dynamic | not stated | 7
GENERICS133 | "A finite width set does not prove a particular machine instruction or performance result." | Finite specialization boundary | A finite width set does not prove a particular machine instruction or performance result | not stated | 10
GENERICS134 | "Finite callable families retain canonical contract-and-tuple rows independently of target discovery." | Finite specialization boundary | Finite callable families retain canonical contract-and-tuple rows independently of target discovery | not stated | 3
GENERICS135 | "Static conformances retain their own complete requirement maps." | Finite specialization boundary | Static conformances retain their own complete requirement maps | not stated | 7
GENERICS136 | "A result may remain inside a statically selected branch or use a common representation." | Finite specialization boundary | A result may remain inside a statically selected branch or use a common representation | not stated | 7
GENERICS137 | "If different specialized results must escape together, author an ordinary finite sum with exact case payloads, a uniform borrowed some package, or an already specified owner/descriptor contract." | Finite specialization boundary | If different specialized results must escape together, author an ordinary finite sum with exact c... | not stated | 2
GENERICS138 | "Static type equality and structural parameter binding do not require runtime type objects." | Reflection boundary | Static type equality and structural parameter binding do not require runtime type objects | not stated | 7
GENERICS139 | "Runtime metadata does not become a generic type argument; dependent applications retain exact subject relationships rather than guessed static type keys." | Reflection boundary | Runtime metadata does not become a generic type argument; dependent applications retain exact sub... | not stated | 7
GENERICS140 | "Reflection does not weaken types before generic binding, change exact `==` into assignability, or make selection records into conformance arguments." | Reflection boundary | Reflection does not weaken types before generic binding, change exact == into assignability, or m... | not stated | 7
GENERICS141 | "Calls through a requirement retain its complete substitution contract, including reach, independent suspension/blocking possibilities, crashes, progress, and context-visible resources." | Requirements and conformance evidence | Calls through a requirement retain its complete substitution contract, including reach, independe... | not stated | 3
GENERICS142 | "Selected implementations must refine that contract." | Requirements and conformance evidence | Selected implementations must refine that contract | not stated | 7
GENERICS143 | "There is no anonymous member requirement inferred or carried by a `requires machine T::member(...)` clause." | Requirements and conformance evidence | There is no anonymous member requirement inferred or carried by a requires machine T::member(...)... | not stated | 7
GENERICS144 | "Expected shape validates an explicitly selected map; it does not discover one or fill its non-lifetime arguments." | Requirements and conformance evidence | Expected shape validates an explicitly selected map; it does not discover one or fill its non-lif... | not stated | 7
GENERICS145 | "Explicit trait type parameters supply all data types needed by the interface." | Requirements and conformance evidence | Explicit trait type parameters supply all data types needed by the interface | not stated | 7
GENERICS146 | "There is no implementing data subject, `Self` binder, or associated-type declaration surface." | Requirements and conformance evidence | There is no implementing data subject, Self binder, or associated-type declaration surface | not stated | 7
GENERICS147 | "Omitted `suspends` and `blocks` still mean never, since their markers need a known bound." | Requirements and conformance evidence | Omitted suspends and blocks still mean never, since their markers need a known bound | not stated | 3
GENERICS148 | "Calling `Order::compare` cannot substitute the reach of another row in `Order`." | Requirements and conformance evidence | Calling Order::compare cannot substitute the reach of another row in Order | not stated | 3
GENERICS149 | "Generic declarations bind types, values, machine declarations, or explicit conformance evidence." | Generic declarations and static machine parameters | Accept only type, value, machine, and conformance-evidence generic binders | not stated | 7
GENERICS150 | "Value binders distinguish runtime-capable `Count: u32` from static `const Count: u32`." | Generic declarations and static machine parameters | Distinguish runtime-capable and const value binder spellings | not stated | 7
GENERICS151 | "Retaining `machine Step` there is also legal." | Generic declarations and static machine parameters | Allow explicit machine binder in generic list with requires-machine clause | not stated | 7
GENERICS152 | "Neither route infers whole-trait conformance evidence." | Generic declarations and static machine parameters | Do not infer whole-trait conformance on either binder route | not stated | 7
GENERICS153 | "A machine requirement admits the same generic binder kinds as an ordinary machine, including a named implementation parameter such as `Order satisfies Ordering<T>`." | Generic machine requirements | Machine requirements admit same binder kinds as ordinary machines | not stated | 7
GENERICS154 | "The caller supplies the exact named ordering implementation at each static application, just as for an ordinary generic machine." | Generic machine requirements | Caller supplies exact named implementation at each static application | at a call | 7
GENERICS155 | "The selected body declares the matching ordered binders, kinds and requirements and satisfies the complete requirement contract for every admitted application." | Generic machine requirements | Implementation must match binders and satisfy requirement contract for every admitted application | not stated | 7
GENERICS156 | "This introduces neither a generic foreign ABI nor implicit native adapters." | Generic machine requirements | Do not introduce generic foreign ABI or implicit native adapters | not stated | 9
GENERICS157 | "Lexically nested machines may refer to enclosing static generic selections (types, static values, machine selections and conformance evidence) by their exact declaration identities." | Applications and specialization | Nested machines reference enclosing generics only by exact declaration identity | not stated | 7
GENERICS158 | "Type parameters use ordinary angle-bracket applications." | Applications and specialization | Use ordinary angle-bracket type applications | at a call | 7
GENERICS159 | "`const N: u64` binds a static value of the declared carrier, usable in layouts, ranges, and contracts." | Applications and specialization | const N binds static carrier value for layouts, ranges, contracts | not stated | 7
GENERICS160 | "`N: u64` binds a value which may be known statically or supplied at runtime." | Applications and specialization | Unqualified N may be static or runtime at application | at a call | 7
GENERICS161 | "Ordinary type/result inference may obtain arguments from runtime arguments, expected destinations, and selected static-machine contracts when the exact application is determined." | Applications and specialization | Inference sources limited to runtime args, destinations, static-machine contracts when application fixed | not stated | 7
GENERICS162 | "Expansion substitutes exact declaration identities, validates every underlying argument and obligation, and precedes canonical type identity and ordinary matching." | Transparent type aliases | Expand aliases with identity substitution and validation before canonical matching | not stated | 7
GENERICS163 | "An alias and its complete expansion are the same type, with the same representation, ABI, multiplicity, cleanup, qualifications, access and lifetime obligations." | Transparent type aliases | Alias equals expansion for type, ABI, multiplicity, cleanup, access, lifetimes | not stated | 7
GENERICS164 | "Alias expansion is acyclic, including indirect and growing generic cycles; normalization reports the alias chain rather than looping." | Transparent type aliases | Reject cyclic alias expansion; report chain in diagnostics | not stated | 7
GENERICS165 | "Substitution is declaration-identity based throughout bodies, contracts, field types, and nested applications." | Closed applications and canonical identity | Use declaration-identity substitution in bodies, contracts, fields, nested apps | not stated | 7
GENERICS166 | "Generic fields obey ordinary ownership, borrowing, and cleanup, including the active payload's substituted linear obligations." | Closed applications and canonical identity | Apply ownership, borrow, cleanup to generic fields with substituted linear ops | not stated | 2
GENERICS167 | "Runtime-capable value index; a static argument is also permitted. (Binder spelling `Count: u32`.)" | Value binders and const requirements | Runtime-capable Count allows static or runtime argument | at a call | 7
GENERICS168 | "Value must be statically known within the compiled specialization. (Binder spelling `const Count: u32`.)" | Value binders and const requirements | const Count must be static in compiled specialization | not stated | 7
GENERICS169 | "Adding it allows the body to rely on static layout or static-only operations without providing a runtime fallback." | Value binders and const requirements | const binder authorizes static-only body without runtime fallback | not stated | 7
GENERICS170 | "`==` between type expressions states exact normalized type equality, not value equality, assignability, compatible layout, or conformance." | Static type equality | Type == means exact normalized type equality only | not stated | 7
GENERICS171 | "Every recovered parameter has an explicit binder and declared kind." | Structural type equations and inference | Each inferred argument must map to explicit binder and kind | not stated | 7
GENERICS172 | "Match known structures by exact constructor and parameter position, including domain applications, fixed arrays, and declared generic applications." | Structural type equations and inference | Structural match uses exact constructor and parameter position | not stated | 7
GENERICS173 | "Inference selects arguments before checking ordinary compatibility." | Structural type equations and inference | Select generic arguments before ordinary compatibility check | not stated | 7
GENERICS174 | "A `const` index needs static inputs; a runtime index can bind only where the value binder and its exact subject/lifetime rules permit dynamic values." | Structural type equations and inference | const index inference requires static inputs; runtime index only where permitted | not stated | 7
GENERICS175 | "Generate only demanded static applications or the complete family needed by a dynamic selection." | Finite specialization boundary | Generate only demanded static apps or full family for dynamic selection | not stated | 7
GENERICS176 | "Dispatch owns one exact selected index throughout an operation, including calls into its specialized loop." | Finite specialization boundary | Hold one selected index across operation including specialized loop | at the operation | 7
GENERICS177 | "Index and result custody stay joined." | Finite specialization boundary | Keep index and result custody joined when results escape | not stated | 2
GENERICS178 | "No implicit boxing, arbitrary existential type, or unspecified variable-layout result ABI is introduced." | Finite specialization boundary | Do not add implicit boxing, existentials, or unspecified result ABI | not stated | 7
GENERICS179 | "Generic code proves declared preconditions and may use declared guarantees." | Requirements and conformance evidence | Generic code must prove declared preconditions; may use guarantees | not stated | 8
GENERICS180 | "Generic bodies use the abstract requirement envelope." | Requirements and conformance evidence | Check generic bodies against abstract requirement envelope | not stated | 7
GENERICS181 | "Callers substitute selected published contracts into those dependencies, not incidental stronger behavior." | Requirements and conformance evidence | Callers substitute published contracts into dependencies only | at a call | 3
GENERICS182 | "A required member operation belongs to a named trait and is supplied through an explicit conformance binder, for example `Order satisfies Ranked<Element>`." | Requirements and conformance evidence | Required trait members need explicit conformance binder not structural guess | not stated | 7
GENERICS183 | "Omitted service reach on a bodiless non-boundary requirement defers to the selected member's published row, and omitted `crashes` defers to its published routes." | Requirements and conformance evidence | Default omitted reach/crashes on bodyless requirements from selected member row | not stated | 3
GENERICS184 | "An explicit clause bounds that row, with `reaches none` requiring it to be empty and `crashes none` admitting only crash-free members." | Requirements and conformance evidence | Explicit reach/crashes clauses bound deferred member row | not stated | 3
GENERICS185 | "A public generic whose body may crash under those requirements states its crash ceiling like any other public machine." | Requirements and conformance evidence | Public generics state crash ceiling when body may crash | not stated | 3
GENERICS186 | "A conformance evidence binder retains a separate dependency for each called requirement member and exact application." | Requirements and conformance evidence | Track separate dependency per called requirement member and application | at a call | 3

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
An explicit `requires machine Step(...)` or `requires machine Step satisfies Trait::requirement` clause can determine the kind of Step introduced without a kind marker in the generic list. | The clause alone fixes the binder kind when no `machine` token appears in the generic parameter list. | The clause may determine kind in some positions but still requires a kind marker in other generic lists; scope of "without a kind marker" is unstated.
Retaining `machine Step` there is also legal. | A kind marker in the generic list may coexist with a `requires machine` clause without conflict. | Legal only when the clause and list agree; unstated whether redundant markers are warnings or silently ignored.
An explicitly kinded `machine` binder on a wrapper may instead acquire exact forwarded requirements. | Wrapper binders always use forwarding when kinded; originating contracts are not used. | Forwarding is optional ("may"); compiler may still require originating contract when forwarding is absent.
Neither route infers whole-trait conformance evidence. | No partial trait evidence is inferred even when all members are structurally present. | Whole-trait evidence is not inferred; whether single-member evidence can be inferred on either route is unstated.
Binder names need not match; identities and substitution must. | Only declaration identity and substitution consistency are checked; spellings are irrelevant everywhere. | Names may differ only where the chapter allows; unstated whether diagnostics still display binder spellings for errors.
For a boundary requirement, Build selects the realizing implementation for the target. | Build fills only boundary requirements; caller still supplies evidence binders at compile time. | Build may realize boundary requirements that replace caller-supplied evidence for that target; timing vs checking is unstated.
Ordinary type/result inference may obtain arguments from runtime arguments, expected destinations, and selected static-machine contracts when the exact application is determined. | Inference runs only after the application tuple is fully determined. | "When the exact application is determined" may mean a fixed point with inference participating in determination.
Inference selects arguments before checking ordinary compatibility. | Compatibility is never used to choose among multiple valid inferences. | Compatibility checks run after inference but may still reject programs that inferred arguments that fail later checks; order of error reporting is unstated.
Bounds proved by `requires`, `ensures`, or flow establish obligations after selection; they never choose specialization or layout identity. | Flow facts never affect which specialization is selected. | Flow facts may affect proof obligations and runtime checks after specialization is fixed; "never choose" applies only to identity keys.
Unanchored disjunctions do not choose a type or integer by declaration order. | Disjunctions without anchoring reject when inference would need to pick an alternative. | Disjunctions reject only when multiple alternatives satisfy structure; single-satisfying disjunction may still infer unstated.
Omitted service reach on a bodiless non-boundary requirement defers to the selected member's published row, and omitted `crashes` defers to its published routes. | Omission always means deferral to the selected implementation's published contract. | Deferral applies only when a member is selected; unstated behavior when selection is still open at requirement declaration time.
Omitted `suspends` and `blocks` still mean never, since their markers need a known bound. | Omission is equivalent to `suspends never` and `blocks never` on requirements. | Omission means never for suspension/blocking markers only; other omitted effect rows may follow different defaults elsewhere.
A runtime-capable family may lower to generated dispatch among those closed bodies when static-only operations require specialization. | Dispatch is mandatory whenever static-only operations appear in a runtime-capable family. | Lowering to dispatch is permitted but not required; direct interpretation without dispatch may still be valid.
Generate only demanded static applications or the complete family needed by a dynamic selection. | "Complete family" means all listed disjuncts for dynamic selection, not cross-product of unrelated parameters. | Which dynamic selections trigger generating the full finite family vs one body is unstated beyond the two alternatives.
Reflected identity uses canonical type equality, not names or matching layouts. | Reflection never uses nominal names or layout equality for identity. | Layout inequality cannot disprove canonical type equality; whether layout is used for disambiguation when canonical equality is unknown is unstated.

## Terms used without definition in this chapter

- some binders
- machine F
- conformance evidence
- dependent-value rules
- const-generic substitution
- unified requirement clauses
- static admission
- runtime entry obligations
- nested callable contracts
- originating contract
- forwarded requirements
- whole-trait conformance evidence
- generic-requirement-evidence-binders
- named implementation parameter
- boundary trait
- boundary requirement
- Build (target selection)
- generic foreign ABI
- implicit native adapters
- Machine values
- closed callable signatures
- finite value families / finite generic method families
- some packages
- asm machine
- generic assembly
- lexically nested machines
- application identity
- flattening
- runtime closure
- angle-bracket applications
- type/result inference
- canonical identity / canonical static identities
- declaration identity
- monomorphization
- transparent alias / alias expansion
- domain-conjunction aliases
- transparent-generic-type-aliases
- numeric_values exact rules
- target-dependent applications
- binding properties
- embed (in requires)
- value-indexed applications
- default-domain coupling
- proof-only applications
- dispatch route
- static-only use of dynamic binder
- runtime index / extent index
- retag
- dependent loans
- typed runs
- stack plan / fixed placement
- static type equality / normalized type equality
- RTTI
- case-dependent default domain
- structural type equations
- occurs check
- bracketed scalar range shells
- explicit telescope rule
- AtMost / domain applications
- canonical domain-index matching
- stable canonical index identity
- proved index compatibility
- proof budget / proof-discovery effort
- Root Build
- explicitly derived bounded storage
- finite specialization boundary
- ByteScanner-style disjunction family
- SIMD / mask conversion
- finite callable families
- semantic reflection
- layout schema
- scoped selection context
- operation record
- substitution contract
- abstract requirement envelope
- reach dependency
- static binder contract dependencies
- result relation
- operation-record schema
- anonymous member requirement
- Named conformances
- bodyless non-boundary requirement
- service reach
- Self binder (absence)
- evidence parameters
- explicit theorem / heuristic theorem search
- predicate equivalence
- greatest-member operation
- satisfiability solver
- higher-kinded arguments
- ambient conformance search
- evaluation admission
- some package (uniform borrowed representation)

CHAPTER COMPLETE
