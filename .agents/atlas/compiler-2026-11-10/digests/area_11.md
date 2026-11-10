# Area 11: Proof, proof admission, psi-compiler, checked trees

Source: records `unit_073.md`, `unit_074.md`, `unit_074_gap_018.md`, `unit_074_gap_019.md`, `unit_075.md`, `unit_076.md`, `unit_077.md`, `unit_078.md` (8 files, all read in full). Checkout `worker-23` at `457adf4a11f`.

Path shorthand (all under `omega-rust/psi/compilation/`):
`adm/` = `proof/admission/src/`, `adm-tests/` = `proof/admission/tests/`, `prf/` = `proof/src/`, `pc/` = `psi-compiler/src/`, `tp/` = `psi-compiler/terminal-production/src/`, `ct/` = `representations/checked-trees/src/`, `fa/` = `representations/checked-trees/facts/src/`, `lp/` = `representations/lowered-psi/src/`.
`bd` = `adm/bounded_denotation.rs`, `md` = `prf/mathematical_declarations.rs`, `c&p` = `fa/contexts_and_places.rs`, `c&o` = `ct/contract_and_obligation_facts.rs`.

Words kept from the records: "read", "grepped", "inferred", "suspected", "open", "stated", "traced". Where I opened the code myself the line says "(checked)" and the result is in "What I checked myself".

---

## What this area does

This area holds the part of the Psi compiler that decides whether a value provably fits its declared range, the small proof checker that re-decides those decisions, the tables that carry every checking result forward to lowering, and the thin facade that sequences parsing, checking and Terminal production. The proof crate walks a typed program, lists "this value must fit this constrained type" obligations, and tries to discharge each one. For integer ranges it first builds an explicit certificate and hands it to the proof-admission crate; when it cannot build one it falls back to older interval reasoning that the admission crate never sees (`prf/checker.rs:373-399`, unit 76). Proof admission checks a certificate rule by rule and then rebuilds it as a term in a small dependent type theory and type-checks that term. The checked-trees and facts crates are mostly passive data: about 50 fact tables plus a few lookups and predicates. psi-compiler and terminal-production are orchestration and caches.

**proof-admission (`proof/admission`, 54,654 lines, about half test code).** In: a `Proposition` goal, assumptions, "semantic axioms", a `ProofNode` certificate tree, a `PropositionContext`. Out: `AcceptedFact`/`CertificateAcceptance` or an error.
- `lib.rs`, `kernel.rs`: `verify_obligation` (`adm/lib.rs:94`) has three routes. `KernelDerived` calls `decide_primitive` (`adm/kernel.rs:18`, a single match that defaults to `false`, `:84`). `CertificateDerived` calls `accept_certificate_with_machine_parameters`. `Admitted` always returns an error (`adm/lib.rs:145-158`); `SourceChecked` also errors (`:159-161`).
- `proof.rs`: checks the certificate tree with 28 rules (`adm/proof.rs:32-61`, exhaustive dispatch `:2359-2421`), requires `proof.conclusion == goal` (`:206`), then asks the mathematical core to re-decide (`:220`).
- `integer_rules.rs`, `integer_affine.rs`: witness checkers for integer bound rules: closed big-integer evaluation under budgets, cast chains, forbidden division roots, shift chains, affine chains and bound mapping.
- `predicate_denotation.rs`: normalises boolean predicates and substitutes cited value equalities under a 4096-node, depth-64 budget (`:31`, `:35`).
- `bounded_denotation.rs` (18,126 lines): rebuilds a checked certificate as a core term. Where it cannot build a derivation for a rule instance it declares that instance as an opaque assumption (`bd:8640-8652`).
- `mathematical_core.rs`, `mathematical_typing.rs`: the type theory (Pi, Sigma, Two, Id, W, Empty, Squash, Box, universe-polymorphic signatures), its type checker and conversion, a certificate verifier, and two fixed schemes (indexed W families, quotients).
- `classicality.rs`: labels each rule constructive, decidable, trusted or classical.

**proof (`proof`, 25,419 lines).** In: a typed program (`dyn validation::CheckingInput`). Out: `ProofPlan` (obligations), `ProofDecisions` or diagnostics, and `CheckedMathematics`.
- `obligations.rs`: builds the obligation list and a string key per obligation.
- `checker.rs`: discharges obligations (`check_proof_plan_with_decisions`, `:6468`).
- `mathematical_declarations.rs` (12,715 lines, nested inline modules): turns authored mathematical definitions, indexed families and source applications into admission-core terms and checks the whole signature once (`md:12386`).
- `lemmas.rs`, `boundary.rs`, `derivation_store.rs`, `lib.rs::proof_surface`: vocabulary and a cache with no production users (see "Unused").

**psi-compiler (`psi-compiler`, 2,708 lines).** In: loaded source files, Build selections, previously retained frontends. Out: parsed pages, a `SourceContinuation`/`CompletedSource`, a `CheckedSource` (`Arc<CheckedTrees>`), and shared Terminal productions. It holds three "have I seen this exact input" caches.

**psi-terminal-production (`psi-compiler/terminal-production`, 1,921 lines).** In: `CheckedTrees` plus roots and options. Out: a `ProducedTerminalProgram` (Psi 06 -> 07 -> 08, `tp/lib.rs:431`) with Build companion records keyed to the artifact's custody identity.

**checked-trees (`representations/checked-trees`, 27,578 lines).** `CheckedTrees { typed, facts }` (`ct/lib.rs:689-693`); `CheckFacts` has 52 fields (`ct/facts.rs:22-79`). Unit 78 lists the only code that makes judgments: the call-plan admission table `CheckedUnitCallPlan::new` (`ct/flow_terminal_units.rs:1113-1279`), the "normal continuation" walks, `CallTransfers` (`ct/flow_ownership_and_roots.rs:768-1608`), the content-claim frontier (`ct/facts.rs:971-1069`), `declared_contract_fact` (`ct/operators_and_proof.rs:2570-2642`), the provider/trust assemblers, and (unit 77) per-target obligation discharge and crash-plan constructors in `c&o`.

**facts (`representations/checked-trees/facts`, 6,167 lines).** `FactPlan` (`fa/lib.rs:1135`): 17 arenas of places, place segments, facts, per-program-point contexts, predicates and ranges, with place construction from expressions and place-equality queries.

**lowered-psi (`representations/lowered-psi`, 266 lines).** `LoweredPsi` (`lp/lib.rs:233-248`): the unsealed carrier between Psi 06, 07 and 08, plus source-custody side tables.

**symbol-resolved-trees.** Only `examples/diagnostic_name_allocations.rs` (50 lines) is in this area: an allocation-count example. The crate's `src/` is not in these units.

---

## How it decides

### Keyed on identity, value, position or structure

- **Certificate rules (admission).** Assumption and axiom citations are by index into the supplied slice, then proposition equality modulo integer-math normalisation (`adm/proof.rs:1629`, `:1646`). Rule conclusions compare by full `Proposition` equality (`adm/proof.rs:1667-1757`). Integer order rules require a `Fixed`, non-address carrier (`adm/proof.rs:271-272`, `:1420`, `:1502`).
- **Witness checkers.** Definition axioms are cited by index and must be strictly increasing (`adm/integer_rules.rs:535-541`, `:2444-2450`; `adm/integer_affine.rs:2986-2992`). The affine checker is an ordered dispatcher of nine recognisers; the first that returns a form wins (`adm/integer_affine.rs:2873-3098`). Bound conversion picks a route from the shape of `endpoint_steps` or the target constructor (`adm/integer_affine.rs:428-451`). Evidence order inside a `Conjunction` is significant (gap 18, tests `adm/integer_affine.rs:3874-3878`, `:4042-4062`).
- **Core typing.** Variables by de Bruijn index, constants by declaration index and level arity (`adm/mathematical_typing.rs:87-94`, `:323`). Conversion matches a fixed list of head-constructor pairs and returns `false` otherwise (`:839-1358`). At a shared type whose sort is `Strict`, any two terms convert (`:1375-1378`).
- **Denotation.** Atoms and terms are interned by whole value (`bd:4467`, `:4506-4510`, `:4756-4787`); structural equality decides orientation (`bd:5253`). Bound rules try derivers in a fixed order, first `Some` wins, else an opaque rule instance (`bd:8310-8398`).
- **Obligations.** An obligation exists only when the declared type is `Constrained` after peeling `Reference` (`prf/obligations.rs:3849-3868`). Loop index + 1 is the certificate/obligation/value id (`prf/checker.rs:6487`).
- **Checker, float path.** Places are compared by symbol lists (`prf/checker.rs:3451`, `:3982`).
- **Mathematical declarations.** Symbols are the primary key (`HashMap<SymbolHandle, ..>` tables, `md:11863-11927`); type correspondence is by type identity, then kernel conversion (`md:4484`, `:4453`).
- **psi-compiler caches.** Reuse is keyed on `Arc::ptr_eq` of the checked program plus field equality (`pc/lib.rs:77-78`, `pc/source_checking.rs:866-870`, `pc/source_compilation.rs:37-47`). Retained layout input compares the whole `TypedTrees` (`pc/source_checking.rs:448`).
- **Terminal production.** Companion records are released only when `self.identity == artifact.custody_identity()` (`tp/build_bindings.rs:343-368`).
- **facts.** `places_equal` is structural: equal root handle and equal segment slice (`fa/lib.rs:1819-1825`). Evidence merge requires equal origin, source symbol, requirement symbol and issuer (`fa/evidence.rs:828`).
- **checked-trees.** Most lookups are by symbol handles and `(statement, ordinal)` coordinates. Duplicates are handled three ways: some lookups require uniqueness (`unique_*`, `root_at`, `term_at`, `declared_contract_fact`), some take the first row (`ct/flow.rs:1339`, `ct/values_and_views.rs:2272`, 14 `for_machine` copies), one takes the last (`ct/operators_and_proof.rs:2659`) (unit 78).
- **CallTransfers.** Branch merges take the minimum transfer count over alternatives (`ct/flow_ownership_and_roots.rs:1327-1331`, `:1137`, `:912`).

### Keyed on source spelling

I count **68 distinct sites** from the records. Counting rule: one function, or one cited line group doing one comparison, counts once; diagnostic text and plain `String` data fields are not counted.

**proof (30)**
- `prf/checker.rs` (10): guard matching by path-segment strings and name-path members (`:4914-4925`, `:4858-4861`); `ensures_witness_bounds` matched by `display_name` equality (`:5200`, `:5215`, `:5329`, three sites); overwrite detection by `display_name` (`:472`); `member_paths_may_alias` prefix compare over `Vec<String>` (`:280`); write/read sets as dotted string paths (`:209-216`); named-constraint names `"exact"`, `"finite"`, `"non_negative"`, `"positive"`, `"wrapping"` (`:6141-6153`); the constraint-name literal table (`:6009-6019`); field found by member name across all machines (`:5459-5473`).
- `prf/lemmas.rs` (1): `proves_element` compares the predicate name string (`:145`).
- `md` (9): level binder by name when the symbol is invalid (`:2702-2705`); universe literal parsed from spelling (`:2365`); `scope_term_for_name` (`:3794`); `name_carrier` fallback (`:1059-1061`); `member_carrier` by `field.name` (`:1033`, `:1040`); unresolved `Name` in `elaborate_expression_term` (`:12241-12249`); const index atoms decoded with `BigInt::from_decimal_str(name.as_str())` (`:11489-11499`); `open_term` dependency scan by `entry.name == name` (`:3152-3158`); declaration parameter types and bodies kept as rendered strings (`:54`, `:62`, `:191`).
- `prf/obligations.rs` (10): `match name.as_str()` over `"u8"|"u16"|"u32"|"u64"` (`:1607-1613`); payload field by `field.symbol == member_symbol || field.name == *member_name` (`:724-725`); key falls back to `display_path` (`:2320-2323`); spelled members for unresolved names (`:2326-2346`); member by name when no symbol (`:2488-2492`); `format!("{:?}", ..)` of enums inside the key (`:2437-2438`, `:2467`, `:2503`, `:2609`, `:2651`, `:2659`, one site); `Name` vs `Name` by member list in `expressions_equivalent_for_precondition` (`:1274-1277`); witness places as strings retired by `frame_paths_overlap` (`:452`, `:464`); domain constraint by name in `from_node` (`:3229`); named `non_negative`/`positive` lift (`:3706`).

**proof-admission (1)**
- `adm/mathematical_core.rs:3226-3230`: `scheme_dsl` resolves `&'static str` variable names by `rposition`; a free name panics (`:3229`). This is for the crate's own fixed schemes, not program input (unit 74).

**psi-compiler (4)**
- Library-slice digest hashes Debug text: `format!("{authority:?}")` (`pc/source_checking.rs:190`), `format!("{demand:?}")` (`:197`), `{:?}` of `application_identity` and `composition_mode` (`:299`, `:335`, one site). The key only feeds counters (`:771-773`).
- Build-helper identity compared as a string (`pc/source_compilation.rs:57`).

**psi-terminal-production (11)**
- `tp/build_bindings.rs` (10): `call.target.as_str()` against `"await_provider"`, `"prepare"`, `"bind"`, `"bind_description"`, `"bind_native"`, `"select"`, `"select_provider"`, `"exclude"` (`:418-425`); protocol signature names in three wrappers (`:609-623`, `:886-890`, `:962-968`); protocol identity by name `"BuildFilesystemOperations"`, `"BuildProductOperations"`, `"BuildRequestOperations"` (`:605`, `:884`, `:959`); provider declaration by `display_path(symbol,"::") == declaration_path` (`:769-770`); machine root by `machine.name.as_str() == name` (`:36`); locator type by `normalized_type_identity_text` equality (`:201-203`).
- `tp/lib.rs:275`: `declaration.declaring_application == outer` (portable application identity text).

**facts (10)**
- `places_match` falls back to equal canonical label strings (`fa/lib.rs:1859-1867`); the label uses the bare `symbols.name(symbol)` (`c&p:1337`) (checked).
- Field segment found by name when the symbol is invalid (`fa/lib.rs:1459-1466`); member by name lookup (`fa/lib.rs:1473-1481`).
- `resolve_place_member_symbol` by `field.name.as_str() == member_name` (`c&p:1584`, `:1594`); `effective_member_symbol` prefers the name-resolved symbol over the stored one (`c&p:1208-1223`); `resolve_member_symbol_from_receiver` by member name and `owned.name` (`c&p:1226`, `:1253`); `data_member_symbol_by_name` (`c&p:1574`); `symbol_type_reference` for a local requires `symbols.name(symbol) == local.name` (`c&p:1414`).
- Write-frame ancestor dropping by string prefix plus `.` or `[` (`c&p:1796-1802`).
- `proves_proposition_label` compares a label string (`c&p:1007`).

**checked-trees (12)**
- Crash predicate identity is canonical bytes whose leaves are spelled names, member names, call targets and `Opaque` display text (`c&o:2353-2400`); equality, order and hash use only those bytes (`c&o:2501-2528`).
- Contract identity hashes sorted service-name strings (`c&o:1059-1071`), type text via `type_key` (`c&o:1148`), and membership `value`/`domain` strings (`c&o:1176-1187`).
- Requirement rows matched to closed applications by rendered identity text: `ct/providers_and_trust.rs:73`, `:164-166`, `:299`, `:378`, and `provider_reach` at `:104`.
- `canonical_suspension_crossing_id` hashes `display_path` and legacy machine/state names (`ct/facts.rs:2858-2930`).
- `CheckedUnitStructuralTypePlan::eq` compares `identity` and native identity strings (`ct/flow_terminal.rs:2044-2058`).
- `StateLocalFacts::named` looks a local up by name (`ct/facts.rs:3467-3478`).

Not counted but recorded: the guard test `checked_trees_carry_no_string_identity_fields` greps six literal field spellings (`ct/lib.rs:131-150`); unit 78 lists text-identity fields it does not catch (`ct/facts.rs:2304`, `:2307`, `:2161-2163`, `:2549-2553`, `:1204`, `:1232-1236`, `:2089`, `:2136`, `:3087-3095`; `ct/operators_and_proof.rs:2372`, `:1506`).

---

## The same job in more than one place

Counts are of sites the records cite.

### Cross-record pairs I opened

- **Structural expression equality for guards, 2 copies in one crate.** `expressions_equivalent_for_proof` (`prf/checker.rs:4832`, unit 76) and `expressions_equivalent_for_precondition` (`prf/obligations.rs:1255`, unit 77). Same job, near line-for-line (checked). Differences: the checker copy first compares flattened path-segment strings (`:4841-4846`) and has an `Indexed` arm (`:4895-4899`); the obligations copy has a `Binary` arm (`:1312-1316`) and no `Indexed` arm. Unit 76 also "suspected" a third in typed-trees (`values.rs:4006`) and `pipeline/03.../equality_and_exhaustiveness.rs:1726`; not read by anyone here. `prf/obligations.rs:2348` (`expression_identity`) is a further full-expression walker that produces key strings.
- **Lemma vocabulary, 2 copies in two crates.** `prf/lemmas.rs:52-93` (five lemmas `IndexInBounds`, `NonEmptyHasFirst`, `WindowLength`, `WindowSubrange`, `TailLengthDecreases`) and `QuantifiedBound {Zero, Literal(i64), Length}` (`prf/lemmas.rs:196`), against `ProofLemmaKind` with the same five names (`ct/operators_and_proof.rs:2059-2066`) and `QuantifiedBoundFact {Zero, Literal(i64), Length}` (`:2083-2088`). Same vocabulary (checked). Difference: the proof copy has logic and keys predicates by `String` (`prf/lemmas.rs:123`, `:145`); the checked-trees copy is data only and keys by `predicate_symbol: SymbolHandle` (`:2077`). Unit 76 called the checked-trees types "different types"; unit 78 listed them without linking.
- **Rule relation checks: shared, not duplicated.** Unit 73 "suspected" that `adm/proof.rs` rule checkers repeat the witness matching in `bd`. Unit 75 grepped the calls. I opened `bd:8315`: the elaborator calls `integer_bound_rules::affine_bound_relation` from `adm/proof.rs:475` and only then tries its own derivers. One implementation, two callers. What `bd` adds is a second, narrower shape recognition to build a real derivation (`bd:8323-8383`).
- **"Operator is builtin", 2 rules.** `has_builtin_operators` (`ct/facts.rs:244-268`) treats any status other than `BuiltinFallback` as not builtin; `resolved_through_builtin_fallback` (`ct/operators_and_proof.rs:477-486`) also accepts `Missing` with zero candidates (checked). A third site passes an always-true builtin check at discharge (`c&o:189`, unit 77).

### proof-admission

- **Replay a chain of cited definitions, 4 copies:** `adm/integer_rules.rs:535` (cast), `:1681` (forbidden root), `:2444` (shift), `adm/integer_affine.rs:2986` (affine). Same `windows(2)` increasing-index check, cycle check, target equality.
- **"Landed literal by earlier axiom index", 3 copies:** `adm/integer_affine.rs:3192`, `adm/integer_rules.rs:1824`, `:2549`.
- **Width list `8 | 16 | 32 | 64`, 2 copies:** `adm/integer_rules.rs:445`, `adm/integer_affine.rs:2552` (re-spelled, not calling `fixed_native`).
- **Correlated bound derivers, 3 near-copies in `bd`:** add `bd:195-342`, subtract `bd:13819-13964`, multiply `bd:11375-11632`. Add and subtract share `BinaryBoundOperation` (`bd:4084`); multiply does not.
- **Interval propagation through endpoint steps, 3 sites:** `adm/integer_affine.rs:404`, `:1967`, `adm/integer_rules.rs:744`.
- **Certificate verification body, 2 copies:** `adm/mathematical_core.rs:28-67` (batch) and `:479-498`.
- **Level equality, 2 notions:** structural `PartialEq` (`adm/mathematical_core.rs:4012`) and normal-form `levels_equal` (`:4227`). Tests pin that they differ (`:5726-5730`).
- **Explicit-stack drivers:** six in `bd` (`:3774`, `:1915`, `:10674`, `:5614`, `:6096`, `:7513`), plus `adm/mathematical_typing.rs:2284`, `adm/proof.rs:2092-2320`, `adm/mathematical_core.rs:1138`.
- **BigInt <-> `IntegerValue` converters, 4:** `bd:1314`, `:15385`, `:15252`, `:514-525`.
- **Apply an order relation to two terms, 3:** `bd:4865`, `:9169`, `:3130`. Numeral to term, 2: `bd:1393`, `:9179`.
- **Equality orientation, 3:** `bd:5253`, `:5229-5239`, `:5009-5024`.
- **Closed-goal inhabitation, 3:** `bd:6661`, `:2184`, `:12562`.
- **Per-operation `Law` enums, 5 plus one global:** `bd:62`, `:1298`, `:13416`, `:11131`, `:8965`, `:15366`.
- **`Term::EmptyElim` built inline 10 times** while helper `empty_elimination` exists at `bd:9648` (grep count, unit 73).
- **Tagged-sum builders:** `bd:7715`, `:9694`, `:5991-6067`, `:8654`, `:9736`. "Suspected same job; not compared line by line."
- **Which rules cite semantic axioms, 2 lists:** `adm/classicality.rs:53` and the elaborator's recording at `bd:7826`, `:8626`. "Suspected same set", not compared.
- **Integer normal forms:** `adm/integer_rules.rs:3069` (`Normalizer`), `adm/proof.rs:896` (`integer_math_normalization`), `bd:12856-12912` (`source_normal_integer`). Unit 74 "suspected"; nobody compared them.
- **Mirror rule functions:** `add_order::check` (`adm/proof.rs:250`) and `subtract_order::check` (`:2054`); `affine_bound_relation` and `cast_bound_relation` share the "Truth root, else convert" structure (`:486-497`, `:724-735`).
- **Test helpers:** term builders in `adm/mathematical_core.rs:9127-9452` and again in `adm-tests/common/mod.rs:27-191`; `conv!` macro 3 copies (`adm/mathematical_core.rs:4861`, `:8036`, `:8801`); three ways to build a test context in `adm/integer_affine.rs` (`value_context`, `from_value_types`, local `context` at `:6020-6026`); `step_type` built by hand (`adm-tests/common/mod.rs:200`) beside `w_step_type` (`adm/mathematical_typing.rs:385`).

### proof

- **Five `check_bounded_*` functions repeat one pattern:** `prf/checker.rs:332`, `:514`, `:591`, `:656`, `:729`.
- **Condition-shape walkers (`== true`, `&&`, literal comparison), 5 in `checker.rs`:** `:1335`, `:1126`, `:4720`, `:6297`, `:3639`. `:1335` and `:1126` differ only in the equality test used and `!` handling. Unit 77 records two more over the same shapes: `prf/obligations.rs:679` and `:535`.
- **"Strongest lower/upper anchor" loop, 2:** `prf/checker.rs:1275-1303`, `:1530-1556`.
- **Literal-bound helpers, 2 pairs:** `prf/checker.rs:1415`/`:1434` and `:4930`/`:4949`.
- **`CallFrameResolver::new(program)` built separately:** `prf/checker.rs:3366`, `:3526`, `:3969`, `:5390`.
- **Type-reference descent (Reference/Constrained/FixedArray/Slice/Generic), 9 walkers:** `md:10348`, `:10434`, `:10531`, `:4602`, `:6453`; `prf/lib.rs:431`, `:647`; `prf/obligations.rs:243`, `:3849`.
- **Kernel context construction that clones the whole declaration vector:** `md:869-873`, `:5252-5259`, `:6355-6358`, `:6784-6787`, `:6859-6862`, `:7303-7306`, `:7712-7715`, `:11448-11455`.
- **Carrier classification:** `md:912`, `:1099`, `:12653`; a private `operand_carrier` in `pipeline/05.../shared.rs:2492` is a "suspected" third.
- **`ProofObligationKind` mirror:** `fa/evidence.rs:868` repeats the seven variant names of `ProofObligation` (`prf/obligations.rs:3353`).

### psi-compiler and terminal-production

- **"Find a prior equal input" cache, 3 copies:** `pc/lib.rs:103-125`, `pc/source_checking.rs:861-874`, `pc/source_compilation.rs:300-341`.
- **"Pick pending machines, resolve bodies, type bodies, repeat" loop, 3 copies:** `pc/source_compilation.rs:838-871`, `:892`, `:1086`.
- **Build protocol capture wrappers, 3 copies over one generic:** `tp/build_bindings.rs:598`, `:879`, `:954` over `:652`; `capture` at `:387` re-implements a similar scan.
- **Stage label table kept parallel to an enum by hand:** `tp/lib.rs:333-364` (inferred).

### facts

- **Place identity, 3 notions:** structural (`fa/lib.rs:1819`, `:1709`), label-capable (`fa/lib.rs:1827`), write-frame path strings (`c&p:1796`).
- **Member symbol by name, 3 paths:** `c&p:1226`, `:1543`, `:1574`; `prf/obligations.rs:711` is a "suspected" further one.

### checked-trees

- **"Does control return normally", 7 implementations plus one stored bool:** `ct/flow_terminal_units.rs:287`, `:802`, `:2124`, `:1311`, `:2019`; `ct/flow_terminal.rs:1816`; `ct/values_and_views.rs:1231`; stored `ct/flow_terminal_units.rs:636`.
- **`iter().find(|x| x.machine == machine)`, about 14 copies:** `ct/facts.rs:366`, `:1736`, `:1936`, `:1968`, `:2029`, `:2053`, `:2683`, `:3272`; `ct/termination_and_cleanup_facts.rs:893`; `ct/flow_terminal.rs:2544`, `:289`, `:311`; `ct/flow_terminal_units.rs:1564`, `:1568`.
- **`KeyedRows` (`ct/lib.rs:258`) re-implemented by hand, 6:** `ct/operators_and_proof.rs:956-998`, `ct/values_and_views.rs:2146-2174`, `:1029`, `ct/flow.rs:417-566`, `ct/facts.rs:1300-1303`, `:3417-3480`.
- **Walks over `CheckedStructuralValueKind`, 4:** `ct/flow_ownership_and_roots.rs:1397-1508`, `ct/values_and_views.rs:1239-1338`, `:1342-1396`, `ct/facts.rs:1073-1142`.
- **Find a call by `(statement, ordinal)`, 3:** `ct/flow_ownership_and_roots.rs:390-415` (two), `ct/flow.rs:2126-2144` (also compares target and receiver).
- **Call coordinate triple spelled in 5 structs:** `ct/flow.rs:248-250`, `ct/flow_terminal.rs:1247`, `ct/operators_and_proof.rs:1729`, `ct/facts.rs:2730`, `lp/lib.rs:29`.
- **Scalar primitive lists, 3:** `ct/flow_terminal.rs:2418-2429` (no floats), `:2458-2471` (with floats), `ct/flow_terminal_units.rs:163-171` (U8..U64).
- **Dense vec by type reference with first-row fallback, 2:** `ct/facts.rs:2973`, `:3121`.
- **`(arena_index, generation)` ordering, 4, plus one reversed:** `ct/facts.rs:1447`, `:90`, `:452`, `ct/operators_and_proof.rs:1400`; `ct/flow.rs:441` uses `(generation, arena_index)`.
- **Per-target obligation discharge, 3 parallel shapes:** `c&o:173`, `:462`, `:765`.
- **Merge worker rows with handle shifting, 2:** `ct/termination_and_cleanup_facts.rs:47`, `ct/facts.rs:749`.
- **`same_effect` copies of `PartialEq`, 2:** `ct/flow_terminal.rs:259`, `:347`.
- **"Which place is this argument", 3:** `ct/flow_ownership_and_roots.rs:1027-1039`, `:694`, `:1556-1568`.
- **`Eligible | FreeUnitEffect` re-spelled, 3:** `ct/flow_terminal_units.rs:2604-2605`, `06.../scalar_lowering.rs:1069`, `06.../attached_unit.rs:1310` (grepped).
- **State-borrow lookups, 4:** `ct/admissibility_and_borrow.rs:2039`, `:2054`, `:2069`, `:2080`.
- **Root lookups in two parallel families:** `ct/values_and_views.rs:232-300` and `:1515-1552`; "sealed index or scan" twice `:2241-2258`, `:2278-2299`.
- **Closed-requirement derivation, 2:** `ct/providers_and_trust.rs:472`/`:484` and `:381-389`.
- **Rank coordinates, 3 families ("suspected", not compared):** `ct/termination_and_cleanup_facts.rs:637-721`, `ct/flow_terminal_units.rs:175`.

### Fixed-width integer lists across the area (my grouping)

Readers recorded these separately; no reader compared them: `adm/integer_rules.rs:445`, `adm/integer_affine.rs:2552`, `prf/checker.rs:1992` (I8..I64, U8..U64), `md:4222-4234`, `prf/obligations.rs:1607-1613` (unsigned only), `fa/predicates.rs:394-404` (unsigned only), and the three checked-trees lists above.

---

## Fixed lists of shapes

### proof-admission

- 28 proof rules (`adm/proof.rs:32-61`); the classifier match is exhaustive, the test array `ALL: [AcceptedProofRule; 28]` is a separate hand list (`adm/classicality.rs:70-99`).
- `primitive_derivation` tries exactly four primitive judgments (`adm/lib.rs:174-179`).
- Kernel carrier bound: only (literal, Value)/(Value, literal) pairs and the two math forms; the exclusive form only for address carriers (`adm/kernel.rs:93-101`, `:123-132`, `:146-151`).
- Closed evaluation supports Add/Subtract/Multiply/ShiftLeft only (`adm/integer_rules.rs:124-131`).
- Fixed native widths: `matches!(integer_type.bits(), 8 | 16 | 32 | 64)` (`adm/integer_rules.rs:445`); 128-bit and address carriers refused.
- Cast bound fallback accepts only `LessOrEqual` shapes (`adm/integer_rules.rs:702`, `:717`).
- Forbidden roots: chains of add/sub/mul with one literal sibling (`adm/integer_rules.rs:1765-1784`); conversion emits exactly `divisor <= -2 | 1 <= divisor | (divisor <= -1 & MIN+1 <= dividend)` (`:1638-1648`).
- Wrapping equality: `_ => return Ok(None)` for anything but Value, literal, wrapping add/sub/mul (`adm/integer_rules.rs:3484`).
- Affine dispatcher: nine recognisers in order (`adm/integer_affine.rs:2878-3098`); `apply_definition` handles a fixed list of operations and `None` for the rest (`:2267-2477`); "exactly one candidate must apply" (`:3061-3068`).
- Bound mapping: nine routes (`adm/integer_affine.rs:416-426`); direct add/sub evidence is `let [left_evidence, right_evidence] = parts.as_slice()` (`:917`, `:962`); direct multiply exactly four parts (`:1027`); correlated multiply exactly two (`:874`); shift-left four recognised evidence forms (`:1438-1440`).
- Remainder recogniser: fixed unsigned carrier, dividend Value or Integer, zero or one definition axiom (`adm/integer_affine.rs:1591-1619`).
- `bd` direct bounds: `let [left_bound, right_bound] = bounds.as_slice() else { return Ok(None) }` (`bd:4141-4146`); `affine_witness_root_expression` zero or one definition axiom (`bd:15203-15215`); correlated roots signed fixed carrier only, `[lower_bound, upper_bound]` exactly two (`bd:9929-9932`, `:9982`); divisor split hard-wired to `d <= -2`, `d == -1`, `d == 0`, `1 <= d` (`bd:9125-9161`); `disjunct` hard-codes indices 0, 1, "2+" (`bd:9736`).
- `bd` operation table: 24 integer operations, arity 1 or 2 (`bd:10872-10969`); `Children` holds at most three fixed children plus a slice (`bd:7186-7219`).
- Source integer theory: `closed_source_predicate` handles only `Id`, `<`/`<=`, `Sigma`, `Box (Empty->Empty)` (`bd:12571-12651`); normal form expands only add/sub/mul and distributes a multiply over an add on the right only (`bd:12856-12912`).
- Coercion and inhabitants: fixed pair lists (`bd:5614-5780`, `:6173-6269`).
- Weak-head reduction table (`adm/mathematical_core.rs:1177-1248`); `MAX_SUBTERMS = 4` (`:4377`); conversion head-pair list (`adm/mathematical_typing.rs:839-1358`).
- Equality transitivity: three proposition families, `_ => RulePremiseMismatch` (`adm/proof.rs:374-438`); `less_or_equal_transitivity` two (`:1239-1264`); `exact_definition_bound_relation` add/subtract with a Value on one side (`:553-612`); `lift_exact_integer_term` five term kinds (`:999-1027`); endpoint number 0 or 1 (`:1322-1335`).
- Predicate denotation: seven definition-subject kinds (`adm/predicate_denotation.rs:170-181`).
- Schemes: 5 indexed declarations (`adm/mathematical_core.rs:1328-1332`), 20 quotient declarations (`:2799-2818`).

### proof

- `fixed_integer_type`: I8..I64/U8..U64 only; no certificate for `Addr` (`prf/checker.rs:1992`).
- `closed_math_term`: literal, name with a literal value, binary add/sub/mul (`prf/checker.rs:1923-1951`).
- `guarded_refold_certificate`: only `place + literal`, `literal + place`, `place - literal` (`prf/checker.rs:1471-1485`).
- `place_refined_binary_range`: add/subtract with a literal on one side (`prf/checker.rs:5121-5134`); `apply_assignment_guard`: only `a - b` with a guard over the same two operands (`:4618-4661`).
- Statement kinds that preserve stability: Assignment, LocalData, Call, Expression (`prf/checker.rs:51-90`, `:3324`); `_ => break` (`:3599`).
- `operand_declared_primitive`: only `self.<field>` (`prf/checker.rs:5453-5457`).
- `contains_index`: `InBounds` proved only for range `(Zero, Length)` (`prf/lemmas.rs:154-157`).
- `primitive_constraints`: `"u8"|"u16"|"u32"|"u64"` only (`prf/obligations.rs:1607-1613`); witnesses only `<=`/`<` against a literal (`:535-543`); transition targets only `Named` (`:1216`); initializer obligations only for primitive-carried constrained types (`:918-920`).
- `md` indexed families: "source W descriptions currently require a Type/Value telescope" (`md:10836`); no quotient, opaque or region carriers (`:10842`); no nested or mutual recursion (`:10815`, `:11269`).
- `md` contract theorems: one erased indexed-family parameter, unit result, Type/Value binders, one index position per telescope entry (`md:4965-5195`).
- `md` kernel integer support: `<`, `<=` (and flipped) with builtin meaning (`md:3504-3532`); `+`, `-`, `*` only when `integer_multiply` is set (`:3039-3063`).
- `md` fixed-core members: Level, Type, Strict, Squash, Identity (`md:2482-2515`); `scalar_operand_carrier` I8..U64, Addr, F32, F64 (`:4222-4234`); alias propagation capped at 128 passes (`:5418`); constructor depth 128 (`:6512`).

### psi-compiler, terminal-production

- `ProductionKey::new`: only `Symbol` and `Declarations` selections are shareable, `_ => return None` (`pc/lib.rs:62-66`).
- `build_bodies`: Toolchain or Build-scope bodies only (`pc/source_compilation.rs:1237`).
- Native locator shapes: `PrimitiveScalar`, `FixedArray` with literal length, `Sum` of Scalar/Structural; else "native locator cannot retain references, views or opaque storage" (`tp/build_bindings.rs:243-267`); `bind_native` needs exactly 3 arguments (`:148`) and one owned structural parameter (`:184-193`).
- Protocol name tables: 15 filesystem, 5 query, 7 request names (`tp/build_bindings.rs:546`, `:861`, `:908`).
- External satisfier: exactly one ABI application, `[application]` / `[]` / `_` (`tp/lib.rs:307-313`).

### facts

- Place construction: Borrow, Name, Member, Indexed; `_ => append_expression_place` (`fa/lib.rs:1498`); `FixedIndex` only for a constant integer index (`:1487-1491`).
- `collection_element_type` depth cap 8 (`c&p:1505`); `symbol_type_reference` only Parameter, Local, Field with parent Machine/Variant/Data (`c&p:1398-1462`).
- `FloatPredicate::comparison` five operators (`fa/evidence.rs:42-48`); `u64_integer_literal` unsigned only (`fa/predicates.rs:398-404`); integer-math trapping walk capped at 4096 terms (`fa/predicates.rs:203-207`).

### checked-trees, lowered-psi

- `CheckedUnitCallPlan::new`: the admission table by `(completion, route, result, transfers)`; `ActivateTask` needs `U8` result, `entries` in `2..=8192`, 3 structural and 1 scalar argument; `ReloadSegments` `1..=8192`; everything else `false` (`ct/flow_terminal_units.rs:1127-1267`).
- `is_bounded_nominal_cleanup_record`: every field `Scalar(Bool|I8|I16|I32|I64|U8|U16|U32|U64|Addr)` (`ct/flow_terminal.rs:2418-2429`); `is_partial_affine_field_type` adds F32/F64 and four other kinds (`:2458-2471`).
- `exact_complement`: two comparison pairs, or case membership with `cases.len() == 2` (`ct/values_and_views.rs:1951-1971`).
- `predicate_operator`: 8 binary operators (`ct/facts.rs:341-353`); six comparison spellings each for integer and float selection (`ct/operators_and_proof.rs:255-263`, `:282-290`).
- `ConstantPredicateObligation::retain`: Integer, Boolean, layout-query Call, const Name, builtin Binary, `!`, non-recast Cast; `_ => return None` (`c&o:74-109`); `IntegerRangeRecipe::binary` add/sub/mul (`c&o:589-594`).
- `direct_reborrow_effect`: fixed 3x3 table (`ct/admissibility_and_borrow.rs:1210-1224`).
- `CheckedUnitPlan::for_machine` precedence: nominal cleanup, partial cleanup, composed control (`ct/flow_terminal_units.rs:2577-2591`).
- `Swapped` operand order: "operand_count == 2 && authored_ordinal < 2" (`lp/lib.rs:128`).

---

## Accepts when it does not know

Classes are the readers'. I did not move any entry between classes; where another record or my own reading bears on an "open" entry I say so on the same line.

### Classed "open" by the reader

**proof-admission**
1. `adm/integer_rules.rs:686-701`: cast bound conversion accepts when the root and target are the embedded chain values on matching sides and the other side is structurally equal; the other side is not required to be a literal. Whether `ProofRule::IntegerCastBound` can supply an arbitrary term was not traced. (checked: code matches.)
2. `adm/integer_affine.rs:2945-2964`: recognisers 7 and 8 return a form from the shape of `witness.target` with no `context.validate` call, so a Value-typed root not in `context` is not rejected there. Whether the proof rule validates witness terms was not traced. (checked: `adm/proof.rs:482` passes the witness straight in; I did not trace validation of node conclusions.)
3. `adm/integer_affine.rs:2953-2964`: the shift count is ignored (`count: _`); the bound mapper later demands count evidence (`:1372`).
4. `adm/integer_affine.rs:2491-2535`: `bounded_bitwise_sibling` finds mask bounds by searching uncited prior axioms and does not validate them; "depends on whether semantic_axioms are validated earlier by the caller". (checked: both entry paths validate every semantic axiom first, `adm/proof.rs:189-193`, `bd:15303-15307`. The axioms are still found by search, not by cited index.)
5. `adm/mathematical_core.rs:3664-3666`: assumption closure skips a constant index missing from the signature. Not traced into callers; the verifier separately rejects unknown declarations (`:1148`).
6. `adm/mathematical_typing.rs:1375-1378`: conversion returns `true` for any two terms at a Strict-sorted shared type and never checks that they inhabit it. The design is "stated" (header `:7-8`); the one external caller `adm/mathematical_core.rs:1311` is "open". Wrong input a reader named: a caller supplying wrongly typed terms with a strict shared type.

**proof**
7. `prf/checker.rs:342-344`, `:472`: an assignment judged "overwritten before consumption" skips all checks, including named constraints; the later assignment is matched by `display_name` equality. Wrong program a reader named: a different place that displays identically, such as a shadowed local with the same spelling. Open for shadowing. (checked: code matches.)
8. `prf/checker.rs:374-399`, `:547-577`, `:623-642`, `:689-715`, `:762-808`: when the certificate builder returns `Uncovered`, the older interval path accepts without the kernel. "Traced" for control flow, "open" for soundness of the `validation::*` helpers.
9. `prf/checker.rs:5427-5473`: the operand's primitive is taken from the first machine whose data has a field with that name, not the current machine, and can lift a lower bound to 0 (`:5220-5224`, `:5334-5337`). Wrong program a reader named: an unsigned field of the same spelling in an earlier machine lifts the bound for a signed one.
10. `prf/checker.rs:6522`: `BoundedValue` and `GuardedTransition` obligations are no-ops here; a measurement comment says "decided elsewhere" (`:5576`); not traced where.
11. `prf/lib.rs:845`: `expression_contains_call_node` returns `false` for unlisted node kinds; feeds `dehoisted_initializer` (`:705-707`) and item 7. Which kinds exist beyond the list was not enumerated.
12. `md:10775-10779`: an application of a family still under construction returns `Sort(Type 0)`. Not traced.
13. `md:4953-5195`, `:5213`: contract theorem retention `continue`s on every shape miss. A derivation is dropped; whether that lets a program pass depends on consumers not read.
14. `md:2211-2215`: identity emission rows whose normal form is not a 3-argument `Id` application are dropped.
15. `md:5418-5518`: alias propagation stops after 128 passes; unresolved names stay unrecorded without error.
16. `md:1109`: `_ => Some(ScalarCarrier::Integer)` for any primitive not Bool/Addr/F32/F64, including `Selector`. (checked: code matches.)
17. `prf/obligations.rs:344`: an assignment whose target type cannot be resolved gets no obligation and no diagnostic.
18. `prf/obligations.rs:3195`, `:3229`: a domain constraint with an unrecognised name disappears from the obligation's constraint list, so the obligation checks less.
19. `prf/obligations.rs:1894`: `proof_obligation_key` returns `None`; unit 77 left callers "open". Unit 76 read them: `store_for` returns `Ok(None)` (`prf/derivation_store.rs:104-106`) and `candidates_for` returns nothing (`:131-133`), and the cache has no production caller.

**psi-compiler**
20. `pc/lib.rs:61-85`: the production cache key does not include `external_declarations`, `provider_inventory` or `publication` (fields at `tp/lib.rs:402-411`). Whether callers can vary those under one key was not traced.
21. `pc/source_checking.rs:741-751`: a prior judgment is reused if `judges_preparation_input` says so; that predicate was not read.
22. `pc/source_compilation.rs:86-88`: for a file with no package identity the alias and authority checks are skipped. Wrong input a reader named: a retained frontend reused after an unpackaged source's declaration authority changed.

**psi-terminal-production**
23. `tp/build_bindings.rs:682-699`: a Build protocol call that cannot be associated is dropped from the companion list.
24. `tp/build_bindings.rs:394-402`: same for binding calls without a source site or valid selection.
25. `tp/build_bindings.rs:33-41`: a `Name` root matching zero or several machines yields root `None`, not an error.
26. `tp/lib.rs:173-177`: per-instance calling-policy failures are stored as `Err` data; consumer not read.
27. `tp/lib.rs:188-198`: `boundary_application` returns `None` for any machine that is not `ExternalRealization` and has no `abi_for_boundary`.

**facts**
28. `fa/lib.rs:1859-1867`: `places_match` returns true for places with equal bare-name labels. Wrong program a reader named: two distinct locals or fields with the same spelling, so a domain-membership fact on one satisfies a requirement on the other through `proves_place_domain_membership_in_program` (`c&p:1062`). Six external callers in Psi 05 were not read. (checked: code matches.)
29. `fa/lib.rs:1466`, `:1479`: an unresolvable member becomes a field segment with an invalid symbol; two such segments are equal, so `a.x` and `a.y` with both unresolved have equal segment lists.
30. `fa/predicates.rs:100-125`: with `maximum_length < bytes.len()` only the first bytes are compared; whether the predicate is used as an assumption or a requirement is open.

**checked-trees**
31. `c&o:109`, `:112-114`: an unsupported constant-predicate shape is not retained as a pending per-target obligation; callers not read. Wrong program a reader named: a target-dependent predicate over an unsupported shape is not re-checked per target.
32. `c&o:189`: discharge treats every binary node as builtin; reachable only if captured nodes differ from those `retain` checked.
33. `c&o:2503`: crash predicate identities that differ only in the symbol behind a same-spelled name compare equal. Wrong program a reader named: crash-route guard matching treats two differently-resolved same-named parameters or fields as one predicate. Consumers (`validation/contract_entailment.rs`, 42 matches) not read.
34. `ct/admissibility_and_borrow.rs:292`: statement index defaults to 0; no callers.
35. `ct/lib.rs:328`: `KeyedRows::with_rows` gives an empty slice for a missing key.
36. `ct/providers_and_trust.rs:107`: a requirement not flagged installation-bound yields no reach declaration.
37. `ct/providers_and_trust.rs:161-163`: a row whose requirement is not a trait requirement gets zero external applications without error.
38. `ct/termination_and_cleanup_facts.rs:99-120`: `append_shifted` copies `path` unshifted. Inferred that it indexes a different arena; not verified.
39. `ct/facts.rs:266`: `has_builtin_operators` returns `true` for any node kind other than Binary/Unary/Borrow without looking at children. Wrong program a reader named: `f(a + b)` where `a + b` is a user operator. Partly traced; 42 callers not traced. (checked: code matches.)
40. `ct/facts.rs:2999-3004`, `:3171-3177`: a type reference past the table end returns row 0. Wrong program a reader named: a type interned after the table was built is classified as the first type. One consumer each (`06.../shared.rs:360`, `:1473`). (checked: `:2999-3004` matches.)
41. `ct/facts.rs:3440-3447`: state locals indexed by arena index alone, generation ignored. Inferred, not traced.
42. `ct/flow.rs:1339-1346`: `state_by_symbol` takes the first of duplicate state rows; a test shows duplicates exist (`:1396`).
43. `ct/flow.rs:2162-2166`: a missing call falls back to the statement's, then the state's, entry constraints.
44. `ct/flow_ownership_and_roots.rs:905-914`: no exit path gives a transfer count of 0; whether an undercount is the safe direction depends on the caller.
45. `ct/flow_ownership_and_roots.rs:379-388`: an empty evaluation-order span means "authored order".
46. `ct/flow_terminal.rs:2044`: structural type plans compare equal ignoring `source_declaration`; `adopt_structural_types` (`ct/flow_terminal_units.rs:1520-1543`) then treats them as a non-conflict.
47. `ct/flow_terminal.rs:1384-1410`: with empty `projection_evaluations`, `FixedIndex` segments are never reported as selectors.
48. `ct/flow_terminal_units.rs:972`, `:1005`: a return arm with no statement ordinal reports 0.
49. `ct/flow_terminal_units.rs:55-73`: with no occurrence index, a state symbol that appears twice resolves to the first.
50. `ct/operators_and_proof.rs:477-486`: status `Missing` with zero candidates counts as builtin fallback. Wrong program a reader named: `+` on a type with no operator declared; two callers (`05.../execution_terminal_unit_control.rs:5547`, `05.../published_facts.rs:6016`). Whether typing already rejects it was not traced. (checked: code matches.)
51. `ct/operators_and_proof.rs:1331-1337`: `resolution_issues` omits `DomainPending`.
52. `ct/operators_and_proof.rs:2659`: `contract_call_at` returns the last row of duplicates.
53. `ct/operators_and_proof.rs:2037`: `_ => {}` lets `TransitionalInput`, `ExactBinary32Literal`, `ExactBinary64Literal` sources through without provenance checks. "Stated: none."
54. `ct/values_and_views.rs:1255-1256`: a cyclic value graph reports `Normal`. "Likely unreachable"; not traced. (checked: code matches.)
55. `ct/values_and_views.rs:1261-1272`: a `Call` node with no scheduled call yields `Normal`. Wrong program a reader named: a call that never returns but whose schedule row is missing is treated as returning. (checked: code matches.)
56. `ct/values_and_views.rs:2272-2300`: `expression_at` takes the first match with no duplicate check.

### Classed "stated" by the reader

- `bd:1199`, `:14469`: an out-of-range numeral falls to the opaque instance path; test message "keeps the explicit instance assumption".
- `adm/mathematical_core.rs:3741`: `universe_parameter_arity` skips `Dummy` (comment `:3738-3740`); tests pin that `Dummy` is refused (`:7594-7612`) and that `structurally_equal(dummy, dummy)` is false (`:6147-6151`).
- `adm/proof.rs:238-240`: when the core reports `Unsupported(family)` the certificate is still accepted with `Refused(family)`; header says the core agrees "on every covered certificate" (`:8-9`). Also "traced": the only non-test reader counts refusals (`prf/checker.rs:5692`). (checked: matches.)
- `adm/proof.rs:486-493`: a `Truth` root bound skips conversion and checks against truth bounds (test `:3455-3457`).
- `adm/integer_affine.rs:377-393`: the older total-image remainder bound is still accepted; test message "the previous total-image certificate remains valid" (`:4269`), "only the tight or legacy mapping is admitted; relaxation needs evidence" (`:4280`).
- `md:12347`, `:12349`, `:3469-3474`, `:3513-3516`: unknown constructs become opaque kernel assumptions; comment "signature occurrence is not authority" (`md:3098-3100`). Traced for theorems, not for other consumers.
- `prf/checker.rs:4991-4998`: a non-literal operand takes its declared range ("stated implicitly"; not traced).
- `tp/custody.rs:87`, `:94`: stored negative judgments are preserved on read (stated by structure).
- `c&o:524-531`: recipe obligations are marked discharged per batch (stated by structure).
- `ct/providers_and_trust.rs:262-269`: no realization skips the telescope check; the BuildNative branch separately requires a closed declaration (`:186-193`).
- `fa/predicates.rs:509-517`, `:709-719`: non-arithmetic variants report no trapping operation (enumerated arms).

### Classed "traced" by the reader

- `bd:8640-8652`: opaque rule instance used as the fallback in 14 arms (`bd:7962` ... `:8551`). Traced for call order: each arm runs its external check first. Unit 73 did not read the checkers; unit 75 read them in `adm/proof.rs` and recorded no wrong accept.
- `bd:7815-7824`: a primitive with no closed inhabitant is interned as an opaque assumption after `decide_primitive` passes. Traced for flow.
- `bd:5313-5315`, `:5348-5350`: `None` equality returns the evidence unchanged; premise equality was checked separately.
- `adm/integer_affine.rs:1584`, `:2123`: recognisers return `Ok(None)` and fall through to later ones, ending in the general chain which rejects (traced within the file).
- `adm/integer_rules.rs:1898-1926`: tightest bound taken among all later axioms, identity checked afterwards (traced within the file).
- `adm/integer_affine.rs:1424-1427`: extra evidence ignored, no weakening. `:1495-1499`, `:1396`, `:1188-1190`: "inferred" sound by type.
- `adm/predicate_denotation.rs:189-195`: dropped equations apply equally to both sides.
- `adm/proof.rs:1643`, `:2178-2181`: local assumption indices and leaf rules.
- `md:4436-4446`: source-alias branch accepts via `checked_argument_matches_type_reference` without kernel conversion (control flow only).
- `md:941`: a known carrier is taken when the other operand's is unknown; operands are re-checked (`md:2886-2897`). (checked: matches.)
- `md:12689-12693`: empty result when there is nothing to check.
- `prf/obligations.rs:453-455`, `:487-491`: missing call frames clear all witnesses (fail-closed).
- `pc/source_checking.rs:435`, `:448`; `pc/source_compilation.rs:99`.
- `ct/admissibility_and_borrow.rs:37-50`, `:202-213`, `:305-315`, `:429-455`, `:625-635`: every acceptance view reports accepted; traced by grep to have no users.
- `c&o:2831-2853`: a zero-commitment call site fails comparison against a real target.
- `ct/lib.rs:361-365`: `KeyedRows::eq` always true (derived data). `ct/facts.rs:134-207`: unsorted rosters fail closed. `ct/flow_terminal_units.rs:373`, `:2124-2130`: `_ => true` covers variants that cannot stop.

---

## Silent drops

Places where a miss produces no row and no diagnostic. 26 sites by my count.

**proof (9)**
- `prf/checker.rs:6522`: `BoundedValue` and `GuardedTransition` obligations produce nothing here.
- `prf/checker.rs:342-344`: an "overwritten" assignment skips every check.
- `md:2211-2215`: identity emission rows.
- `md:4953-5195`, `:5213-5215`: contract theorems, including when the proposition elaborates to `None`.
- `md:5515-5519`: unresolved evidence aliases after the last pass.
- `prf/obligations.rs:344`: no obligation when the target type does not resolve.
- `prf/obligations.rs:3195`, `:3229`: unconverted constraints.
- `prf/obligations.rs:918-920`: no initializer obligation unless the constrained type is primitive-carried.
- `prf/obligations.rs:947-975`: array element obligations for anything but borrow, match arms, array literal.

**proof-admission (5)**
- `adm/predicate_denotation.rs:189-195`: non-matching or duplicate-left value equations.
- `bd:6299-6318`: rewrite equations whose carrier is not `Int`.
- `bd:6381-6385`: no-op or duplicate transport equations.
- `adm/integer_affine.rs:1424-1427`: extra shift evidence.
- `adm/mathematical_core.rs:3664-3666`: unknown constant index in the closure.

**psi-compiler (1)**
- `pc/source_compilation.rs:326`: a completion with incomplete usage accounting is not retained.

**psi-terminal-production (4)**
- `tp/build_bindings.rs:682-699`, `:394-402`, `:33-41`; `tp/lib.rs:188-198`.

**facts (2)**
- `fa/lib.rs:1466`, `:1479`: unresolved member becomes an invalid field segment.
- `c&p:1700-1741`: `(OpaqueRange, FixedIndex|Index)` composes to an index with an invalid expression (the index value is lost; a widening).

**checked-trees (5)**
- `c&o:109`, `:112-114`: constant predicate not retained.
- `ct/providers_and_trust.rs:107`, `:161-163`.
- `ct/operators_and_proof.rs:1331-1337`: `DomainPending` uses not reported.
- `ct/values_and_views.rs:2436-2448`: `SealedExpressionStateIndex` is written as nothing and read as default (inferred).

Also by design: `LoweredPsi::into_parts` drops all side tables except source custody (`lp/lib.rs:251-265`).

---

## Unused or unwired

Counts are the readers' greps over `omega-rust` unless marked.

- **Derivation cache.** `ProofDerivationCache|DerivationStore|DerivationConsultation|derivation_store::`: 53 occurrences, `prf/checker.rs` 45 and `prf/derivation_store.rs` 8, none elsewhere. `check_proof_plan_inner`'s only caller passes `None` (`prf/checker.rs:6472`). The store header says it "serialize[s]" certificates and is "Persistent" (`prf/derivation_store.rs:1-7`); the file has no I/O. (checked: 53 in 2 files.)
- **`check_proof_plan`** (`prf/checker.rs:6454`): 0 callers. `check_proof_plan_with_decisions` has one, `pipeline/05.../checking.rs:422`. (checked.)
- **`recipe_obligations`**: created empty (`prf/checker.rs:6482`), never pushed; `ProofDecisions.constant_recipe_obligations` is always empty (10 hits, all in `checker.rs`).
- **`prf/lemmas.rs`** (`ProofLemma`, `ForAllInRangeFact`, `ElementIndex`, `QuantifiedBound`, `LemmaFact`): 0 non-test users ("inferred from grep"). (checked: 86 hits in `lemmas.rs`, none of these names elsewhere.)
- **`ProofRuleFoundation` / `.foundation()` / `cites_semantic_axioms`**: outside `adm/classicality.rs`, 10 occurrences in 2 files: `prf/lemmas.rs` 9, `adm/lib.rs` 1. With the previous item, the rule classification's only consumer is an unreferenced module. `Classical` has no producer (`adm/classicality.rs:101-110`). (checked: 23 in 3 files, 13 of them in `classicality.rs`.)
- **`prf/boundary.rs`** types: 42 occurrences in 1 file, the file itself; `source/` not searched. (checked.)
- **`build_proof_surface_report` / `ProofSurfaceReport`**: only `prf/lib.rs`. (checked: 17 in 1 file.)
- **Quotient scheme.** `quotient_scheme`, `QuotientFamily`, the 20 `QUOTIENT_*` constants: no non-test caller under `omega-rust` (`QuotientFamily|QUOTIENT_*` 102 hits in 2 files). All 20 builders are used only from tests (gap 19). The indexed scheme does have users (18 hits in 7 files). (checked: `quotient_scheme` 11 lines in 2 files; the record's "14 hits" does not reproduce, its conclusion does.)
- **`identity_substitution`** (`adm/mathematical_core.rs:4673`): definition and two re-exports, zero call sites. (checked.)
- **`ObligationClass::Derivable`**: "only declared" in `adm/lib.rs`; not grepped separately.
- **Acceptance views** (`AcceptanceView`, `StateAcceptance`, ... `AcceptanceSummary`): 117 matches in 2 files, `ct/admissibility_and_borrow.rs` 113 and `ct/lib.rs` 4. `state_acceptance(`: 1 match, its definition. No implementation constructs a rejection. (checked: no `.rs` user outside the crate; one README mention.)
- **`CrashContractCapsule::new(`, `CheckedCrashCallSite::new(`**: 0 qualified-path matches (`c&o:2743`, `:2882`). (checked.)
- **`LibrarySliceKey`** (`pc/source_checking.rs:100`): only feeds counters (`:771-773`).
- **`w_step_type`, `box_body_type`**: used only inside `adm/mathematical_typing.rs` (8 occurrences in 1 file).
- **`convert_retaining`**: 1 caller outside its definition (`adm/mathematical_core.rs:1311`).
- **`shifted_rows::*`**: used only in `ct/facts.rs` (9) and `ct/termination_and_cleanup_facts.rs` (9).
- **`visit_value_ids`** (`lp/lib.rs:171`): unit 78 "suspected unused". It is used; see "What I checked myself".
- One-caller items recorded: `terminal_operand_position` 1 (`06.../retention.rs:2943`); `CheckedUnitPlan::admits_signature` 1 and `is_duplicated_in` 1 (`06.../machine_lowering.rs:1578`, `:1575`); `unique_contract_call_at` 1 (`c&o:2856`); `resolution_issues` 1; `provider_declarations` 1; `append_shifted` 1.

---

## Stated limits

- `md:509-510`: "references itself or a later declaration; the signature admits strictly earlier ones".
- `md:743-746`, `:759-761`, `:2547`, `:12378`: "has no kernel denotation yet".
- `md:2389`: "computed universe levels are a later leg".
- `md:2521`: "applying mathematical declaration ... as a carrier is a later leg".
- `md:2459-2462`: "Other predicates remain unsupported here."
- `md:6606`: "has no constructor derivation yet". `md:6850`: "has no checked derivation yet".
- `md:10815`: "nested source-family recursion has no checked encoding yet". `md:11269`: "nested or mutual source-family recursion has no checked encoding yet".
- `md:10836`: "const/machine telescope denotation remains unsupported".
- `md:10842`: "quotient, opaque or region-bearing source indexed carriers require their own checked description".
- `md:11346`: "open dependent carrier whose encoding is unsupported". `md:11017`, `:11027`: "no relevant proposition denotation yet".
- `md:12282`: "member call has no mathematical denotation yet".
- `md:5524-5539`: "This file does not turn legacy constructor-success verdicts into evidence".
- `bd:12932`: "dependent source predicate premises require value abstraction".
- `bd:11969`: "translated numeral exceeds existing numeral evidence"; `bd:11974`: "translated numeral lacks exact addition evidence".
- `adm/integer_affine.rs:4904`: "a direct signed remainder cannot assume MIN / -1 is representable"; `:5791`: "zero and the MIN/-1 discontinuity are not monotone division"; `:5062`: "a signed remainder cannot inherit the unsigned nonnegative image".
- `adm/mathematical_core.rs:3728-3730`: "This describes syntax only; the ordinary kernel judgment still checks every parameter."
- `adm/mathematical_core.rs:6699`: "pointwise uniqueness is not function extensionality"; `:6548`: "boxed inhabitants are propositionally, not definitionally, equal".
- `adm-tests/indexed_nested.rs:39-41`: omega-branching because "kernel has no `Fin` former".
- `adm-tests/source_checked_authority.rs:12`: "after the unused exported admission shortcut is retired".
- `adm/lib.rs:157`: every admission returns `AdmissionNotAcceptedByProfile`.
- `ct/flow_terminal_units.rs:1612`: "this compiler does not lower {construct} of it yet ({detail})".
- `ct/flow_ownership_and_roots.rs:1088-1090`: "following production again turns that identity edge into infinite execution".
- `ct/flow_terminal.rs:1297-1298`: "this identity never authorizes evaluating the expression again".
- `ct/values_and_views.rs:879-880`: "This is diagnostic data, not a partial executable plan or permission to omit a selected call."
- `ct/operators_and_proof.rs:2490`: "Typed Id formation supplies no identity witness or executable Boolean."
- `ct/facts.rs:1223-1226`: defaults `Err("checked compiler-evaluation facts are absent")`, `Err("checked synchronous invocation facts are absent")`.
- `fa/lib.rs:1741-1745`: "Only supported membership and float payloads suppress a new occurrence."
- `lp/lib.rs:9-16`: sidecars are "producer-private" and discarded "at the `PSITERM\0` wire boundary".
- `pc/source_compilation.rs:222`: "selected retained source still has a deferred authored body".
- Numeric limits: `MAX_ELABORATION_NODES = 1<<16` (`bd:38`); source theory depth 128 (`bd:12467`); closed evaluator 8192 nodes, depth 128, 65536 result bits (`adm/integer_rules.rs:19-23`); 4096 monomials (`:3045`); `DEFAULT_CONVERSION_STEPS` 65_536 (`adm/mathematical_core.rs:1091`); predicate denotation 4096 nodes, depth 64 (`adm/predicate_denotation.rs:31`, `:35`).

---

## Disagreements

1. **`visit_value_ids` in lowered-psi.** Unit 78: "Suspected unused" (`lp/lib.rs:170-191`). The code shows callers: `occurrence.frontier_before_call.visit_value_ids(..)` at `pipeline/07.../scalar_and_value_passes.rs:153`, `:395`, `:608` and `control_flow_and_retention.rs:640`; `frontier_before_call` is a `LoweredSourceFrontierBindings` (`lp/lib.rs:200`). The record is wrong; the method is used.
2. **`quotient_scheme` count.** Unit 74 lists 11 lines by number and then says "14 hits in 2 files (mathematical_core.rs 12, lib.rs 2)". My count is 11 occurrences in 2 files (10 and 1). The conclusion (no non-test caller) holds.
3. **End of `mod tests` in `mathematical_core.rs`.** Unit 74 says 4769-9621; gap 19 says it closes at `:9620`. The code closes at `:9620`; `bounded_observations` starts at `:9622`. Gap 19 is right.
4. **`conv!` macro.** Gap 19 says "defined twice" and then lists three sites. There are three (`adm/mathematical_core.rs:4861`, `:8036`, `:8801`).
5. **`md:941` carrier rule.** Unit 76 marks it "open" in the carriers section and "traced" in its fail-open list. The re-check exists at `md:2886-2897` (an operand whose carrier is not the expected one, including unknown, is an error). "Traced" is the supported one.
6. **What `bd` validates.** Unit 73 says the entry "validates goal and every assumption". The code also validates every semantic axiom (`bd:15303-15307`). The record understates.
7. **Whether `adm/proof.rs:180` and `bd:15299` are test-only.** Gap 19 inferred yes. Both are under `#[cfg(test)]` (`adm/proof.rs:179`, `bd:15298`).
8. **Lemma types.** Unit 76 calls the checked-trees lemma types "different types"; they carry the same five names and the same bound shape (see "The same job"). Not a contradiction about use, but the two are one vocabulary.

No second-pass record contradicts a first-pass claim about production code (gap 19 says so itself).

---

## What I checked myself

Opened in the checkout or grepped across `omega-rust` in count mode. 29 checks.

| # | Claim | Where | Result |
|---|---|---|---|
| 1 | `quotient_scheme` has no non-test caller; "14 hits" | grep `\bquotient_scheme\b` | partly: 11 in 2 files; conclusion holds |
| 2 | `identity_substitution` has zero call sites | grep | matches |
| 3 | `boundary.rs` types used nowhere else, 42 in 1 file | grep | matches |
| 4 | `lemmas.rs` types have no outside users | grep | matches |
| 5 | `ProofRuleFoundation` outside users: `lemmas.rs` 9, `lib.rs` 1 | grep | matches |
| 6 | `check_proof_plan` 0 callers; inner always passed `None` | grep, `prf/checker.rs:6472` | matches |
| 7 | Derivation cache names only in 2 files, 53 | grep | matches |
| 8 | Acceptance views have no user outside checked-trees | grep | matches (one README mention) |
| 9 | `build_proof_surface_report` only in `prf/lib.rs` | grep | matches |
| 10 | `lp` `visit_value_ids` "suspected unused" | grep, `lp/lib.rs:160-201` | does not match: 4 callers in Psi 07 |
| 11 | `CrashContractCapsule::new(` / `CheckedCrashCallSite::new(` 0 matches | grep | matches |
| 12 | `places_match` accepts equal bare-name labels | `fa/lib.rs:1827-1868`, `c&p:1272-1341` | matches |
| 13 | Two "is builtin" rules disagree on `Missing` | `ct/operators_and_proof.rs:477-486`, `ct/facts.rs:244-268` | matches |
| 14 | `completion_with` returns `Normal` on a cycle and on a missing scheduled call | `ct/values_and_views.rs:1255-1272` | matches |
| 15 | `ContentClassFacts::classes` falls back to row 0 | `ct/facts.rs:2999-3005` | matches |
| 16 | Overwrite detection by `display_name` equality | `prf/checker.rs:420-511` | matches |
| 17 | Affine recognisers 7 and 8 return without `context.validate`; shift count ignored | `adm/integer_affine.rs:2945-2964` | matches |
| 18 | Cast bound shortcut compares the other side structurally only | `adm/integer_rules.rs:686-701` | matches |
| 19 | `bounded_bitwise_sibling` risk "depends on caller validation" | `adm/proof.rs:181-193`, `bd:15300-15307` | partly: both entry paths validate all semantic axioms first |
| 20 | Two structural expression-equality functions in the proof crate | `prf/checker.rs:4832-4928`, `prf/obligations.rs:1255-1323` | same job, near-copies; arms differ |
| 21 | Lemma table in proof vs lemma kinds in checked-trees | `prf/lemmas.rs:52-200`, `ct/operators_and_proof.rs:2054-2104` | same vocabulary, different keying |
| 22 | `Refused` core decision is accepted and only counted | `adm/proof.rs:228-242`, `prf/checker.rs:5680-5693` | matches |
| 23 | Closure skips unknown constant index | `adm/mathematical_core.rs:3660-3669` | matches |
| 24 | `bd` re-uses the relation functions from `proof.rs` | `bd:8310-8341`, `adm/proof.rs:475-497` | matches: shared, not a copy |
| 25 | `bd` entry "validates goal and every assumption" | `bd:15300-15307` | partly: also semantic axioms |
| 26 | Observation counters are test-only | `adm/proof.rs:179-180`, `bd:15298-15299` | matches |
| 27 | `md:941` rule is re-checked per operand | `md:934-954`, `:2876-2898` | matches |
| 28 | `conv!` "defined twice" | grep | does not match: three |
| 29 | `mod tests` ends at 9621 (unit 74) vs 9620 (gap 19) | `adm/mathematical_core.rs:9616-9623` | 9620 |

Also seen while checking, not in any record: `ProofLemmaKind|QuantifiedRangeFact|QuantifiedBoundFact|LemmaFacts` occur only inside checked-trees (14 occurrences: `ct/operators_and_proof.rs` 11, `ct/lib.rs` 3). I did not search for users that reach these through a field without naming the type.

---

## Not covered

- **Records:** none unread. All 8 files were read in full.
- **Source ranges:** every row of `unit_073.tsv` to `unit_078.tsv` is reported read in full by a first- or second-pass record. The three ranges unit 74 left unread (`adm/integer_affine.rs:3301-6415`, `adm/mathematical_core.rs:521-1059`, `:4774-9639`) are covered by gaps 18 and 19; all three are test code.
- **Read at reduced depth, by the readers' own statement:** test modules in `prf/checker.rs` (`:96-126`, `:2013-2854`, `:4138-4579`, `:5775-5871`) and `md` (`:6048-6168`, `:7087-7160`, `:9079-10328`) were "read at test depth"; about 1,300 test lines in checked-trees and the test regions of `prf/obligations.rs`, `fa/lib.rs`, `c&p`, `fa/evidence.rs` were described briefly.
- **Outside this area:** `symbol-resolved-trees/src/` (8,835 of the crate's 8,885 lines) is not in these units. The `validation` crate helpers that the `Uncovered` path and several "open" entries depend on, the Psi 05 callers of `places_match`, `has_builtin_operators` and `ConstantPredicateObligation::retain`, and `terminal-psi/semantics/descriptors.rs` (10 uses of `accept_certificate`) were not read by these readers or by me.
- **Counts the readers gave only as combined greps** (several names in one pattern) were not split: `fa` query helpers (46 in 12 files), `CheckFacts::*_is_proven` (17 in 7 files), psi-compiler counters (32 in 9 files).
