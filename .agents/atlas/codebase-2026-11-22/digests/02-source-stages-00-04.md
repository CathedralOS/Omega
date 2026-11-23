Built from records unit_051 (its area-02 half), unit_052, unit_052_gap_026, unit_052_gap_027, unit_053, unit_054, unit_055, unit_056, unit_057, unit_057_gap_028, unit_057_gap_029 and unit_058 (its stages 03 and 04 files), plus `crate-map/02-source-stages-00-04.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

# Digest: source stages 00 to 04

Path prefixes used below. All are under `source-rust/subsystems/compiler/source-to-psi/pipeline/`:
- `L/` = `00_source-files-to-tokens/` (the lexer)
- `P/` = `01_tokens-to-syntax-trees/` (the parser)
- `R/` = `02_syntax-trees-to-symbol-resolved-trees/` (name resolution)
- `T/` = `03_symbol-resolved-trees-to-typed-trees/src/` (typing)
- `Q/` = `04_typed-trees-to-prepared-typed-trees/` (preparation)

Records not used for this area: unit_051's `driver/` entries belong to area 06, and unit_058's three stage-05 files (`05_.../lib.rs`, `src/body.rs`, `src/boundaries.rs`) belong to area 03.

Words used here:
- **Deferred refusal**: a row recorded on the declaration that holds an unfinished construct. The program is refused only if something it compiles demands that declaration.
- **Fail-open**: a place that lets the program through when the code cannot decide. The class is **stated** (a comment gives the reason), **traced** (the reader followed the input to a later refusal or to a harmless result), or **open** (neither).
- **Spelling-keyed**: decided by comparing source text, rather than a symbol handle, a position or a value.

## 1. What this area does

Stages 00 to 04 turn loaded Omega source text into the typed, prepared program that stage 05 lowers. The lexer cuts bytes into tokens. The parser builds syntax trees. Resolution rewrites the syntax, gives every name a symbol, and leaves machine bodies pending until the driver asks for them. Typing attaches types and selects calls and operators. Preparation specializes static applications and freezes the result. `LoadedSources` comes in (`S/representations/source-files/source_files.rs:50`, per the crate map) and `PreparedTypedTrees` goes out (`S/representations/prepared-source/lib.rs:32`). The six crates total 287 files and 152,667 lines. The driver in area 06 calls them (`driver/parse_source_batch/batch.rs:141,149`, `driver/resolve_and_type.rs:139-476`, `driver/judge_lowered_source.rs:44`). Most decisions in this area that cannot be finished are recorded as deferred refusals rather than raised as errors. A program that never reaches the holding declaration still compiles.

**source-lexer** (`L/`, 9 files, 1,412 lines). Source text goes in and a `TokenStream` comes out, with exact spans. Whitespace and comments are kept as tokens. The work happens in `ByteScanner::scan_token` (`L/lexer.rs:127`), which dispatches on the first byte (`:128-177`). It reports only the first lexical error (`LexError` `:231`). Batch lexing runs in parallel when there is more than one source (`lex` `:23-37`).

**source-parser** (`P/`, 89 files, 26,039 lines, of which 11,870 are tests). Tokens go in and `SyntaxTrees` pages plus root items come out. `parse_into` (`P/parser.rs:57`) loops over `parse_item` (`P/declarations/items.rs:142`). Expressions are parsed by a loop over an explicit frame stack (`P/expressions/parse.rs:122-156`), so deep expressions cost heap rather than host stack. The cursor still caps nesting at 1,024 levels (`P/token_input/cursor.rs:15,84-90`; checked, section 10). The parser records what was written and leaves many judgments to later stages, for example asm mnemonic validity (`P/assembly/parse.rs:2`) and positional pattern fields (`P/patterns/positional.rs:1-10`).

**source-syntax** (`P/source-syntax/lib.rs`, 34 lines). It parses one in-memory string and keeps both the tokens and the trees (`parse_text` `:19`). The package manager uses it (crate map).

**syntax-trees-to-symbol-resolved-trees** (`R/`, 134 files, 74,090 lines). `SyntaxTrees` goes in inside a `ResolutionRequest` (`R/resolver.rs:59`). A `DeferredResolvedFrontier` comes out (`R/deferred_bodies.rs:152`), holding `SymbolResolvedTrees` with machine bodies pending. The work runs in four phases:
- `declarations::prepare` (`R/declarations/mod.rs:63`) rewrites syntax before any symbol exists, in a fixed order (`:70-102`): assembly and dispatch expansion, builtin-name checks, target families, alias expansion, integer widths, domain heads, constant folding, generic-data synthesis and trait defaults.
- `lowering/` copies syntax into the resolved form with invalid symbols.
- `names/` assigns symbols, walking each declaration's children by position (`R/names/top_level.rs:1352`).
- `selections/` records which declaration each authored occurrence chose.

`resolve_deferred` (`R/resolver.rs:91`) repeats a provisional resolution until machine-equation completion adds no root items (`:103-126`). `complete_machine_bodies` (`R/deferred_bodies.rs:372`) lowers selected bodies transactionally.

**Closed applications of data equations (2026-11-23).** The private instantiator previously received only type roots created while completing machine calls. An authored field of type `Box<Mode<4>>` was refused even when `Box<Payload, const Count: u32>` declared `Payload == Mode<Count>`. Stage 02 now passes such authored applications to the same instantiator. Processing nested arguments first lets the equation recover `Count = 4` and diagnose a conflicting explicit `Count = 5`.

**symbol-resolved-trees-to-typed-trees** (`T/` plus a 44-line root `lib.rs`; 34 files, 39,160 lines). `SymbolResolvedTrees` goes in and `TypedTrees` comes out, wrapped in `SeededTypingBase` for continuations (`T/lowerer.rs:650`). `type_resolved_cohort` (`T/lowerer.rs:1693`) types 12 declaration families in a fixed order. `close_typed_cohort` (`:1867-1985`) then runs about 20 passes in a fixed order. Among them `construction_destinations::prepare` runs twice (`:1920`, `:1937`) and `bind_calls` runs twice (`:1879`, `:1968`). Expression typing is `ExpressionTableLowerer::lower` (`T/expression_lowering.rs:1110`). Its `Call` arm is an ordered chain of special forms (`:1473-1851`).

**typed-trees-to-prepared-typed-trees** (`Q/`, 20 files, 11,932 lines). `TypedTrees` plus `ValueInputs` and the selected providers go in. `PreparedTypedTrees` comes out. `prepare_typed_trees` (`Q/prepare.rs:33`) runs five fixed steps (`:38-46`): specialization (`Q/passes/specialization.rs:91`), conformance requirement headers, applied service lineage, domain applications (`Q/passes/domain_applications.rs`) and record patterns (`Q/passes/record_patterns.rs:19`). Specialization makes one generated machine per distinct closed application (`Q/passes/specialization.rs:212-218,228`).

## 2. How it decides

The counts are of places the records cite, per crate. They are not counts of defects.

**Identity** (symbol handles, item handles, sealed intrinsics, canonical type identity). Lexer 0. Parser 0. Resolution 12. Typing 9. Preparation 3.
- Resolution: trait identity from the header table, "never by leaf spelling" (`R/declarations/trait_defaults.rs:17-21`); the equality-trait gate `sealed_intrinsic == EqualityProtocol` (`:315-316`); `ClosedArgumentIdentity` by item handle or symbol (`R/declarations/generic_data/arguments.rs:527-532`); instance reuse by template plus argument identity (`synthesis.rs:243-246`); `BinderClosures` by symbol index (`R/declarations/machine_equations.rs:1534-1563`); pending bodies by `(arena_index, generation)` binary search (`R/deferred_bodies.rs:376-408`); operand owners by `ItemHandle` (`R/lowering/operand_owners.rs:4-5`); type role by symbol kind, "A spelling never supplies this role" (`R/names/type_predicates.rs:2-3,124-141`); domain lookups that count one symbol reached by two routes once (`R/names/lookup.rs:392-396`); machine supply accepted only with sealed custody, "the leaf spelling alone selects nothing" (`R/lowering/machine_supply.rs:61-63,118-119`); recursive tail calls by binary search on handles (`R/names/recursive_tail_calls.rs:209-225`); the pending body ordinal remap (`R/selections/mod.rs:156-168`).
- Typing: domain-cast atoms by symbol plus semantic id (`T/domain_cast_accounting.rs:785,816`); callable contracts and scopes by handle, with stale handles refused (`T/runtime_callables.rs:27-44,637-648`); the `Equatable` implementation by `package_qualified_type_identity` (`T/equality_preparation.rs:419-426`); exhaustiveness subjects by symbol (`T/equality_and_exhaustiveness.rs:2153-2240`); the atomic receiver by builtin atom (`T/atomic_calls.rs:50-61`); the view factory by sealed intrinsic (`T/access_borrow_factory.rs:52-59`); private layout operations by sealed intrinsic (`T/expression_lowering.rs:2273-2286`); the payload-case `==` refusal by `SymbolKind::Variant` (`T/equality_and_exhaustiveness.rs:261-278`); field selection by `exact_field_symbol` (`T/lowerer.rs:2110-2119`).
- Preparation: `ApplicationKey` (template, requirement, operand identities) (`Q/passes/specialization.rs:212-218`); original binders by `generated_from` (`:821`); provider lookup by `symbol_order` (`Q/passes/specialization/provider_methods.rs:56-61`).

**Place** (field path, source file, source span, scope). Parser 1. Resolution 7. Typing 4. Preparation 2.
- Parser: a retained parse is reused only for equal bytes (`driver/parse_source_batch.rs:41`, cited by unit_051).
- Resolution: `ConstantSelection` name lookups anchored at the use span (`R/declarations/declaration_selection.rs:418,741,834`); declaration custody rejoined by span, kind and name (`:1254-1299`); the receiving-application join by declaration span (`R/constants/receiving_applications.rs:141-175`); the alias body by source span (`R/lowering/type_aliases.rs:34-42`); the body scope filter by source file origin (`R/deferred_bodies.rs:316-336`); constant owners by `symbol_source_span == record.declaration` (`R/constants/initializer_normalization.rs:1039-1054`); the generic-data template by source span (`R/names/top_level.rs:701-708`).
- Typing: call selection ownership by `source_span == call.target.source_span()` (`T/expression_lowering.rs:1159-1166`); static-argument occurrences by span, exposure and kind (`:4090-4108`); derived machines by template source span (`T/declarations.rs:630`); domain candidates by visibility from the reference span (`T/type_reference.rs:406`).
- Preparation: `reflect::field` chosen as the unique machine in the same source file (`Q/passes/specialization/reflected_fields.rs:679-695`); the reflected-record visit requires the same source package (`:515-522`).

**Value** (canonical encodings, decoded values). Resolution 5. Typing 3.
- Resolution: the fold receipt is checked by comparing canonical encodings (`R/constants/initializer_normalization.rs:101`); normalized arguments by decoded encoding (`R/constants/normalized_expressions.rs:71-93`); direct const arguments by encoding (`R/declarations/generic_data/mod.rs:231`); core layout storage by comparing the source bytes with the bundled file (`R/declarations/declaration_selection.rs:84-95`); nominal carriers by pointer identity of the selected declaration (`R/declarations/generic_data/const_evaluation/values.rs:804-807`).
- Typing: const-argument admission by declared carrier identity plus `value.encoding` (`T/type_reference.rs:1092-1105`); a closed Boolean guard from a constant's evaluated value (`T/equality_and_exhaustiveness.rs:1747-1759`); the exhaustiveness rule that a literal `0` or `false` `requires` fact makes a case impossible (`:1559-1573`).

**Position** (child order, watermarks, ordinals). Parser 1. Resolution 6. Typing 3.
- Parser: retained parses own exactly one page (`driver/parse_source_batch/batch.rs:56-58`).
- Resolution: symbol children are assigned by insertion order (`R/names/symbol_table.rs:25-27`; `next_child_of_kind` `R/names/top_level.rs:1352-1365`); the synthesis work domain by root index frontier (`R/declarations/generic_data/work_domain.rs:60-64`); type-input owners by root watermarks (`R/lowering/type_inputs.rs:75-98`); statement links by pending-list offset (`R/lowering/state.rs:755-761`); evidence requirements by child ordinal (`R/names/requirement_namespaces.rs:111-122`); lifetime canonicalization "deliberately positional" (`R/declarations/generic_data/arguments.rs:87-92`).
- Typing: cohort frontiers per family as plain `usize` (`T/lowerer.rs:1628`); continuation retention by prefix equality of whole rows (`:1124-1140,1239`); expression-meaning sites by arena index (`T/expression_meanings.rs:40,112-114`).

**Spelling**. Lexer 3. Parser 20. Resolution 45. Typing 30. Preparation 2. Spelling is the largest key in every crate except the lexer.
- Lexer: keywords by `KeywordKind::from_lexeme` (`L/lexer.rs:156`); punctuation by longest prefix (`:166`); the escape list (`L/literals/quoted_bytes.rs:63-106`).
- Parser: `at_contextual` compares text for any identifier or keyword token (`P/token_input/cursor.rs:479-484`). Clause words decide every machine clause (`P/contracts/clauses.rs`, retired forms cite "decision 23" at `:102,113,121`). Exact strings decide:
  - `"ForeignBinding"` (`P/contracts/bindings.rs:10`, `P/contracts/conformance.rs:55`)
  - `"jmp"`/`"b"` (`P/assembly/parse.rs:52`)
  - `"tests"`/`"group"`/`"service"` (`P/expressions/postfix.rs:746-795`)
  - six retired Build method names (`:96-116`)
  - `"Slice"` (`P/declarations/data.rs:27`) and `"Join"`/`"Slice"` (`P/types/references.rs:240,245`)
  - `"bind"`/`"Build"`/`"roots"` (`P/statements/expressions.rs:13,24,43`)
  - `"Trap"`/`"Abort"` (`P/contracts/effects.rs:24`, `P/statements/parse.rs:110-113`)
  - `entry`/`invariant` (`P/machine_bodies.rs:95,134`)
  - the `real` suffix (`P/literals/numbers.rs:54`)
  - `case` in a destructure (`P/expressions/parse.rs:1794`)
  - twelve guard words in `reject_ambiguous_outcome_specific_ensures` (`P/contracts/clauses.rs:420-431`)

  Some names are built as strings: domain names (`P/declarations/domains.rs:103-123`), struct-literal constructors (`P/expressions/parse.rs:1269-1274`) and cast targets (`P/expressions/postfix.rs:159-162,288-291`). Duplicate names are found by string compare (`P/parameters/generics.rs:86,104,183`; `P/patterns/positional.rs:164-173`; `P/expressions/parse.rs:1627-1643`). Pattern-binding uses are rewritten by name, with shadowing by name (`P/patterns/bindings.rs:85-89,233-238,349`).
- Resolution, `declarations/`:
  - substitution maps keyed by binder name (`HashMap<String, TypeReferenceHandle>` at `R/declarations/declaration_selection.rs:1550`, `generic_data/substitution.rs:41,72`, `trait_defaults.rs:97`)
  - const-name replacement (`const_evaluation/templates.rs:404-410`)
  - the literal `self` (`substitution.rs:399`, `const_evaluation/facts.rs:370`)
  - `Rat`/`Nat` and their field names (`const_evaluation/values.rs:495,1869-1937`)
  - `ClosedArgumentIdentity::Constant(String)` (`arguments.rs:504-505`)
  - type-equation binders by name (`type_equations.rs:1294-1303,2109-2118`)
  - asm scratch names and `"zf"` (`assembly/generated_statements.rs:143-146`, `assembly/instructions.rs:247`)
  - `"equals"` (`trait_defaults.rs:574`)
  - `"IntervalSet" | "CountedQuantity"` (`generic_data/synthesis.rs:93-99`)
  - target-family name and target (`target_families.rs:32-55`)
  - call contexts keyed by target name (`generic_data/uses.rs:247-248,312-326`)
- Resolution, `lowering/`:
  - evidence forwarding (`R/lowering/statement.rs:496-508`)
  - `"value"`/`"_"` (`:254-257`)
  - build operations by call name (`:180-186`)
  - the integer-embed builtin name (`R/lowering/statement/indexed_read_hoisting.rs:186-195`)
  - reference-struct parameters (`match_subject_hoisting.rs:190-197`)
  - asm jump labels (`R/lowering/statement/assembly.rs:80-91`)
  - outcome result data by name (`R/lowering/state.rs:578-596`)
  - conformance edge traits (`R/lowering/item.rs:151-159`)
  - the sealed intrinsic by file plus machine name (`R/lowering/machine.rs:1016-1026`)
- Resolution, `names/`:
  - `"entry"` (`R/names/mod.rs:67-68`)
  - `"X86StaticInterruptScope::run"` (`R/names/scopes.rs:369-380`)
  - `"Slice::Length"` (`R/names/statements.rs:998-1000`)
  - `"$predicate_subject"` (`R/names/symbol_table.rs:85-88`)
  - `rsplit_once("::")` and `"{name}[{index}]"` (`R/names/lookup.rs:107,271-281,406`)
  - `format!("{module}::{target}")` (`R/names/targets.rs:113-117`)
  - `"Build"` (`:759-773`)
  - proof-fact locals (`R/names/domain_facts.rs:672-697`)
  - repeated locals (`R/names/repeated_locals.rs:137`)
  - `select_visible_trait` (`R/names/requirement_namespaces.rs:251`)
  - target families (`R/names/target_machine_families.rs:221-235`) and siblings (`R/names/target_siblings.rs:91,124-145`)
- Resolution, `selections/`:
  - conformance slots by requirement name (`R/selections/conformance_blocks.rs:933-941`)
  - overloads by a rendered string such as `"arithmetic:Saturating&declared:Tagged"` (`:1140-1251,1288-1292`)
  - realizations by leaf name (`:1275-1279`)
  - token-binding duplicates by a rendered operand shape (`R/selections/machine_token_bindings.rs:1028-1317`)
  - static-argument rows (`R/selections/static_arguments.rs:24,259-270`)
  - boundary traits sorted by name (`R/selections/service_reaches.rs:130`)
  - `invokes` parameters by name (`:533-540`)
  - evidence forwardings (`R/selections/evidence_forwardings.rs:50-64`)
  - establishment parameters by name (`R/selections/domain_establishment.rs:698-707`)
  - operator-home binders (`R/selections/domain_operator_homes.rs:257-274`)
- Typing, `expression_lowering.rs`:
  - case and data by name (`T/expression_lowering.rs:338-356,2730-2735`)
  - `"LayoutStorage"` (`:2699-2706`) and `"CodeLayout"` (`:485-493`), both plus origin or core source
  - `!name.contains('<')` (`:2561`)
  - the integer-embed name (`:1545-1553`)
  - layout queries (`:2318-2321`)
  - fields and variants (`:3255,3269,3290,3328`)
  - collection length (`:2968`)
- Typing, other files:
  - domain-cast parents by `strip_prefix` (`T/domain_cast_accounting.rs:325-340,366-379`)
  - member fields by name (`T/expression_meanings.rs:193-206`)
  - requirement returns by name (`T/statement_lowering.rs:809-830`)
  - progress subjects (`T/lowerer.rs:398-425`)
  - exhaustiveness fallbacks (`T/equality_and_exhaustiveness.rs:2061-2069,2089-2092`)
  - atomics (`T/atomic_calls.rs:70,107,368`)
  - prelude operations (`T/call_and_product_selections.rs:35-53`), Build helpers (`:238-253,313-327`), product queries (`:3613-3618`), `exclude_crash` (`:3815-3819`), `"tests"` (`:3740-3752`) and interrupts `"run"` plus `"X86StaticInterruptScope"` (`:3368-3377`)
  - `contains("::")` (`T/constructor_applications.rs:540-544`)
  - `plain_path` (`T/contracts_and_signatures.rs:84-90`)
  - record fields (`T/data_patterns.rs:204-208`)
  - `::provider_defaults` (`T/declarations.rs:1778`)
  - refinement requirements (`:2306`)
  - `"Extent::"` (`T/access_borrow_factory.rs:213-217`)
  - domain selection and `Equality::domain` (`T/type_reference.rs:330-352,3315-3317`)
  - `CanonicalConstValue::from_atom` (`:1041,1105,1451`)
  - `case_call` and `unbound_call_path` (`T/typing_queries.rs:1590,1148-1151`)
- Preparation: record-pattern rows by field name (`Q/passes/record_patterns.rs:67-77`); test-group and generated names are labels only (`Q/passes/specialization.rs:1590-1594`; `reflected_fields.rs:737-741`).

## 3. The same job in more than one place

- **Parser item-start and clause-word lists, six copies.** `starts_following_item` (`P/declarations/items.rs:26-63`), `domain_requires_terminator` with 18 words (`P/declarations/domains.rs:242-262`), `machine_contract_fact_boundary` (`P/contracts/clauses.rs:377-388`), twelve contextual words in `try_generic_type_operand` (`P/expressions/primary.rs:176-206`), `at_contract_continuation_keyword` (`P/token_input/cursor.rs:486-497`) and `starts_retired_entry_member` (`P/machine_bodies.rs:116-124`). The lists differ (unit_051, unit_052).
- **Clause arms, three copies.** `P/contracts/clauses.rs:133-323`, `P/contracts/signatures.rs:91-196` and `P/declarations/traits.rs:218-373`, for `reaches`, `invokes`, `suspends`, `blocks`, `crashes` and `requires`/`ensures` (unit_051).
- **Small parser duplicates.**
  - The same `boundary` retirement message appears at `P/declarations/capabilities.rs:138-142` and `P/contracts/clauses.rs:236-238`.
  - `"ForeignBinding"` is tested twice (`P/contracts/bindings.rs:10`, `P/contracts/conformance.rs:55`).
  - The membership chain is built in `P/expressions/membership.rs:12` and in `P/contracts/facts.rs:114-179`.
  - Lifetime-before-type static lists are parsed in three loops (`P/contracts/conformance.rs:89`, `P/expressions/postfix.rs:656` and a third, inferred).
  - `punctuation_label` re-lists spellings that `PunctuationKind::lexeme()` already holds (`P/expressions/operators.rs:365-400`).
- **Parser "find a top-level delimiter" scanners, seven.** `starts_fixed_array_type` (`P/expressions/primary.rs:218`), `find_top_level_punctuation` (`P/token_input/delimiters.rs:4`), `find_top_level_keyword` (`P/transitions/guards.rs:597`), `leading_paren_group_has_top_level_comma` (`P/token_input/cursor.rs:218`), `split_tuple_pattern_components` (`P/transitions/guards.rs:239`), `starts_positional_arm` (`P/patterns/positional.rs:30`) and `after_static_argument_list` (`P/token_input/cursor.rs:412`). They differ in which bracket kinds count and in what they do on an unmatched closer (unit_052).
- **Literal landing.** The parser lands integer suffixes (`P/literals/numbers.rs:39-43`). Resolution lands constant carriers again (`R/constants/carrier.rs:13-110`).
- **Constant canonical encoding, twice.** At syntax level in `canonicalize_const_expression_with_admission` (`R/declarations/generic_data/const_evaluation/values.rs:1266-1539`) and at resolved level in `Encoder` (`R/constants/initializer_normalization.rs:70-524`). `closed_fixed_array_length` has a sibling `array_length` (`initializer_normalization.rs:504-566`).
- **Constant custody, twice by design.** The fold builds it (`const_evaluation/initializers.rs:625-653`). `finalize` re-derives it and refuses drift (`R/constants/initializer_normalization.rs:1599-1627`). The header says this is deliberate (`initializers.rs:23-28`).
- **Literal-only shape walks, three.** `scalar_literal_tree` (`R/constants/literal_declarations.rs:117-131`), `requires_const_initializer_evaluation` (`R/constants/mod.rs:97-132`) and the `retain` shape walk (`initializer_normalization.rs:914-1011`).
- **Domain index telescope, two implementations.** `domain_index_telescope` (`R/declarations/generic_data/arguments.rs:676-693`) calls itself "the one rule shared by identity, argument normalization and the structural matcher" (`:668-675`). `domain_index_parameters` (`const_evaluation/domains.rs:59-77`) computes the same rule. That is used by `facts.rs:800,1082` and `domains.rs:188`. Checked, section 10.
- **"Does this type mention a binder", two.** `type_mentions_parameters` (`R/declarations/declaration_selection.rs:1559-1611`) and `type_reference_mentions_parameter` (`generic_data/substitution.rs:820-876`). `closed_name_identity` (`substitution.rs:450-467`) mirrors `arguments.rs:525-536`.
- **`StaticMachineArgument` to type reference, two.** `R/declarations/callable_types.rs:161-204` and `R/declarations/declared_types.rs:526-606`. They differ in `evidence_projection` and `application.member`.
- **Binary fact evaluation, two in one file.** `R/declarations/generic_data/const_evaluation/facts.rs:73-248` and `:1574-1685`, with the same `builtin`/`authored_operator_can_bind` guards (`:218-239` vs `:1628-1670`).
- **Expression walkers, nine or more.**
  - `collect_expression_handles` (`R/declarations/generic_data/uses.rs:1273`)
  - `expression_contains` (`R/lowering/statement/value_call_hoisting.rs:138`)
  - `collect_expression_binder_mentions` (`R/declarations/type_equations.rs:2473`)
  - `templates.rs:367,375`
  - `collect_expression_subtree` (`R/selections/authored_selections.rs:1201`)
  - `assign_expression_table_symbols` (`R/names/expressions.rs:1048`) and `assign_expression_symbols_in_role` (`R/names/propositions.rs:105`)
  - `retarget_expression` (`R/selections/body_calls.rs:288`)
  - `R/names/repeated_locals.rs:216`

  Each has its own closed match (unit_054, unit_055).
- **Symbol assignment sequences.** `assign_symbols` and `assign_symbols_against_resolved_base` are near duplicates (`R/names/mod.rs:304-346` vs `:348-394`). `build_symbol_table` and `extend_symbol_table` repeat the per-family list (`R/names/symbol_table.rs:905,1146`). The `TypeParameterKind` to `SymbolKind` mapping is written at least 7 times in `R/names/top_level.rs` (`:70-74,311-314,760-763,994-1005,1145-1150,1323-1327,1517-1521`) and again at `symbol_table.rs:400-409,750`. The 13-field `MachineScope { .. }` literal appears in several files (unit_055 grep: statements.rs 9, type_references.rs 6, others 1 each).
- **Outcome contract data by name, twice.** `R/lowering/state.rs:578-596` and again after symbols exist at `R/names/contracts.rs:624-682`. Trait requirement collection appears twice (`R/names/scopes.rs:525`, `R/names/requirement_namespaces.rs:212`). Proposition binder conversion appears twice with different kind mappings (`R/names/runtime_callable_types.rs:175-187` vs `R/names/propositions.rs:31-40`). The sealed-intrinsic lookup chain appears twice (`R/lowering/machine.rs:1016-1026`, `R/lowering/machine_supply.rs:44-48`).
- **Domain lookups in `selections/`.**
  - Four type-reference walkers look for domain constraints: `R/selections/domain_establishment.rs:457`, `domain_operator_homes.rs:197`, `machine_token_bindings.rs:624` and `conformance_blocks.rs:1153`.
  - Three resolvers answer "which domain does this spelling name": `domain_named` (`machine_token_bindings.rs:803`), `domain_name_reaches` (`domain_establishment.rs:564-579`) and `DomainHeaders::candidates` (`domain_operator_homes.rs:64`).
  - Rows are looked up by requirement name three times (`conformance_blocks.rs:436`, `static_arguments.rs:60`, `evidence_forwardings.rs:61-64`).
  - Domain selection by pooled candidates also happens in `generic_data/arguments.rs:736-758` and `const_evaluation/facts.rs:769-821`.
- **Result-domain set, S02 string vs typed.** `result_dispatch_shape` (`R/selections/conformance_blocks.rs:1140`) and `normalized_result_dispatch_set` used at `T/constructor_applications.rs:513-517,770`. A typed `collect_result_dispatch_terms` is at `representations/typed-trees/type_records/type_identity.rs:7582` (suspected, not read).
- **`peel`.** It is defined in `T/call_and_product_selections.rs:484-500` and in `R/names/expression_paths.rs:178` (definition grepped only; suspected same job).
- **Typing duplicates.**
  - Operator and equality selection each finalize an intrinsic receipt (`T/operator_calls.rs:347-358`, `T/equality_preparation.rs:317-369`).
  - Two type lowerings work from the tree and table forms with near-copy arms (`T/type_reference.rs:2336` vs `:3362`; `:2120` vs `:2163`).
  - Domain selection appears twice (`select_domain_candidates_where` `:397` vs `Equality::domain` `:3287`).
  - Expression types are computed twice (`scoped_expression_type_reference` `T/typing_queries.rs:767` vs `DeclaredExpressionTypes::read` `:1647`).
  - `application` and `application_scanned` do one selection (`T/type_reference.rs:2838,2935`).
  - `callee_is_value` has two definitions, one forwarding (`T/runtime_callables.rs:170`, `T/expression_lowering.rs:2881`).
  - The match-field action is written twice in a row (`T/expression_lowering.rs:711-750`).
  - Case lookup by spelling appears twice (`T/expression_lowering.rs:338-356`, `T/equality_and_exhaustiveness.rs:2061-2069`).
  - A 19-field `TableCallExpression` literal appears three times (`T/call_and_product_selections.rs:286-307,3668-3690,3076-3094`).
  - Prefix-retention checks appear twice (`T/lowerer.rs:1123-1140` and the macro at `:1242-1254`).
- **Preparation duplicates.**
  - `record_patterns` gives fields by name although typing already gave positional patterns exact field symbols (`Q/passes/record_patterns.rs:67-77` vs `T/../tests/positional_patterns.rs:41-53`). `OWNER_CODE_CONCERNS` names this (crate map).
  - Generated machines are made by two paths (`GeneratedMachine::install` `Q/passes/specialization/reflected_fields/active_cases.rs:528` and `copy_declarations` `Q/passes/specialization.rs:1543`).
  - Test helpers are duplicated (`tests.rs:42-633` vs `implementation_tests.rs:43-549`). Three near-identical `compilation(...)` builders exist (`Q/tests/support/constant_array_extents.rs:89`, `prepared_package.rs:53`, `qualified_boolean_constants.rs:78`).
  - Call-site enumeration in `defer_unbound_calls` and specialization's `call_sites` (`T/typing_queries.rs:1172`, `Q/passes/specialization.rs:362`) is suspected related.

## 4. Fixed lists of shapes

Lexer:
- Escapes `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, `\xHH` only. `\u` is "outside the lexical profile" (`L/literals/quoted_bytes.rs:63-106`).
- ASCII identifiers only (`L/lexer.rs:220`). `r"`/`r#"` are refused (`:148-152`). Any other byte is "spelling is outside the current language profile" (`:228`).

Parser:
- `ForeignBinding::{Syscall, CompilerIntrinsic, TableFunction, VtableField}`. `VtableSlot` and `DllImport` are retired (`P/contracts/bindings.rs:24-59`).
- Crash causes `Trap`/`Abort` (`P/contracts/effects.rs:24-25`). Five compound assignments (`P/statements/parse.rs:164-174`).
- Unary prefixes `@ ! ~ $ - suspend block move` (`P/expressions/operators.rs:180-290`). 26 arithmetic and 8 logical operator rows (`:17,38`). `ExpressionContext` has 10 variants (`P/expressions/context.rs:2`).
- Transfer mnemonics `jmp`/`b` only (`P/assembly/parse.rs:52`).
- Unannotated `const` takes its type only from a suffixed literal, a cast or `true`/`false`. Anything else is "not supported yet" or "wholly anonymous" (`P/declarations/constants.rs:86,103-134`; tests `tests/unit/unannotated_constants.rs:76-101`).
- Block values refuse seven statement kinds (`P/statements/sequence.rs:95-105`). Only `erased` is accepted in a relevance bracket (`P/parameters/relevance.rs:26-30`). Only `[copy]`/`[linear]` type properties (`P/types/properties.rs:24-71`).
- `GenericParameterSyntax` has seven contexts (`P/parameters/generics.rs:21`). `public` is kept for ten item kinds only (`P/declarations/items.rs:149-165`).
- A transition target is `Named` only with a receiver of at most one segment (`P/transitions/targets.rs:78-84`). Integer suffixes come from `FixedInteger::from_decimal_spelling` (`P/literals/numbers.rs:71-77`; tests accept `7u13` and refuse `7usize`, `tests/unit/numbers.rs:23-69`).

Resolution:
- `retain` accepts a closed expression list. Anything else is "outside constant leaf evaluation" (`R/constants/initializer_normalization.rs:1004-1009`). `Encoder::value` accepts arrays, builtin scalars and checked-shape non-quotient data, with depth 64 (`:147,274-280,636`).
- `closed_shape` folds only builtin Integer/Bool destinations from literals, names and exact builtin binaries (`R/declarations/generic_data/const_evaluation/initializers.rs:420-469`).
- Anonymous arithmetic covers `+ - * /` only, with `Exact` policy (`anonymous.rs:399-407`). The integer operator list is at `arguments.rs:485-497`: "only integer arithmetic, shifts, and bitwise operators are supported" (`:499`).
- `validate_selected_const_type` refuses Reference, Slice, Callable, ConstExpression and StaticSelection (`values.rs:1054-1264`, message `:1260`).
- Boolean dispatch coverage handles 1..=8 subjects (`R/declarations/dispatches.rs:210-235`).
- Assembly: only known-contract instructions (`R/declarations/assembly/instructions.rs:172-177`), 32-bit registers only (`:278-338`; `generated_statements.rs:26`, always `u32`), x86 EFLAGS formulas (`generated_statements.rs:273-348`).
- `call_context_types_agree` covers Named, Unit, Reference, Slice, FixedArray and Generic (`R/declarations/generic_data/uses.rs:446-524`). `expression_types` covers Name, Borrow, Member, Call, block tail and Match (`R/declarations/machine_equations.rs:512-619`). Binder classification depth is 128 (`:1631,1664`).
- `match_type_structure` handles a closed list (`R/declarations/type_equations.rs:1310-1464`). Type equations use `== != && || !` only (`:2163-2200`).
- The machine-supply catalog has 6 families (`R/lowering/machine_supply.rs:115-138`). The byte-observation row is a conjunction of about 35 header predicates (`:326-405`). The supply-mode cascade is at `R/lowering/machine.rs:430-494`.
- A wire schema is derived only under seven conditions (`R/lowering/wire.rs:22-39`). The zero-gate byte predicate shape is at `R/names/domain_facts.rs:505-561`. `RECEIVER_MEMBER_KINDS` is Field, State, Parameter, Variant (`R/names/expression_paths.rs:383-388`). `ANY_DECLARATION_LOOKUP_KINDS` has 15 kinds (`R/selections/mod.rs:37-53`). `leaf_carrier_type_reference_pattern!` lists 7 kinds (`:223-233`).
- The domain-establishment walker stops at 4,096 steps and depth 64 (`R/selections/domain_establishment.rs:410,464`). `ShapeNormalizer` emits `"pending machine"` for an inferred machine parameter (`R/selections/machine_token_bindings.rs:1184-1187`).
- Type predicates accept Borrow and single-element arrays to depth 128 (`R/names/type_predicates.rs:170-184`). Build operand routes go to five resolvers (`R/names/targets.rs:697-744`). Nested receiver chains must be `self`-rooted, length 3 or more, plain named fields (`R/names/expressions.rs:79-125`).
- Callable inference refuses eight shapes with "not implemented" (`R/selections/callable_inference.rs:317,367,375,425,558,615,633,825`).

Typing:
- Core operator application takes integer carriers only (`T/operator_calls.rs:282-286`). `Equatable` selection needs one argument, one row and two shared-reference parameters (`T/equality_preparation.rs:54-97`).
- Exhaustiveness: `classify_arm` handles a fixed list, everything else is Opaque (`T/equality_and_exhaustiveness.rs:1794-1875`). Work is capped at `MAX_MATRIX_WORK = 65_536`, refused as "too large" (`:1591,919-924`). The builtin conversion and index rules are at `:1458-1474,2303-2330`.
- Propositions take three binder kinds and integer literals only (`:2624-2702,2862-2874,2955-2961`).
- Quotients: `Define` takes one theorem and `Lift` takes one or two, in role order (`T/expression_lowering.rs:2194-2244`). `code_layout` takes one machine operand (`:453-471`).
- Product operand capture refuses Callable and AliasUse types (`T/call_and_product_selections.rs:993,996`) and Block values (`:1263`). Build protocol cases are a fixed `match` (`:152-190`). Product operand counts are fixed (`:3856-3863`).
- The atomic operation table is at `T/atomic_calls.rs:114-165`. The view-factory shapes are at `T/access_borrow_factory.rs:98-166`. A `let` pattern head must be `Named` (`T/data_patterns.rs:365-381`).
- `lower_type_parameter_kind` refuses `Machine{Inferred}` and `AuthoredNominal` (`T/contracts_and_signatures.rs:1452-1556`). `infer_initializer_type` knows a fixed list of initializer shapes (`T/statement_lowering.rs:476-669`). `declared_type` knows Name, Call, Borrow, Indexed, Member and SavedSubject (`T/expression_lowering.rs:3141-3219`).
- `argument_in_carrier` serves Integer and Boolean only (`T/type_reference.rs:1220-1233`). Record or case const arguments get "no product route yet" (`:982-985`). `Equality::reference` has a closed list, and Callable is never equal (`:3226`).

Preparation:
- "Served today: type parameters given plain named types, implementation parameters given plain named implementations, structural callable parameters given exact named machines, and value parameters given one integer literal or `true`/`false`" (`Q/passes/specialization.rs:24-29`).
- The copier needs a `plain` machine (`:1308-1342`). Copied statements are limited to six kinds (`:1395-1411`). Some call metadata is refused (`:1434-1446`).
- Reflected records must be closed plain records in the same package (`Q/passes/specialization/reflected_fields.rs:483-522`). Active-case visitation needs exactly 4 binders and 3 or 4 arguments (`reflected_fields/active_cases.rs:77-101`). `CaseInfo` must have 7 fields (`reflected_fields.rs:687`).
- `admits_types_only` covers four node kinds (`Q/passes/specialization/callable_binder.rs:280-286`). Record patterns serve plain records only (`Q/passes/record_patterns.rs:84-87`). Domain applications need a direct index telescope (`Q/passes/domain_applications.rs:32-37`).

## 5. Accepts when it does not know

Each entry keeps the record's class. "Deferred" means a row is recorded and the refusal waits for a demand.

Parser:
- `reject_ambiguous_outcome_specific_ensures` accepts (routes to an ordinary `ensures`) when the guard has `;` or one of twelve words (`P/contracts/clauses.rs:406-435`). Open. It only routes the parse.
- `starts_following_item` returns `false` for an unknown word, which decides where a fact list ends (`P/declarations/items.rs:62`; used at `P/contracts/facts.rs:271`). Open, routing only.
- `type_reference_target_label` renders `"?"` for other carriers, so two domains could share a name prefix (`P/declarations/domains.rs:302`). Open. The bracket path is inferred to produce only Slice or FixedArray.
- A measure with 0 or 2 or more parameters gets an invalid parameter handle (`P/declarations/measures.rs:38-42`). Open. Downstream, `R/names/measures.rs:17-19` skips its body (open).
- `UnaryPrefix::Move => Ok(expression)`: `move x` gives the same node as `x` (`P/expressions/operators.rs:361`). Open.
- `try_parse_static_machine_arguments` drops its failure reason and rewinds (`P/expressions/postfix.rs:420-433`). Open.
- `Build::bind` is matched by string, so a user `Build::bind` would be taken as a root binding (`P/statements/expressions.rs:13,24,43`). Inferred, not traced.
- `rewrite_block_statements` passes other statement kinds through (`P/patterns/bindings.rs:379`). Traced for block arms (`P/statements/sequence.rs:95-105`).
- Kept on purpose and pinned by tests: `!0`, `~false`, `[T; -1]` and `[T; N + 1]` (`tests/unit/type_constraints.rs:39,281`); duplicate match arms (`tests/unit/value_dispatch.rs:29-30`); non-case heads with proof selectors, "head classification belongs to declaration selection" (`tests/unit/domains_and_transitions.rs:819`); `-1u8 as i32`, "typing must reject that negation" (`tests/unit/expression_stack.rs:428`); any asm mnemonic or register (`tests/unit/inline_assembly.rs:59,63`). Whether later stages refuse each one is open.

Resolution, constants:
- `validate_const_definition` returns `Ok(())` for constants needing evaluation or proof (`R/constants/declaration_values.rs:258-265`). Stated.
- `validate_scalar_initializer` returns `Ok` for non-named or non-builtin destinations (`:201-210`). Open. `prepare_initializer_destinations` continues on unknown shapes (`:79,88,97-98`). Open.
- `retain_value_carrier` and `retain_scalar_carrier` skip non-literal nodes (`R/constants/carrier.rs:36,48,51,110`). Open.
- `initializer_leaves` accepts `Name`/scalar literals at a nominal destination without a leaf (`R/constants/initializer_leaves.rs:170-172,232-236`). (a) is traced to `declaration_values.rs:443-474`. (b), scalar at a nominal destination, is open.
- `initializer_dependencies`: a `Name` of another kind adds no dependency (`R/constants/initializer_dependencies.rs:1330`). Open; covered only for resolved selections. An unknown symbol kind returns `Ok` when deferred (`:912`). Stated (`:766-770`). A data miss (`:253-258`) is stated, not traced.
- `retain` skips constants with no receipt (`R/constants/initializer_normalization.rs:877-879`). Stated and traced. `append_retained_custody` ignores Module, Data and Variant targets (`:1233-1237,1278-1284`). Open, inferred harmless.
- `validate_module_literal` returns `Ok` without a module (`R/constants/literal_declarations.rs:21-28`). Stated. `requires_const_initializer_evaluation` returns false when target observation is pending (`R/constants/mod.rs:102-104`). Open.
- `finalize_const_declarations` skips empty scope (`R/constants/selections.rs:85-87`). Stated. A non-nominal constant with no slot passes (`:262-269`). Open, inferred intended.
- `bind_receiving_application` ignores invalid handles (`R/constants/receiving_applications.rs:26-28`). Traced, caught later at `:215-219`.

Resolution, declarations:
- `prepare_closed_application` returns `Ok(None)` and leaves the application unspecialized: "skip, never reject" (`R/declarations/generic_data/const_evaluation/arguments.rs:29-37,52-296`). Stated.
- `Constant(String)` identity could collide for two constants with one spelling (`arguments.rs:504-505`). Not traced.
- `canonicalize_closed_domain_indices` skips unselected families and names (`const_evaluation/domains.rs:294-296,376-424`). Stated (`:154`); open for `concrete_data_positions`.
- `parameter_binds` returns true when unreadable (`anonymous.rs:349-353`). Stated, conservative. `has_builtin_const_operator` treats unspelled operators as builtin (`:195-197`). Stated.
- `variant_fact_verdict` counts any nonzero integer literal as Holds (`R/declarations/generic_data/substitution.rs:371-377`). Stated for folded witnesses. A user literal `5` reaching it is not traced.
- `selected_type_label` falls back to a spelling label (`values.rs:550`). Open. `replace_uncaptured_const_names` depends on capture analysis (`templates.rs:383-415`). Open.
- Synthesis skips excluded templates, missing lookup paths, and the unmoduled names `IntervalSet`/`CountedQuantity` (`synthesis.rs:90-109,269-271`). (c) is open; see section 10 for what those names are.
- `ConstantSelection` returns `None` for absent or ambiguous names and defers to "ordinary validation" (`R/declarations/declaration_selection.rs:1473-1477`). Stated and traced. `domain`/`domain_family` return `None` for contested owners (`:645-706`). Stated.
- `builtin_type_names::validate` does not check modules, domains, conformances or constants, though its header lists "module" (`R/declarations/builtin_type_names.rs:12-14,41-49`). Open.
- `integer_widths` leaves `Named`/`Unevaluated` widths (`R/declarations/integer_widths.rs:162`). Stated (`:17-23`), not traced.
- `target_families::normalize` with no source map merges families across packages (`R/declarations/target_families.rs:76-94`). Open.
- Asm: `bind` renames any user name equal to a scratch register (`R/declarations/assembly/generated_statements.rs:128-148`), open. Non-register mov operands are not target-checked (`instructions.rs:230-267`); inferred, typing validates.
- `validate_materialized_type_equations` returns `Ok` when the head selects no data (`R/declarations/type_equations.rs:2001-2011`). Open. It is skipped while `structural_type_equations_pending` is true (`R/lowering/type_reference.rs:84-90`). Partly traced; see section 10 for who clears the flag.
- `machine_equations` drops solver diagnostics to stderr (`R/declarations/machine_equations.rs:969-971,1372-1374`). Open. Corrected in section 10: the `:1372` site has no error filter.

Resolution, lowering and names:
- A private const whose canonical value fails gets `evaluated_value = None` through `.ok()` (`R/lowering/item.rs:346-352`). Open. Checked in section 10.
- Evidence forwarding removes `a = b` from the body when either name equals an evidence name (`R/lowering/statement.rs:134-159`). Open for a same-spelled unrelated local. The removed assignment is recorded as an `EvidenceForwarding` row (checked, section 10).
- Duplicate asm labels choose the first (`R/lowering/statement/assembly.rs:83-86`). Open.
- `lowering/domain.rs` gives same-named domains one identity when there is no source map (`:57-67`). Open. `R/resolver.rs:62-64` states that source-free checks "stay permissive".
- Outcome data picks the minimal key among same-named data (`R/lowering/state.rs:578-596`). Open.
- `machine_supply` runs no header check for `Collection`/`FloatProjection` (`R/lowering/machine_supply.rs:97-98`). Stated.
- `integer-embed` spelling is treated as proof-only at hoist time (`indexed_read_hoisting.rs:186-195`). Stated (`:178-185`).
- `membership_selection` gives `Contested => Ok(None)` (`R/names/lookup.rs:427`). Stated, downstream untraced. `assign_membership_symbol` and `normalize_qualified_case_value` drop errors (`R/names/expressions.rs:311-315,605`). Traced; re-reported at `R/selections/authored_selections.rs:1407-1418,1472-1477`.
- `select_visible_trait` falls back to the first candidate (`R/names/requirement_namespaces.rs:268`). Open. `attached_call_target` takes the first of two (`R/names/scopes.rs:818-821`). Open. `DeclarationTypeScopes::parameters` returns `&[]` for two owners (`:245-247`). Open.
- `public_interfaces` accepts private domains as constraints (stated `:16-18`) and callable or const types (`:70-72`, unstated).
- `unresolved_scoped_name` returns no error for a value head (`R/names/paths.rs:383-385`). Stated.
- `resolve_index_subject` can overwrite a valid symbol with an invalid one (`R/names/type_references.rs:2081-2083,1771,1816`). Open. `stamp_receiver_path_symbols_in_table` writes the head symbol into the leaf field (`R/names/expression_paths.rs:654-668`). Open.
- `repeated_locals` skips invalid-symbol locals (`:132`). Open. Type predicates deeper than 128 are treated as values (`R/names/type_predicates.rs:170`). Open.
- `reconcile_copy_targets` accepts `(Resolved, LateBound)` without comparing (`R/selections/authored_selections.rs:111`). Stated (`:6-7`).
- `callable_inference::infer` returns early with an empty inventory (`R/selections/callable_inference.rs:102-104`). Open.

Resolution, selections:
- `collect_requirement_closure` drops an unknown parent trait (`R/selections/conformance_blocks.rs:1120-1126`). Open. `named_implementation_requirement` leaves the call unresolved (`:417-434`). Stated (`:218-219`).
- `domain_operator_homes` keeps an unmatched operator as a root (`R/selections/domain_operator_homes.rs:138-148`). Open; checked in section 10.
- `evidence_forwardings` leaves owner and conformance unset with no diagnostic (`R/selections/evidence_forwardings.rs:47-65`). Open.
- `machine_parameter_requirements` skips non-`Named` arguments although the comment says "fails closed" (`:83-85,107-109`). Open.
- `machine_token_bindings` skips the primitive-family and foreign-home rules without custody (`:325-327,333,421-429`). Stated. Dependent shapes are deferred to typing (`:155-200`). Stated and traced (consumed at `T/runtime_callables.rs:1350`).
- `service_reaches` drops an unregistered parent (`R/selections/service_reaches.rs:141`). Open. An unresolved `invokes` name is traced to `T/contracts_and_signatures.rs:71-107`.
- `static_arguments` leaves `NotFound` unbound (`:95-106`). Stated (`:409-411`).

Typing:
- Domain casts the stage cannot account for become deferred refusals (`T/domain_cast_accounting.rs:278-296`). Stated (`:17-22`). `refuse_qualified_carrier_operand` returns early (`:424-432`). Open.
- Members stay `Unresolved` when owners disagree or the type is unknown (`T/expression_meanings.rs:187-214`; `T/expression_lowering.rs:2985`). Partly stated; later refusal open.
- Operators with conformance bounds or explicit evidence stay builtin nodes (`T/operator_calls.rs:107-127,398-405`). Stated (`:162-163`), not traced.
- Equality with an unknown operand type is left unchanged (`T/equality_preparation.rs:227-249,279-293`). Stated.
- `prepare_named_value` returns `Ok(false)` for unrecognized sources (`T/runtime_callables.rs:358,244`). Open. The destination contract is discarded with `let _ = contract;` (`:418`). Stated (`:414-417`).
- `infer_initializer_type` returns `None` for unknown shapes (`T/statement_lowering.rs:668`). Open. An `Equal` field pattern becomes `Waive` (`:1542`). Open.
- `pending_ranking_default` gives no default for non-name subjects (`T/lowerer.rs:233-235`). Open.
- The payload-case `==` check returns `Ok` on invalid or non-variant names (`T/equality_and_exhaustiveness.rs:267-285`). Open. An axis with zero possible values counts as Complete (`:1599-1601`). Open, semantically uninhabited. A mis-evaluated constant `true` guard closes a run (`:855,1811-1813`). Open.
- `combine_conjunction` of no facts is `true` (`T/expression_lowering.rs:365-368`). Open. With no program, `retain_builtin_type_reference` gives an invalid handle (`:1019-1021`) and call selection links are lost (`:1148-1150`). Both open. An intrinsic-bound member with an unselected field is accepted (`:2840`). Open. `static_machine_argument_selection` returns `None` for generated arguments (`:4079-4089`). Open.
- `bind_generated_call_selection` accepts a differing ledger answer for replaced prelude operations (`T/call_and_product_selections.rs:88-104`). Stated. `construction_destinations::prepare` skips arity mismatches (`:1618-1620`). Open. Unlanded struct literals are stated (`:1714-1717`). Qualification casts keep an invalid domain symbol (`:4181-4189`). Open.
- Expressions with several owners are skipped (`T/constructor_applications.rs:39-41`). Open. The `ExternalBinding` locator zero-fill (`:932-966`) is stated.
- `access_borrow_factory` treats a non-public sealed machine as "not a factory" (`T/access_borrow_factory.rs:65-67`). Open.
- `data_patterns::check_head` passes an unknown record (`T/data_patterns.rs:429`). Stated (`:356-357`).
- `declarations.rs`: an unresolved `satisfies` keeps its row (`:1291-1295`), open. `::provider_defaults` is matched by suffix (`:1778`), open; see section 10.
- `type_reference.rs`:
  - `argument_in_carrier` does not judge non-scalar calls (`T/type_reference.rs:1374-1377`). Stated (`:1335-1343`).
  - `normalize` skips unknown declarations (`:910-933`). Open.
  - Domain or policy carriers are neither admitted nor refused (`:1226-1233`). Stated.
  - Selections with no exposure are not recorded (`:3760-3762`). Open.
  - `integer_family_width` skips non-`Named` arguments (`:2299-2307`). Stated.
  - An unevaluated const extent is not checked (`:2573`). Open.
  - A domain with no visible match is kept (`:184-205`). Stated.
  - `closed_materialized_value` is a stub, so const expressions compare equal in the Declaration phase whenever both have a materialization source (`:3220,3255-3266`). Recorded by unit_058 as a case rule, unclassified.
- `defer_unbound_calls` records unbound calls as deferred refusals (`T/typing_queries.rs:1161-1172`). Stated. Only `NoVisibleDeclaration` names get a row (`:1210-1223`). Open. Runtime calls skip target checks (`:1236-1241`). Open.
- Target families: generic families get a deferred refusal (`T/lowerer.rs:1335-1337,1359-1364,1407-1413`). Stated and traced. Only the first state is compared (`:1414-1420`). Open. A targetless body is dropped from its family (`:1319-1321`). Open. `check_static_value_shadowing` skips type-kind parameters (`:1848-1852`). Open.

Preparation:
- `Stop::Unserved` becomes a `DeferredRequest::StaticApplication` on the holder (`Q/passes/specialization.rs:172-179`). Stated (`:21-23`); pinned by `tests.rs:1232-1264`.
- `Stop::Rejected` in a conditional holder is downgraded to a deferred row (`:127-134,181-188`). Stated (`:97-99`). `serve` returns early in six cases (`:414-507`). Stated. Open applications are left (`:605-615`). Stated.
- `copy_progress` keeps a field with no rebinding (`:1519`). Open.
- `callable_binder` does not compare the callback's own `reaches` (`Q/passes/specialization/callable_binder.rs:140-143`). Open.
- `provider_methods` silently skips a pair (`:67-72`). Open.
- Per-case active-case refusals are deferred (`reflected_fields/active_cases.rs:135-158`). Stated.
- `record_patterns` leaves unresolved subjects and ambiguous names unresolved and accepts: "the checker refuses both where a program reaches the pattern" (`Q/passes/record_patterns.rs:7-9,35,74`). Stated, checker not traced.
- `domain_applications` stores an `Err` on the row only (`Q/passes/domain_applications.rs:16-18`). Open.

## 6. Silent drops

These places produce no row and no diagnostic when something cannot be planned, resolved or lowered. Cross-references point to section 5.

- Parser: `move` is dropped from the tree (`P/expressions/operators.rs:361`). A multi-parameter measure loses its parameter (`P/declarations/measures.rs:41`). The static-argument failure reason is dropped (`P/expressions/postfix.rs:427`).
- Resolution:
  - Non-error and error-level solver diagnostics are printed with `eprintln!` and not returned (`R/declarations/machine_equations.rs:969-971,1372-1374`; section 10).
  - `nop` is removed from asm blocks (`R/declarations/assembly/mod.rs:181-185`). The flag update is not spliced when a statement is missing (`R/declarations/assembly/flags.rs:68-79`; inferred unreachable).
  - `Item::Use(File)`, `Capability` and `Package` are dropped at lowering (`R/lowering/item.rs:398`).
  - A measure with no parameter gets no body resolution (`R/names/measures.rs:17-19`).
  - Receiver-qualified calls in propositions are left invalid with no unresolved record (`R/names/propositions.rs:214-216,239`).
  - Private nested calls through a non-machine receiver are not reported here (`R/names/private_nested_calls.rs:89-91`).
  - Non-Product target machines are not grouped (`R/names/target_machine_families.rs:217`).
  - A `RootBinding` operand of another name kind is dropped (`R/names/statements.rs:385-388`).
  - `symbol_table` short child streams yield invalid handles (`R/names/symbol_table.rs:1177,1195,1201`). Traced to unresolved-name errors.
  - Selections: `evidence_forwardings.rs:47-65`, `service_reaches.rs:141`, `machine_parameter_requirements.rs:107-109`, `conformance_blocks.rs:1120-1126`, `domain_operator_homes.rs:138-148` (the operator stays a root).
- Typing:
  - `construction_destinations::prepare` skip (`T/call_and_product_selections.rs:1618-1620`); `land_one` `Ok(false)` (`:2025-2027`); qualification-cast invalid domain (`:4181-4189`).
  - `atomic_calls::publish` continues past unmatched ledger rows (`T/atomic_calls.rs:190-192`).
  - Selection link lost with no program (`T/expression_lowering.rs:1148-1150`).
  - Targetless family body (`T/lowerer.rs:1319-1321`).
  - `Equal` field action turned into `Waive` (`T/statement_lowering.rs:1542`).
  - Unrecorded selections (`T/type_reference.rs:3760-3762`).
  - `retain_arithmetic_result_type` on unknown shapes (`:660-683`).
  - Unbound names with outcomes other than `NoVisibleDeclaration` (`T/typing_queries.rs:1210-1223`).
- Preparation: `provider_methods.rs:67-72`. `domain_applications.rs:16-18` keeps an `Err` on the row and raises no diagnostic.

## 7. Unused or unwired

Counts are the records' grep line counts, including the definition, unless noted.
- `Lowerer.current_state_parameter_names`, `current_state_parameters` and `current_state_return_type` are written and saved or restored but never read (`R/lowering/lowerer.rs:226,245,247`; `R/lowering/state.rs:311-322`). The record counted 14 matching lines. My grep gives 12 lines, with no reader (section 10). Their comments say transition classification reads them (`lowerer.rs:219-225,240-247`).
- `resolved_lexical_inputs` is `allow(dead_code)` outside tests (`R/lowering/lowerer.rs:172-173`; 3 lines).
- `access_borrow_factory_of`: no caller outside its own file and the re-export (`T/access_borrow_factory.rs:35,38`; `T/lib.rs:120`; checked). `declared_symbol_type`: `pub(crate)` with 1 use in its file (`T/expression_lowering.rs:3356,3383`; 2 lines).
- Test or test-support only:
  - `synthesize_trait_defaults` (`R/declarations/trait_defaults.rs:100-101`)
  - `body_demand`, `into_complete_trees` (`R/deferred_bodies.rs:232,350`)
  - `lower_state_lexical_inputs` and `assign_lexical_inputs` (`R/lowering/state.rs:106`; `R/names/mod.rs:565`)
  - `resolve`, `resolve_extension` and `pre_resolution` (`R/resolver.rs:11-12,45`). The pre_resolution API has 7 matches in 4 files, one of them archived (`_recycling/tools/benchmark/selected-bodies/nonempty.rs`).
  - `lower_symbol_resolved_trees` and `lower_seeded_extension` (`T/lib.rs:119-122`)
- A dead reference: `machine_equations.rs:965` says "see report_preparation_diagnostics". No function of that name exists (1 grep match, the comment itself).
- Comments that claim a single owner the grep does not back:
  - `lookup.rs:3-8` says every builtin lookup reads its roster, but `BuiltinTypeAtom::ALL` has 10 matches in 9 files.
  - `signature_free_requirements.rs:7-8` names "lookup", but there are 19 lines in 4 files and none in `names/lookup.rs`.
  - `arguments.rs:668-675` claims "the one rule" while a second implementation exists.

  Claims that hold: `name_positions` as "the one owner" (4 callers, all in its file); `hoist_temps.rs:2` as the one `__hoist_` constructor (`next_hoist_name` 1 caller); `targets.rs:680-686` with 2 callers outside the definition.
- Misleading comments:
  - `R/lowering/expression.rs:513` calls binary lowering "stack-bounded", but it recurses (checked).
  - `R/lowering/data.rs:261-265` describes a function not in the file.
  - `Q/passes/mod.rs:1-2` names `preparation.rs`, which does not exist (checked).
  - `P/expressions/parse.rs:4` quotes "no expression or type depth limit" while the cursor caps nesting at 1,024 (checked).
- One-caller functions the records name: `machine_equations::complete` (1, `R/deferred_bodies.rs:423`); `normalize_applied_operands` (1); `prepare_instruction` (1); `reference_struct_parameter_names` (1); `bind_constant_uses` (1); `validate_equality_operands_in` (2 lines); `retain_type_reference_recipe_selection` (2); `land_owned_fixed_byte_literal` (2); stage entries such as `normalize_domain_establishment_routes`, `normalize_service_reaches`, `bind_evidence_forwarding_owners`, `static_arguments::bind` (1 each from `R/selections/mod.rs`).
- Spellings with a single code site: `"IntervalSet"|"CountedQuantity"` has 1 match in `source-to-psi` (`synthesis.rs:95`).
- Crate-map README defects: links to a nonexistent `streaming.rs`, `machine_bodies/states.rs` and `defaults.rs` in `P/README.md`; `../` counts off by one in the 01, 02 and 03 READMEs; `tools/benchmark/README.md` is missing; the 04 README table lists three passes where `prepare.rs` runs five steps.

## 8. Stated limits

Parsing:
- Retired forms are refused with a replacement message. Roughly a fifth of the 202 parser unit tests pin them (`tests/unit/*`, unit_052_gap_026).
- "`public` is not yet retained for this declaration kind" (`P/declarations/items.rs:162`).
- "not supported yet: a constant without a type annotation ..." (`P/declarations/constants.rs:86`).
- A quotient "does not yet retain lifetime arguments" (`P/declarations/data.rs:116`).
- A policy operator "is not available in a constant expression yet" (`P/expressions/parse.rs:688-697`).
- "Existing library sources still use namespace imports during migration" (`P/declarations/imports.rs:13`).
- The retired `established by` list is still read (`P/declarations/domains.rs:309-311`, `DOMAIN-ACCESS-AUTHORITY`).
- `MAX_NESTING_DEPTH = 1024` (`P/token_input/cursor.rs:15`).

Constants and compile-time values:
- "v0 initializers are literals all the way down" (`R/constants/declaration_values.rs:414`) and "richer const expressions arrive with build-time evaluation" (`:485-488`).
- "until quotient-backed canonical representatives land" (`values.rs:1133`); "unit const values do not yet have a source literal" (`:1533-1535`).
- Phase 1 "skip, never reject" (`arguments.rs:29-37`).
- "A symbolic const expression cannot cross that boundary yet" (`templates.rs:421-425`).
- `CONSTRAINED_CONST_FENCE` (`facts.rs:1328`).
- "Supporting that applied type remains compiler work" (`callable_types.rs:109-111`).
- Alias RHS custody (`declared_types.rs:171-174,869`).
- "A width written as an expression is left for the specialization" (`T/type_reference.rs:2291-2292`).
- Record or case const argument: "no product route yet" (`:983`).

Assembly:
- "register aliases require shared-bit state modeling" (`R/declarations/assembly/mod.rs:253`).
- "scratch-register CFG flags cannot yet be combined with hardware popfq restoration" (`:281`).
- "no structured operand provenance/permission contract is modeled for it yet" (`instructions.rs:198-201`).

Generic machines and equations:
- "machine structural equations currently require explicit calls" (`R/declarations/machine_equations.rs:1134`).
- "requires anonymous reference lifetimes until lexical lifetime identity is retained" (`R/declarations/type_equations.rs:907`).
- "This is not an arithmetic solver" (`:69-70`).
- Callable inference "not implemented" at 8 sites (`R/selections/callable_inference.rs`).
- Target-family multiplicity is "unimplemented" (`R/names/target_machine_families.rs:118-122`).

Names and visibility:
- "Nested data declarations do not parse yet" (`R/names/member_namespaces.rs:14`).
- A domain written as a constraint "is not judged here yet" (`R/names/public_interfaces.rs:16-18`).
- "The language has no import alias yet" (`R/declarations/builtin_type_names.rs:13-14`).
- Source-free checks "stay permissive" (`R/resolver.rs:62-64`).

Typing:
- Bundled libraries are typed with every program, so unaccounted casts are deferred (`T/domain_cast_accounting.rs:17-21,285-288`).
- Product capture "cannot yet retain a complete scoped callable contract" or alias obligations (`T/call_and_product_selections.rs:993,996`). "no product route yet: a value outside every declaration" (`:1785`).
- "a data pattern cannot name `{}` with type arguments yet" and the case-head question (`T/data_patterns.rs:368-390`).
- "this position cannot use a suffixed operator yet" (`T/expression_lowering.rs:1294-1295`). Proposition "must be the complete fact in this implementation slice" (`:1561`). The late-selected method equation (`:1775`).
- "The family binds generic parameters, which typing does not compare yet" (`T/lowerer.rs:1374-1376,1397-1400`).
- "A missing required type is unavailable rather than reconstructed" (`T/typing_queries.rs:3-4`).

Preparation:
- An unserved application keeps the call and records a deferred refusal (`Q/passes/specialization.rs:21-29`).
- "does not yet retain this machine's complete interface" (`:1339-1341`). "Other independent binder kinds need their full interface copied" (`:1250-1251`).
- An applied or quotient record "is not served yet" (`reflected_fields.rs:503`). A branch "needs a refinement proof for its full interface" (`active_cases.rs:198`).

Dispatch:
- Exhaustiveness is capped at 65,536 steps (`T/equality_and_exhaustiveness.rs:1591`). Boolean dispatch coverage handles 1 to 8 subjects (`R/declarations/dispatches.rs:211`).

## 9. Disagreements between records, and with the crate map

- **Parser depth limit.** unit_051 took `P/expressions/parse.rs:4` ("no expression or type depth limit") at its word and only suspected that `deepen()` enforces one. unit_052 recorded `MAX_NESTING_DEPTH = 1024`. The code enforces 1,024 (`P/token_input/cursor.rs:84-90`), so unit_052 is right and the header comment is stale (section 10). The gap_026 tests nest to 1,000, below the cap (`tests/unit/expression_stack.rs:381-422`).
- **`static_argument_delimiters.rs` length.** gap_026 says the unit row gives 72 lines but the tool returned 198. The second pass split the file into 1-72 (gap 026) and 73-198 (gap 027). gap_026 read all 198, overlapping gap_027. `wc -l` gives 198.
- **Who clears `structural_type_equations_pending`.** unit_054 says only `R/resolver.rs:129` clears it. `R/deferred_bodies.rs:473` also copies the frontier's stored value into the lowerer, and `R/lowering/type_aliases.rs:22-24` sets and restores it (section 10).
- **`resolver_timing.rs` fixture path.** unit_052 inferred the `imports` `include_str!` with one `../` may not resolve. It resolves to the crate's own `R/tests/fixtures/calls/pass/ordinary_family_binding/` (section 10).
- **Machine-equation warnings.** unit_054 says non-error warnings are dropped and doubts that any are pushed. The `:1372` site prints every diagnostic from `desugar_generic_data_instances_in` without an error filter. `anonymous.rs:128` can push a `Diagnostic::error` into that list (section 10). **Follow-up (2026-11-23).** Stage 02 now returns diagnostics from closed data instantiation to its caller. The equation solver already returned its errors directly; its unused warnings argument and the earlier print site have been removed.
- **Consumers unit_056 left open.** Resolved by other records:
  - `pending_token_shape_comparisons` is read by `T/runtime_callables.rs:1350` (`validate_pending_token_shapes`, unit_057; close-cohort pass, gap_029).
  - unit_054's open question about `evidence_forwardings` consumers: `R/selections/evidence_forwardings.rs:13` binds owners (unit_056), and `T/lowerer.rs:2077` lowers them (gap_029).
  - `ProvisionalBodyCallRouting` is consumed in `R/constants/initializer_dependencies.rs` (unit_056 grep).
- **Member selection by name.** unit_057 says `T/expression_meanings.rs:193-200` uses `declared_projection_field`. gap_029 suspects it duplicates `declared_data_field` (`T/expression_lowering.rs:3232`). Neither record compared them.
- **`declared_types::normalize` caller.** unit_053 inferred it is called from `declarations/mod.rs`. unit_054 lists it in the `prepare` order (`R/declarations/mod.rs:70-102`). They agree.
- **Crate map vs unit_058.** The crate map puts `prepare_typed_trees` at `Q/prepare.rs:28` and its calls at `:36-42`. The code has `:33` and `:38-46` (section 10). unit_058 is right. The crate map's resolver loop `:91-130` and unit_055's `:103-126` agree: the loop sits inside `resolve_deferred` at `:91`.
- **Crate map vs records on 04 passes.** The crate map says the README lists three passes and `prepare.rs` also runs two more. unit_058 confirms five steps. unit_058 also found that `passes/mod.rs` names a missing `preparation.rs` (section 10).
- **`lower_type_parameters`.** unit_054 counts 20 matches including 8 in `T/declarations.rs`, which it suspects is a different function. unit_056 read `T/declarations.rs` and does not mention a duplicate. Unresolved.
- **unit_057 coverage.** It is marked incomplete. gap_028 and gap_029 cover the unread 8,583 lines. No contradictions between the first pass and the gaps were found.

## 10. What I checked myself

All checks are on `4f3c255d`, using file reads and grep only. Tally: 12 matched, 2 partly right, 2 wrong.

1. **Claim (unit_052): the cursor caps nesting at `MAX_NESTING_DEPTH = 1024`, against the parse.rs header.** `P/token_input/cursor.rs:15` defines the constant. `deepen` (`:84-90`) returns "expression or type nesting is too deep (exceeds the maximum of 1024 levels)". `P/expressions/parse.rs:1-4` still quotes "no expression or type depth limit". **Matched.** This also settles unit_051's "suspected".
2. **Claim (unit_054): three `Lowerer` fields are written but never read.** Grep over `source-rust/` returns declarations (`lowerer.rs:226,245,247`), initialisers (`:335,340,341`) and save/restore in `state.rs:311-322`. There are no other readers, in 12 lines, not 14. **Matched.** Only the count differs.
3. **Claim (unit_053): two implementations of the domain index telescope.** `domain_index_telescope` (`R/declarations/generic_data/arguments.rs:676-693`) and `domain_index_parameters` (`const_evaluation/domains.rs:59-77`) compute the same generic-carrier test and slice. The grep count is 13. **Matched.**
4. **Claim (unit_054): evidence forwarding drops an assignment by name match.** `R/lowering/statement.rs:134-159` returns `Ok(Vec::new())` when `evidence_forwarding_names` matches. `:496-508` matches when either bare name equals any `evidence_names` string. The assignment is not lost: it is pushed as an `EvidenceForwarding` row (`:142-157`). **Matched.** Section 5 notes the row.
5. **Claim (unit_054): `machine_equations` prints non-error warnings with `eprintln!` and drops them.** At `:966-971` errors are returned first, which matches. At `:1352-1374` the warnings from `desugar_generic_data_instances_in` are all printed, with no `is_error()` filter. `const_evaluation/anonymous.rs:128` pushes a `Diagnostic::error` into such a list. Inside the type-equation solver nothing pushes (`_warnings` at `type_equations.rs:3068,3111`). The comment at `:965` cites `report_preparation_diagnostics`, which does not exist. **Partly right.** Corrected in sections 5, 6 and 7: an error-level diagnostic can be printed and dropped at `:1372`. Whether that input reaches this path is suspected, not traced.
6. **Claim (unit_053): synthesis skips data spelled `IntervalSet`/`CountedQuantity` when unmoduled.** `R/declarations/generic_data/synthesis.rs:93-99` matches. The two names are bundled core library types (`source/library/core/content.omg`, `core/extent.omg`) and are used by samples. So the skip is a name-keyed exception for core types, not for arbitrary user types. **Matched.**
7. **Claim (unit_054): the "stack-bounded" `unreachable!` message is wrong.** `R/lowering/expression.rs:512-513` says "binary expressions use the stack-bounded lowering path". `lower_binary_expression_into_table` (`:421-436`) calls `lower_expression_into_table` on both operands (`:426-427`), which recurses back into `:410`. **Matched.**
8. **Claim (unit_056): provider defaults are recognized by the name suffix `::provider_defaults`.** `T/declarations.rs:1778` matches. The driver documents `Type::provider_defaults` as the authored form (`driver/compile_product/provider_choices/authored_defaults.rs:10`), so the suffix is the language surface rather than an accident. The class stays open: there is no custody check at the site. **Matched.**
9. **Claim (unit_058): `passes/mod.rs` names a `preparation.rs` that may not exist.** `Q/passes/mod.rs:1-2` says "Their order is the list in `preparation.rs`". `find` over `Q/` returns no such file; the list is in `Q/prepare.rs:38-46`. **Matched.**
10. **Claim (crate map): `prepare_typed_trees` is at `Q/prepare.rs:28`, with calls at `:36-42`.** The function starts at `:33` and the calls run `:38-46`; line 28 is the doc example. **Wrong.** Corrected in section 9.
11. **Claim (unit_054): `lowering/item.rs` drops a private const's evaluation error with `.ok()`.** `R/lowering/item.rs:346-352` matches, and it also applies to public constants outside `Complete` mode or needing evaluation. A `None` value then sends the constant to compile-time evaluation (`T/constant_initializers.rs:12-69`, unit_056), so the failure is not necessarily final. The class stays open. **Matched.**
12. **Claim (unit_056): `domain_operator_homes` keeps an unmatched operator as a root operator.** At `R/selections/domain_operator_homes.rs:138-148`, an empty `matches` with no inferred homes pushes the operator to `remaining_roots` and continues, with no diagnostic. **Matched.**
13. **Claims (unit_056): `access_borrow_factory_of` has no outside caller, and it is open whether typing consumes `pending_token_shape_comparisons`.** Grep shows `access_borrow_factory_of` only at `T/access_borrow_factory.rs:35,38` and the re-export `T/lib.rs:120`. `pending_token_shape_comparisons` is read at `T/runtime_callables.rs:1350`. **Matched** for "unused". The open question is now answered.
14. **Claim (unit_052): the `resolver_timing.rs` `imports` fixture path "may not resolve".** `R/benchmarks/resolver_timing.rs:73-81` uses `../tests/fixtures/calls/pass/ordinary_family_binding/`. Relative to `R/benchmarks/`, that is `R/tests/fixtures/...`, which exists (`kit/`, `main.omg`, `build.omg`). **Wrong.** Corrected in section 9.
15. **Claim (unit_054): only `resolver.rs:129` clears `structural_type_equations_pending`.** Grep for assignments finds `R/resolver.rs:129` (`= false`), `R/deferred_bodies.rs:416` (`= true`), `:473` (copied from the frontier, which `resolver.rs:147` captured after clearing) and `R/lowering/type_aliases.rs:22-24` (set and restore). **Partly right.** Body completion also runs with the cleared value through `:473`. Corrected in sections 5 and 9.
16. **Claim (gap_026): `static_argument_delimiters.rs` is 198 lines, not 72.** `wc -l` gives 198. **Matched.** The 72 is the second-pass split, not the file length.
