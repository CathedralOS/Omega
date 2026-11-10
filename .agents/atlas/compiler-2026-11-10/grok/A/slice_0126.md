# slice_0126

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0968 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/bodies_and_transitions.rs:1068-1107 | REACHABLE | Syntactic boolean arm matrix can force `Always` on a non-boolean subject; S03 exhaustiveness treats `Always` as default-closing. |
| A0969 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/contracts_and_parameters.rs:743-757 | REACHABLE | A second `terminates by` silently overwrites ranking fields from the first with no diagnostic. |
| A0970 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs:51-55 | REACHABLE | Optional `;` between fields and struct-literal fields without commas both parse as valid syntax. |
| A0971 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs:1205 | REACHABLE | Bracket-carrier domains get the label `?::Name` because non-`Named` type refs render as `?`. |
| A0972 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/statements_and_assembly.rs:1900-2107 | REACHABLE | `out`/`in` and related asm shapes parse operands without `reject_bracketed_asm_operand`. |
| A0973 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tokens_and_storage.rs:117-125 | UNDECIDED | `push` never verifies adjacency; no user syntax path to non-adjacent handles was traced. |
| A0974 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs:201-210 | REACHABLE | `machine_arguments`, `evidence_arguments`, and `target` bypass destructure rewrite inside calls. |
| A0975 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs:94-179 | REACHABLE | Several const-validation paths return `Ok(())` without rejecting ill-typed or mis-qualified initializers. |

## A0968

Digest row (read): stage-01 transition parsing may set the last arm guard to `Always` when boolean-literal pattern tuples cover all combinations, without checking subjects are boolean-typed; stage 05 was not traced in the digest.

Opened `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/bodies_and_transitions.rs:934-1110` (read). `parse_transition_block_handles` hoists non-`expression_is_closed_boolean` subjects (`:954-969`) but builds `arm_bool_tuples` from pattern expressions that are only `ExpressionNode::Boolean` (`:397-406` via `parse_transition_guard_node` at `:337-415`). Coverage at `:1085-1092` is purely syntactic over those tuples; when `covered`, the last arm guard becomes `TransitionGuardNode::Always` (`:1098-1105`).

Caller search: pattern `\bparse_transition_block_handles\b` in `omega-rust`, count **3** (1 definition, **2** call sites — read `statements_and_assembly.rs:2159-2161` from `parse_statement_handles`).

Upward acceptance: `validate_case_dispatch_exhaustiveness_in` in `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:829-873` (read) calls `check_dispatch_run` per dispatch run. `classify_arm` (`:1319-1323`, read) maps any guard other than `When` — including `Always` — to `ArmShape::Default`; `check_dispatch_run` (`:941-943`, read) returns `Ok(())` on the first `Default` arm without typing subjects. Stage 05 `execution.rs:517` (read) treats `TransitionGuardNode::Always` as unconditionally true when building execution behavior.

Plain input: a machine state whose transition block binds one integer dispatch subject (not a closed boolean expression) but lists arms `true -> ...` and `false -> ...` so the parser marks the second arm `Always` and S03 exhaustiveness accepts the run.

**Verdict: REACHABLE**

## A0969

Digest row (read): repeated `terminates by` overwrites earlier ranking witness; last wins.

Opened `parse_machine_clauses` in full at `contracts_and_parameters.rs:704-760` (read). Each `terminates by` branch assigns `ranking_subjects`, `ranking_view`, `ranking_view_arguments`, and `ranking_range` (`:752-755`) with no check for a prior witness and no error.

Caller search: pattern `\bparse_machine_clauses\b` under `01_tokens-to-syntax-trees`, count **3** (1 definition at `:704`, **2** uses in `declarations.rs` — read grep listing only; enclosing machine declaration parse accepts the clause bundle).

Plain input: a machine header spelling two different `terminates by ...` clauses; only the second witness is retained in the syntax tree while the first is dropped silently.

**Verdict: REACHABLE**

## A0970

Digest row (read): optional semicolons after data fields, case members, and capability fields; struct literals without commas between fields.

Read `declarations.rs:51-55`, `:760-764`, `:806-810` (read): after `parse_type_reference_handle`, semicolon is optional (`if ... Semicolon ... else rest`). Same for case members (`:806-810`) and capability fields (`:51-55`).

Read `expressions.rs:2081-2124` (read): `parse_struct_literal_handle` pushes fields in a loop; only an optional comma separates fields (`:2099-2102`); no semicolon required between `a: 1` and `b: 2`.

Caller paths: field parsers are reached from data/capability declaration parsing (read enclosing functions in `declarations.rs`); struct literals from expression parsing (read `parse_struct_literal_handle` signature and callers via grep — production parse path, not tests).

Plain input: a data declaration body `a: u8 b: u8` and a value `T { a: 1 b: 2 }` both parse though a stricter grammar could reject missing separators.

**Verdict: REACHABLE**

## A0971

Digest row (read): domain on a reference or generic carrier with no type parameters is named `?::Name`.

Read `declarations.rs:905-935` and `type_reference_target_label` at `:1187-1206` (read). When the domain head is introduced with a bracketed type reference (`:917-924`), `label = type_reference_target_label(syntax_trees, handle)`. For non-`Named` references the match arm `_ => "?".to_owned()` at `:1205` supplies the label, yielding `?::` plus the domain identifier segment parsed at `:936-938`.

Plain input: `domain [&u8]::MyPred requires ...` (carrier parsed as a bracket type reference) stores the public label `?::MyPred` rather than rejecting or spelling the carrier.

**Verdict: REACHABLE**

## A0972

Digest row (read): bracketed-operand rejection skipped for port, MSR, control-register, system-register, flags, fence, and GDT shapes; example `out [x], y`.

Read `reject_bracketed_asm_operand` at `statements_and_assembly.rs:1549-1566` (read): errors when an operand is `ExpressionNode::ArrayLiteral` (bracket syntax).

Read `parse_asm_instruction_statement_handle` shape dispatch `:1698-2107` (read). `AsmInstructionShape::PortOut` (`:1900-1911`) parses `port` and `value` via `parse_expression_handle` only — no call to `reject_bracketed_asm_operand`. Same for `PortIn`, MSR, control/system register, flags, fence intrinsics, and GDT (`:1900-2107`).

Caller search: pattern `\breject_bracketed_asm_operand\b` in `omega-rust`, count **4** (1 definition, **3** call sites in register-move/arithmetic shapes at `:1720-1725`, `:1854`, `:1860` — read).

Plain input: `asm { out [x], y }` where `[x]` becomes an array-literal expression operand; parsing succeeds while mov/add-style instructions would reject the same operand shape.

**Verdict: REACHABLE**

## A0973

Digest row (read): `HandleSpanBuilder::push` never checks that pushed handles are adjacent; digest says callers were adjacent (inferred).

Read `tokens_and_storage.rs:104-125` (read): `push` increments `count` only; no comparison to prior handle index.

Read representative caller `bodies_and_transitions.rs:972-1054` (read): statements are appended sequentially via `append_statement_handle` then `statements.push(handle)`, which assumes arena allocation order matches span contiguity (inferred).

Repository search: pattern `\bHandleSpanBuilder\b` in `omega-rust`, count **31** matches across stage-01 parser files (read grep count output); not every `push` site was opened.

Could not determine a smallest source program whose parse intentionally pushes non-adjacent handles into one finished span; deciding would require opening all 31 builder sites or a failing test demonstrating corrupt spans.

**Verdict: UNDECIDED**

## A0974

Digest row (read): destructure rewrite skips `machine_arguments`, `evidence_arguments`, and `target` on calls; `SavedSubject` keeps original capture when no copy matches (`:145`).

Read `rewrite_expression` `ExpressionNode::Call` arm at `type_syntax_and_patterns.rs:188-212` (read): `machine_arguments`, `evidence_arguments`, and `target` are copied verbatim; only `arguments` go through `rewrite_expression_span`. Read `SavedSubject` arm `:136-148` (read): `unwrap_or(saved.capture)` when no copied capture matches.

Caller search: `rewrite_destructure_guard_expression` is used from transition guard parsing and match lowering (read `bodies_and_transition.rs:540`, `expressions.rs:174` via grep); the call rewrite path is inside the same `rewrite_expression` used for guard bodies.

Plain input: a transition arm guard call that passes a destructured binding name in `machine_arguments` or as the call `target` string/id leaves the pre-rewrite capture, so the tree still names the outer pattern slot instead of the generated subject copy.

**Verdict: REACHABLE**

## A0975

Digest row (read): `retain_value_carrier` returns `Ok(())` for non-builtin/non-numeric carriers; nominal and scalar validation skip several destinations; invalid head on a const path still resolves by name.

Read `constant.rs:94-179` `retain_value_carrier` (read): non-array, non-`Named` declared types return `Ok(())` at `:115-116`; non-builtin named atoms return `Ok(())` at `:118-119`; for `ExpressionNode::Integer` on a `bool` carrier, neither `integer_type` nor `float_format` is set for `Bool`, so the match falls through `_ => {}` at `:176` and returns `Ok(())`.

Read `validate_nominal_destinations` `:373-409` (read): `TypeReference::Named` with empty generic application and other non-struct cases `continue` without checking initializer shape.

Read `validate_scalar_initializer` `:519-533` (read): if `as_named()` fails or `!is_builtin_scalar_carrier(...)`, returns `Ok(())` immediately — a type alias spelling of `f32` skips scalar rules.

Read `selected_expression_constant` `:690-718` (read): only rejects paths whose `head_symbol` is Local/Parameter/MachineParameter/TypeParameter/ConformanceParameter/Field (`:696-705`); otherwise joins member names and `find_top_level_by_name_and_kinds_from_source` for `Const` (`:714-717`), ignoring a wrong/invalid head qualifier.

Caller search: pattern `\bretain_value_carrier\b|\bretain_declared_carrier\b` in `02_syntax-trees-to-symbol-resolved-trees`, count **6** (read grep count).

Plain input examples: `const flag: bool = 0` survives carrier re-landing; `const x: MyAlias = 1` when `MyAlias` aliases `f32` skips `validate_scalar_initializer`; `wrong_qualifier::C` where `C` is a top-level const still records as a const dependency via name lookup.

**Verdict: REACHABLE**

## What I did not read

- Full bodies of every `HandleSpanBuilder` push site (31 matches) for A0973.
- Every stage-05 file referencing `TransitionGuardNode::Always` beyond `execution.rs:517` and the cited exhaustiveness path.
- `git show 457adf4a11f:` paths (current checkout lines matched cited sites on read).
- Build-time evaluation and stages 06–08 for any row in this slice.

SLICE COMPLETE
