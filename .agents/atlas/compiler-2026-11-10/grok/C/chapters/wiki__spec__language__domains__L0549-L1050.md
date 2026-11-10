# Domains (wiki/spec/language/domains.md L549–L1050)

Source: `wiki/spec/language/domains.md` lines 549–1050 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
DOMAINS1 | "A declared type gains or loses a domain only through a cast that names that domain." | Domain casts | Declared-type domain attachment may change only via an explicit named-domain cast | not stated | 5
DOMAINS2 | "No proof, expected type, literal, call boundary or join attaches or drops one." | Domain casts | Proofs, expected types, literals, call boundaries, and joins must not add or remove domains on a declared type | not stated | 5
DOMAINS3 | "Only a cast changes the set of domains a type carries, and operator and overload selection read only declared types and written casts." | Domain casts | Operator and overload resolution must use only declared types and written casts; domain sets on types change only by cast | at the operation | 5
DOMAINS4 | "A `requires x in Positive`, a guard or a call guarantee proves a fact about `x`. The fact supports later proofs and leaves the declared type of `x` unchanged." | Domain casts | Proved domain facts must not alter the declared type of the value | not stated | 8
DOMAINS5 | "`as` never calls user code, rounds, saturates, traps, allocates, reaches a service, suspends or fails at runtime." | Domain casts | Domain and carrier casts must not lower to runtime user calls, rounding, saturation, traps, allocation, services, suspension, or runtime failure | not stated | 5
DOMAINS6 | "All predicates and representability obligations are discharged before lowering." | Domain casts | Every cast predicate and representability obligation must be proved or rejected before lowering | not stated | 5
DOMAINS7 | "Scale changes between units and other policy-bearing conversions are ordinary named machines or explicitly selected operators, never conversions the cast discovers." | Domain casts | Unit scale and policy conversions must not be inferred or performed by casts | not stated | 5
DOMAINS8 | "A cast with no carrier, `value as =A -B +C`, keeps the carrier and states what happens to every domain:" | Domain-only casts | Domain-only cast syntax must keep the operand carrier and enumerate domain entries | at the operation | 5
DOMAINS9 | "The entries are one simultaneous accounting, not a sequence, and their order has no meaning." | Domain-only casts | Domain cast entries must be checked as one simultaneous accounting; entry order must not affect meaning | at the operation | 5
DOMAINS10 | "Every domain attached to the operand's type appears exactly once, as `=` or `-`." | Domain-only casts | Each domain on the operand static type must appear exactly once as `=` or `-` | at the operation | 5
DOMAINS11 | "That includes each inherited parent, each constituent of an alias and each compiler-owned domain." | Domain-only casts | Parent domains, alias constituents, and compiler-owned domains must each appear once in the accounting | at the operation | 5
DOMAINS12 | "Every `+` entry names a domain the operand lacks." | Domain-only casts | Each `+` entry must name a domain not on the operand static type | at the operation | 5
DOMAINS13 | "The result carries exactly the `=` and `+` domains." | Domain-only casts | Result static type must carry precisely the domains listed with `=` and `+` | at the operation | 5
DOMAINS14 | "A missing entry, a repeated domain, `=D` or `-D` for a domain the operand lacks, and `+D` for a domain it already carries each reject, and the diagnostic names the domain." | Domain-only casts | Reject incomplete accounting, duplicates, `=`/`-` on absent domains, and `+` on present domains; diagnostic names the domain | at the operation | 5
DOMAINS15 | "\"Carries\" means attached to the static type, not merely provable." | Domain-only casts | Cast accounting must use static attachment, not flow-proved membership alone | at the operation | 5
DOMAINS16 | "A plain `x` with a proved `x in Positive` lacks the domain and may write `x as +Positive`; it may not write `x as =Positive`." | Domain-only casts | Provably-in-domain without static attachment allows only `+D`, not `=D` | at the operation | 5
DOMAINS17 | "Has predicates | Prove them for this value." | Domain-only casts (`+D` establishment) | For `+D` on a predicate domain, all predicates must be proved for the value | at the operation | 5
DOMAINS18 | "Has establishment routes | Retain exact existing or authorized evidence; a cast cannot mint it." | Domain-only casts (`+D` establishment) | `+D` on routed domains requires existing or authorized evidence; casts must not create route evidence | at the operation | 5
DOMAINS19 | "`-D` discards no fact: after `both as =Km -Positive`, the value is still known to exceed zero." | Domain-only casts | Removing a domain with `-D` must leave proved facts available | at the operation | 8
DOMAINS20 | "Removal also never increases authority. Owned claims must be consumed or transferred, not cast away." | Domain-only casts | `-D` must not remove obligations that would increase authority; owned claims cannot be cast away | at the operation | 2
DOMAINS21 | "A cleanup selection survives." | Domain-only casts | Domain removal must not drop an active cleanup selection | at the operation | 2
DOMAINS22 | "Access policies survive whenever they are needed to prevent authority amplification, and a view that cannot retain them rejects." | Domain-only casts | Required access policies must survive removal or the view/cast rejects | at the operation | 2
DOMAINS23 | "Carry demands survive until the underlying claim is discharged." | Domain-only casts | Carry obligations must remain until discharged; casts must not erase them early | not stated | 2
DOMAINS24 | "No cast infers a relation between domains." | Domain-only casts | Casts must not relate or convert between domains implicitly | at the operation | 5
DOMAINS25 | "A child domain travels with its ancestors, and a cast names each one. The result must be ancestor-complete:" | Domain-only casts | Parent and child domains must be accounted together; result must include required ancestors | at the operation | 5
DOMAINS26 | "// Rejected: the result would carry the child without its parent." | Domain-only casts | Reject carrying a child domain without its parent on the result type | at the operation | 5
DOMAINS27 | "// Rejected: `Positive` is not attached and not added." | Domain-only casts | Reject adding a child domain when its parent is neither attached nor added with `+` | at the operation | 5
DOMAINS28 | "Naming an alias names its expansion: for `Socket::Usable` below, `-Usable` removes `Connected` and `Authenticated`, and the same cast may not also name either constituent." | Domain-only casts | Alias entries expand to constituents; the same cast must not name both alias and expanded constituent | at the operation | 5
DOMAINS29 | "A data declaration's default domain is never removed." | Domain-only casts | Default domain of a data declaration must not be removable by any cast entry | at the operation | 5
DOMAINS30 | "It belongs to the carrier: no cast names it, and no type spells the value without it, so every machine that receives a value of a data type receives it in that domain." | Domain-only casts | Default carrier domain must be implicit on all values of that data type and on all receiving parameters | at a call | 5
DOMAINS31 | "`value as T` changes the carrier through a unique exact compiler-derived transformation: exact widening, or narrowing with representability proved." | Carrier casts | Carrier casts must use the unique exact widening or proved narrowing transformation | at the operation | 5
DOMAINS32 | "The denoted value does not change." | Carrier casts | Carrier cast must preserve the denoted mathematical value | at the operation | 5
DOMAINS33 | "The operand of a carrier cast must carry no domain." | Carrier casts | Reject carrier cast when operand static type carries any domain | at the operation | 5
DOMAINS34 | "The cast may add domains to the result, so `plain as i64 +Km` abbreviates `(plain as i64) as +Km`, and it accepts no `=` or `-` entry." | Carrier casts | Combined carrier-plus-domain cast may only use `+` on the result, not `=` or `-` in the same spellings | at the operation | 5
DOMAINS35 | "Casts chain left to right, so moving a qualified value to another carrier reads as three visible steps:" | Carrier casts | Qualified carrier changes must chain as explicit separate casts in source order | at the operation | 5
DOMAINS36 | "// Rejected: each of these changes the carrier of a qualified value." | Carrier casts | Reject single-step carrier change on a domain-qualified operand | at the operation | 5
DOMAINS37 | "Membership is carrier-exact, so a domain in the new carrier is a different application whose obligations are established there." | Carrier casts | Domain identity after carrier change is carrier-exact; obligations must be established on the new carrier application | at the operation | 5
DOMAINS38 | "A domain with establishment routes cannot be re-added without its route, which effectively pins the carrier of such a value." | Carrier casts | Re-adding routed domains after carrier change requires route evidence and constrains carrier movement | at the operation | 5
DOMAINS39 | "For an anonymous expression, `as` to an integer destination explicitly lands the complete operand under the numeric landing rules." | Carrier casts | Anonymous expression cast to integer must satisfy numeric landing rules for the full operand | at the operation | 5
DOMAINS40 | "`((5 / 2) * 2) as u32` is `5`, while `(5 / 2) as u32` rejects." | Carrier casts | Reject integer cast when fractional intermediate would violate landing; accept when landing rules show integral in-range value | at the operation | 5
DOMAINS41 | "The whole-type cast targets `value as T in D` and `value as T::D` are retired, not alternative spellings." | Carrier casts | Reject retired spellings `as T in D` and `as T::D` | at the operation | 5
DOMAINS42 | "An expected type selects the carrier in which an anonymous literal lands. It never inserts a domain cast." | Literals and expected types | Expected type for literals selects carrier only; must not insert domain cast | not stated | 5
DOMAINS43 | "the supplied expression's static type must carry exactly the domains the destination declares." | Literals and expected types | At let, argument, field init, return, and arm sites, expression static domains must match destination exactly | not stated | 5
DOMAINS44 | "Arms and joins that produce one result agree on that exact set; a join neither intersects the arms' domains nor weakens to their common part." | Literals and expected types | Joined arms must agree on the full domain set; join must not intersect or weaken domains | not stated | 1
DOMAINS45 | "The expected type only selects among declared machines. The selected machine returns its own declared type, and that type must equal the destination's exactly:" | Result-domain overloads (cross-ref Literals) | Result-domain overload selection must not insert cast; destination must match selected machine result type exactly | at a call | 3
DOMAINS46 | "because predicate-only domains contribute no key, a destination that differs from the selected result by one still needs a written domain cast." | Result-domain overloads | When overload keys omit predicate-only domains, destination differing only by such a domain still requires an explicit cast | at a call | 5
DOMAINS47 | "Builtin operators and comparisons, including the suffixed policy operators, accept unqualified operands only." | Operands | Builtin and policy-suffixed operators require operands with no domain qualification on static type | at the operation | 5
DOMAINS48 | "The rule is the same in executable code, `requires` and `ensures`." | Operands | Same unqualified-operand rule in executable code, requires, and ensures | at the operation | 5
DOMAINS49 | "A qualified operand either has a declared operator for its exact type under semantic roles and operators or is cast first:" | Operands | Qualified operands need a matching domain operator declaration or a stripping cast before builtin use | at the operation | 5
DOMAINS50 | "In a domain's own `requires`, `self` is the value in the underlying carrier without the domain being defined, so `self > 0` in `Positive` is a builtin comparison on `i32`." | Operands | In domain `requires`, `self` is unqualified carrier value for builtin comparisons | not stated | 5
DOMAINS51 | "`[]` applies when one declaration's parameter types equal the collection and index types, so a qualified collection or index has such a declaration or is cast." | Operands | Indexing requires an operator declaration matching qualified collection and index types or a cast | at the operation | 6
DOMAINS52 | "A named call admits an argument only at its parameter's exact declared domain set." | Operands | Named calls must match each argument to the parameter's exact declared domain set | at a call | 3
DOMAINS53 | "Explicit zero defaults are not literals." | Operands | Zero-default establishment must not be treated as literal domain insertion | not stated | 1
DOMAINS54 | "A cast applies to a reference by accounting for the domains displayed on its referent, with the same syntax: `&value as =Km -Positive`." | Mutable views | Reference casts account for referent static domains with the same entry rules | at the operation | 2
DOMAINS55 | "A borrow groups before `as`, so no parentheses are needed." | Grouping / Mutable views | Parse `&expr as …` with borrow binding before cast | at the operation | 5
DOMAINS56 | "The cast preserves the referent, lifetime and access mode, and it changes neither the stored owner's declared type nor the carrier's default domain." | Mutable views | Reference cast must preserve referent identity, lifetime, access mode, owner declared type, and default domain | at the operation | 2
DOMAINS57 | "Through a shared reference any removal a by-value cast permits is accepted and nothing is owed, because nothing can be written." | Mutable views | Shared-mutable/shared ref domain removal permitted when by-value removal would be; no re-establishment debt | at the operation | 2
DOMAINS58 | "Through a mutable reference, `-D` is accepted exactly when the lender could write `as +D` on that place itself when the loan ends." | Mutable views | Mutable-ref `-D` allowed only if lender could satisfy `+D` on that place at loan end | at the operation | 2
DOMAINS59 | "The end of the loan is that `+D`: it is not written, and it is checked by the same establishment rules for whatever the place then holds." | Mutable views | At loan end, require `+D` obligations for removed domains without a written cast | not stated | 2
DOMAINS60 | "// Rejected: `Positive` is not re-established for `trip` after `scribble`." | Mutable views | Reject use or handback when mutable loan ends without re-establishing removed domains | not stated | 2
DOMAINS61 | "Has establishment routes | Accepted only where `+D` is accepted, which is inside its authorized routes. Elsewhere the removal rejects." | Mutable views | Mutable-ref removal of routed domains allowed only where `+D` would be accepted in authorized routes | at the operation | 2
DOMAINS62 | "A removal that a by-value cast would reject stays rejected: owned claims, cleanup selections and access policies are not removed through a reference either." | Mutable views | By-value illegal removals remain illegal through references | at the operation | 2
DOMAINS63 | "A domain kept with `=D` is part of the callee's parameter type, so the callee maintains it on every write and the lender owes nothing for it." | Mutable views | `=D` on mutable view makes callee responsible to maintain domain on writes | across a pause | 2
DOMAINS64 | "Each removed domain is a debt on the lender's place under invariant windows and contract-based repair." | Mutable views | Removed domains create lender debts tracked under invariant and repair rules until closed | not stated | 2
DOMAINS65 | "Until the debt closes it cannot be used under the domain, passed where the domain is required or handed back to its own owner." | Mutable views | Open domain debt blocks use, pass, and handback requiring that domain | not stated | 2
DOMAINS66 | "An open debt at the lender's completion or handback rejects, and the diagnostic names the domain, the place and the cast that removed it." | Mutable views | Reject lender completion or handback with open domain debt | at normal return | 2
DOMAINS67 | "A callee that removes further domains and lends again owes only what it removed." | Mutable views | Nested lending owes re-establishment only for domains that callee removed | not stated | 2
DOMAINS68 | "The owner's declared type is unchanged for the whole loan, and exclusivity means nothing else observes the place while a debt may be open." | Mutable views | Owner declared type fixed for loan duration; exclusivity during open debt | not stated | 2
DOMAINS69 | "Earlier facts about the contents do not return; only fresh proof about the current contents does." | Mutable views | After loan, only fresh proofs about current contents restore facts; old content facts do not revive | not stated | 8
DOMAINS70 | "An exact carrier conversion applies to values and never through a reference;" | Mutable views | Exact carrier conversion must not apply through references | at the operation | 2
DOMAINS71 | "After `as` comes an optional carrier type and then the longest run of entries, each a `=`, `+` or `-` followed by a domain path. At least one of the two is present." | Grouping | Cast syntax must have carrier type and/or at least one domain entry | at the operation | 5
DOMAINS72 | "Parsing never consults name lookup: a `+` or `-` after a cast always begins an entry and never an addition or subtraction, whether the cast names a carrier or not." | Grouping | After `as`, `+` and `-` start domain entries, not arithmetic, without name lookup | at the operation | 5
DOMAINS73 | "Arithmetic on a cast result therefore takes parentheses, as in `(distance as -Km) + adjustment` and `(byte as u32) + 1`." | Grouping | Require parentheses when arithmetic follows a cast result | at the operation | 0
DOMAINS74 | "A cast produces a result expression. Ordinary copy, move and borrow rules decide whether the source remains usable afterward." | Grouping | Apply standard copy, move, and borrow rules to cast source and result | at the operation | 2
DOMAINS75 | "Role identity is closed and compiler-owned. Packages supply theories within these roles, not new roles by declaration:" | Semantic roles and operators | Reject user-declared new semantic roles; only compiler-owned role set | not stated | 5
DOMAINS76 | "Competing contributions to one role reject: `Wrapping & Trapping` has conflicting arithmetic policies." | Semantic roles and operators | Reject multiple incompatible contributions to the same semantic role on one type | not stated | 5
DOMAINS77 | "An operator applies only when the instantiated parameter types of one declaration equal the operands' complete static qualified types, after alias expansion, normalization and generic substitution." | Semantic roles and operators | Operator resolution requires exact match of one declaration's instantiated parameter types to operand static qualified types | at the operation | 5
DOMAINS78 | "Nothing composes automatically." | Semantic roles and operators | Must not auto-compose operators from separate domain contributions | at the operation | 5
DOMAINS79 | "`Km & Wrapping` has no `+` because `Km` declares one and `Wrapping` declares one; it has a `+` only when a declaration names that exact set, and that declaration states the combined meaning in its own body:" | Semantic roles and operators | Combined-domain operators exist only when a declaration names the exact combined domain set | at the operation | 5
DOMAINS80 | "No matching declaration is a missing operator and several are an ambiguity; neither selects a nearest candidate, drops a domain to find one or runs unrelated implementations in sequence." | Semantic roles and operators | Zero matches is missing operator; multiple matches is ambiguity; no fallback selection or domain dropping | at the operation | 5
DOMAINS81 | "Resolution reads declared types and written casts, never flow facts: a proved `x in D` or a `requires x in D` on a plain `x` selects nothing." | Semantic roles and operators | Operator selection must ignore flow-proved domain facts and requires-only facts on plain types | at the operation | 5
DOMAINS82 | "The selected operation is unambiguous and retained in the checked artifact; an unrelated import cannot inject a competing meaning." | Semantic roles and operators | Checked artifact must record one unambiguous selected operation immune to import competition | not stated | 5
DOMAINS83 | "Predicates do not synthesize operators." | Semantic roles and operators | Predicate domains must not implicitly generate operators | not stated | 5
DOMAINS84 | "An operator must publish and prove its relational contract: a degree result in `[0, 360)` alone would incorrectly admit an implementation always returning zero. Its relation to the operands must also be established." | Semantic roles and operators | Domain operators must prove full relational contracts tying results to operands | not stated | 8
DOMAINS85 | "Standing ranges must remain valid at their consumption points under the chosen operations; optional flow refinements may be invalidated and re-proved." | Semantic roles and operators | Range facts must hold at use sites under selected operations; flow refinements may require re-proof | at the operation | 8
DOMAINS86 | "Removing the policy domain with a cast leaves a bare numeric carrier, whose unsuffixed operators are Exact, with all subsequent safety obligations intact." | Semantic roles and operators | After policy domain stripped by cast, unsuffixed operators are Exact with remaining safety obligations | at the operation | 5
DOMAINS87 | "Predicate-only domains without access policies contribute no key." | Result-domain overloads | Dispatch projection keys must omit predicate-only domains without access policies | at a call | 5
DOMAINS88 | "The expected result projection must equal one declared projection; without an expected result, lookup selects the empty projection." | Result-domain overloads | Result-type projection must match one overload declaration; absent expected result uses empty projection | at a call | 3
DOMAINS89 | "Only afterward check predicates and ordinary compatibility." | Result-domain overloads | Predicate and compatibility checks run only after result-projection overload selection | at a call | 3
DOMAINS90 | "Equal projections on the same path and parameter signature are declaration-site duplicates, even if written results differ by predicate refinement." | Result-domain overloads | Reject duplicate overload declarations with equal projections on same path and parameters | not stated | 5
DOMAINS91 | "Fixed operator spellings retain operand-directed selection, not this result rule." | Result-domain overloads | Fixed operators must use operand-directed resolution, not result-domain overload rule | at the operation | 5
DOMAINS92 | "A transparent alias is a nonempty conjunction over compatible subjects." | Aliases and identity | Transparent aliases must be nonempty conjunctions over compatible subjects | not stated | 5
DOMAINS93 | "Expansion precedes sorting, deduplication, and identity formation." | Aliases and identity | Alias expansion must run before sorting, deduplication, and identity formation | not stated | 5
DOMAINS94 | "The alias and expanded atoms have one normalized identity; edits affect every published contract using that expansion." | Aliases and identity | Alias and its expansion share one normalized identity used across contracts | not stated | 5
DOMAINS95 | "Alias expansion is acyclic." | Aliases and identity | Reject cyclic alias expansion | not stated | 5
DOMAINS96 | "Every constituent of a public alias must be legal to publish for its subject." | Aliases and identity | Each atom in a public alias must be publishable for its subject | not stated | 5
DOMAINS97 | "A domain cast that names the alias accounts for every expanded atom, each under its own predicate, route or custody rules." | Aliases and identity | Casts naming an alias must account for every expanded constituent under its rules | at the operation | 5
DOMAINS98 | "Stronger entailment may accept more programs but cannot change identity or operator meaning." | Aliases and identity | Entailment strengthening must not alter domain identity or operator meaning | not stated | 8
DOMAINS99 | "Physical ABI remains the carrier's; semantic interface identity retains the domain theory." | Aliases and identity | ABI is carrier-only; semantic interface identity keeps domain theory separate | not stated | 9
DOMAINS100 | "The family explicitly binds its carrier and canonical proof-static index." | Indexed families | Indexed family declarations must bind carrier and canonical proof-static index | not stated | 5
DOMAINS101 | "It imposes no carrier-wide arithmetic requirement and contributes no layout." | Indexed families | Indexed families must not impose carrier-wide arithmetic or layout | not stated | 5
DOMAINS102 | "Ordinary operator conformances require only the carrier operations they use." | Indexed families | Operator conformances on families need only declared carrier operations | not stated | 5
DOMAINS103 | "Each applies to the exact domain set its parameters declare under semantic roles and operators; an operand that also carries a policy or predicate domain is cast to that set first or has its own declaration." | Indexed families | Family operators apply to exact parameter domain sets; extra policy/predicate on operands requires cast or dedicated declaration | at the operation | 5
DOMAINS104 | "One declaration owns the entire family's route set." | Indexed families | Single family declaration must own all establishment routes for the family | not stated | 5
DOMAINS105 | "Instantiation substitutes indices into those routes; it cannot append routes or create a per-application registry." | Indexed families | Instantiation may substitute into routes only; must not add routes or per-app registries | not stated | 5
DOMAINS106 | "Closed index expressions evaluate under canonical static identity." | Indexed families | Closed index expressions must evaluate per canonical static identity rules | not stated | 8
DOMAINS107 | "Open indices preserve their selected operation and structural expression under the fixed core profile." | Indexed families | Open indices must preserve selected operation and structure under fixed core profile | not stated | 8
DOMAINS108 | "Compatibility with an expected index is a verification condition." | Indexed families | Index compatibility with expected index must be proved as a verification condition | not stated | 8
DOMAINS109 | "The domain selects no hidden algebra and adds no `using` clause." | Indexed families | Indexed families must not imply hidden algebras or implicit using clauses | not stated | 5
DOMAINS110 | "A canonical primitive operation needs no extra user conformance to receive its fixed simplifications." | Indexed families | Canonical primitives get simplifications without extra user conformance | not stated | 5
DOMAINS111 | "Preserve qualified carriers such as `u8 in Wrapping` and `u8 in Saturating` and each exact operation identity." | Indexed families | Specialization must preserve qualified carriers and exact operation identity | not stated | 5
DOMAINS112 | "A predicate-only qualification grants no arithmetic law." | Indexed families | Predicate-only domain qualification must not confer arithmetic laws | not stated | 5
DOMAINS113 | "Its default 64-term search allowance is configurable through root Build proof-work policy, never a domain parameter." | Indexed families | Distributivity search allowance is Build policy, not a domain parameter | not stated | 9
DOMAINS114 | "An unresolved comparison reports missing evidence or the exhausted allowance, not mathematical inequality." | Indexed families | Failed index compatibility must diagnose missing evidence or exhausted allowance, not inequality | not stated | 8
DOMAINS115 | "Carry successful evidence through specialization and artifact consumption." | Indexed families | Proved index compatibility evidence must flow through specialization and artifact use | not stated | 8
DOMAINS116 | "Exact conformance matching continues to use keys, so authors needing identical applications use a common expression or alias." | Indexed families | Conformance matching uses keys; identical applications require same expression or alias | not stated | 7
DOMAINS117 | "No builtin `String` or `Bytes` type, encoding intrinsic, or implicit encoding qualification is required." | Byte containers and encoding domains | Compiler must not require builtin string/bytes types or implicit encoding qualification | not stated | 6
DOMAINS118 | "Quoted literals supply raw bytes; an explicit qualification must establish the selected library predicate even when the known bytes permit compile-time proof." | Byte containers and encoding domains | Encoding qualification on literals requires explicit establishment of library predicate | not stated | 6
DOMAINS119 | "Validation establishes the exact predicate once; subsequent operations preserve it through their contracts instead of implicitly rescanning." | Byte containers and encoding domains | Encoding validation proves predicate once; later ops preserve via contracts without implicit rescan | not stated | 6
DOMAINS120 | "Reading a byte does not break the encoding, while a slice preserving UTF-8 must prove codepoint-boundary endpoints." | Byte containers and encoding domains | Single-byte read preserves encoding fact; UTF-8 slices need codepoint-boundary endpoint proofs | at the operation | 6
DOMAINS121 | "Mutation must preserve or re-establish the facts on affected places." | Byte containers and encoding domains | Mutations must preserve or re-establish encoding facts on touched storage | at the operation | 6
DOMAINS122 | "this includes loan handback after a call, even when the callee receives a view from which a cast removed the encoding qualification." | Byte containers and encoding domains | Loan handback must re-establish encoding obligations when view had encoding removed for callee | not stated | 2
DOMAINS123 | "An additional entry-only fact over otherwise unqualified storage remains distinct from a declared storage obligation." | Byte containers and encoding domains | Entry-only encoding facts must not be confused with declared storage-domain obligations | not stated | 6
DOMAINS124 | "Fixed capacities do not define different meanings for one normalized byte-domain name." | Byte containers and encoding domains | Same normalized byte-domain name must mean the same across fixed capacities | not stated | 6
DOMAINS125 | "Declarations over different carriers may share that name when their normalized facts agree; differing facts reject rather than selecting a declaration by order." | Byte containers and encoding domains | Shared domain name across carriers allowed only when normalized facts agree; else reject | not stated | 6
DOMAINS126 | "Construction of an explicit variable-fill carrier such as `FixedVec<u8, N> in D` proves its live byte length fits its capacity; it neither truncates nor defers capacity failure." | Byte containers and encoding domains | Variable-fill construction must prove live length within capacity at construction; no truncate or defer | not stated | 6
DOMAINS127 | "A raw `[u8; N] in D` remains an exact length-`N` fixed array and never acquires live-length storage from `D`." | Byte containers and encoding domains | Fixed array with encoding domain stays exact length N; domain must not imply variable live length | not stated | 6
DOMAINS128 | "Matching the successful case imports its established facts; matching does not repeat validation." | Byte containers and encoding domains | Sum-case match imports established encoding facts without re-validation | at the operation | 1
DOMAINS129 | "Wire decoding ordinarily supplies structure and raw bytes, not encoding facts." | Byte containers and encoding domains | Decode does not establish encoding facts unless separately requested | not stated | 6
DOMAINS130 | "A schema may explicitly request encoding validation and include its failure in the decode outcome; it is not the default." | Byte containers and encoding domains | Encoding validation on decode is opt-in via schema, not default | not stated | 6
DOMAINS131 | "Encoding validity never proves external authority." | Byte containers and encoding domains | Encoding predicate proof must not be treated as external authority proof | not stated | 8
DOMAINS132 | "`=D` | carries `D` | carries `D`" | Domain-only casts | `=D` keeps domain `D` on operand and result static types | at the operation | 5
DOMAINS133 | "`-D` | carries `D` | does not carry `D`" | Domain-only casts | `-D` removes domain `D` from result static type when operand carries `D` | at the operation | 5
DOMAINS134 | "`+D` | does not carry `D` | carries `D`" | Domain-only casts | `+D` adds domain `D` to result when operand lacks `D` on static type | at the operation | 5
DOMAINS135 | "Field projection and pattern matching are not operators and keep their own rules." | Operands | Field projection and pattern matching must not use operator resolution rules | at the operation | 1
DOMAINS136 | "Named machine and requirement calls may overload by result domain. Their dispatch projection contains normalized semantic-role contributions, routed provenance, access-policy atoms, and empty explicit tags." | Result-domain overloads | Overload dispatch projection must include only those normalized components | at a call | 3
DOMAINS137 | "A policy operator written directly reads no qualification and contributes to no role." | Semantic roles and operators | Direct policy-operator syntax must ignore operand qualification and semantic roles | at the operation | 5
DOMAINS138 | "A local proof that two policies coincide on a bounded operation does not make them globally equal." | Semantic roles and operators | Local policy coincidence proof must not unify policies for general operator resolution | not stated | 8
DOMAINS139 | "Adding or removing conjuncts changes the normalized requirements and guarantees; compatibility checks attribute the consequences to affected callers, implementations, and consumers." | Aliases and identity | Alias conjunct changes must update normalized requirements and re-check dependent compatibility | not stated | 5
DOMAINS140 | "Type indices use normalized type identity and substitute structurally at generic calls; different applications have no implicit variance relationship." | Indexed families | Type-index substitution uses normalized identity; distinct applications have no implicit variance | at a call | 7
DOMAINS141 | "Const indices use canonical value identity, not authored record-field order." | Indexed families | Const-index identity must use canonical value identity, not source field order | not stated | 8
DOMAINS142 | "Commutativity is not required to form an indexed family." | Indexed families | Indexed-family formation must not require commutativity proof | not stated | 5
DOMAINS143 | "An annotated `let` is no exception, and the same rule governs arguments, field initializers, returns and arm results:" | Literals and expected types | Domain-match rule applies equally to annotated let, arguments, field inits, returns, and arm results | not stated | 5

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"The rule is uniform: a predicate-only domain is named in the same way as a unit, an arithmetic policy or a routed domain." | All domain kinds use the same cast-entry syntax and completeness rules without exceptions. | "The same way" means only naming syntax is shared; establishment obligations may still differ by kind (as the `+D` table states). |
"Has neither predicates nor establishment routes | None beyond carrier validity; ordinary ownership rules still apply." | `+D` on such domains requires only that the carrier type is valid; no extra proof. | "Carrier validity" might include additional carrier-specific checks not listed in this table cell. |
"Access policies survive whenever they are needed to prevent authority amplification, and a view that cannot retain them rejects." | Compiler must statically determine when policies are needed and reject casts/views that drop them. | "Whenever they are needed" might include cases decided only with interprocedural or dynamic custody reasoning not spelled here. |
"Through a shared reference any removal a by-value cast permits is accepted and nothing is owed, because nothing can be written." | Shared-ref `-D` never creates re-establishment debt regardless of predicate or route obligations on the referent. | By-value `-D` might still be rejected for authority reasons; shared ref only accepts removals that by-value would accept, and "nothing is owed" applies only to those. |
"The end of the loan is that `+D`: it is not written, and it is checked by the same establishment rules for whatever the place then holds." | At loan end the compiler checks the place's current value against `+D` rules as if an implicit cast occurred. | The check might use the value at last write by callee only, or only at explicit handback point; "when the loan ends" timing is not further specified. |
"Without an expected result, lookup selects the empty projection." | Calls with no contextual expected result type use overloads whose result projection is empty. | "Without an expected result" might mean only certain syntactic positions; others might still infer an expected type from context not described in this sentence. |
"Equal projections on the same path and parameter signature are declaration-site duplicates, even if written results differ by predicate refinement." | Predicate refinement on the written result type cannot distinguish overloads with equal projections. | Duplicates are diagnosed even when predicate refinements would make result types incompatible under ordinary type equality. |
"Open indices preserve their selected operation and structural expression under the fixed core profile." | Open index forms stay syntactically stable in the artifact under core profile. | "Preserve" might mean only that rewriting is forbidden, or that evaluation must not simplify them; effect on compatibility checking is not stated in this sentence. |
"Differing facts reject rather than selecting a declaration by order." | Overload or declaration choice never uses source order when normalized facts differ. | Rejection might apply only to name sharing across carriers, not to all declaration conflicts involving byte domains. |
"An additional entry-only fact over otherwise unqualified storage remains distinct from a declared storage obligation." | Entry-only facts and `in D` on storage are different proof tracks that must not be merged. | Entry-only facts might still imply some of the same checks at use sites as declared obligations; "distinct" addresses identity only, not checking overlap. |

## Terms used without definition in this chapter

- named-domain-casts / NAMED-DOMAIN-CASTS
- design record
- predicate-only domain
- unit (domain sense)
- arithmetic policy
- routed domain
- carrier (type carrier)
- call boundary
- join (control-flow join)
- overload selection
- scale changes (units)
- policy-bearing conversions
- named machines
- compiler-owned domain
- inherited parent (domain hierarchy)
- establishment routes
- authorized evidence
- cleanup selection
- access policies
- authority amplification
- view (mutable/shared reference sense)
- carry demands
- content custody / content projection
- multiplicity / divisibility
- data declaration's default domain
- mutable view (cross-link)
- exact compiler-derived transformation
- representability (narrowing)
- carrier-exact membership
- numeric landing rules / fractional intermediate
- already-typed carrier coercions
- floating landing
- whole-type cast targets (retired spellings)
- result-domain overloads
- dispatch projection
- routed provenance
- access-policy atoms
- explicit tags
- predicate refinement
- transparent alias
- compatible subjects
- sorting, deduplication, identity formation
- compiler-owned atoms / positive carry permissions
- normalized identity
- deterministic normalizer
- semantic interface identity
- specialization keys
- physical ABI
- canonical proof-static index
- operator conformances
- route set
- per-application registry
- lifetime and static-machine domain indices
- canonical static identity
- fixed core profile
- checked algebra laws
- literal connections
- neutral-element rules
- distributivity / 64-term search allowance
- root Build proof-work policy
- metadata erasure
- runtime witnesses
- view geometry
- byte container
- encoding-validity domain
- codec/operation contracts
- encoding intrinsic
- implicit encoding qualification
- library predicate
- encoding recognition and preservation
- codepoint-boundary endpoints
- declared local and field qualifications / lent storage
- entry-only fact
- abstract codepoint-text quotient
- normalized byte-domain name
- variable-fill carrier / FixedVec
- live byte length
- wire decoding
- schema (decode)
- codec contracts
- external authority
- mutable-view-reestablishment (named decision)
- invariant windows
- contract-based repair
- representation recasts
- temporary qualified views
- policy operators / policy domains
- Wrapping, Saturating, Trapping
- semantic roles (table entries)
- relational contract
- flow refinements
- declaration-site duplicates
- entailment (stronger entailment)
- domain theory
- Unit (const index type)
- Frame (type index example)
- using clause (absence)
- user conformance
- commutativity (indexed family)
- artifact consumption

CHAPTER COMPLETE
