# slice_0137

Digest preamble: `digests/area_12.md` lines 1–8 (`457adf4a11f`; path prefix `TT/` = `omega-rust/psi/compilation/representations/typed-trees/src/`, `DTI` = `TT/declaration_tables_and_index.rs`). Read.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1053 | TT/callable_applications.rs (multiple) | REACHABLE | Requirement and application preparation skips scope checks, drops runtime bindings, and passes through unsubstituted types/lifetimes on several branches. |
| A1054 | TT/calls_and_control_flow.rs (multiple) | REACHABLE | Unknown contract publicity, permissive runtime operand ordering, and unremapped `static_machine_parameter` let ill-formed calls stay eligible or copied. |
| A1055 | DTI + TT/declaration_tables_and_index.rs (multiple) | REACHABLE | Index and target selection keep stale or ambiguous rows; invalid handles can match index rows (test-pinned pattern). |
| A1056 | TT/declarations.rs (multiple) | REACHABLE | Completion, naming, zip, and invokes handling default to permissive outcomes instead of errors. |
| A1057 | TT/access_views_and_commitments.rs (multiple) | REACHABLE | Recipe array extents admit any fixed index; toolchain path trust does not always compare bytes. |
| A1058 | TT/type_identity.rs + TT/type_system.rs (multiple) | REACHABLE | Unreachable static context forces type match; several identity and constraint paths accept empty or invalid handles. |
| A1059 | TT/values.rs (multiple) | REACHABLE | Structural equality and integer/subtree utilities ignore call shape and static operands relevant to rejection. |
| A1060 | TT/references.rs (multiple) | REACHABLE | Non-calls skip stamping; `complete` is ignored; several call payload fields are not visited. |

## A1053

**Verdict:** REACHABLE  
**Sites (read):** `callable_applications.rs:6386-6394`, `:6548-6587`, `:6779-6781`, `:6821-6823`, `:7370-7414`, `:7459-7475`, `:7629`, `:346-375`, `:976-1018`, `:1066-1071`, `:3660` (digest cites match at read checkout).

- `requirement_invocation_signature` (`6331-6427`, read): When `closed_requirement_application` matches and `requirement_application` is invalid, `6386-6393` scope checks are skipped and `6394` returns `Ok(closed.signature)`. **Input:** closed requirement symbol with valid closed table row, invalid/default requirement application handle, valid authored application as needed for earlier branches.
- `prepare_requirement_invocation_application` (`6548-6587`, read): Returns `Ok(None)` without error when closed application, invalid authored application, missing trait signature, or empty type parameters (`6548-6559`). `RuntimeValue` binding arm is empty (`6584-6586`), so runtime actuals are dropped silently.
- `CallableApplication::nominal` loop (`6779-6781`, read): `nominal(...)?` yielding `None` causes `continue` with no error—requirement refinement application omitted.
- `refinement_invocation_scopes` (`6821-6823`, read): Missing `machine_by_symbol(record.binding_declaration)` `continue`s without recording failure.
- Lifetime/symbol plumbing (`7370-7414`, `:7400-7406`, read): Missing trait region substitutes pass original lifetime name (`7370-7375`); unmapped symbols pass through `7408-7414`.
- `nominal_contract` (`7459-7475`, read): Stores `instantiated: applied.map(Box::new)` allowing `None` in `MachineParameterContract::Nominal`.
- Elided lifetimes pushed without argument (`7629`, read).
- Applied contract call (`346-375`, read): When selected equals parameter with no supplied application, `374` returns `Ok(None)` leaving call unsubstituted.
- `domain_identity` (`1066-1071`, read): Unknown/missing domain returns `Ok(original)` identity.
- `apply_expression` default arm (`3660`, read): Unlisted expression kinds become `None` without inner type substitution (surrounding `3662-3664` only applies when `applied` is `Some`).

**Callers (search, `omega-rust`, word-boundary):** `requirement_invocation_signature` — 5 matches (`callable_applications.rs` 4, `references.rs` 1); `prepare_requirement_invocation` — 6 matches (`callable_applications.rs` 4, `contextual_and_requirement_calls.rs` 2). Read signatures only at `references.rs:6316-6328` (`requirement_signature_for_call` uses `.ok()`).

**Smallest program (plain words):** A module with a closed trait requirement used from evidence such that the written requirement type application handle is absent or invalid; the compiler still resolves the public requirement signature without matching application scope and accepts the call surface.

## A1054

**Verdict:** REACHABLE  
**Sites (read):** `calls_and_control_flow.rs:267`, `:1130-1139`, `:1647-1649`, `:2875-2879`, `:3192`, `:3407-3414`, plus `467-516` for publicity helper.

- `requirement_contract_names_private_identity` (`255-268`, read) uses `contract_identity_is_public(...) == Some(false)` at `267`. `contract_identity_is_public` (`467-516`, read) returns `None` when the symbol appears in none of nine declaration tables.
- **Effect:** Symbols absent from all tables are not treated as private (`None != Some(false)`), so `dynamic_non_signature_eligibility` (`237-253`, read) does not return `PrivateContractIdentity` for proof-fact symbols that are not declaration roots. Production use: `omega-rust/psi/compilation/validation/src/traits.rs:5815` calls `dynamic_non_signature_eligibility` (read call site name only).
- Runtime operand order (`1130-1139`, read): If every binder fails `derives_from` match and closed static operands exist, `1138` returns `natural()` with no error.
- `statement_call_matches_signature` (`1647-1649`, read): Invalid receiver with other guards passing yields `true`.
- Statement remap (`2875-2879`, read) remaps several symbols; `static_machine_parameter` field not among them (contrast `copy_transition_target_deep_from` `3367`, read, which copies `static_machine_parameter` verbatim).
- `collect_direct_return_values` (`3180-3193`, read): Only terminal `StatementNode::Expression` on last statement; other statement kinds fall through `_ => {}`.

**Callers:** `contract_identity_is_public` — 2 matches, both `calls_and_control_flow.rs` (definition + use); `dynamic_non_signature_eligibility` — 2 matches (`calls_and_control_flow.rs`, `validation/traits.rs`).

**Smallest program (plain words):** A dynamic-slot requirement whose `requires` proof mentions a symbol handle that does not appear in any of the nine publicity tables (for example a locals-only or stale handle), so dynamic non-signature eligibility passes when it should treat the contract identity as non-public.

## A1055

**Verdict:** REACHABLE  
**Sites (read):** `declaration_tables_and_index.rs:2373-2375`, `:2358`, `:4222-4224`, `:1698-1705`, `:4539-4541`, `:1581-1609`, tests `71-111`.

- Trait implementation filter (`2369-2384`, read): When `selected.source_scope.scope(self)` is `None`, filter keeps the row (`2373-2375` `return true`).
- `machine_for_target` (`4214-4225`, read): `target_family_interface` false and no authored target ⇒ `is_none_or(|declared| declared == target)` is true for every `target` string; returns `Some(machine)`.
- `machine_containing_state` (`1698-1705`, read): No `state.is_valid()` guard before `index.first(state)`; wrong-generation handle with same arena index can match a row (pattern pinned in `data_definition_rows_preserve_unique_and_first_match_queries`, `71-111`, read).
- `machine_kind_type_parameters` (`4539-4541`, read): Non-listed `SymbolKind` arms fall through to `unwrap_or_default()` (empty type parameters).
- `structural_fingerprint` (`1581-1609`, read): Adds revisions/counts beyond `tables_fingerprint` (`1400-1438`, read) but digest notes five arenas omitted from revision sum (`digests/area_12.md:223`, read); whether stale memo affects acceptance not fully traced.

**Callers:** `machine_for_target` — grep not fully counted; `machine_containing_state` — 9 production call sites listed in earlier grep (read count mode listing).

**Smallest program (plain words):** A machine with no authored target string used through `machine_for_target` with an arbitrary target label, or a state lookup via a stale-generation handle sharing an arena index with a live state symbol, yielding the wrong machine container.

## A1056

**Verdict:** REACHABLE  
**Sites (read):** `declarations.rs:55`, `:113`, `:312`, `:1088-1096`, `:1527-1535`, `:972-973`, `:2069-2086`, `:2199-2212`.

- Family completion disagreement ⇒ `NormalReturn` (`55`, read); invalid/duplicate boundary completion rows ⇒ `NormalReturn` (`112-113`, read).
- Per-file `Calling` / `CallingPolicy` map (`310-314`, read): second `"Calling"` trait overwrites `entry.0`.
- Domain const identity (`1088-1096`, read): Unrecognised name becomes `binder:...` string, not error.
- `Closure::extend` (`1527-1535`, read): Kinds not in the listed open-parameter set (including e.g. `PropositionMachineParameter` if present) do not set `open`.
- `zip` truncates domain index substitutions (`970-973`, read).
- `declared_targets` drops `AuthoredInvocationTarget::Unresolved` (`2069`, read) silently.
- `layout_target_facts_result` (`2199-2212`, read): Toolchain path check only; no byte compare on source (contrast sealed layout check later in function).

**Smallest program (plain words):** Two `Calling` traits in one source file so the second overwrites the first in the census map, or an `invokes` entry left unresolved so the boundary target list omits it without diagnostic.

## A1057

**Verdict:** REACHABLE  
**Sites (read):** `access_views_and_commitments.rs:1972-1977`, `:357-370`, `:479-493`, `:768-776`, `:2178`.

- `DeclaredArrayExtent::admits_fixed_index` (`1972-1977`, read): `Recipe` ⇒ always `true`.
- **Callers (grep, `omega-rust`):** `admits_fixed_index` — 4 matches: definition; `value_custody_core.rs:6734,6743,6752` reject when false; `execution_terminal_unit_types.rs:965` accepts path segment only when true (read). Recipe extent therefore admits any fixed index in the accepting branch at `965`.
- Sealed placement `layout.omg` / `run.omg` (`357-370`, read): Requires exact byte equality to `LAYOUT_SOURCE` / `RUN_SOURCE`; otherwise `Ok(None)` (ordinary declaration), not error—right path, wrong bytes.
- Template source `768-776` and accessor `479-493` (read): Toolchain origin and path `layout.omg` only; no byte compare at `768-776`.
- `2178` (read): Merges recipe contexts without rejecting ambiguous scope (`2174-2180`).

**Smallest program (plain words):** A fixed-array type whose length is a non-literal recipe extent, indexed at a constant larger than any closed length, passing the `admits_fixed_index` guard on the checked-unit path.

## A1058

**Verdict:** REACHABLE (with one UNDECIDED sub-point on zero-world producers)  
**Sites (read):** `type_identity.rs:7850`, `:326-330`, `:7654-7711`, `:7485`, `:552`, `:5765-5842`; `type_system.rs:760-767`, `:1889-1890`, `:884`.

- `type_identities_match_in_static_context` (`7850-7851`, read): Unreachable static context ⇒ `true` before comparing identities. Caller: `expression_types_and_fields.rs:6729` (grep 2 total).
- Callable memo (`326-330`, read): Failed `derive()` ⇒ `unwrap_or_default()` shared invalid id for distinct failures.
- `normalized_type_identity_text` (`7654-7657`, read): Failed text ⇒ `""`.
- `data_field_by_authored_or_retained` (`7485`, read): `!retained.is_valid()` accepted in disjunction.
- `sweep_callable_identities` skips machines with `target.is_some()` (`552-553`, read).
- `TypeWorld::constrain` (`879-884`, read): Non-closed index arguments ⇒ returns `Some(self)` without exclusion when unequal.
- Static context install (`1889-1890`, read): `!context.reachable` clears worlds (dead code path). **UNDECIDED:** whether a producer can mark `reachable: false` incorrectly while execution still accepts the program—producers of `reachable` not read beyond `type_system.rs:1880-1898`.

**Smallest program (plain words):** Two incompatible types compared for equality in a static context that the type-context table marks unreachable, so the comparison returns match.

## A1059

**Verdict:** REACHABLE  
**Sites (read):** `values.rs:4065-4097`, `:112-130`, `:5183-5260`, `:5046-5059`, `:2279-2283`; `const_integer.rs:177-183`.

- `expressions_structurally_equal` on `Call` (`4091-4096`, read): Compares target symbols/text, receiver, argument spans only—no `case_variant`, machine arguments, dispatch, or build selection (digest confirmed; read).
- `is_integer_expression` (`112-130`, read): `fold` with `_ => Some(Open)` for partially unknown integer ops (`103-107`); `apply` uses `checked_div` (`177-183`, read) so literal `Closed/0` fails fold, but `Open` divisor shape still yields `Some(())` at `127-128`.
- `subtree_children` on `Call` (`5227-5232`, read): Visits receiver and expression arguments only—not static machine arguments, quotient, interrupt, layout fields.
- `direct_pattern_component` (`5046-5059`, read): Non-local `Name` with case pattern component can classify as fields via `PatternComponent::Fields` default at `5046`.
- Copy remap (`2279-2283`, read): `StatementArgument` call handle forced invalid on copy.

**Callers:** `expressions_structurally_equal` — 53 matches across validation, pipeline, proof (grep count); used for equivalence, not sole gate (read).

**Smallest program (plain words):** Two call expressions that differ only in static machine argument tuples or requirement dispatch but are treated as structurally equal for validation that relies on this equality.

## A1060

**Verdict:** REACHABLE  
**Sites (read):** `references.rs:164-166`, `:241-305`, `:3117-3143`; callers `85`, `111`, `168`, `322`, `340` (read).

- `retain_call_static_argument_context` (`164-166`, read): Non-call expression ⇒ `Ok(())` without stamping static context.
- `retain_static_operand_sources` (`241-305`, read): Computes `complete` but callers at `111`, `168` use `?` on `Result` only; `322`, `340` ignore `bool`. Incomplete operands proceed.
- `visit_call_expression` (`3117-3143`, read): Visits quotient, interrupt, layout, and expression arguments; omits other `Call` fields present in the struct (e.g. static requirement dispatch fields not shown in this block—read through `3143`).

**Callers:** `retain_call_static_argument_context` — 2 call sites (`references.rs:85`, `assembly_and_operators.rs:1890`, read second path name); `retain_static_operand_sources` — 6 matches, all `references.rs` (grep).

**Smallest program (plain words):** A non-call expression submitted to the static-argument retention pass, which returns success without attaching static machine application metadata that later stages expect for evidence.

## What I did not read

- Full bodies of `callable_applications.rs:976-1018` cleanup path beyond the opened header and early returns.
- Every downstream validation consumer for fingerprint staleness (`structural_fingerprint` memo sweeps in `type_identity.rs` and `immutable_checking_input.rs`).
- All producers that set `StaticTypeContext.reachable` outside `type_system.rs:1880-1898`.
- `git show 457adf4a11f:` relocations for lines that already matched at read (none required for cited blocks opened above).
- Omega product compiler outside `omega-rust/psi` for acceptance decisions beyond named call sites.

SLICE COMPLETE
