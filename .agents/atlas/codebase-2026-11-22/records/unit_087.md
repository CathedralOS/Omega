# Record: unit_087 (area 17-omega-source)

Unit: 39 Omega files under `source/subsystems/compiler/source_to_psi/pipeline/`, all of them read in full:
`symbol_resolved_trees_to_typed_trees/` (statements, typer), `syntax_trees_to_symbol_resolved_trees/` (the resolver:
19 files), `tokens_to_syntax_trees/` (the parser: 17 files) and `typed_trees_to_checked_trees/aliases.omg`.
All paths below are relative to `source/subsystems/compiler/source_to_psi/pipeline/` unless they start with `source/`.
"Read" means I read the lines cited. Counts marked "grepped" come from `grep -rwo --include=*.omg <name> source | wc -l`
(word occurrences, including the definition) unless stated. Every machine in these files is an Omega `machine`; nearly
all carry `crashes Trap`. No machine in the unit declares `terminates` or `ensures`; the grep hits for those words are
strings and comments (clauses.omg, clause.omg, forms.omg, facts.omg, names.omg, traits.omg; inspected by eye, not each one).
`requires` appears as a precondition on array-indexing machines (for example `syntax_trees_to_symbol_resolved_trees/clauses.omg:52,156,194,521`,
`overloads.omg:37,50,59,82,101`, `operators.omg:157`, `pattern_binders.omg:170,187`) and on the `Parser` and
`CallableHeaderFrame` data declarations (`tokens_to_syntax_trees/parser.omg:30-45`, `callable_header.omg:21`).

Common pattern, stated once: every stage is a fixed-capacity table filled by `machine`s that return a "reject at span"
by writing a diagnostic code into `output` and then falling off the end; the next step tests `output.accepted` and stops.
The first rejection wins. Table bounds are literal numbers repeated at each use (131072 expressions/bindings/roles/typed
marks, 65536 type rows/tokens/integer literals/arguments, 32000 members, 10240 machines, 4096 roots, 2048 clauses/name
paths/path bindings, 1024 uses, 256 data/traits/conformances/domains/propositions/definitions/generics/contracts).

---------------------------------------------------------------------------------------------------------------------
## A. symbol_resolved_trees_to_typed_trees (2 files, 1,042 lines)

### A1. symbol_resolved_trees_to_typed_trees/statements.omg (670 lines)
1. Purpose. Types one machine body's statements in authored order: `let` initializers against the declared type, assignments
   against the target place, the final unterminated expression against the body's result, and the arms of a `transition`
   (pattern, guard, target). It also refuses a non-Unit expression statement (silent discard) and a body that can fall off its end.
2. Shape. `type_body` :47 (loop over `first_statement..+count`); `require_body_completion` :78; `type_statement` :119 (dispatch on
   `StatementKind`); `type_arms` :272; `type_guard_clause` :344; `type_pattern` :371; `pattern_head` :562 (public; returns named case + 1,
   0 for other arms or on rejection); `type_target` :630; `never_type` :665.
3. Inputs/outputs. Reads `SyntaxTrees`, `NameResolution`, `SourceSet`; mutates `typer.output` (`typing_reject`, `set_binder_member`,
   `set_binder_type`). Calls out to helpers not defined here: `type_tree`, `typer_admit`, `is_place`, `place_type`, `writable_place`,
   `member_target`, `target_reference`, `substitute`, `intrinsic_equality`, `float_like`, `covers`, `validate_pattern_fields`
   (other files of the typed stage; not read).
4. How it decides. By node kind and by type-table row: `StatementKind` match :123-131; `ArmPatternKind` match :374-380;
   `value_binding(resolution, arm.pattern)` is `DataCase{item,data_case}` :591-593 compared to the subject's row `kind == Data && item == item` :607
   (identity of the data declaration, not a name). Silent discard rejects when `value_type != unit && !never` :185. `never_type` tests
   `row.builtin == BuiltinType::Never` :669. A literal/value pattern lands at the subject's type and must be "intrinsically comparable" or float :574-579.
5. Cases by shape. Pattern/arm shapes: Guard, Wildcard, Binder, Literal, Case (:374-380). A `transition` is subjectless (`has_value` false, subject type 0)
   or has a subject; subject of type 0 rejects NotAValue :228-230. Terminal target kinds Unit / other (:633-635).
6. Same job elsewhere. The two arms of "is this a place / is it writable" are in other files (suspected, not read). The `type_arms` loop
   duplicates its own `arms_next` (:323-337 vs :284-287).
7. Unused or one-caller. grepped: `type_body` 2, `require_body_completion` 2, `pattern_head` 4 (def + callers in other typed files). Not unused.
8. Stated limits. Comment :37-46 states the rule; none says "unsupported".
9. Fail-open. (a) `StatementKind::Crash -> done()` :129: typed stage does no check on a crash statement; open (no reason at site; wrong program: none seen, the parser
   already restricts the cause to `Trap`/`Abort`, statement.omg:119-134). (b) `require_body_completion` :81 skips any body whose result is not Unit
   (`result != typer.output.unit -> done()` is on the `true` branch of `has_body && accepted && result != unit` :81-84, i.e. runs only for Unit-result
   bodies); so a body with a non-Unit declared result that has no tail is not refused here; open (not traced to the checker). (c) :576 `!admitted || integer || float -> none()`:
   float-typed subjects accept literal patterns; open. (d) Everything is gated on `typer.output.accepted`, so after the first rejection later statements are skipped; stated by pattern.

### A2. symbol_resolved_trees_to_typed_trees/typer.omg (372 lines)
1. Purpose. Entry of the typing pass and the scratch it owns: `Typer` holds the `Typing` output plus generic-binding and substitution scratch.
   `type_check` runs the passes in fixed order and stops at the first rejection.
2. Shape. `data Typer` :42 (`output: Typing`, `typed: [bool;131072]`, `bound/bound_stamps: [u64;256]`, `machine_bound/machine_stamps: [..;256]`,
   `memo/memo_stamps: [u64;8192]`, `work: [u64;4096]`, generation counters); `type_check` :63 (type_references, constants, bodies, facts, clauses);
   `typer_clear` :106 (interns absent/unit/Boolean/anonymous-integer/anonymous-float rows, zeroes per-expression, per-binder, per-construct tables);
   `type_constants` :181; `type_bodies` :213; `declared_result` :237; `typer_referent` :257; `typer_read_type` :285; `typer_intern` :319;
   `is_typed` :340; `mark_typed` :357.
3. In/out. Reads `SyntaxTrees`, `NameResolution`, `SourceSet`; writes `Typer`/`Typing`.
4. How it decides. `typer_referent` strips `TypeKind::Reference` layers by row kind :261-276 (recursive `inner` with no depth bound; reference chains in a type
   table are finite by construction, inferred). `typer_read_type` strips reference layers only when none is `WriteOnly` and the end is `TypeKind::Builtin`
   :298-310, with a loop bound of `types.row_count` :288. Constants land at their declared type :193-206.
5. Cases by shape. Fixed array sizes above (bounds repeated as literals 131072 at :45,:343,:360).
6. Same job elsewhere. `typer_clear` clears per-expression type/selection/adaptation/value/roster tables one index at a time (:141-149).
7. Unused. grepped: `is_typed` 2 (def + expressions.omg:102), `mark_typed` 3, `typer_intern` 22, `typer_referent` 12, `typer_read_type` 10, `declared_result` 9.
8. Stated limits. Comment :34-41 lists the passes; none unsupported.
9. Fail-open. none seen (an index >= 131072 crashes, :352-354, :369-371).

---------------------------------------------------------------------------------------------------------------------
## B. syntax_trees_to_symbol_resolved_trees (the resolver, 19 files, 9,187 lines)

Order of passes is owned by resolver.omg `resolve` :26 (listed in B19). Many files share one idiom: "find the first visible declaration with
this spelling; then scan the rest for a visible rival and answer Ambiguous". Spelling is `spans_equal(sources, a, b)` or `span_spells(sources, span, "text")`
(source.omg:476,510), i.e. byte comparison of source text; visibility is `resolver_visible` (B12).

### B1. clauses.omg (535 lines)
1. Purpose. Resolves names inside machine clauses: `reaches` service rows, `satisfies Trait<args>::requirement [via value]`, and the `terminates by ... -> View` ranking witness.
2. Shape. `bind_clause_names` :39 (clears `service_traits[0..member_count]`, then per machine per clause by `ClauseKind`); `bind_service_row` :136; `ground_service` :165;
   `bind_satisfied_requirement` :204; `bind_ranking` :341; `data ViewSpelling` :434; `spelled_view` :441; `set_requirement` :518.
3. In/out. Writes `resolver.output.service_traits[member]` (u16 = trait handle+1, :195), `requirements[clause]` (`ValueBinding::Machine`), ranking view/measure rows.
4. How it decides. A bare `reaches` service must resolve to a boundary trait with `parent_count == 0` and `generic_parameter_count == 0 && handle < 256`
   (:176-195). The ranking view is matched by source spelling: owner `Nat` or `Slice`, name `Descending`, `BoundedDistance`, `IncreasingTo`, `Length`
   (`span_spells` :469-474). Anything outside that vocabulary must be a two-member path naming a declared measure (:388-401) else UnsupportedRankingView.
5. Cases by shape. A `satisfies` path must have exactly two members `Trait::requirement` (:258, `member_count == 2`, else UnsupportedClause :325); a ranking view is a two-member path
   or a one-argument call of one (:441-457). Quote :37-38: "A trait reached through a module path rejects as unsupported."
6. Same job elsewhere. The 4-way spelling chain for views (:466-516) repeats the builtin-name chains in type_names.omg:586-699 and domains.omg:136-172.
7. Unused. grepped: `bind_clause_names` 2 (def + resolver.omg:182).
8. Stated limits. :37-38 (above). Header :39 text on the ranking vocabulary "by spelling rather than a declaration" :338.
9. Fail-open. (a) `_ -> next_clause` :84 skips every clause kind other than Reaches/Satisfies/Terminates; traced: Requires/Ensures bind in facts.omg, Crashes/Suspends/Blocks/Invokes name nothing.
   (b) `ground_service` :188 sends `handle >= 256` to MissingServiceArguments, a diagnostic code that names a different problem; rejection, not acceptance.

### B2. containers.omg (421 lines)
1. Purpose. Gives each machine an owner (root, trait, refinement, conformance, measure) and resolves machine paths, trait members and lexical (nested-machine) lookups.
2. Shape. `place_machines` :39 (writes `machine_owners[0..machine_count]`, bound 10240 :150); `resolver_machine_scope` :165 (a machine's own generics plus its container's);
   `container_scope` :182; `bind_machine_path` :210; `machine_path_lookup` :237; `trait_member_named` :264; `conformance_requirement_named` :299;
   `set_path_binding` :336 (bound 2048); `lexical_root` :354; `qualified_lexical_machine` :369; `lexical_owner_visible` :406.
3. In/out. Reads syntax tables, `NameResolution`; writes `machine_owners`, `path_bindings`.
4. How it decides. By table position (a trait's `first_machine..+machine_count` range gets the owner; a nested machine `parent != 0` is skipped :133) and by spelling
   (`spans_equal` on name :279, :375, :392). `machine_path_lookup` accepts only 1 or 2 members (:240-243).
5. Cases by shape. Quote :235-236: "A longer path crosses a package, which this source set cannot bind." One-member free path, two-member owned path.
6. Same job elsewhere. Ancestor walks over nested machines are also in names.omg:1118 (`lexical_callee`), :1573 (`lexical_value_binder`), :1592 (`lexical_static_value_binder`),
   generic_scopes.omg:342, :373. `lexical_owner_visible` :406 and `qualified_lexical_machine` :369 each rescan every machine (`child` loop :381).
7. Unused. grepped: `qualified_lexical_machine` 2, `conformance_requirement_named` 2, `machine_path_lookup` 2, `bind_machine_path` 2, `place_machines` 2 (each def + one caller), `resolver_machine_scope` 10, `lexical_root` 4.
8. Stated limits. :235-236; :367-368 "it cannot open a sibling's private scope".
9. Fail-open. none seen.

### B3. contracts.omg (136 lines)
1. Purpose. Attaches each `requires machine Name(...)` contract to the machine binder of that spelling.
2. Shape. `bind_machine_contracts` :25 (clear all `binder_contract` rows, then per machine per contract); `contract_binder` :112.
3. In/out. Writes `BinderContract{present, contract}` per generic parameter.
4. How it decides. Spelling (`spans_equal` :126) and `parameter.kind == GenericParameterKind::Machine` :127. Missing binder rejects ContractNamesNoBinder :67; second contract for one binder DuplicateContract :84.
5. Cases by shape. One contract per binder.
6. Same job elsewhere. none seen.
7. Unused. grepped: `bind_machine_contracts` 2.
8. Stated limits. none. 9. Fail-open. none seen.

### B4. declaration_names.omg (432 lines)
1. Purpose. Checks that root declarations and data members do not collide: duplicate roots in a namespace, names that spell a builtin type, duplicate generic names, duplicate fields/cases/payload fields.
2. Shape. `data RootNamespace` :40 (None/Type/Constant/Proposition), `data RootName` :47, `root_name` :55, `declare_roots` :85, `value_path_taken` :168, `check_declaration_namespaces` :238, `check_members` :313.
3. In/out. Reads syntax tables; writes diagnostics only.
4. How it decides. Namespace by root kind: Data/Trait/Conformance share the Type namespace :60-62; Constant, Proposition each their own :63-64; other kinds (`_ -> none`) :65.
   Duplicate = same namespace, `spans_equal` name, same owner flag/spelling, `resolver_same_module` :142-145. Builtin shadow = `builtin_type_spelling` :107. A constant also may not spell a case `Owner::NAME` or a machine
   path of its module (:123-136, :168-233).
5. Cases by shape. Quadratic pairwise scans (`prior < index`) :114-149, :348-365, :391-425. Duplicate data reports DuplicateData, other pairs DuplicateDeclaration :152-155.
6. Same job elsewhere. The same pairwise `prior`/`compare` duplicate scan is repeated in generic_scopes.omg:125-141, machine_signatures.omg:104-134 and :181-215, domains.omg:100-126, identities.omg:202-235. Header :33-39 says machines (overloads), domains (carrier) and `use` rows "are checked apart".
7. Unused. grepped: `declare_roots` is called in resolver.omg:46 (definition not public); `check_members` and `check_declaration_namespaces` 1 caller each (resolver.omg).
8. Stated limits. :38-39 (checked apart). 9. Fail-open. `RootNamespace::None -> next` :100 and non-Constant spaces skip the value-path check :126: traced, those declarations are checked in machine_signatures.omg/domains.omg.

### B5. domains.omg (659 lines)
1. Purpose. Domain declarations: binds carriers and index arguments, rejects names of compiler-known domains, finds the declared domain a qualifier `in Name` selects by carrier, and resolves alias constituents and establishment routes.
2. Shape. `bind_domain_carriers` :41; `check_domains` :79; `known_domain` :136 (spells `Wrapping`, `Saturating`, `Trapping`, `Finite`); `resolver_declared_domain` :176; `domain_admits` :226; `carrier_matches` :252;
   `same_head` :350; `same_bound_rows` :372; `bind_domain_paths` :417; `constituent_domain` :475; `domain_by_head` :504; `domain_head_admits` :552; `domain_named` :609.
3. In/out. Reads type bindings (`type_binding_at`), writes path bindings (`PathBinding::Domain`).
4. How it decides. A candidate domain matches by `spans_equal` name, `visible_if`, and a carrier walk: both type runs are walked from root to first row; a carrier row that names one of the domain's own generic parameters skips the whole subtree
   (:299-318). Two matching candidates give Ambiguous (:196-209). Carrier rows of kind Reference/Array/Slice/Literal "bind by form" and compare true (:380-383, comment :369-371), Builtin/Data/GenericParameter by identity (:377-379, :387-409), all else false (:384).
5. Cases by shape. Alias constituent paths of 1 or 2 members only (:478-482); `domain_head_admits` handles only carriers that are GenericParameter, Builtin or Data (:573-578). Quote :474: "A longer path crosses a package, which this source set cannot bind."
6. Same job elsewhere. Three near-identical unique-or-ambiguous scans over domains: :176-222 (`resolver_declared_domain`), :504-550 (`domain_by_head`), :609-659 (`domain_named`); the same two-phase scan is `declared_named` (imports.omg:650) and `machine_named` (overloads.omg:160). The type-run comparison also exists in machine_signatures.omg:388-443 (`same_type`) and :445-560.
7. Unused. grepped: `carrier_matches` 2, `domain_named` 2, `bind_domain_paths` 2, `check_domains` 2, `bind_domain_carriers` 2 (def + one caller each); `domain_admits` 4, `known_domain` 5, `resolver_declared_domain` 3, `domain_by_head` 3.
8. Stated limits. :474. Header :26-37 (predicate facts "belong to checking").
9. Fail-open. `same_bound_rows` :380-383 returns true for Reference/Array/Slice/Literal rows; traced: the caller `same_head` first requires `same_type_form` (syntax_trees.omg:93-110), which compares kind, access, argument count and, for integer rows, the decoded value, so the "true" is bounded by the form check; it does not compare lifetime arguments (inferred from same_type_form).

### B6. facts.omg (452 lines)
1. Purpose. Resolves names inside `requires`/`ensures` fact expressions of machines, callable contracts, data declarations and domains.
2. Shape. `bind_fact_names` :42; `fact_scope` :114 and `subject_scope` :139 and `within_fact` :158 (build `NameScope`); `bind_clause_facts` :177; `bind_data_facts` :215; `bind_data_name` :289; `data_value_name` :406.
3. In/out. Writes `value_bindings[expression]` (bound 131072 :373).
4. How it decides. By clause kind (`kind == ClauseKind::Requires/Ensures` :199, :237) and scope: a machine's `requires` sees parameters; its `ensures` also sees `ReturnedValue{machine}` (:58); a callable contract's `ensures` sees `ContractReturnedValue` (:75); a data fact sees `self`, direct fields, then value binders (:406-452); `self` matched by `span_spells(..,"self")` :409. A proposition applies only as the whole fact (`proposition_site`, names.omg:1336).
5. Cases by shape. Data fact names: only expression kinds Name and Path are bound (:302-306); a bare Name in a data fact is a field, then a binder; a callee a machine or proposition.
6. Same job elsewhere. Duplicates names.omg's scope building (`fact_scope` here vs the `NameScope` literals at names.omg:159-173, :280-294).
7. Unused. grepped: `bind_fact_names` 2.
8. Stated limits. :35-36 "case-local predicates are retired" ; :38-39 proposition inside a larger fact "rejects as unsupported".
9. Fail-open. (a) :305 `_ -> done()` leaves any other expression kind with no binding; traced: such nodes (calls, binaries) hold no names themselves, their children are visited by index (:260). (b) :316-319: a `MatchedSubject` name whose role is not `WholeSubjectBinder` is skipped with no binding or error; open (data facts have no `match`, so no input seen reaches it).

### B7. forms.omg (303 lines)
1. Purpose. Early refusal of forms the resolver does not yet record.
2. Shape. `check_declaration_forms` :54; `check_binder_forms` :150; `check_machine_forms` :186; `callable_contract_clauses` :246; `bindable_token` :282.
3. In/out. Writes diagnostics (UnsupportedDeclaration, UnsupportedGenericParameter, UnsupportedMachineContract, UnsupportedOperatorToken).
4. How it decides. By root kind (`RootItemKind::Definition` always rejects :69; a refinement trait rejects :105-106; a trait with any contract rejects :112-125); by binder kind (Proposition always rejects :165; Evidence rejects unless `admit_evidence`, true only for a trait :119);
   an operator token binds only if `item.operator == None` or (`bindable_token` and root container, `parent == 0`, `has_owner`) :190; bindable tokens are Add, Subtract, Multiply, Divide, Modulo, Less, LessEqual, Greater, GreaterEqual :282-294.
   A machine contract is accepted only if kind Machine, no generics, no nested contracts, and clauses are Requires/Ensures/Crashes/CrashesNone/Suspends/Blocks :215, :258-268.
5. Cases by shape. Closed token list above; closed clause list above. Quote :32 "Forms whose bindings resolution does not record yet." and :37-38 "a contract's binder needs a binding table this resolver does not keep yet."
6. Same job elsewhere. The header :32-38 says a machine with a `requires machine` contract "rejects", but the code at :195-199 and :212-219 admits a structural contract and contracts.omg:25 binds it: the comment and the code disagree. Parser-side clause admission is clause.omg:867 (`clause_admitted`); the parser admits every clause kind inside a contract (:871) while this file refuses Reaches/Invokes/Satisfies/Terminates there.
7. Unused. grepped: `check_machine_forms` 2, `bindable_token` 2, `check_declaration_forms` 2 (def + one caller).
8. Stated limits. :32-53 whole header.
9. Fail-open. `_ -> root(...index+1)` :75: Use, Constant, Machine, Proposition-not-listed roots pass this file; traced: machines are checked in `machine_at` :128; constants/uses are not form-restricted here.

### B8. generic_scopes.omg (404 lines)
1. Purpose. Scopes and names of generic parameters and lifetimes: scope constructors, duplicate/builtin-shadow checks for binders, annotation roles, lexical generic lookup and lifetime declaration checks.
2. Shape. `data_scope` :23, `resolver_scope` :28, `data GenericCoordinate` :40 (Declaration/Container/Lexical), `generic_coordinate` :46, `same_generic_coordinate` :69, `check_generic_names` :93, `bind_generic_annotations` :157, `generic_annotation_role` :188,
   `generic_named` :201, `empty_scope` :231, `lifetimes_declared` :238, `lifetime_named` :270, `lifetime_among` :280, `lifetime_arity` :308, `scoped_generic_named` :342, `scoped_lifetime_named` :373, `scope_parent` :393.
3. In/out. Reads syntax tables; `TypeScope` values.
4. How it decides. Spelling (`spans_equal`) and parameter kind. A `Type` binder may not spell a builtin (`builtin_type_spelling` :114); other kinds may. Lexical lookup walks `item.parent` links; a found `GenericParameterKind::Value` binder of an ancestor is skipped (:362-364) so ancestor runtime values are never inherited.
   Lifetime arity counts `Lifetime` kinds of a data declaration only (:308-319).
5. Cases by shape. Signature-local generics compare by position, lexical ones by exact handle (comment :38-39).
6. Same job elsewhere. Ancestor walks: containers.omg:354-421, names.omg:1573-1613. Pairwise duplicate scan: see B4 item 6.
7. Unused. grepped: `scoped_generic_named` 2, `same_generic_coordinate` 2, `empty_scope` 5, `resolver_scope` 19, `check_generic_names` 7, `bind_generic_annotations` 7.
8. Stated limits. none. 9. Fail-open. none seen (`lifetime_arity` returns 0 for non-data bindings :313, so a Trait/Builtin applied with lifetime arguments is refused by the arity check in type_names.omg:148, not accepted).

### B9. identities.omg (432 lines)
1. Purpose. Checks stable schema identities `#N` and retired identities on data fields, cases and payload fields: all-or-nothing numbering per scope, valid and distinct values.
2. Shape. `IdentityScopeKind` :29, `IdentityScope` :37, `MemberIdentity` :46, `IdentityValue` :55, `check_stable_identities` :60, `check_identity_scope` :155, `resolver_member_identity` :329, `identity_value` :358, `identity_scope_has_retirements` :384, `retirement_in_scope` :406.
3. In/out. Reads `IntegerLiteral` rows; writes diagnostics (PartialStableIdentities, InvalidStableIdentity, DuplicateStableIdentity, UnsupportedDeclaration).
4. How it decides. By decoded value, not spelling (comment :25-26: "`#1` and `#0x1` collide"): valid iff `exact && !empty_digits && suffix == None` :368. A scope that has any retirement, or any numbered member, is "numbered" and every member must carry an identity :164-192.
   A retirement belongs to a scope by owner equality (:409) and kind (:420-430).
5. Cases by shape. A data declaration with both fields and cases plus a declaration-level retirement rejects (:74-76, :86-100). Quote :80-82: "Keep the existing refusal rather than donating the number to both scopes."
6. Same job elsewhere. none seen.
7. Unused. grepped: `check_stable_identities` 2.
8. Stated limits. :80-82. 9. Fail-open. A scope with no numbered member and no retirement is accepted unchecked :159-161; stated by the rule "unnumbered, or numbered throughout" :152.

### B10. imports.omg (813 lines)
1. Purpose. Package/module visibility and declaration lookup: which package a `use` path selects, whether the module exists, which declarations a unit can see, and the shared lookup machinery (`Lookup`, `DeclarationKind`, `declared_named`, `visible_if`).
2. Shape. `ImportTarget` :67; `check_imports` :74; `import_target` :112; `names_toolchain_mount` :179; `edge_names` :216; `undeclared_dependency` :243; `names_some_unit` :297; `resolver_visible` :338; `resolver_same_package` :398; `resolver_same_module` :405 (byte compare of module path);
   `use_names_unit` :444 (members joined by `::`, byte 58); `data Lookup` :558 (Absent/Found/Ambiguous); `DeclarationKind` :568; `DeclarationPath` :579; `declaration_count` :586; `declaration_path` :600; `path_spells` :638; `declared_named` :650; `data_named` :749; `measure_named` :758; `visible_if` :791.
3. In/out. Reads `SourceSet` package/dependency/unit rows; no writes except diagnostics.
4. How it decides. Import target keyed on spelling of the first member: the mount `omega::language::core` (three members spelled "omega","language","core" :189-192, `member_count >= 3`) selects the first package with `row.toolchain` :131; a first member spelling a product dependency alias selects that dependency :152; the requester's own package name or any other first member selects the requester's package.
   Visibility: same unit or same module sees everything :345; otherwise a declaration is "open" if `public` or same package :347 and the importing unit must have a `use` row that names the declared unit :357-385 (only the importing unit's own uses count :366). Another package's name or a build-scope alias with no module rejects UndeclaredDependency :243-293.
5. Cases by shape. Quote :51-52 (header): "Otherwise a first member spelling one of the requester's product dependency aliases selects that dependency". Mount is exactly three members.
6. Same job elsewhere. `declared_named` :650-746 is one of five unique-or-ambiguous scans (see B5 item 6). `resolver_same_module` is the module-equality test used by declaration_names, domains, machine_signatures, overloads (grepped 9 occurrences).
7. Unused. grepped: `use_names_unit` 3, `declaration_count` 2, `measure_named` 2, `resolver_visible` 4, `declared_named` 11, `visible_if` 7, `resolver_same_package` 7, `import_target` 4, `data_named` 3, `declaration_path` 4, `path_spells` 3.
8. Stated limits. none. 9. Fail-open. `names_toolchain_mount` has no toolchain package -> `found:false` -> unresolved import :125, :172-174 (refuses). `own` :159-165: any first member not matching selects the requester's own package with `selector_count 0`, so an import resolves against own modules; refused later if no unit matches (:306-333).

### B11. machine_signatures.omg (560 lines)
1. Purpose. Duplicate-machine and duplicate-parameter checks, and the signature equality used for overloads and operator shapes.
2. Shape. `check_machines` :39; `check_parameter_names` :144 (public); `same_machine_signature` :227; `same_parameters` :289 (public); `same_type` :388 (public); `same_type_row` :445.
3. In/out. Reads resolved type bindings and domain bindings; writes diagnostics DuplicateMachine, DuplicateParameter, DuplicateLocalBinding.
4. How it decides. Two machines duplicate when same parent, `spans_equal` name, same owner flag/spelling, same generic kinds and parameter types position by position, and not "two distinct targets" (:246, :268-273); only within one module (`resolver_same_module` :128). Types are equal when their runs have equal length and agree row by row: `same_type_form` + domain binding + `type_binding` identity;
   generic parameters by position via `GenericCoordinate` (:541-545).
5. Cases by shape. Target-scoped families: both have a target and targets differ -> not duplicates :270.
6. Same job elsewhere. `same_parameters` is reused by operators.omg:116; type-run comparison parallels domains.omg:252-414.
7. Unused. grepped: `same_type` 3, `same_parameters` 4, `check_parameter_names` 3.
8. Stated limits. none. 9. Fail-open. `_ -> formed()` :491 makes Reference/Array/Slice/Literal bindings equal; traced: `TypeBinding` has exactly those four remaining cases besides the enumerated ones (symbol_resolved_trees.omg:50-66, grepped), and the form check ran first; lifetime arguments are not compared (inferred).

### B12. measures.omg (141 lines)
1. Purpose. Validates declared ranking measures: a computed measure (one parameter, result type, one-statement body) or a lexicographic measure (fields of a data owner).
2. Shape. `check_measures` :24.
3. In/out. Writes `component_field` rows via `set_component_field(..., handle + 1)` :132.
4. How it decides. `declared.container == MachineContainer::Measure` :38; `component_count > 0` selects the lexicographic form :45; computed form requires `parameter_count == 1 && has_return_type && has_body && statement_count == 1` :52-53; components matched to owner fields by `spans_equal` :115.
5. Cases by shape. Two fixed forms. 6. Same job elsewhere. `measure_named` imports.omg:758 looks measures up. 7. Unused. grepped: `check_measures` 2. 8. Stated limits. none.
9. Fail-open. Component types and duplicate components are not checked here; open (not traced to the checker).

### B13. names.omg (1,614 lines)
1. Purpose. Binds every name and path in machine bodies and constant values, by position (role) and in a fixed search order; also memberships, domain paths, callee lookup, propositions, and the primitive spellings `size_of`/`align_of`/byte predicates.
2. Shape. `NameRole` :78; `NameScope` :90; `bind_body_names` :114; `bind_constant_values` :145; `assign_name_roles` :182; `bind_body` :268; `bind_statement` :368; `resolver_bind_arm` :436; `bind_tree` :560; `bind_name` :614; `name_binding` :688; `local_name` :765; `check_local_binding` :869;
   `bind_membership` :920; `DomainPath` :997 + `resolver_domain_path` :1004; `constant_name` :1055; `machine_name` :1073; `owned_callee` :1091; `lexical_callee` :1118; `callee_name` :1152; `declaration_callee` :1172; `free_callee` :1217; `resolver_layout_query` :1262; `resolver_byte_predicate` :1289;
   `proposition_site` :1336; `generic_name` :1370; `resolver_binder_named` :1422; `binder_names` :1451; `data_name` :1473; `case_or_machine` :1490; `lexical_value_binder` :1573; `lexical_static_value_binder` :1592.
3. In/out. Writes `value_bindings[expression]` (bound 131072) and `path_bindings`; reads everything else.
4. How it decides. Roles: callee of Call/Apply, construction of Construct, every other node is a value (:216-221); case patterns and targets get roles at :468, :551. Value search order (comment :760-763): latest earlier `let` (spelling, scanning back :771-783), arm binders, parameters, domain subject `self` (:820), `result` in an `ensures` (:828), enclosing value binder, then a constant.
   Callee search: machine binder, lexical (nested) machines, fellow member of the enclosing trait/conformance, free machine, then layout query then byte predicate (:1155-1329). A machine and a proposition of one spelling are Ambiguous :1222-1225. Spellings tested as source text: "self" :820, "result" :828, "size_of"/"align_of" :1265-1266, "valid_utf8"/"no_nul"/"ascii_only"/"non_empty" :1292-1295.
5. Cases by shape. Paths must have exactly two members (`expression.count == 2` :719), else Unresolved; memberships need `expression.count == 1` (:930) and a domain path of 1 or 2 members (:937-941). Quote :716-717: "a longer path crosses a package, which a single unit cannot bind." Quote header :72-73: "domain predicates and proposition bodies bind no names yet."
6. Same job elsewhere. Lexical ancestor walks: containers.omg:354-421, generic_scopes.omg:342-404. `NameScope` literals duplicated at :159-173, :280-294, :331-345, :348-363 (and facts.omg:121-174). `data_name`/`constant_name`/`machine_name` are three wrappers over the same `Lookup`->`ValueBinding` mapping (:1055-1086, :1473-1486).
7. Unused. grepped: `name_binding` 3, `machine_name` 4, `free_callee` 3, `proposition_site` 3, `declaration_callee` 4, `binder_names` 4, `resolver_role` 4, `resolver_domain_path` 4, `bind_tree` 16; `bind_body_names` (def + resolver.omg:166). `target_name` :1110 just calls `callee_name`.
8. Stated limits. :72-73, :716-717, :917-919 ("a membership in several domains at once is an expression form resolution does not bind yet").
9. Fail-open. (a) :634 `NameRole::Value -> done()` for a name pre-marked `MatchedSubject`: no lookup, no error; traced: pattern_binders.omg:172 stored the `MatchedSubject` binding earlier, so the name is bound to the match subject, not unbound. (b) :378 `StatementKind::Crash -> done()` binds nothing (a crash has no expression). (c) :881 `_ -> done()` in `check_local_binding` lets any non-colliding binding through (intended). (d) `bind_tree` visits only Name, Path, Membership nodes :575-579; other nodes carry no names. none else seen.

### B14. operators.omg (165 lines)
1. Purpose. Assigns each operator-token machine its home data declaration and rejects foreign or duplicate homes.
2. Shape. `bind_operator_homes` :23; `earlier_operator_shape` :86; `homed_at` :133; `set_operator_home` :148 (bound 10240).
3. In/out. Writes `operator_homes[machine]` (`TypeBinding::Data{item}` or `Unresolved`).
4. How it decides. Owner resolved with `type_binding` on the owner spelling :46; home must be a `Data` declaration in the same package (`resolver_same_package` :57); else ForeignOperatorHome :74. Duplicate = same token, same home, `same_parameters` :116.
5. Cases by shape. Only a data owner. Quote header :18-19: "an owner naming a builtin carrier, a trait or another package's data rejects at the owner."
6. Same job elsewhere. Duplicate-shape detection also in machine_signatures.omg:227 (`same_machine_signature`).
7. Unused. grepped: `bind_operator_homes` 2, `homed_at` 4. 8. Stated limits. :14-22. 9. Fail-open. none seen.

### B15. overloads.omg (315 lines)
1. Purpose. Publishes, per machine-name use, the overload family a call may walk, split into all-members (same package) and public-members (other packages); and the machine lookup/visibility/public tests.
2. Shape. `bind_visible_overloads` :26; `visible_overloads` :69; `link_overloads` :117; `overload_unit_index` :150; `machine_named` :160; `machine_spells` :243; `machine_visible_if` :262; `machine_public` :290.
3. In/out. Rewrites `value_bindings[expr]` from `Machine{handle}` to `Overloads{representative, candidates}` (:63); fills `overload_heads[10240]` once per representative.
4. How it decides. Family = machines with the same spelled path (`machine_spells` :243, excludes `MachineContainer::Measure` :257), same lexical `parent` (:128) and same module (:135). Exported = `machine_public` by container (root marker, or trait/conformance visibility :290-310). The use sees the family via `resolver_visible(..., true)` :74; local if same package :75.
   Ambiguity: another visible machine of the path in a different module :220-222.
5. Cases by shape. Top-level only: `machine_named` considers `item.parent == 0` :181.
6. Same job elsewhere. Unique-or-ambiguous scan family (B5 item 6).
7. Unused. grepped: `machine_named` 5, `machine_spells` 6, `machine_visible_if` 3, `machine_public` 5, `bind_visible_overloads` 2.
8. Stated limits. none. 9. Fail-open. `visible_overloads` returns an empty family when not visible or `representative >= 10240` :76-78 (typing then finds no candidate); refusal, not acceptance.

### B16. pattern_binders.omg (192 lines)
1. Purpose. Marks, before name binding, which names inside an arm's guard/target refer to a whole-subject binder pattern.
2. Shape. `mark_subject_patterns` :23; `mark_arm_range` :71; `mark_subject_binder` :95; `mark_subject_uses` :129; `mark_subject_name` :162; `pending_value_binding` :179.
3. In/out. Writes `ValueBinding::MatchedSubject{subject}` into `value_bindings` and role `WholeSubjectBinder`.
4. How it decides. By spelling: any Name node of role Value in the arm's guard/target whose text equals the binder pattern's name (`spans_equal` :149). Statements first, then expressions in descending row order so outer matches precede nested ones (:48-61; comment :20-22).
5. Cases by shape. Binder arms only (`ArmPatternKind::Binder` :83). A nested binder with the same name as an enclosing marked one rejects DuplicateLocalBinding :99-101.
6. Same job elsewhere. none seen. 7. Unused. grepped: `pending_value_binding` used in names.omg, facts.omg. 8. Stated limits. none.
9. Fail-open. none seen. Note: marking is by spelling, so a `let` or parameter with the same spelling inside the arm is also marked (inferred from :143-150).

### B17. resolver.omg (198 lines)
1. Purpose. Owns pass order and the first rejection.
2. Shape. `data Resolver` :15 (`output: NameResolution`, `name_roles: [NameRole;131072]`, `overload_heads: [OverloadLinks;10240]`); `resolver_initialize` :22; `resolve` :26.
3. Order (each step stops on `!accepted`): place_machines, check_declaration_forms, check_imports, declare_roots, check_members, check_measures, check_declaration_namespaces, bind_machine_contracts, check_stable_identities, bind_domain_carriers, check_domains,
   bind_references, bind_declaration_references, bind_machine_references, bind_body_references, check_machines, bind_operator_homes, bind_domain_paths, bind_body_names, bind_fact_names, bind_clause_names, then `publish` (:189-195: bind_visible_overloads, set the four counts).
4. How it decides. Only by `output.accepted`. 5. Cases by shape. fixed order. 6. Same job elsewhere. `name_resolution_clear` is called both in `resolver_initialize` :23 and at the start of `resolve` :29.
7. Unused. grepped: `resolve` is the product entry (area map). 8. Stated limits. none. 9. Fail-open. `place_machines` :30 and `check_declaration_forms` :31 run before an `accepted` test, but the test follows immediately :32. none seen.

### B18. type_names.omg (705 lines)
1. Purpose. Binds one type reference row by row (builtin, generic parameter, data, trait, conformance, machine argument) under the role its position allows, plus `in` domain qualifiers and the builtin-spelling tests.
2. Shape. `TypeRole` :26; `bind_type_reference` :47; `bind_type_row` :95; `type_binding` :325; `trait_or_conformance_named` :406; `bind_domain` :441 (`requires reference < 65536`); `builtin_type_spelling` :543; `builtin_type` :586; `trait_name_admitted` :703.
3. In/out. Writes `type_bindings[row]`, `domain_bindings[row]`, `domain_items[row]` (bound 65536 :275).
4. How it decides. `type_binding` order (comment :321-324): generic parameter of the scope, enclosing generic, builtin by spelling, then the single visible data, trait or conformance; a visible data and a trait/conformance of one spelling are Ambiguous :367-370. Role admission: a trait only at the root of Trait/StaticParameter positions :703-705; a conformance not directly under a reference/array/slice :220.
   Builtin spellings (exactly 21, `span_spells`): bool i8 i16 i32 i64 u8 u16 u32 u64 addr f32 f64 Slice SyscallResult Terminal Never UInt Int AtomicBool AtomicU32 AtomicU64 (:589-688). `builtin_type_spelling` also treats `i` or `u` followed by zero or more digits as taken :554-573 (so `u` and `i` alone count).
5. Cases by shape. Quote :439: "A longer path crosses a package, which this source set cannot bind." Lifetime arguments must equal the data's lifetime-binder count or be absent :148. A Named name that resolves to no type is accepted only as a root static argument without domain, resolved as a machine :189-201.
6. Same job elsewhere. Spelling chain parallels domains.omg:136-172 and clauses.omg:466-516.
7. Unused. grepped: `builtin_type` 4, `builtin_type_spelling` 3, `type_binding` 8, `bind_type_reference` 16.
8. Stated limits. :439. 9. Fail-open. `TypeReferenceKind::Literal` always binds as a Type :130 (no check of its value here); open (value checked in typed stage references.omg:336, not read).

### B19. type_reference_owners.omg (710 lines)
1. Purpose. Walks every owner of a type reference (data members, traits, conformances, constants, propositions, machine signatures, `let` types, casts, static applications) and binds each in the right lexical scope.
2. Shape. `bind_references` :48; `bind_declaration_references` :125; `bind_machine_references` :223; `bind_signature` :299; `bind_body_references` :377; `detached_scope` :575; `clauses_hold` :656; `tree_holds` :689; `resolver_statement_end` :708.
3. In/out. Calls `bind_type_reference` / `bind_generic_annotations`; no new tables.
4. How it decides. By table position: machines own contiguous statement and expression runs, so one cursor `owner` advances (:389-463). An expression outside any machine takes the scope of the declaration whose fact tree contains it (`tree_holds` :699: `first_row <= index <= root`), scanned in order domains, transparent propositions, data (then each case), else empty scope (:578-618).
5. Cases by shape. Domain-only casts (`cast.flag`) bind nothing :543-546 (comment :530-533). Expressions in trait/conformance/refinement clause trees are not among the detached owners (inferred: they get the empty scope at :618).
6. Same job elsewhere. none seen. Note: `detached_scope` is documented as asked "rarely" (:573-574) but is computed eagerly by `let scope = detached_scope(...)` at :450 and :474, i.e. per expression visited (inferred from reading; not measured).
7. Unused. grepped: none public; each has one caller in resolver.omg. 8. Stated limits. none. 9. Fail-open. `data :618 found(empty)`: an expression held by no known tree binds in an empty generic scope, which can only make names fail, not pass.

---------------------------------------------------------------------------------------------------------------------
## C. tokens_to_syntax_trees (the parser, 17 files, 11,427 lines)

Parser state is one big `data Parser` (parser.omg:29-247) with explicit stacks and the `requires` bounds listed at :30-45 (expression operands 64, operators 64, frames 32, pending args 256, fields 64, arms 64, type frames 16, type args 32, lifetimes 32, ranked subjects 8,
header frames 257, header clauses 2048, header contracts 256). Contextual words are matched by decoded spelling through `at_word` (input.omg:337), keywords by `KeywordKind`.

### C1. parser.omg (887 lines)
1. Purpose. Whole-file driver: classifies each root item by its first token and dispatches to the declaration grammar; parses `use` rows.
2. Shape. `data Parser` :29; `parser_initialize` :252; `begin_unit` :258; `parser_source_span` :266; `identifier_span` :277; `parse` :299 (`next_root`, `classify_root`); root-head builders :841-887.
3. In/out. Reads `TokenStream`; appends to `parser.output` (`SyntaxTrees`).
4. How it decides. By first token: keywords `use`, `data`, `machine`, `let`, `public` (:342-346); identifiers by contextual word in a fixed chain `boundary` -> `trait` -> `domain` -> `const` -> `proposition` -> `measure` -> `operator` (retired) -> `target machine` -> `target boundary` -> conformance (next token `satisfies` or `<`) -> else UnsupportedRootItem/ExpectedItemAfterPub (:376-475).
   `public measure` rejects MeasureNotPublic :429. `public` must be followed by `data`, `machine`, `let` or an identifier :363-370 (not `use`).
5. Cases by shape. Use path: members separated by `\` or `::` ending with `;` (:720-722); keywords other than `self` count as members :684. Retired introducers `operator`, `boundary requirement` reject at the word (:437-441, :516-528).
6. Same job elsewhere. none seen. 7. Unused. grepped: `parse` is the stage entry; `parser_initialize` per area map. 8. Stated limits. Header :287-298 (retired introducers).
9. Fail-open. none seen.

### C2. input.omg (641 lines)
1. Purpose. Token cursor and one-token helpers: take/expect/at/next_is, name rules, integer literals, erased marker, declaration-head lookahead, decoded-spelling compare.
2. Shape. `load_current` :21; `expect_punctuation` :56; `expect_name` :81; `at_path_separator` :101; `at_punctuation` :108; `at_keyword` :134; `at_name` :160; `take_current` :190; `take_integer_literal` :202; `reject_current` :245; `parse_erased_marker` :271; `record_erased` :305; `at_word` :337; `at_integer` :364;
   `next_is_punctuation/keyword/word/adjacent_punctuation` :393-459; `at_declaration_head` :468; `generic_conformance_ahead` :518; `decoded_spells` :599.
3. In/out. Reads `TokenStream` (token array bound 65536 :29, decoded bytes bound 131072 :617); writes the cursor and spans.
4. How it decides. A name is an Identifier or any keyword except `self` :172-175. Path separator is `\` or `::` :104-105. `at_declaration_head` (comment :461-467): keyword `use|data|machine|public`, or two adjacent names unless the second is `as` or `in`, or `domain` followed by `<`/`[`, or a generic conformance `Name<...> satisfies` (scan accepts only a fixed token set :547-565).
5. Cases by shape. Erased marker is exactly `[erased]` :284. Quote :264-266: the comment "Whether the current token is an identifier spelling exactly `spelling`" sits above `parse_erased_marker`, not above `at_word` :337.
6. Same job elsewhere. The static-argument lookahead scan expression.omg:2459-2521 is a second copy of the `generic_conformance_ahead` token-set scan (:518-596).
7. Unused. grepped: `parse_erased_marker` 5, `record_erased` 8; `next_is_adjacent_punctuation` 1 caller (expression.omg:855, by sight).
8. Stated limits. none. 9. Fail-open. `at_declaration_head` is a heuristic: a statement that starts with two names (for example `a b`) is classified as a declaration head (:502), changing how a `;`-ending clause continuation ends (clause.omg:831); not a wrong accept, a different parse.

### C3. machine.omg (757 lines)
1. Purpose. Machine declaration grammar: head (`MachineHead`), operator token, owner `Owner::name`, generics, lexicographic measure components, signature, header clauses/contracts, body statements, nested machines, bodyless `;`.
2. Shape. `data MachineHead` :23; `parse_machine` :47; `machine_clause_owner` :461; `parse_operator_token` :480; `retain_contract` :547; `machine_at_root` :565; `parse_signature` :583; `parse_parameters` :622.
3. In/out. Appends `MachineItem` (:394-434) at the next machine handle; root item only for root/measure containers :384-392; nesting via `machine_parent = machine_current + 1` :338.
4. How it decides. Operator tokens accepted by the parser: `+ - * / % == != < <= > >= []` and `[..]` (:492-527); a conformance member that spells a token rejects OperatorTokenNotAdmitted :88-98. `Owner::name` only at root with `machine_parent == 0` :115. Measure container with word `lexicographic` takes `{ a, b }` components :142-218.
   Clause owner: Trait container -> TraitMember, everything else Machine :461-473. Body members: `machine` nests, anything else but `}` rejects ExpectedMachineBodyEnd :321-351.
5. Cases by shape. Capacities: machines 10240, parameters 32768, contracts 256, roots 4096, members 32000. No trailing comma in parameters (comment :732).
6. Same job elsewhere. The parser admits `==`, `!=`, `[]`, `[..]` tokens here while the resolver binds only the nine tokens in forms.omg:282-294 (stated in forms.omg:49-53).
7. Unused. grepped: `retain_contract` 3. 8. Stated limits. none. 9. Fail-open. none seen.

### C4. callable_header.omg (414 lines)
1. Purpose. Parses a callable's clause header including nested `requires machine Name(...) ... ;` contracts with an explicit frame stack.
2. Shape. `CallableHeaderFrame` :20; `parse_callable_header` :29; `header_child_first` :323; `callable_contract` :335; `top_header_frame` :363; `replace_header_frame` :376; `push_header_frame` :389; `pop_header_frame` :405.
3. In/out. Stages clauses/contracts in `parser.header_*` then publishes contiguous ranges (:212-282).
4. How it decides. A nested contract starts when the word `requires` is followed by the keyword `machine` (:95-96). A late return arrow `-> Type` is read once per frame, only if the frame has no return yet (:173-176). A crash ceiling `crashes none` beside a `crashes <cause>` in one frame rejects ConflictingCrashCeiling (:236-247).
5. Cases by shape. Depth `< 257` :392; clauses `<= 2048`, contracts `<= 256` including open and staged ones (:57, :104-108). Comment :14-19 states the single-terminator rule.
6. Same job elsewhere. Clause parsing proper is clause.omg:23. 7. Unused. grepped: `parse_callable_header` 2. 8. Stated limits. none. 9. Fail-open. none seen.

### C5. clause.omg (1,157 lines)
1. Purpose. Parses one run of clauses: reaches, invokes, suspends, blocks, crashes, requires, ensures, satisfies, terminates (with ranking subjects, view and rank range).
2. Shape. `parse_clauses` :23; `ClauseContinuation` :774; `after_clause_semicolon` :789; `at_clause_word` :846; `clause_admitted` :867; `append_member` :912; row builders :933-1096; `stage_subject` :1099; `commit_subjects` :1119.
3. In/out. Appends `Clause` rows (bound 2048 :676), members (32000 :915), ranked subjects staged (8 :1102) then copied into `arguments` (65536 :1136).
4. How it decides. By clause word spelling (:30-95); `requires` followed by keyword `machine` is a contract, not a clause (:70-75). Which clause kinds an owner admits (:867-909): Machine/TraitMember/Contract all; Refinement only Reaches, Suspends, Blocks, Terminates; Domain and Data only Requires; DataCase none
   (and `requires` on a DataCase rejects RetiredCasePredicate :116). `where` rejects RetiredWhereClause :107-111. Crash causes: `Trap`, `Abort`, or `none` (:616-642). After a `;` a non-callable owner looks one token ahead to decide whether the list continues (:802-836).
5. Cases by shape. `satisfies Trait[<args>]::requirement [as label] [via value]`; a contract owner takes no label/via (:244-250). `terminates [by subject | (subjects)] [-> View] [in lo..hi]`; `by` on a refinement rejects :317-319.
6. Same job elsewhere. Resolver-side clause acceptance for contracts: forms.omg:246 (`callable_contract_clauses`) refuses Reaches/Invokes/Satisfies/Terminates that this file admits (B7 item 6).
7. Unused. grepped: `parse_clauses` 6; `at_clause_word` 2 (def + clause.omg:820).
8. Stated limits. Header :18-22. 9. Fail-open. `retired_where` `_ -> done()` :111 leaves the next token to the caller, which then rejects; none seen as accept.

### C6. conformance.omg (172 lines)
1. Purpose. `[public] Name [<generics>] satisfies Trait<args> { machine members }`.
2. Shape. `parse_conformance` :25. 3. In/out. Appends `ConformanceItem` and root (:148-164); cap 256 conformances, 4096 roots.
4. How it decides. Only the keyword `machine` is accepted at a member position, else ExpectedConformanceMember :97-101; each member must have a body :116-122.
5. Cases by shape. Quote :20-22: "any token other than `machine` at a member position rejects as `ExpectedConformanceMember`".
6. Same job elsewhere. Trait members parse in traits.omg:243-333 (also `machine`-only plus body parents). 7. Unused. grepped: via parser.omg:572 caller. 8. Stated limits. :20-22 (retired `Trait::requirement = Path;`). 9. Fail-open. none seen.

### C7. constant.omg (148 lines)
`[public] const [Owner::]NAME: Type = value;` :9-13, `parse_constant` :13. Caps 512 constants, 4096 roots. One owner level only (:43-65). Value parsed by `parse_expression(..., true, true)` :93. No fail-open seen; no stated limits.

### C8. data.omg (861 lines)
1. Purpose. Data declarations: head, generics, `[copy|linear]` properties, clauses, fields, cases with payloads, stable identities, retirements, `boundary data Name;`.
2. Shape. `parse_data` :33; `parse_properties` :658; `parse_retired_identity` :737; `parse_stable_identity` :817; `retain_member_identity` :856.
3. In/out. Appends fields, cases, payload fields, data members, `DataItem`, root, retired identities, erased bindings.
4. How it decides. `case` keyword at a member position starts a case unless followed by `:` or `[` (then a field named `case`) :183-201. `retired` identifier (word) starts a retirement :152, :347. Multiplicity: `copy` -> Unrestricted, `linear` -> Linear, repeats reject Duplicate/ConflictingDataMultiplicity :668-697.
5. Cases by shape. Caps: data 256, fields 1024, cases 1536, data members 2048, payload fields 1024, retired identities 2048, roots 4096. Header comment :22 mentions `[where facts]` but clause.omg:107-111 rejects `where` (stale comment, read).
6. Same job elsewhere. none seen. 7. Unused. grepped: `parse_properties` 3. 8. Stated limits. :20-21 "compatibility parsing does not extend the language surface." 9. Fail-open. none seen.

### C9. definition.omg (163 lines)
`[public] [boundary] let Name [<g>] [(params)] : Type [= term];` :9-16, `parse_definition` :16, cap 256. `boundary let` has no term (:88-91). The resolver rejects every `Definition` root (forms.omg:69), so this grammar's output is accepted by the parser and refused by `check_declaration_forms`. No fail-open.

### C10. domain.omg (323 lines)
`domain [<g>] Carrier::Name [<indices>] tail` where tail is `= Path {& Path};` or `[requires facts] [established by Path{, Path}];` (:12-20), `parse_domain` :21, cap 256. Authored `(subject)` rejects RetiredDomainSubject :143. Carrier is a bracketed type or a single name (:61-99). Retained at its last token. No fail-open seen.

### C11. generics.omg (297 lines)
1. Purpose. `<parameter {, parameter}>` list: lifetime `'a`, `machine f`, `proposition p`, `const N: T`, `name satisfies Trait<..>`, `name: T [props]`, `name [props]`.
2. Shape. `parse_generic_parameters` :25; constructors :247-297.
3. How it decides. Words by spelling `const` :73, `proposition` :81, `satisfies` :135; keyword `machine` :65. Lifetimes must precede every non-lifetime binder (GenericLifetimeOrdering :48-49, :163). Cap 256 generic parameters in the table (:33, :211).
4. Same job elsewhere/unused. grepped: `parse_generic_parameters` 9. Fail-open: none seen.

### C12. path.omg (70 lines)
`parse_name_path` :11: `name {:: name}` or `\`; cap 2048 name paths (:49); first name's diagnostic given by caller. No fail-open.

### C13. proposition.omg (184 lines)
`parse_proposition` :15: body `;` (Primitive), `evidence Type;`, or `= fact;` (Transparent), found by `;`, then the word `evidence`, then `=` (:69-90). Cap 256. Fact parsed with membership allowed, construction not (`parse_expression(...,false,true)` :108). No fail-open.

### C14. statement.omg (1,178 lines)
1. Purpose. Statement grammar for bodies: `let`, `crash`, `_ = value;`, assignment, expression statement, tail value, `transition [subject] { arms }` with patterns, guards and targets.
2. Shape. `parse_statements` :32; `acknowledges` :1124; `pattern_binder` :1142; `renamed_binder` :1155; `valued_pattern_field` :1168.
3. In/out. Appends statements (cap 16384 :399), arms (8192 :1069), pattern fields (2048 :729).
4. How it decides. By token: keywords `let`, `transition`; `if|loop|while|return` reject with the caller's `unsupported` code (:54-57, comment :29-31); identifier word `crash` always opens a crash; `_` followed by `=` opens a discard (:66-71). An expression followed by `=` is an assignment, `;` a statement, `}` or the keyword `machine` a tail (:302-310).
   Arm patterns: `_` wildcard; with a subject a Name is a Binder (a Name followed by `{` is a Case), a Path is a Case, anything else a Literal; without a subject every pattern is a Guard (:526-579). Arm target: `{}` Unit; otherwise an expression; a Call target must have a Name/Path callee (`names_declaration`, :1054-1057); an acknowledged (`suspend`/`block`/both) target must wrap a Call (:1018-1024); any other expression is a Value target.
5. Cases by shape. Pattern field forms `name`, `name as other`, `name as _`, `name: value`, `..` rest (:616-720).
6. Same job elsewhere. Typed side of the same shapes: statements.omg (A1). 7. Unused. grepped: `parse_statements` used by machine.omg:292 (by sight). 8. Stated limits. :29-31. 9. Fail-open. none seen.

### C15. traits.omg (552 lines)
1. Purpose. Trait declarations: header, parents, `requires proposition` contracts, machine requirements, body parents, boundary reach-ceiling rule, and transparent refinements `= Base { machine (*|[Base::]req) clauses; }`.
2. Shape. `parse_trait` :33; `append_trait_parent` :386 (cap 512); `member_machine_head` :406; `parse_refinement_member` :421.
3. How it decides. A `boundary trait` requires every member machine to carry at least one `Reaches` clause row (MissingBoundaryReachCeiling :275-300). Trait body accepts `machine` or `requires Parent;` (:251-308). Caps trait 256, machines 10240, roots 4096.
4. Same job elsewhere. The resolver refuses what this accepts: every refinement trait (forms.omg:105-106) and every trait with a `requires proposition` contract (forms.omg:112-125). Unused: `parse_trait` via parser.omg:566. Fail-open: none seen.

### C16. types.omg (1,102 lines)
1. Purpose. The single type grammar: references `@['tag] [mutable|write] T`, arrays/slices `[T; len]`/`[T]`, applied names `Name<args>`, integer static arguments, `in Domain` qualifiers, paths.
2. Shape. `TypeFrameKind` :14; `TypeFrame` :20; `parse_type_reference` :49; `parse_static_arguments` :876; `parse_static_argument` :951; `commit_type_arguments` :818; `commit_type_lifetimes` :1049; `push_type_argument` :797; `push_type_lifetime` :1028; row constructors :532-731, :981-1025.
3. In/out. Appends rows to `type_references` (cap 65536), `type_arguments` (1024), `type_lifetimes` (1024); explicit frame stack depth 16 (:203); staged arguments 32, lifetimes 32.
4. How it decides. Reference access by spelling `mutable` :168 and `write` :169; an integer is a type only inside `<...>` or as a root static argument :127-134; a lifetime tag is an argument only inside a list or a root static argument :61-65; lifetime arguments must precede others (:96-103). A cast or other type may carry an `in` qualifier path (:277-318).
5. Cases by shape. Header grammar :34-39 omits the `write` access that :168-169 accepts (read).
6. Same job elsewhere. Six "stage on a scratch stack then copy contiguous" commits: types.omg:818, :1049, expression.omg:2186 (arms), :2309 (arguments), :2363 (fields), clause.omg:1119 (subjects), differing only in cap and table.
7. Unused. grepped: `parse_static_argument` 2, `parse_static_arguments` 4. 8. Stated limits. none. 9. Fail-open. none seen.

### C17. expression.omg (2,521 lines)
1. Purpose. One shift-reduce expression parser over explicit stacks: operands, operators, bracket frames; calls, indexes, ranges, constructions, array literals, value-position `match`, casts, memberships, static applications, borrows and acknowledgements.
2. Shape. `PendingOperator` :37; `ExpressionFrameKind` :46; `ExpressionFrame` :65; `parse_expression` :88 (flags `allow_construct`, `allow_membership`); `emit_expression` :1538; `push_operand` :1632; `pop_operand` :1652; `push_operator` :1677; `reduce_until` :1715; `reduce_operator` :1796; `parser_push_frame` :1850;
   `construct_admitted` :2135; `stage_arm`/`commit_arms` :2165/2186; `stage_argument`/`commit_arguments` :2260/2309; `stage_field`/`commit_fields` :2278/2363; `names_declaration` :2419; `static_arguments_ahead` :2459.
3. In/out. Appends `Expression` rows (cap 131072 :1541) children-first (comment :1546-1547); members (32000), arguments (65536), construct fields (4096), match arms (1024 table, 64 staged).
4. How it decides. Precedence ladder (comment :22-23; codes at :473-489): `||`1 `&&`2 `==`/`!=`3 `<` `<=` `>` `>=`4 `|`5 `^`6 `&`7 shifts 8 `+ -`9 `* / %`10, prefix 11. `>>` is two adjacent `>` tokens (:855). `<` after a name or path is a static argument list only if its matching `>` is followed by `(` (:808-817, :2455-2521), else a comparison.
   Contextual words by spelling: `suspend`, `block`, `move` (when not followed by `(`) :141-148; `in` (membership, only if `allow_membership`) :496-500; `_` in match arms :698; `self` accepted only if `expression_allow_subject` :246-253.
   Cast forms: `as Type` (an `in` qualifier on that type rejects RetiredCastQualifier :933-934), `as + Path`, `as - Path`, `as = Path`; after a carrier cast only `+` is allowed (:976-977); a second entry rejects UnsupportedCastEntries (:1013-1014).
5. Cases by shape. Membership joins domains by one connective only (`&` or `|`, MixedDomainConnectives :592-594); `Domain<..>` after a membership rejects UnsupportedDomainArguments :544-548. Match needs at least one arm (EmptyMatch :684). Quote :151-153: "`move place` spells an ownership transfer the transfer itself already means" - `move` is read and dropped.
6. Same job elsewhere. Token-set lookahead scan duplicates input.omg:518-596. Stage/commit copies: see C16 item 6.
7. Unused. grepped: `parse_expression` has many callers (clause, constant, definition, proposition, statement); `names_declaration` used by clause.omg:419-420, statement.omg:1054 and here.
8. Stated limits. :535-536 "An indexed domain application (`AtMost<8>`) is not read here"; :1006-1007 second cast entry is "an explicit coverage refusal".
9. Fail-open. (a) `move` is accepted as a no-op prefix :161-164 (stated :151-153). (b) Suspected defect, open: `emit_expression`'s `stored` writes `literal: 0` into every stored node :1620 although `literal_node` set `literal: literal_handle` :327 and the typer reads `expression.literal`
   (symbol_resolved_trees_to_typed_trees/expressions.omg:187, grepped `\.literal\b`); I did not trace whether another path restores it. (c) `match_frame`, `match_arm_frame`, `match_value_frame` build `ExpressionFrame` literals without `range_start`/`range_inclusive` :2069-2080, :2085-2096, :2101-2112 while `started_frame` includes them :2047-2060; I do not know whether omitted fields default to zero in this language.

---------------------------------------------------------------------------------------------------------------------
## D. typed_trees_to_checked_trees/aliases.omg (344 lines)
1. Purpose. Tracks Boolean aliases for the checker: a Boolean place bound from a replayable condition stands for that condition, so assuming the place at a branch assumes the condition again over unchanged places.
2. Shape. `data AliasTable [copy]` :14 (16 entries: `places`, `conditions`, `limits`, `count`); `record_alias` :34; `assume_alias` :77; `assume_subject_alias` :107; `alias_condition` :133; `add_alias` :167; `forget_aliases` :190; `drop_aliases` :198; `copy_alias` :249; `alias_table_join` :265.
3. In/out. Mutates `checker.aliases` (type `Checker` is in another file); reads syntax and typing.
4. How it decides. Keyed on place identity (`places[index] == place` :154) and on `place_count` limits: a write to place `p` drops aliases of `p` and aliases formed while `p` existed (`older <= limit` :215). A condition is recorded only for expression kinds Binary, Unary, Membership (own condition), Name or Field (copy of an aliased place) :45-52, and only if `replayable`, `place > 0`, and the type is Boolean :39. At a join only identical (place, condition) pairs survive, limits take the larger value :298-327; an unreachable predecessor contributes nothing :268-276.
5. Cases by shape. Fixed capacity 16; a full table "keeps none, which only forgets facts" (comment :165-166, `count < 16` :171). Negation recognized only as `Unary` with `OperatorKind::Not` :81.
6. Same job elsewhere. none seen in unit (other checker files not read).
7. Unused. grepped (word count incl. def): `record_alias`, `assume_alias`, `assume_subject_alias`, `alias_condition`, `forget_aliases`, `copy_alias`, `alias_table_join` - I did not run per-name counts for these; not reported.
8. Stated limits. :12-13, :165-166, :263-264.
9. Fail-open. Forgetting is the conservative direction (comment :165-166, :263-264 "remains conservative"): a full table drops an alias, losing a fact. `assume_alias` :91-94 does nothing when no alias exists. `alias_table_join` with `other_live` false returns without touching `table` :268-270 (stated by comment :263 "an unreachable predecessor contributes no constraint"); that keeps the current table, which is correct only if the other path is truly unreachable; open (callers not read).

---------------------------------------------------------------------------------------------------------------------
## Coverage
Read in full (all 39 ranges of `units/unit_087.tsv`, 22,000 lines in total):
- symbol_resolved_trees_to_typed_trees/statements.omg 1-670 (670); typer.omg 1-372 (372).
- syntax_trees_to_symbol_resolved_trees/ clauses 1-535, containers 1-421, contracts 1-136, declaration_names 1-432, domains 1-659, facts 1-452, forms 1-303, generic_scopes 1-404, identities 1-432, imports 1-813, machine_signatures 1-560, measures 1-141, names 1-1614 (read as 1-820 and 821-1614), operators 1-165, overloads 1-315, pattern_binders 1-192, resolver 1-198, type_names 1-705, type_reference_owners 1-710.
- tokens_to_syntax_trees/ callable_header 1-414, clause 1-1157, conformance 1-172, constant 1-148, data 1-861, definition 1-163, domain 1-323, expression 1-2521 (read as 1-850, 851-1700, 1701-2521), generics 1-297, input 1-641, machine 1-757, parser 1-887, path 1-70, proposition 1-184, statement 1-1178, traits 1-552, types 1-1102.
- typed_trees_to_checked_trees/aliases.omg 1-344 (344).
Skimmed: none. Not read: none.
Outside the unit I read only: `representations/syntax_trees/syntax_trees.omg:85-124` (`same_type_form`), `representations/symbol_resolved_trees/symbol_resolved_trees.omg:50-66` (`TypeBinding`), `symbol_resolved_trees_to_typed_trees/expressions.omg:165-205`. I also listed that directory (Glob) to confirm `binders.omg` and `generics.omg` exist.
Counting caveat: the per-name counts are `grep -rwo` word occurrences over `source/*.omg` (definition included); for A-D file-by-file counts of "lines containing span_spells/spans_equal" I used the Grep count mode (97 matching lines in 21 files; 83 in the resolver directory). Aliases.omg per-name counts were not run (item D.7).

## Five things a maintainer most needs to know about this unit
1. Name resolution is by source spelling, not identity: nearly every lookup compares bytes with `spans_equal`/`span_spells` (97 lines in 21 files, grepped) and then tests visibility; `self`, `result`, `size_of`, `align_of`, the four byte predicates, the 21 builtin type names, the four compiler-known domains and the ranking-view vocabulary (`Nat`, `Slice`, `Descending`, `BoundedDistance`, `IncreasingTo`, `Length`) are all recognised by spelling (names.omg:820,828,1265-1295; type_names.omg:589-688; domains.omg:139-162; clauses.omg:469-474).
2. The parser accepts more than the resolver binds. The parser takes `==`, `!=`, `[]`, `[..]` operator tokens, refinement traits, trait `requires proposition` contracts, `let` definitions, nominal clauses inside contracts; the resolver refuses them in `check_declaration_forms` (forms.omg:54-303). The header comment of forms.omg:32-38 on `requires machine` contracts contradicts its code at :195-219.
3. Every stage is a set of fixed arrays with the bounds typed as literals at each use, and the pass order in resolver.omg:26-195 is the contract: overload publication (`bind_visible_overloads`) must run last because it rewrites `value_bindings` from `Machine` to `Overloads` (overloads.omg:63).
4. Five copies of the same unique-or-ambiguous declaration scan (imports.omg:650, overloads.omg:160, domains.omg:176, :504, :609) and six copies of the stage-then-commit scratch copy (types.omg:818, :1049, expression.omg:2186, :2309, :2363, clause.omg:1119) are repeated rather than shared.
5. Open defect candidates found while reading, not traced to a failing program: `emit_expression` writes `literal: 0` into every stored node (expression.omg:1620) although the typer reads `expression.literal` (typer-side expressions.omg:187); three `ExpressionFrame` literals omit two fields (expression.omg:2069-2112); `require_body_completion` only runs for Unit-result bodies (statements.omg:81).

## What I am unsure about
- Whether the `literal: 0` at expression.omg:1620 is a real defect: I read that the typer takes `expression.literal` and `literal_value(syntax, expression.literal)` (literals.omg:27, not read), but I did not read how integer rows are otherwise found, nor any test.
- Whether Omega struct literals may omit fields (expression.omg:2069-2112), and so whether those frames compile as written.
- Whether non-Unit-result bodies without a tail are refused in another stage (statements.omg:81); the checker was not read.
- The behaviour of helpers named but defined elsewhere (`type_tree`, `typer_admit`, `covers`, `validate_pattern_fields`, `member_target`, `substitute`, `literal_value`, `name_resolution_reject`, `syntax_trees_reject`); I recorded only their call sites.
- `names.omg:634` is safe only because `pattern_binders.omg:172` stores `MatchedSubject` before `bind_name` runs (resolver order `bind_body_names` -> `assign_name_roles`, `mark_subject_patterns` first, names.omg:117-118); I traced this by reading, not by running.
- Per-name call-site counts are word occurrences, so a name that also appears in a comment or a string is over-counted by that much; counts of 2 were taken to mean definition plus one caller after I confirmed that for `is_typed` and by sight for the others named in the text.
