# Unit 40 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/`, crate `tokens_to_syntax_trees`, Stage 01 of the Psi pipeline):

1. `bodies_and_transitions.rs` (1-1402)
2. `contracts_and_parameters.rs` (1-2656)
3. `declarations.rs` (1-3285)
4. `expressions.rs` (1-2466)
5. `lib.rs` (1-53)
6. `parser.rs` (1-268)
7. `statements_and_assembly.rs` (1-2550)
8. `tests.rs` (1-8312)  [see section 9]
9. `tokens_and_storage.rs` (1-948)

(Not in this unit but named by `lib.rs:34`: `type_syntax_and_patterns.rs`; it exists on disk. I did not read it. Calls into it are noted as "outside unit".)

All claims are "read" unless marked "grepped" or "inferred". Paths below are relative to the `src/` folder above.

---

## 1. `lib.rs` (53 lines)

**Purpose.** Crate root. Declares the module list, re-exports the public parser API, and defines a `grammar` facade that re-exports the internal grammar modules under short names. Two header comments state the stage contract: it consumes `LexedSources` from Stage 00 and produces `SyntaxTrees` for Stage 02 (`lib.rs:19-23`).

**Shape.** Modules at `lib.rs:27-34`. `pub(crate) use tokens_and_storage::{diagnostics, storage, tokens}` (`:36`). `pub(crate) mod grammar` (`:38-44`) maps `bodies`, `contracts`, `parameters`, `declarations`, `expressions`, `patterns`, `type_syntax` to the files. Public re-exports (`:46-50`): `ParseError`, `ParsedSyntaxPages`, `ParsedSyntaxSource`, `parse`, `parse_source_pages_from`, `parse_source_units`, `parse_sources`, `parse_syntax_trees`, `parse_syntax_trees_with_id`. `#![forbid(unsafe_code)]` at `:25`.

**Inputs/outputs.** None of its own.

**How it decides / cases by shape / fail-open.** Nothing. None seen.

**Unused or one-caller code (grepped, `*.rs` under `omega-rust`).**
- `parse_sources` (batch, takes `&mut SyntaxTrees`): called only from `tests.rs:7369,7407,7521,7552,7588`; no caller outside this crate's tests (`grep \bparse_sources\b`).
- `parse_source_units`: called only from `tests.rs:7487,7566`; no caller outside this crate.
- `parse_source_pages_from`: called from `psi/compilation/psi-compiler/src/lib.rs:421,436` (first page 1 and 5).
- `parse`: called from `psi-compiler/src/lib.rs:346` and `build-time-evaluation/src/machine_execution.rs:1299`.
- `parse_syntax_trees_with_id`: grepped call sites are `build-time-evaluation/src/machine_execution.rs:1282`; nine sites in `02_syntax-trees-to-symbol-resolved-trees` (`preparation.rs:5212,5258,5294`, `symbols.rs:2834`, `resolution.rs:550,948,2724,2834`; by their line positions in very long files and `.unwrap()` use I infer these are test code, not confirmed); and `tests.rs:4690,4712,5607` in this crate.
- `ParsedSyntaxPages::syntax_for` and `ParsedSyntaxSource::syntax_handle`: used only at `tests.rs:7499,7500,7574` (grepped, whole `omega-rust`).
- `parse_worker_count`/`with_parse_workers`: used from `psi-compiler/src/lib.rs:261,289`.

**Stated limits.** None.

---

## 2. `parser.rs` (268 lines)

**Purpose.** Entry points that turn lexed sources into syntax arenas, plus the dedicated parser thread pool. A "page" is one `SyntaxTrees` arena owned by one worker.

**Shape.**
- `parse_sources` (`:19-38`): sequential; parses every lexed source into the one `&mut SyntaxTrees` passed in; returns `ParsedSources`.
- `ParsedSyntaxSource` (`:40-53`) = `{source: ParsedSource, syntax: Handle<SyntaxTrees>}`. `ParsedSyntaxPages` (`:55-83`) = `{pages: SealedPagedArena<SyntaxTrees>, sources}`; `syntax`, `syntax_for` (`:69-71`, builds `Handle::from_parts(1, handle.generation())`, i.e. it assumes the page directory slot is 1 and the generation is the page identity), `page_count`, `into_parts` (`:77-82`).
- `parse_source_pages_from` (`:85-95`): `desired_pages = min(source_count, workers*4).max(1)`. `parse_source_units` (`:97-101`): one page per source, first page identity 1.
- `PARSE_STACK_SIZE = 256 MiB` (`:103`); `parse_thread_pool` (`:113-122`, `OnceLock`, threads named `omega-parse-N`, `expect("parser worker pool must spawn")`).
- `parse_source_pages_with_count` (`:124-177`): `chunk_size = ceil(source_count / desired_pages)`, reserves one page per chunk in a `ConcurrentPagedArena`, parses chunks in parallel (`par_chunks_mut`), then `arena.seal()`.
- `parse_source_page` (`:179-198`): one `SyntaxTrees::new_with_generation(first_source, page_identity)` per page; parses its sources in order; `page.push`, `page.publish`.
- `parse_recorded_source` (`:200-225`): calls `parse`, maps `ParseError` to one `Diagnostic` string `path:line:col: message` using `SourcePosition::of(source, error.source_span.span.start)`; builds `ParsedSource` (moves `path` out of the lexed source with `mem::take`, clones the `source` Arc).
- `parse` (`:227-255`): main loop `while !input.tokens.is_empty()` calling `parse_item`; `Item`s go to `push_root_item`, mathematical definitions go to `push_root_mathematical_definition` (not into `roots.items`). `parse_syntax_trees`/`_with_id` (`:257-268`) wrap `parse`.

**Inputs/outputs.** Reads `LexedSources`/`TokenStream`; mutates `SyntaxTrees` (root items, root math definitions) and `ParsedRootItems { items, import_range }`.

**How it decides.**
- Import range (`:239-244`): `import_range.start` is set at the first `Item::Use`, `import_range.end` is `items.len()+1` at every `Use`. So the range spans from the first to the last `use`, including any non-`use` root item between them. Keyed on item variant and position.
- Errors: the first `ParseError` aborts that source (`?` at `:192`, `:29`), so a run reports at most one parse diagnostic per page and `collect::<Result<..>>` (`:156`) returns only the first failing page's diagnostic.
- When a `ParseError` has the default span (EOF errors, `ParseError::new`), `SourcePosition::of(.., 0)` is used (inferred from `tokens_and_storage.rs:21,55`), so the printed line/column is that of offset 0.

**Cases by shape.** None.

**Same job elsewhere.** `parse_sources` (`:19-38`) and `parse_source_page` (`:179-198`) both loop `parse_recorded_source` over sources; one writes into a caller's tree, the other into a fresh per-page tree. Same job done two ways.

**Unused / one-caller.** See section 1 greps: `parse_sources` and `parse_source_units` have 0 non-test callers.

**Stated limits.** None. Header comment says "dedicated 256 MiB stack parser worker pool" (`:4`).

**Fail-open.** None seen.

---

## 3. `tokens_and_storage.rs` (948 lines)

**Purpose.** Shared parser plumbing: the `ParseError` type and message rendering, the span builder used to publish contiguous handle ranges, accessors for the four syntax tables, delimiter scanning, integer/float literal validation, path joining, and the `Input` token cursor (the object every grammar function takes and returns).

**Shape.**
- `diagnostics::parse_error::ParseError {message, source_span}` (`:11-31`). `render_diagnostic` (`:33-97`): `expected_at` builds "expected X, found Y" / "expected one of A, B" / "..., found EOF" (EOF gets `SourceSpan::default()`, `:54-56`); `describe_token` (`:81-96`) names identifiers, numerics, keywords, punctuation by their source text.
- `storage::handle_spans::HandleSpanBuilder<T>` (`:104-147`): holds `start: Handle<T>` and `count`; `push` records `start` only for the first handle and then just increments `count` (`:117-125`).
- `storage::tables` (`:149-187`): `exprs/exprs_of`, `items/items_of`, `stmts/stmts_of`, `type_refs/type_refs_of`.
- `tokens::delimited::find_top_level_punctuation` (`:195-239`): depth-counted scan over `()`, `[]`, `{}`; for `LeftBrace` as delimiter it returns the first `{` at depth 0.
- `tokens::literals` (`:241-362`): `parse_integer_literal` (`:246-284`), `validate_float_literal` (`:286-303`), suffix tables `INTEGER_SUFFIXES` (`:327-340`: `isize`,`usize`,`nat`,`Nat` map to no landing; `i8..i64`,`u8..u64` map to a landed type) and `FLOAT_SUFFIXES` (`:342`: `f32`,`f64`). One unit test (`:344-361`).
- `tokens::paths::join_path_identifier` (`:367-394`): joins `Identifier`s with `::`, span from first start to last end; `expect`s on empty input.
- `tokens::token_cursor` (`:399-916`): `MAX_NESTING_DEPTH = 1024` (`:415`); `Input {source_id, tokens, stream, source_owner, depth, pending_greater}` (`:418-425`), `Copy`.

**Inputs/outputs.** `Input` is an immutable cursor: every `take_*` returns the next `Input`. It reads the `TokenStream` text; it mutates `SyntaxTrees.text` once in `Input::new` (`:441`, retains the source).

**How it decides.**
- Contextual words are matched by source spelling: `at_contextual`/`take_contextual` (`:633-642`, `:770-775`) accept a token that is an `Identifier` *or any* `Keyword` whose text equals the name. `is_identifier_token_for_parser` (`:874-880`): identifier, or a keyword for which `!is_strict_identifier_keyword()`.
- `>>` splitting: `pending_greater` flag (`:424`); `take_punctuation(Greater)` on a `>>` token consumes the token and sets `pending_greater` (`:579-588`); while it is set, only `>` can be taken (`:570-578`) and `expect_token` errors (`:543-545`); `restarted` clears it (`:463-470`).
- `at_contract_continuation_keyword` (`:777-787`): fixed list `terminates, decreases, reaches, effects, invokes, suspends, blocks, crashes, boundary` by spelling.
- `at_same_stack_demand` (`:832-843`): token shape `stack <integer> bytes` by spelling.
- `has_newline_before` (`:845-860`): looks only at the whitespace/comment tokens immediately before `later`, searching their text for `\n`.
- `take_integer` (`:688-697`) rejects values that do not fit `i64`; `take_identity` (`:699-708`) rejects values that do not fit `u64`; `take_string` (`:710-724`) rejects non-UTF-8 byte strings.
- `take_optional_label` (`:654-667`): a name followed by `:` is a label, else none (speculative, no rewind needed because `Input` is `Copy`).
- Depth: `deepen` (`:487-497`) errors above 1024. Grepped users of `.deepen()` in this crate: `declarations.rs:1471`, `expressions.rs:391,406`, and `type_syntax_and_patterns.rs:602` (outside unit). No call in nested-machine, statement, or transition-block recursion (see `bodies_and_transitions.rs:73-75`).

**Cases by shape.** `parse_integer_literal` only handles bases Binary/Octal/Decimal/Hex and the 12 suffixes in the table; unknown suffix -> "unknown integer literal suffix" (`:314`). `validate_float_literal` rejects the retired `real` suffix with a directed message (`:293-297`); any other suffix must be `f32`/`f64` (`:318-324`).

**Same job elsewhere.** Depth-counted bracket scans appear four times: `find_top_level_punctuation` (`tokens_and_storage.rs:195`), `leading_paren_group_has_top_level_comma` (`:605-631`), `find_top_level_keyword` (`bodies_and_transitions.rs:858-890`), `split_tuple_pattern_components` (`bodies_and_transitions.rs:560-605`), plus `starts_fixed_array_type` (`expressions.rs:2043-2079`). The first and third are the same loop over a different match key. Path copying between tables: `copy_item_path_to_expression_path` (`contracts_and_parameters.rs:267-288`), `copy_identifier_path_to_statement_table` (`bodies_and_transitions.rs:1320-1334`) and `expressions.copy_identifier_path_prefix` (used at `statements_and_assembly.rs:543,2404`).

**Unused / one-caller.** `Input::peek`, `at_contextual_then_string`, `at_identifier_then_contextual` are used inside this crate (not counted). `tables::exprs_of` etc. are used widely. Not grepped individually.

**Stated limits.** `take_string`: "raw byte string literal requires the terminal byte-sequence lowering path" (`:716`). `take_integer`: "this position needs a parse-time number" (`:692`).

**Fail-open.**
- `HandleSpanBuilder::push` (`:117-125`) never checks that a pushed handle is `previous + 1`; the span is `start + count`. A caller that pushes non-adjacent handles would silently produce a span covering other entries. Status: open for the general builder. Every caller I read appended its handles back to back (inferred); I did not verify the arena's append order for each.
- `join_path_identifier` (`:381-388`) has `debug_assert_eq!(first.source_id, last.source_id)` only (`:389`).
- Otherwise none seen.

---

## 4. `bodies_and_transitions.rs` (1402 lines)

**Purpose.** Parses the inside of a machine `{ ... }`: the implicit entry state, nested machine declarations, trait default bodies, and `transition` dispatch blocks (guards, patterns, targets).

**Shape.**
- `bodies::parse_body::parse_body` (`:26-134`) returns `ParsedMachineBody {states, nested_machines, bodyless}` (`:20-24`). `;` makes a bodyless machine with one synthetic empty entry state named `entry_name` or generated `"entry"` (`:33-51`). `{` parses statements into one implicit entry state (`parse_implicit_entry_state`, `:218-237`), then loops on nested `machine` declarations.
- `starts_machine_member` (`:136-144`), `starts_retired_entry_member` (`:146-185`), `starts_retired_invariant_member` (`:187-204`), `starts_state_member` (`:206-216`).
- `bodies::sequence::parse_statements` (`:248-283`).
- `bodies::trait_default::parse_trait_default_machine_body` (`:295-308`).
- `transitions::guards` (`:311-916`): `parse_transition_guard_node` (`:337-416`), `parse_tuple_destructure_pattern_arm` (`:440-558`), `parse_destructure_pattern_arm` (`:737-856`), `retain_value_pattern` (`:607-646`), `arm_pattern_is_condition` (`:418-438`), `split_tuple_pattern_components` (`:560-605`), `find_top_level_keyword` (`:858-890`), trait `FromTransitionMatchComponent` (`:892-915`), struct `DestructureBindings {subject, fields, pattern, proof_selectors}` (`:330-335`).
- `transitions::parse_transition::parse_transition_block_handles` (`:934-1111`), `expression_is_closed_boolean` (`:1113-1132`).
- `transitions::targets::parse_target` (`:1159-1315`): `parse_transition_block_target_with_bindings` (`:1191-1228`), `classify_transition_target_handle` (`:1230-1248`), `classify_call_target_handle` (`:1250-1297`), `expression_handle_identifier_depth` (`:1299-1314`).
- Table-copy helpers `copy_identifier_path_to_statement_table`, `append_identifier_path_member_to_statement_table`, `copy_expression_handles_to_statement_table`, `copy_expression_to_statement_identifier_path` (`:1320-1401`).

**Inputs/outputs.** Reads `Input`, calls `parse_item` (declarations), `parse_statement_handles`, `parse_expression_handle*`, `parse_destructure_fields` (patterns, outside unit). Writes states, statements, transition nodes, generated locals, and expression nodes into `SyntaxTrees`.

**How it decides.**
- Body layout (`:72-119`): in a body, after the implicit entry statements, only nested `machine` declarations may follow (`:73`); any statement after a nested machine errors "executable statements cannot follow nested machine declarations" (`:95-99`); `state ...` errors as retired (`:90-94`); `entry` members and `public entry` error as retired (`:100-109`); `invariant` members error as retired (`:111-117`). Keyed on spelling of the next token(s).
- A nested machine marked `public` errors (`:81-85`).
- `starts_retired_entry_member` (`:146-185`): spelling `entry` followed by an identifier, or by `(...)` followed by `{`, `->`, or one of `requires ensures crashes reaches blocks suspends` (`:173-178`).
- `parse_statements` loop condition (`:254-256`): `(requires_result && at_identifier_then_contextual("machine")) || !starts_machine_member(input)`. `requires_result` is `return_type.is_valid()` (`:226`), so whether the machine authored a return type changes how `ident machine ...` is read: with a return type it is a statement, without it is a target-qualified machine declaration.
- `completes_body` (`:263-277`): if the last parsed statement is a `Transition` or `Expression` and the rest starts a machine declaration, statement parsing stops.
- Subject capture (`:953-970`): a transition subject that is not a closed boolean (`Boolean` literal, or `Boolean == Boolean`, `:1113-1132`) is replaced by a name `__transition_subject#{arena_index}` and a `LocalData` with role `DispatchSubject` is emitted before the arms (`:1026-1038`). The generated name embeds the expression's arena index.
- Pattern arms: `parse_transition_guard_node` tries single-subject destructure (`:353-358`), then tuple destructure when `subject.len() >= 2` (`:360-365`), else a match list (`:367`). No-subject block: one pattern becomes `When(expr)` or `Always` for `_`; multiple patterns error "anonymous transition blocks do not support tuple patterns" (`:373-383`).
- Value patterns (`retain_value_pattern`, `:607-646`): a `Name` becomes a head binding; an expression that `arm_pattern_is_condition` accepts becomes the guard; anything else becomes a synthesized `subject == value` guard (`:622-628`).
- Destructure arm (`:737-856`): only if the arm starts with a name (`:743`). It splits at the first top-level `if` keyword (`:747-764`), parses a path, and requires `{` after it. If there is no `{` and an `if` guard exists it errors "expected `{` to open a destructure pattern before `if` guard" (`:769-774`), otherwise returns `Ok(None)` and the arm is read as an ordinary expression.
- Duplicate tuple bindings: compared by spelling (`:517-531`).
- Closing exhaustiveness rewrite (`:1068-1107`): see Fail-open.
- Call targets (`:1230-1297`): `TransitionTargetNode::Named` is produced only when the expression is a call, `target_is_static`, has no `machine_arguments`, was target-shaped (name-like, not `true`/`false`/`self`, no unary prefix), and the receiver identifier depth is at most 1 (`:1264-1268`). Everything else is `Value(expression)`.

**Cases by shape.**
- `arm_pattern_is_condition` (`:418-438`) is a closed list: binary `And Or Equal NotEqual Greater GreaterOrEqual Less LessOrEqual`, `Membership`, unary `LogicalNot`; every other expression shape is treated as a value and compared with `==`.
- Named transition targets: a call path of at most two segments (receiver depth <= 1, `:1264`). A path like `a::b::c()` becomes a `Value` call.
- Boolean-coverage rewrite only when `subject.len() <= 8` (`:1079`).
- `expression_is_closed_boolean` (`:1113-1132`): only `Boolean` and `Boolean == Boolean`.

**Same job elsewhere.** `parse_statements` (`:248-283`) and `parse_trait_default_machine_body` (`:295-308`) both loop `parse_statement_handles`; the second has no nested-machine handling. `find_top_level_keyword` duplicates `find_top_level_punctuation` (section 3).

**Unused / one-caller (grepped, `omega-rust`).** `parse_transition_expression_list`: 1 call (`:1144`) plus its definition and import. `parse_transition_block_target_handle`: 1 call outside its file (`statements_and_assembly.rs:1879`); `parse_transition_block_target_with_bindings` is its only callee besides `parse_transition.rs:1001`. `copy_identifier_path_to_statement_table`: 1 call (`:1388`). `parse_trait_default_machine_body`: 1 call (`declarations.rs:2822`). `has_unary_expression_prefix`: 1 call (`:1196`).

**Stated limits.** "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported" (`:259`); "nested `state` declarations are retired" (`:92`); "nested machines are lexical declarations and cannot be package-exported" (`:83`); "outcome proof selectors are admitted only on a single exact result-case arm" (`:496`); "a transition block must carry at least one arm" (`:1022`).

**Fail-open.**
- Boolean-coverage rewrite (`:1068-1107`). When one subject has exactly two arms with opposite Boolean literals, or there are >= 2 subjects, every arm has a full-length tuple of `Boolean`/`_` patterns, and the tuples cover all 2^n combinations, the last arm's guard is replaced by `Always` and its patterns are cleared (`:1098-1105`). Input reaching it: a transition block whose arms are Boolean literals / wildcards. The wrong program this could let through is one where the Boolean tuples cover all cases only syntactically (for example subjects that are not Boolean-typed). Status: open; I did not trace what Stage 05 does with an unguarded last arm.
- `move` erasure: see `expressions.rs` (section 7 of these notes).
- Otherwise none seen. `ParseError::new("call target must be a path or member access")` (`:1275`) carries no span.

---

## 5. `contracts_and_parameters.rs` (2656 lines)

**Purpose.** Parses the clause language that follows a machine/signature head (`requires`, `ensures`, `terminates`, `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `stack N bytes`), proof-fact lists, `satisfies ... via` conformance rows, and the parameter side: generic parameter lists, value parameters, return types, `requires machine` contracts, binding-relevance brackets.

**Shape.**
- `contracts::conformance::external_binding::parse_external_provider_binding` (`:14-70`): accepts `ForeignBinding::` + `Syscall(n)`, `CompilerIntrinsic`, `TableFunction(field)`, `VtableField(field)`; `VtableSlot` and `DllImport` give retirement errors; others give "unknown ForeignBinding case".
- `conformance::parse_conformance`: `parse_satisfies_traits` (`:85-161`), `parse_optional_satisfies_type_arguments` (`:163-190`), `parse_conformance_trait_application` (`:198-245`).
- `contracts::facts` (`:248-658`): `parse_proof_facts_until*` (`:327-378`), `parse_fact_list` (`:380-593`), `range_membership_expression` (`:595-627`), `parse_proof_fact_domain_arguments` (`:629-657`), `expand_carry_portable_item_paths` (`:292-325`).
- `contracts::parse_contract_clauses` (`:659-1315`): `MachineClauses` (`:677-695`), `parse_machine_clauses` (`:704-1032`), `parse_outcome_specific_ensures_group` (`:1119-1179`), `reject_ambiguous_outcome_specific_ensures` (`:1062-1117`), `parse_crash_header` (`:1191-1202`), `parse_ranked_subjects` (`:1204-1314`).
- `contracts::signature` (`:1316-1562`): `SignatureClauses` (`:1340-1351`), `parse_signature_clauses` (`:1353-1561`).
- Shared: `parse_crash_cause` (`:1566-1579`), `reject_retired_operational_reach` (`:1581-1595`), `take_service_reach_row_member` (`:1596-1611`), `take_operational_reaches_clause` (`:1613-1661`), `publish_capability_contracts` (`:1663-1672`), `finish_crashes_none` (`:1679-1694`).
- `parameters::binding_properties::parse_binding_relevance_brackets` (`:1702-1740`).
- `parameters::contracts` (`:1742-1987`): `parse_machine_parameter_contracts` (`:1756-1904`), `validate_machine_parameter_contracts` (`:1906-1931`), `parse_nominal_requirement_path` (`:1939-1986`).
- `parameters::parse_generic_parameters` (`:2193-2486`): enum `GenericParameterSyntax` with 7 variants (`:2213-2222`), `ParsedGenericParameters` (`:2206-2211`), `parse_generic_parameters` (`:2241-2485`).
- `parameters::parse_parameters` (`:2487-2655`): `parse_optional_parameters` (`:2499-2526`), `parse_optional_return_type` (`:2528-2538`), `parse_parameter` (`:2540-2588`), `parse_callable_signature` (`:2612-2629`), `blank_state_signature` (`:2631-2654`).
- Test module `generic_parameters_tests` (`:1988-2192`, 4 tests).

**Inputs/outputs.** Read `Input`; call into expressions (`parse_expression_handle*`, proof-fact expression parsers) and type syntax (outside unit); append proof facts, capability contracts, identifier path members, type parameters into `SyntaxTrees.items`. `parse_machine_parameter_contracts` mutates the already-published `TypeParameter.kind` in place (`:1837-1844`, `:1895-1899`).

**How it decides.**
- Clause dispatch is by spelling (`at_contextual("requires")`, `"terminates"`, ...) in a `while` loop until `{` or `;` (`:730-1006`, `:1373-1542`).
- `requires machine ...` vs. `requires <facts>` is decided by looking one token after `requires` (`:733-737`, `:1376-1380`); in signature clauses `requires proposition` breaks the loop (`:1386-1392`).
- Machine parameter kind check (`:1793-1814`): contract allowed only when the parameter is `Machine{contract: None | RequirementIdentity}` or a `Type` with default bounds; a parameter already holding a contract errors; anything else errors.
- Generic parameter admission is table-driven by `GenericParameterSyntax` (`:2246-2272`): machine parameters unless `TypeAndConst`/`MathematicalDefinition`; proposition parameters only `TraitRequirements`; conformance binders for `TraitRequirements | RequirementSignature | MachineDeclaration | DataDeclaration`; value parameters for `RequirementSignature | MachineDeclaration | MathematicalDefinition | DataDeclaration`.
- `Carry::Portable` is recognized by comparing the joined path string to `"Carry::Portable"` (`:298-306`, `:646`) and expanded to every `CarryPermission::ALL`; `CarryPermission::from_name(&name)` also keyed on spelling (`:647`).
- Crash cause: `Trap`/`Abort` by spelling (`:1571-1577`).
- `reject_retired_operational_reach` (`:1581-1595`): names `Suspend`, `Block`, `thread_block`, `sync_wait` by spelling.
- Proof fact parsing (`:380-593`): a fact is an expression optionally followed by `in <domain>[ & | domain ...]`, `in <range>`, or a trailing predicate name on the same line; separators are `;`, `,`, or a newline (`:571-587`). `has_newline_before` is the line-break test.

**Cases by shape.**
- Domain membership chain: `&` and `|` mixed in the same chain; if any `|` appears the whole chain becomes one `ProofFact::Expression` and indexed domain arguments are rejected (`:476-486`); otherwise one `ProofMembershipFact` per domain (`:488-500`).
- `parse_nominal_requirement_path` (`:1939-1986`): at most one trait application (`<...>`) in the path, which must be followed by `::requirement` (`:1954-1969`).
- Nominal machine parameter requirement needs >= 2 path members (`:1820-1825`); `as` and `via` are rejected (`:1826-1835`).
- Reaches (`take_operational_reaches_clause`, `:1613-1661`): forms `reaches A + B`, `reaches A, B`, `reaches <= Bound`, `reaches none`; installation-bound exactly once and not combined with others (`:1624-1636`).
- Outcome-specific ensures: only `Path::Case -> { ... }` with >= 2 path members and an arrow (`:1047-1060`), rows must carry exactly one guarantee (`:1151-1155`).
- `stack N bytes`: positive integer, once, `bytes` keyword required (`:823-845`).
- Same-stack demand only on bodyless machines with a `via` binding is enforced in `declarations.rs:1669-1690`, not here.

**Same job elsewhere.**
- Clause loops: `parse_machine_clauses` (`:704-1032`) and `parse_signature_clauses` (`:1353-1561`) both parse `reaches/invokes/suspends/blocks/crashes/requires/ensures/terminates`; the `suspends`, `blocks`, `invokes`, `crashes` arms are copies (`:795-821` vs `:1414-1440`; `:847-888` vs `:1442-1481`; the machine version also has `crashes none` through `parse_crash_header`, the signature version inlines it). Third copy for refinements: `parse_refinement_clause` (`declarations.rs:2855-3028`), a fourth for operators: `parse_operator_contract` (`declarations.rs:2030-2083`), a fifth for capability states (`declarations.rs:114-161`).
- Crash cause parsing: `parse_crash_cause` (`:1566-1579`) and an inline copy at `statements_and_assembly.rs:2246-2255` (grepped `"Trap" =>`: 2 hits in non-test files).
- Clause-end predicates: `machine_contract_fact_boundary` (`:1034`), `machine_reaches_clause_boundary` (`:1181`), `signature_clause_boundary` (`:1325`), `refinement_reaches_clause_boundary` (`declarations.rs:3030`), `capability_contract_terminator` (`declarations.rs:163`), `domain_requires_terminator` (`declarations.rs:1143`), `operator_contract_terminator` (`declarations.rs:2110`): seven hand-written lists of keywords that end a clause, each slightly different.
- Machine/trait/data parameter contract parsing: `parse_machine_parameter_contracts` is called from machines (`declarations.rs:1626`), data (`:404,553`), conformance (`:202`), trait definitions (`:2697`, `:3162`) and recursively from itself and from `parse_signature_clauses` (`:1381`).

**Unused / one-caller (grepped).** `parse_required_proof_facts_until`, `parse_required_logical_proof_facts_until`: callers in this file (`:946,1494`) and `declarations.rs:499,1132,2925`. `parse_callable_signature`: 1 caller (`declarations.rs:80`). `parse_ranked_subjects`: 1 caller (`:750`). Not individually counted otherwise.

**Stated limits.** "one-off `requires machine T::member(...)` requirements are unsupported" (`:1775`); "a ranking witness (`terminates by ...`) does not belong on a bodyless requirement" (`:1520`); "a standalone `decreases` clause is retired (decision 23)" (`:764`); "`increases` ... retired" (`:771`); "`effects` reach clause is retired" (`:779`, `:1398`); "trailing `boundary host` ... clauses are retired" (`:892`); "`reaches none` ... stays accepted until the retirement sweep" (comment at `:1637-1640`); `crashes none` "explicit spelling exists so that promise is readable" (`:1674-1678`); "`self` receivers are retired" (`:2564`); "property brackets attach to the name they follow" (`:2287`).

**Fail-open.**
- Repeated `terminates by <...>` in `parse_machine_clauses` (`:743-757`) overwrites `ranking_subjects/view/range` with no duplicate check, unlike `suspends`/`blocks` (`:805-808`, `:815-818`) and `stack` (`:824`). `invokes` and `reaches` also accumulate without limit. The last ranking witness wins. Status: open (no comment; I did not trace later stages).
- `crashes` ordering: a `crashes Cause` after `crashes none` errors (`:862-865`), and `crashes none` after a cause errors through `other_route` (`:851-861`), so both orders are closed. Status: traced within this file.
- `reaches` followed directly by a clause boundary adds no members and is accepted (`:1650`): stated at `:1637-1640` ("stays accepted until the retirement sweep").
- `HandleSpan::from_parts(handle, 1 + authored_facts.count())` for domain facts is in `declarations.rs:1050` (see section 6).
- Otherwise none seen; many paths return errors for unknown spellings.

---

## 6. `declarations.rs` (3285 lines)

**Purpose.** Top-level and nested declaration grammar: dispatch (`parse_item`) plus one parser per item kind: capability, conformance (named implementation), const, data/type alias/quotient, domain, mathematical `let`, machine, measure, module/package, operator, proposition, trait (including refinements), `use`.

**Shape.** Modules in order: `capability` (`:8-173`), `conformance` (`:174-321`), `const_item` (`:322-363`), `data` (`:364-883`), `domain` (`:884-1346`), `let_definition` (`:1347-1578`), `machines` (`:1579-1759`), `measure` (`:1760-1866`), `namespace` (`:1867-1932`), `operator` (`:1933-2118`), `parse_declaration` (`:2119-2559`), `proposition` (`:2560-2634`), `trait_definition` (`:2635-3268`), `use_item` (`:3269-3285`). Key functions: `parse_item` (`:2269-2311`), `parse_boundary_item` (`:2313-2355`), `parse_declared_item` (`:2357-2558`), `starts_following_item` (`:2147-2187`), `starts_machine_declaration` (`:2260-2267`), `named_implementation_prefix` (`:2202-2241`), `target_machine_prefix` (`:2243-2258`), `parse_machine` (`:1601-1728`), `parse_data_definition` (`:387-536`), `parse_domain_definition` (`:905-1086`), `parse_trait_definition` (`:2656-2853`), `parse_refinement_clause` (`:2855-3028`).

**Inputs/outputs.** `Input` in; `Item`/`ParsedDeclaration` out (`ParsedDeclaration::{Item, Mathematical}`, `:2142-2145`); appends items, states, proof facts, data members etc. to `SyntaxTrees`.

**How it decides.**
- `parse_item` (`:2269-2311`): `public` prefix then recursion; flag set by item variant. Items that cannot carry `public` (module, package, use, measure, capability, `Item::*` other than Conformance/Const/Data/TypeAlias/Domain/Machine/Operator/Proposition/Trait) error "refusing to compile a silently private API" (`:2287-2291`).
- `parse_declared_item` (`:2357-2558`) is a chain of `if input.at_contextual("...")` / `at_keyword` on spelling, with explicit retirement errors for `abi`, `target`, `invariant`, `library`, `host`, `provides`, `platform` (`:2402-2406`, `:2438-2495`). Order matters: `target_machine_prefix` (`:2503`) before `named_implementation_prefix` (`:2519`) before the fallback `name :` / `name <` conformance route (`:2522-2527`).
- `named_implementation_prefix` (`:2202-2241`): `Name satisfies` or `Name<...> satisfies`, found by scanning ahead with an angle/operand depth counter; returns `None` on `; { }` or underflow.
- `target_machine_prefix` (`:2243-2258`): `ident machine` or `ident boundary machine`. The leading identifier is stored as `machine.target` / `authored_target` (`:2506-2508`) with no check that it names a known target (tests use `linux_x86_64` at `tests.rs:2059,6749` and `x86_64` at `tests.rs:6970`).
- `starts_following_item` (`:2147-2187`): keyword/prefix tests; for `operator` or `domain` it speculatively parses a proof fact and then a whole declaration using `append_checkpoint`/`rewind_appended` (`:2179-2183`) and decides on whether the full declaration parses. Keyed on the parse succeeding.
- Machine item (`:2408-2430`): a bodyless machine with a `satisfies` clause but no `via` is rejected; the message says to spell `boundary machine` or an external leaf (`:2421-2428`).
- Installation-bound `reaches <= Bound` permitted only on top-level bodyless `boundary machine` requirement (`:2339-2345`, `:2411-2415`, `:2509-2515`, and in conformance members `:262-266`, trait members `:2804-2808`).
- Data (`:387-536`): after name/generics/property brackets/machine contracts/`dropped by`: `= Type;` becomes a transparent alias only if properties are default (`:413-434`); `= Type % relation ...;` is a quotient (`:435-494`); else `requires` facts then `{ members }`.
- Stable identities (`validate_data_identity_modes`, `:597-641`; `validate_identity_scope`, `:643-679`): all-or-nothing numbering per scope (record fields, sum cases, payload fields), retired identities must not collide, `retired #N;` ambiguous in mixed record+sum data (`:623-628`). Uses a local `HashSet<u64>` (`:665`).
- Domain (`:905-1086`): carrier either a bracketed type or a name plus applied generic; chain `A::B::C` becomes a refinement of its parent when `name_segments.len() > 1` (`:965-990`) with a synthesized `self in Parent` membership fact; alias form `= A & B;` (`:1021-1036`); otherwise `requires` facts and an optional `established by` route list. Domain name string: `"{target_label}::{segments}"` when there are no type parameters, else only the segment chain (`:991-1012`); `type_reference_target_label` (`:1187-1207`) returns `"?"` for any target type other than slice/fixed array/named (`:1205`).
- Machine (`:1601-1728`): operator spelling is taken when the first token is punctuation other than `;` (`:1605-1612`); path `A::b` splits into attached data `A` and entry name `b` (`split_machine_path`, `:1736-1758`); a `satisfies` machine cannot also have an operator spelling (`:1628-1633`); `stack N bytes` only on bodyless machines with a `via` binding (`:1669-1690`).
- Conformance members (`:233-298`): each member must start (after optional `public`, `boundary`) with `machine` (`:241-257`); inline satisfies clauses must name exact requirements (`:278-287`); the entry state is renamed to the requirement name only when its name is `"entry"` and not source-backed (`:300-309`).

**Cases by shape.**
- `parse_declared_item` expected list (`:2535-2557`): `use data domain abi machine capability let library measure host module operator package platform public trait boundary let/operator/requirement/data/trait`.
- `parse_boundary_item` (`:2313-2355`): `boundary` followed by `let`, `requirement` (retired), `data`, `machine`, `operator`, else trait.
- `parse_trait_definition` (`:2656-2853`): two shapes: refinement (`= Base`, only `machine` members that narrow the base, `:2706-2740`) and ordinary (parents `: A + B`, `requires Trait;`, `machine` members, `:2742-2834`). Boundary-trait machine members must declare `reaches` (`:2797-2803`); `native callback` parameters only on bodyless boundary-trait members (`:2791-2795`).
- Refinement clause (`:2855-3028`): members `ensures`, `reaches`, `suspends [true|false]`, `blocks [true|false]`, `terminates`, each once; qualifier must be the base trait name and only one segment (`:2875-2893`).
- `parse_domain_classification` (`:1088-1117`): only `ProgressProfile`, at most one.
- Operator spelling (`:2001-2028`): accumulates punctuation tokens until `OperatorSpelling::from_symbol` matches.
- Measure (`:1772-1820`): `lexicographic { ... }` or one-parameter body block; `parameter` is the single parameter handle when exactly one, else invalid (`:1799-1804`).
- Mathematical `let` types (`:1466-1541`): arrows, `(x: T) -> U` dependent binders, parenthesized, applications.

**Same job elsewhere.**
- Item-start recognition is implemented three ways: `starts_following_item` (`:2147`), `starts_machine_declaration` (`:2260`), `starts_boundary_item` (`:2189`), plus `starts_machine_member` (`bodies_and_transitions.rs:136`). `domain_requires_terminator` (`:1143-1162`) is a fourth hand list of item starters.
- Contract clause parsers copied for capability states (`:114-161`), operators (`:2030-2102`), refinements (`:2915-3017`), machines and signatures (section 5).
- `take_optional_semicolon` (`:1923`) used for capability contracts and module/package; many other sites inline the same `if at ; take` (`:51-55`, `:760-764`, `:806-810`, `:1469-1471`).
- Path joining: `Identifier::new(format!(...))` at `:992-1002` and `join_path_identifier` (`tokens_and_storage.rs:367`).

**Unused / one-caller.** `parse_boundary_data_definition`: 1 caller (`:2332`). `parse_let_definition`: 2 callers (`:2300`, `:2319`). `starts_boundary_item`: callers inside this file (`:2186`); I did not grep other crates.

**Stated limits (quoted <20 words).** "`public` is not yet retained for this declaration kind" (`:2289`); "the initial quotient surface excludes custody" (`:439`); "does not yet retain lifetime arguments" (`:457`); "domain route bodies are retired" (`:1240`); "empty domain bodies are retired" (`:1236`); "`default` keyword is retired" (`:2763`); "`operator` introducer is retired" (`:2768`).

**Fail-open.**
- Data fields, case members and capability fields treat the trailing `;` as optional (`:51-55`, `:760-764`, `:806-810`), so `a: u8 b: u8` and `case A case B` parse. Grammar leniency: accepts programs without separators; no semantic input accepted that the later stages could not see. Status: open.
- Refinement chain (`:1047-1053`): when a parent exists the predicate is forced `Present` and the span is `from_parts(parent_fact_handle, 1 + authored_facts.count())`, relying on the parent fact having been appended immediately before the authored facts. Status: inferred (the parent fact handle is appended at `:1039-1042`, authored facts parse afterward at `:1046`, and `parse_proof_facts` only appends proof facts to that table).
- `type_reference_target_label` returns `"?"` for any other carrier type (`:1205`): a domain on e.g. a reference or generic carrier is named `?::Name` when it has no type parameters. Status: open.
- Otherwise none seen.

---

## 7. `expressions.rs` (2466 lines)

**Purpose.** Expression grammar: precedence chain, unary/postfix operators, primaries, calls with static (generic) arguments, `match`, domain membership, casts, atomics, struct literals, and the proof-fact expression variants. A large part is speculative parsing of `<` as either comparison or generic-argument opener.

**Shape.** `context::ExpressionContext` (`:7-92`, 10 variants and 6 helper methods). `match_dispatch::parse_match_expression_handle` (`:111-210`), `parse_pattern` (`:212-251`). `membership::parse_membership_expression_handle` (`:265-321`). `parse_expression` (`:323-761`): entry points (`:340-384`), `parse_or_expression_handle` (`:411-441`), `parse_bitwise_or_expression_handle` (`:443-470`), generic `parse_binary_chain_handle<const N>` (`:472-519`), unary prefix handling (`:538-739`). `parse_postfix` (`:762-1644`). `primary` (`:1645-2135`). `static_argument_values` (`:2136-2191`). Tests `static_targets_tests` (`:2192-2428`). `proof_fact_expressions::parse` (`:2430-2466`).

**Inputs/outputs.** `Input` + `ExpressionContext` in; `ExpressionHandle` out; appends `ExpressionNode`s with source spans, type references, static arguments.

**How it decides.**
- Context flags (`:22-91`): struct literals allowed only in `Default`, `GroupedProofFact`, `GroupedLogicalProofFact`, `LogicalValue` (`:23-31`); membership disallowed in three contexts (`:33-38`); type expressions allowed in six contexts (`:40-50`).
- Precedence (`:411-470`): two chains. Boolean/comparison chain with levels `|| (0) && (1) == != (2) <= >= < > (3)` over membership; arithmetic chain `| ^ & << >> + - * / %` with 6 levels (`:448-469`). Comparison and equality operators chain left-assoc with the generic stack algorithm; there is no `<` ambiguity handled here.
- `<` after a name/member: `try_parse_static_machine_arguments` (`:1278-1342`) scans forward, tracks `()[]{}` depth, and accepts only when a `)`-opening `(` follows a `>` or `>>` token at depth 0, then trial-parses the argument list with `append_checkpoint` (`:1305`) and keeps it only if it consumed all tokens in the prefix and did not end at `>`; otherwise `rewind_appended` (`:1315`). Stops at `;`, `:`, `=`, `->` at depth 0 (`:1329-1336`).
- Static arguments (`try_parse_static_argument`, `:1369-1456`): type forms (`[`, `&`), evidence projection `a.b`, paths `A::B<...>::C`, then value arguments; falls back to a value expression when the path is followed by something other than `,`/`>` (`:1448-1453`). `try_parse_static_value_argument` rewinds unless the argument is an integer literal (`:1464-1466`). Note it keeps the speculative arena nodes only for integer literals; non-literal const-expression arguments are re-created each time.
- Unary prefixes (`:577-672`): `suspend [block]`, `block` (acknowledgement, only when not followed by `(`), `&`, `& mut`, `& write`, `& state`, `!`, `~`, `-`, `move <name>`. `suspend`/`block` ordering "suspend block" only (`:617-621`). Acknowledgement is attached to the following call via `set_call_acknowledgement`; errors if the operand is not a call (`:721-733`).
- Negation (`:695-715`): integer literal negated in place (`literal.negated()`), float re-spelled `-<text>` as generated spelling, other expression becomes `0 - expr`.
- `move` (`:668-669`, `:737`): `move <name>` is accepted when the next token is name-like and `UnaryPrefix::Move => Ok(expression)` drops it; no syntax node records it (grepped `Move`/`"move"` in `syntax-trees/src`: no match).
- Atomics: member names `load`, `store`, `fetch_add/sub/xor/or/and`, `swap`, `compare_exchange` and single-attempt members (`AtomicCompareExchangeObservationAxis::single_attempt_member`) are recognized by source spelling in `parse_postfix_suffixes_handle` (`:959-992`) and `validate_atomic_call_orderings` (`:1226-1276`). `load` becomes an `Atomic` node only when the argument list is exactly one ordering; the receiver is any expression.
- Retired receiver-style Build calls are recognized by member spelling when followed by `<` (`:932-957`).
- Casts (`:1007-1084`): `as [&[mut|write]] Type [in Domain<args>]`; `in` name is a compiler arithmetic domain if `ArithmeticDomain::from_name` knows it, else a semantic domain; recasts (`as &`) reject arithmetic domains (`:1046-1050`).
- Primary: `true/false`, `self` (allowed only when `context.allows_logical_subject()`, `:1712-1730`), `match`, `zero_value<T>()`, integer, float, `utf16 "..."` (expanded to an array literal of integer nodes, `:1779-1797`), string, parenthesized, array literal / fixed-array type, struct literal, name path.
- Names: `try_applied_requirement_head` (`:1913-1924`) speculatively parses `Trait<Args>::member(` / `::member<` using type syntax; `try_generic_type_operand` (`:1989-2041`) accepts `Name<...>` as a type expression only when the next token is one of a fixed list (`Arrow EqualEqual ExclamationEqual PipePipe AndAnd Comma Semicolon LeftBrace RightBrace RightParen` or one of twelve contextual words, `:2003-2033`) and `{` only when `allow_body_delimiter`.
- `starts_fixed_array_type` (`:2043-2079`): `[` ... `;` at bracket depth 1.
- `static_argument_values::parse_static_argument_expression` (`:2144-2190`): parse a const integer expression; if it does not end at `,`/`>` rewind, split at top-level `>>` boundaries, and try each prefix from the last boundary backward.
- `proof_fact_expressions::parse` (`:2438-2465`): trial-parse as type-expression context; keep only if the result is a binary `And`/`Or` with a non-empty span; else rewind and parse in the fact context.

**Cases by shape.**
- Binary operator tables (`:422-439`, `:453-468`) are closed lists; `punctuation_label` (`:741-760`) returns `"?"` for others.
- `try_generic_type_operand` follow-token whitelist (`:2003-2033`).
- `starts_fixed_array_type` requires `;` at bracket depth 1.
- Static-machine-argument probe: `>` or `>>` immediately before `(` (`:1296-1302`).
- Atomic ordering must be a single-segment built-in name (`:1212-1217`).

**Same job elsewhere.**
- Atomic member-name lists appear in three places: `validate_atomic_call_orderings` (`:1226-1276`), `try_desugar_atomic_let` (`statements_and_assembly.rs:92-134`), `try_desugar_atomic_store` (`:211-254`) plus the `load` special case at `:959`.
- Range-in-brackets and `in a..b` parsing: `parse_index_or_range_expression_handle` (`:1092-1127`), `parse_fact_list` range branch (`contracts_and_parameters.rs:503-539`), `parse_ranked_subjects` range (`:1273-1303`) and `range_membership_expression` (`:595`).
- Membership parsing: `parse_membership_expression_handle` (`:265-321`) and the `in` branch of `parse_fact_list` (`contracts_and_parameters.rs:406-501`) build the same `Membership`/`And`/`Or` chain separately (the second also expands `Carry::Portable`).
- Save/rewind speculation is implemented ad hoc at least eight times: `:1305-1315`, `:1374-1451`, `:1462-1467`, `:1513-1518`, `:1931-1951`, `:1994-2040`, `:2148-2182`, `:2443-2463`, plus `declarations.rs:2179-2183`.

**Unused / one-caller (grepped).** `parse_const_integer_expression_handle`: 2 callers, both in `static_argument_values` (`:2149,2174`). `parse_argument_list_after_open_paren_handle`: 1 caller (`:1974`). `has_unary_expression_prefix`: 1 caller (`bodies_and_transitions.rs:1196`). `memory_ordering_from_expression`: used in this file and `statements_and_assembly.rs:63-130,238`.

**Stated limits.** "implicit `self` is retired" (`:1728`); "`spawn { ... }` is retired" (`:1862`); "`load` takes exactly one ordering argument" (`:990`); receiver-style Build calls "retired" (`:934-950`).

**Fail-open.**
- `UnaryPrefix::Move => Ok(expression)` (`:737`): `move x` is accepted and the keyword vanishes. Input reaching it: `move <name-like>` anywhere a unary expression is allowed. Wrong program that passes: one whose author relied on `move` to mark a transfer; the parse is identical to `x`. Status: open (no stated reason; I only grepped for any node in `syntax-trees`, none).
- `parse_struct_literal_handle` (`:2081-2124`): after a field, a missing comma is accepted (the loop `continue`s only if a comma is present, otherwise falls to the next `take_identifier`), so `T { a: 1 b: 2 }` parses. Status: open.
- Match arm trailing comma is optional (`:189-193`). Grammar leniency only.
- `build_call_expression_handle` (`:1191-1193`) errors with a span-less `ParseError::new`; the diagnostic then points at offset 0 (inferred).
- Otherwise none seen.

---

## 8. `statements_and_assembly.rs` (2550 lines)

**Purpose.** Statement grammar: `let`, destructuring `let`, proof-output `let`, assignments, compound assignment, discard `_ =`, `crash`, calls, atomic desugaring, root bindings (`Build::bind`), and the whole inline `asm { ... }` block including its synthesized flag/scratch-register arithmetic.

**Shape.**
- `atomic_lets` (`:12-255`): `try_desugar_atomic_let` (`:39-202`), `try_desugar_atomic_store` (`:211-254`).
- `destructure_and_proof_output` (`:256-474`): `try_parse_destructure_let` (`:268-383`), `try_parse_proof_output_binding` (`:385-453`), `reject_retired_proof_output_binding` (`:455-473`).
- `discard_and_local_data` (`:475-613`): `parse_discard_statement_handle`, `parse_if_transition_statement_handle` (always errors, `:507-515`), `copy_compound_assignment_target`, `parse_local_data` (`:571-612`).
- `inline_assembly` (`:614-2123`): inner `scratch` module (`:615-1018`, builds expression trees for x86/AArch64 flag emulation), `parse_asm_block_statement_handles` (`:1042-1375`), `parse_asm_contract` (`:1403-1475`), `validate_asm_clobber_contract` (`:1477-1507`), `parse_asm_instruction_statement_handle` (`:1654-2112`), `collect_generated_primitives` (`:2114-2122`).
- `parse_statement` (`:2124-2374`): `parse_statement_handles` (`:2154-2187`), `parse_statement_handle` (`:2198-2373`).
- `statement_tables` (`:2375-2549`): `copy_expression_as_place`, `root_binding_declaration`, `expression_handle_to_statement_call`.

**Inputs/outputs.** Input cursor; writes statements, assembly graphs (`AssemblyControlFlow` with blocks, scratch registers, primitives), generated locals, expressions. Consumes the instruction catalog from `language_core::inline_assembly` (outside unit): `asm_catalog_entry`, `AsmInstructionShape`, `AsmCatalogEntry::{Contract, Refused}`, `scratch_register`, `NAMES`.

**How it decides.**
- Statement dispatch order (`:2154-2187`, `:2198-2373`): retired proof-output form check; `transition`; `asm`; proof-output `let (...)`; destructure `let {...}`; plain `let`; then `if` (always error), `relax`, `spawn {`, `_ =`, `trap`, `crash`, then an expression statement.
- Expression statements: if the expression is a `Build::bind(...)` call -> `RootBinding` (`:2277`, matched by spelling `bind`, owner path `["Build"]`, 3 args, `:2435-2509`); atomic `store` -> assignment (`:2285`); `=` -> assignment; `+= -= *= /= %=` -> assignment with a copied read target (`:2306-2340`); `}` or a machine declaration next -> a value `Transition` (`:2342-2362`); else `;` then `Call` or `Expression`.
- Compound assignment target must be a name/member/index chain; index must be literal, name or member (`:517-569`).
- Atomic let desugar (`:39-202`): a `let` whose initializer is a non-static call with receiver and one of `fetch_*`, `swap`, `compare_exchange`, or single-attempt compare-exchange members is rewritten into `let x = 0` + `target = atomic(...)` (two statements); the compare-exchange value is built from arithmetic: `result + (prior == expected) * (replacement - prior)` (`:152-181`). Orderings parse through `memory_ordering_from_expression`; a bad ordering returns `None` (not an error) and the plain `let` remains (`:111`, `:120`, `:129`); validation errors come earlier in `build_call_expression_handle`.
- Destructure let (`:268-383`): `let { a, b as c, d as _ } = place;` requires every value to be a name/member chain (`is_place`, `:327-336`); emits a `RecordPattern` marker local named `__destructure#a#b...` (`:338-352`) then one local per binding. Any failure returns `None` and the caller falls through to `parse_local_data` (which then reports a different error).
- Proof output let (`:385-453`): `let (value_binding; field: binding, ...) = call;`; the first binding maps to output field `"value"` (`:402-405`); non-call RHS returns `None`.
- Asm block (`:1042-1375`): labels recognized as `ident :` or `ident requires|ensures` (`:1388-1394`); control transfers (jump shapes) must end a block (`:1094-1096`); jump instructions recognized by catalog shape `JumpState`/`JumpZeroFlag` followed by an identifier and `;` or `}` (`:1097-1127`); every instruction intersects `target_requirement` (`AsmTargetApplicability::intersect`) and errors on incompatible targets (`:1108-1113`, `:1130-1134`); realized clobbers are collected from the catalog contract and from assignments to scratch registers (`:1135`, `:1140-1148`); declared `clobbers` must match the realized set exactly (`:1266-1268`, `:1477-1507`); `ensures` requires a falling-through final block (`:1258-1264`); unmodeled x86/AArch64 scratch register views are rejected by scanning every `Name` expression appended since `expression_start` (`:1223-1243`); hardware-flag handling inserts generated blocks and statements (`:1269-1338`).
- Scratch registers (`scratch::bind`, `:735-801`): scans `iter_expressions().skip(first)` for single-segment `Name` nodes whose spelling is in `NAMES` (`:744-760`) and rewrites them to hidden `asm#scratch#{first}#{name}` locals; type is `u64` for indices 15,16,19,20, `bool` for `ASM_ZERO_FLAG_SLOT`, else `u32` (`:765-773`). Keyed on source spelling and on arena position `first`.
- Instruction parsing by catalog shape (`:1698-2111`): arithmetic shapes require a modeled 32-bit register destination and a modeled register or fitting immediate source (`:1788-1806`; AArch64 immediate <= 4095 or 12-bit-shifted <= 4095<<12, `:1792-1794`); mnemonic `b.<cond>` is joined from `b` `.` `cond` (`:1631-1637`).

**Cases by shape.**
- Fixed list of atomic rewrites (`:92-134`).
- `unknown asm instruction` message lists the known mnemonics (`:1663-1668`): `hlt in out jmp mov/movq lfence sfence mfence dmb cli sti serialize isb pause yield wfe wfi sev sevl nop pushfq popfq rdmsr wrmsr wbinvd invd wbnoinvd ldr str read_crN/write_crN/read_<sysreg>/write_<sysreg>`.
- `AsmInstructionShape` match (`:1698-2111`) handles each catalog shape; `DerivedExit | DescriptorTableLoad` hit `unreachable!` because deriver-only entries are refused earlier (`:1690-1696`, `:2108-2110`).
- Scratch `NAMES`-based type selection: indices `[15, 16, 19, 20]` hard-coded (`:765`).
- `dmb` requires an explicit option `ld`, `st`, or `sy` (`:1930-1938`).
- `reject_bracketed_asm_operand` is called only for the arithmetic/move group (`:1719-1726`) and the memory-transfer group (`:1854-1865`).
- Flag arithmetic constants: `0x8d5` mask (`:966`, `:1004`), `0x80000000` sign, parity table `0x9669` (`:945`), bit shifts 20/24/25 (`:919`, `:933`, `:939`): x86 EFLAGS layout baked into the scratch builder, applied under both the Add and Subtract producers.

**Same job elsewhere.**
- Atomic member recognition: `try_desugar_atomic_let` (`:39-202`), `try_desugar_atomic_store` (`:211-254`), `validate_atomic_call_orderings` (`expressions.rs:1226-1276`), `load` handling (`expressions.rs:959-992`).
- Place copying: `copy_expression_as_place` (`:2388-2426`, accepts Member/Name/Indexed/Borrow, shares the index handle) vs `copy_compound_assignment_target` (`:517-535`, accepts Name/Member/Indexed but deep-copies a stable index).
- Crash cause match `:2246-2255` vs `parse_crash_cause` (`contracts_and_parameters.rs:1566`).
- Call node vs call statement: `expression_handle_to_statement_call` (`:2512-2532`) vs `parse_discard_statement_handle` (`:488-505`) which reuses it.

**Unused / one-caller (grepped).** `parse_if_transition_statement_handle`: 1 caller (`:2203`), always returns an error. `try_desugar_atomic_store`: 1 caller (`:2285`). `reject_retired_proof_output_binding`: 1 caller (`:2158`). `collect_generated_primitives`: 2 callers (`:1283`, `:1324`).

**Stated limits.** "scratch-register CFG flags cannot yet be combined with hardware popfq restoration" (`:1254`); "register aliases require shared-bit state modeling" (`:1235`, `:1241`); "the `if` statement is retired" (`:512`); "`relax` is retired" (`:2208`); "statement `trap` is retired" (`:2234`); "`spawn { ... }` is retired" (`:2218`); "may access memory, but no structured operand provenance/permission contract is modeled" (`:1683`).

**Fail-open.**
- `try_parse_destructure_let` (`:268-383`) and `try_parse_proof_output_binding` (`:385-453`) call `parse_expression_handle` (appending nodes) and then may return `None` without `rewind_appended` (`:323-336`, `:435-441`); the nodes stay in the arena as orphans while the caller re-parses. Orphan expression nodes are harmless to the tree but are visible to arena-position scans. The asm scanners (`scratch::bind`, `:744-760`, and the unmodeled-view scan `:1223-1243`) look at nodes appended since `expression_start`, so any orphan created inside an asm block would be scanned. Status: inferred; I did not find an orphan-producing path inside an `asm` body.
- `reject_bracketed_asm_operand` is skipped for port, MSR, control-register, system-register, flags, fence and GDT shapes, whose operands come from `parse_expression_handle` and so may be array literals (`:1900-2107`). The wrong program that gets through would be `out [x], y` style bracketed operands. Status: open.
- `try_desugar_atomic_let` returns `None` for a malformed ordering (`:111`) and the statement falls back to an ordinary `let`; the malformed ordering was already rejected by `validate_atomic_call_orderings` for member calls (`expressions.rs:1236-1253`), so traced as unreachable for named atomic members. Status: traced.
- Compound assignment target not a place errors (`:2320-2323`): closed.
- Otherwise none seen.

---

## 9. `tests.rs` (8312 lines)

**Purpose.** Unit tests for the parser. Every test lexes a source string with `Lexer`, parses it with `parse_syntax_trees` (or a lower entry such as `parse_expression_handle`, `parse`, `parse_sources`, `parse_source_units`), and asserts either on the tree shape or on a substring of the error message.

**Shape.** Test count: `grep -c '#\[test\]'` gives 265 in this file (and 2 in `tokens_and_storage.rs`, 4 in `contracts_and_parameters.rs`, 7 in `expressions.rs`; 278 across the unit's files). Modules (all `pub(crate) mod`): `bindings_and_conformances` (`:13-1057`), `body_sequences` (`:1058-1231`), `constructors` (`:1232-1277`), `definitions_and_contracts` (`:1278-2885`), `domains_and_transitions` (`:2886-3854`), `expression_stack` (`:3855-4089`), `inline_assembly` (`:4090-4670`), `items_and_selections` (`:4671-5748`), `mathematical_definitions` (`:5749-5995`), `numbered_data` (`:5996-6049`), `properties_and_requirements` (`:6050-7330`), `source_parsing` (`:7331-7661`), `static_parameters` (`:7662-7713`), `type_constraints` (`:7714-8072`), `value_dispatch` (`:8073-8210`). Shared helpers at file level: `parse_source`, `parse_source_verbose`, `parse_source_unwrap`, `argument_path`, a `root_item!` macro generating `root_machine`, `root_trait`, `root_data`, `root_domain`, `root_conformance`, `root_const`, `root_operator`, and the `all` variants (`:8212-8282`), `entry_state_handle`, `entry_state`, `parse_source_error`, `parse_tokenized_error`, `expect_error` (`:8284-8312`).

**Inputs and outputs.** Read the parser's public/crate-visible API and the `syntax_trees` tables. They produce only assertions. Two tests use the parser worker pool (`source_parsing`, `:7463-7506` with a 4-thread pool and 16 sources).

**How it decides (what the tests pin, by judgment key).**
- Error tests pin the rendered message by `contains` on source-spelled phrases (e.g. `expect_error(&error, "found identifier `where`")`, `:816`). Several tests only assert `is_err()` with no message (e.g. `named_implementation_requires_a_closed_body` `:667-680`, `rejects_every_authored_domain_subject_shape` `:2959-2973`, `rejects_malformed_requires_fact_lists` `:6621-6635`, `rejects_unknown_or_multiple_fixed_operator_tokens` `:6857-6870`, `comma_constraint_slots_inside_type_brackets_reject` `:8058-8071`).
- Positions: `every_machine_body_rejects_bare_arrows` pins the error start offset at the `->` (`:1228`); `rejection_preserves_the_parsed_prefix...` pins the first error offset `17` and the message prefix `appended.omg:2:3: ` (`:7514`, `:7527`).
- Source custody: many tests pin source spans of `via`, contract keywords, `suspends`/`blocks`, `bind`, names, array/struct literals, the `[` index operator, and binary operator spans.
- Arena identity: `parser_retains_one_worker_page_identity_through_nested_handles` (`:7413-7460`) pins that every handle produced under `new_with_generation(.., 7)` carries generation 7; `parallel_parser_pages_preserve_source_order_and_direct_handle_lookup` (`:7463-7506`) pins 16 pages and `syntax_for`; `declaration_lookahead_rewinds_private_rows_and_preserves_source_custody` (`:2721-2769`) pins that `starts_following_item` leaves `roots`, expression count, `Arc` strong count and the next handle unchanged.
- `import_bounds_preserve_interleaved_root_ordinals_on_both_parser_routes` (`:7537-7582`) pins `import_root_range` values `0..0`, `0..0`, `0..1`, `1..2`, `1..4` for five sources (`:7538-7548`); in the last case the range 1..4 covers `use alpha`, `data Middle` and `use beta` (the `let` is a mathematical definition and takes no root ordinal), so it includes one non-`use` item; the test's `expected_ordinals` `[1, 3]` shows the range is a hull, matching the behaviour at `parser.rs:239-244`.
- `opposite_boolean_subject_arms_test_the_subject_once` (`:4924-4983`) and `unrelated_or_nonpair_boolean_guards_keep_their_tests` (`:4986-5022`) pin the Boolean-coverage rewrite from `bodies_and_transitions.rs:1068-1107` for one pair of opposite literal arms, and four shapes that must keep their tests.
- `tail_targets_preserve_other_receiver_calls_without_reclassifying_state_coordinates` (`:4865-4921`) pins the named-target rule: `next` and `Main::next` become `Named`; `other.next`, `main.child.next`, `other.child.next` stay `Value` calls. This is the test for `classify_call_target_handle`.
- `iterative_prefixes_preserve_borrows_literal_folding_and_call_acknowledgements` (`:4019-4059`) includes `parse_expression("&write move value")` and checks only that the borrow target prints as `value` (`:4029-4034`): it pins the erasure of `move`.
- `long_operator_chains_keep_bounded_parser_stack_and_exact_prefix_order` (`:4062-4088`) pins 4096 chained `~` prefixes and 1024 repeated binary groups; `nested_aggregates_and_grouped_unary_prefixes_use_the_default_stack` (`:3864-3909`) pins 64 nested unary prefixes inside 9 struct and 8 array layers.
- Inline assembly (`:4090-4670`): fences, interrupt control, flags, MSR, control registers, mnemonic intrinsic calls, register moves, memory transfers, exact clobber contracts, `requires`/`ensures`; rejection tests for ambiguous, empty, availability, unmodeled-memory, bracketed-operand (only on `mov`/`movq`, `ldr`/`str`: `:4507-4549`), inexact clobbers and unknown mnemonics.
- Retirement tests: spawn, `provides`, `library`, `abi`, `target`, `invariant`, capability `entry`, trailing `boundary`, `Join<T>`, `[send]`, standalone `boundary requirement`, receiver-style `Build` calls (`:5275-5299`), operator `spelling`/`provider` clauses, `where` clauses, `reserved`, `version` blocks: each asserts a directed message.
- Boundary/reach/crash rows: `explicit_empty_rows_spell_none` (`:2822-2884`), `parses_installation_bound_reach...` (`:6927-6945`), the three `rejects_*installation_bound*` tests (`:6966-7012`), `rejects_operational_members_in_service_reach_rows` (`:7015-7026`).

**Cases by shape.** The tests' own `for` tables enumerate shapes: e.g. numbered-data declarations (`:6001-6008`), contract clause orderings (`:6520-6618`), `try_generic_type_operand` follow-tokens through `boolean_type_operands_preserve_constructor_and_value_contexts` (`:2346-2388`, five roles), atomic member names (`:4759-4767`: `fetch_add/sub/xor/or/and`, `swap`, `compare_exchange`), and unmatched atomic shapes that must stay ordinary bindings (`:4805-4811`).

**Same job elsewhere.** The helper `instruction_statements` (`:4097-4120`) flattens the single-block assembly graph; it asserts one block, so it fails for any `asm` with labels. `parse_source_verbose` vs `parse_source` vs `parse_source_unwrap` (`:8212-8224`) are three wrappers over the same two calls.

**Unused / one-caller code.** Helpers: `parse_source_unwrap` used at `:4776,4813,4876,4932,4994,5033` and others; `argument_path` 5 call sites in this file by grep-free reading (not counted). `source_parsing::lexed_source` is a test fixture used 3 times.

**Stated limits.** Test comments: "the opposite arm is the fall-through and tests nothing" (`:4980`); "parsing retains the authored value; resolution owns tail normalization" (`:1935`); "head classification belongs to declaration selection" (`:3697`, a non-case `Record { ; selected: local }` arm is accepted by the parser and left for later classification); "computed extents are retained for semantic evaluation" (`:7994`).

**Fail-open (observations from what the tests do and do not pin).**
- `move` erasure is pinned as intended behaviour (`:4029-4034`); no test checks that `move` on a non-name or in a call argument position is refused. Status: open.
- No test in this file covers a repeated `terminates by` clause (grepped `terminates by`: 5 hits in `tests.rs`, all single; `tests/diagnostics.rs` has 2, outside this unit), a struct literal missing a comma, or bracketed operands on `out`/`in`/`wrmsr` asm shapes. Status: open (absence of tests, not of behaviour).
- `declaration_lookahead_...` (`:2726-2734`) covers seven `operator`/`domain` look-ahead inputs; no test exercises the `starts_following_item` path through `parse_fact_list`'s `machine_final_semicolon` branch (`contracts_and_parameters.rs:575-579`) beyond `domain_semicolon_preserves_following_mathematical_declarations` (`:1557-1573`) and `parses_consecutive_bodyless_boundary_machines` (`:548-565`) (inferred from reading both).

---

## Coverage

- **Read in full (every line):**
  - `bodies_and_transitions.rs` 1-1402 (1402 lines)
  - `contracts_and_parameters.rs` 1-2656 (2656 lines)
  - `declarations.rs` 1-3285 (3285 lines)
  - `expressions.rs` 1-2466 (2466 lines)
  - `lib.rs` 1-53 (53 lines)
  - `parser.rs` 1-268 (268 lines)
  - `statements_and_assembly.rs` 1-2550 (2550 lines)
  - `tests.rs` 1-8312 (8312 lines)
  - `tokens_and_storage.rs` 1-948 (948 lines)
  - Total read: 21,940 lines (sum of the unit file's line-count column: 1402+2656+3285+2466+53+268+2550+8312+948 = 21,940).
- **Skimmed:** 0 lines. **Not read:** 0 lines.
- Greps used for counts and call-site claims (all over `omega-rust`, `*.rs`, content or count mode, uncapped except where noted): `parse_source_units|parse_source_pages_from|with_parse_workers|parse_worker_count`; `\bparse_sources\b`; `\bparse_syntax_trees_with_id\b`; `\.syntax_for\(|ParsedSyntaxSource|\.syntax_handle\(`; `parse_transition_expression_list|parse_transition_block_target_handle|copy_identifier_path_to_statement_table|has_unary_expression_prefix|parse_const_integer_expression_handle|parse_argument_list_after_open_paren_handle|parse_trait_default_machine_body`; `starts_following_item|starts_boundary_item|starts_machine_declaration|starts_machine_member`; `"Trap" =>|CrashCause::Trap` (this crate, non-test); `\.deepen\(\)` (this crate); `\bMove\b|"move"` in `representations/syntax-trees/src` (0 matches); `#\[test\]` count (this crate `src`); `terminates by` count (this crate).

## Five things a maintainer most needs to know about this unit

1. The parser is a set of `Copy` cursor functions (`Input`, `tokens_and_storage.rs:418`) that return the next cursor; many decisions are made by looking ahead at token spelling with `at_contextual(...)` (identifier or any keyword with that text, `:770-775`), and the contextual-word lists are hand-copied in at least seven places (`..._clause_boundary`/`..._terminator` functions listed in section 5, and `at_contract_continuation_keyword`, `:777-787`).
2. Speculative parsing is done two ways: by `Copy`ing the cursor (free) and by `append_checkpoint`/`rewind_appended` on the arena (about nine sites: `expressions.rs:1305-1315,1374-1451,1462-1467,1513-1518,1931-1951,1994-2040,2148-2182,2443-2463` and `declarations.rs:2179-2183`). `try_parse_destructure_let` and `try_parse_proof_output_binding` (`statements_and_assembly.rs:268-453`) parse an expression into the arena and can return `None` without a rewind.
3. Parse-time rewrites that are more than syntax: `move x` becomes `x` (`expressions.rs:737`); a transition block over Boolean-literal arms gets its last arm's guard replaced by `Always` when the literals cover every combination (`bodies_and_transitions.rs:1068-1107`); non-boolean transition subjects are hoisted into a generated `__transition_subject#N` local (`:953-970`); atomic `let` calls are rewritten into two statements (`statements_and_assembly.rs:39-202`); compound assignment becomes `x = x op y` (`:2306-2340`); `utf16 "..."` becomes an array literal (`expressions.rs:1779-1797`); `asm` blocks are expanded to full flag/scratch-register expression trees (`statements_and_assembly.rs:615-1018`, `:1269-1338`).
4. Only the first parse error per source (per page in parallel mode) is reported, formatted `path:line:col: message`, and an error with a default span prints position of offset 0 (`parser.rs:192,200-215`; `tokens_and_storage.rs:21,55`).
5. Many surface forms are retired, and the parser keeps them as dedicated error branches with migration text (`spawn`, `trap`, `if`, `relax`, `effects`, `decreases`, `increases`, `abi`, `library`, `host`, `platform`, `provides`, `target`, `invariant`, `where`, `self` receivers, standalone `boundary requirement`, receiver-style `Build::` calls, `ForeignBinding::VtableSlot`/`DllImport` string forms). `tests.rs` has a test per retirement.

## What I am unsure about

- Whether Stage 02/05 re-check what the parser rewrites silently: the Boolean-coverage rewrite (`bodies_and_transitions.rs:1068-1107`) and the `move` erasure (`expressions.rs:737`). I did not read those stages.
- Whether `HandleSpanBuilder` contiguity (`tokens_and_storage.rs:104-147`) always holds for the contract/proof-fact paths; I assumed arena appends are back-to-back where the builder is used between appends of other tables (notably `declarations.rs:1050`).
- The inner list `[15, 16, 19, 20]` and `ASM_ZERO_FLAG_SLOT` used in `scratch::bind` (`statements_and_assembly.rs:765-767`) refer to indices into `language_core::inline_assembly::ASM_SCRATCH_NAMES`, which I did not read.
- I did not read `type_syntax_and_patterns.rs`, so claims about `parse_type_reference_handle*`, `parse_destructure_fields`, `parse_domain_argument_handles`, `parse_property_brackets`, `bindings::{DestructureBinding, rewrite_destructure_guard_expression}` are about call sites only.
- Count of `#[test]` in `tests.rs` is from `grep -c '#\[test\]'` (265); I did not verify that every `#[test]` is inside a module that compiles under `#[cfg(test)]` (lib.rs:52-53 gates the whole file).
- The claim that the `02_syntax-trees-to-symbol-resolved-trees` call sites of `parse_syntax_trees_with_id` are test code is inferred from line numbers.
