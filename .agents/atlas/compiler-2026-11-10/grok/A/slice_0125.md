# slice_0125

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0960 | omega-rust/psi/compilation/build-time-evaluation/src/const_initializers.rs:3288 | UNDECIDED | `target_observations::retain_entries` only retains scalar destinations; aggregate pending-target path not traced to accept/reject. |
| A0961 | omega-rust/psi/compilation/build-time-evaluation/src/syntax_probes.rs:652-661 | CLOSED | Shipped compile always passes `sources: Some`; empty bindings apply only when `sources` is absent and that path is not the product driver. |
| A0962 | omega-rust/psi/compilation/build-time-evaluation/src/product.rs:242-253 | REACHABLE | Product admission can push `None` for a captured native binding when the requirement file is not loaded, with no error. |
| A0963 | omega-rust/psi/compilation/build-time-evaluation/src/machine_execution.rs:455 | REACHABLE | Call targets with no machine-parameter or trait signature skip requires, linearity, and termination checks along the call-edge walk. |
| A0964 | omega-rust/psi/compilation/build-time-evaluation/src/layouts.rs:3955 | UNDECIDED | Without a source map, `Access` with two arguments is treated like the toolchain template; callers that pass `sources: None` were not traced. |
| A0965 | omega-rust/psi/compilation/build-time-evaluation/src/layouts.rs:5338-5348 | REACHABLE | Open layout policies with no applicable targets are skipped and never evaluated, so layout obligations can be left unset. |
| A0966 | omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lexer.rs:63-69 | CLOSED | Malformed numeric lex flags are rejected only in stage 01 `parse_integer_literal` / `validate_float_literal`, on every literal consume path. |
| A0967 | omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/expressions.rs:668-669 | CLOSED | Expression `move` is erased at parse time; no later `omega-rust` stage reads a move marker, so checking is identical to omitting `move`. |

## A0960

**Trace (read).** At `const_initializers.rs:3278-3307`, `target_observations::retain_entries` keeps const declarations only when `domain.contains`, `evaluated_value.is_none()`, no valid `evaluation_entry`, and `scalar_probe_destination(typed, declaration.declared_type).is_some()` (`:3285-3288`). `settle` at `:3316-3325` applies the same `scalar_probe_destination` filter. Pending-target flags are set in the syntax initializer driver (`:3864-3868`, `:3913`, `:4134-4137`) and in typed pre-check deferral via `has_pending_target_observations` (`const_evaluation.rs:3872-3890`, `lib.rs:643-646`), which also lists consts with `pending_target_observation` (`:3886-3889`). The digest (area_07) states that aggregates with a pending target observation are not handled here and where they are handled was not traced.

**Verdict.** UNDECIDED: whether a program that should be rejected can complete constant/layout target work without passing through `target_observations` for non-scalar pending targets depends on handlers not opened in this slice.

## A0961

**Trace (read).** `normalization_request` at `syntax_probes.rs:645-664` sets `top_level_bindings` to `Vec::new()` and `retained_base` to `None` when `sources.is_none()` (`:652-661`). It is used from `normalize_type_aliases` (`:535`), `normalize_generic_data` (`:556`), and `normalize_initializer_probe_data` (`:575`) — search `normalization_request` in `omega-rust`: 4 matches, all in `syntax_probes.rs`. Upward: `evaluate_pre_resolution` (`lib.rs:208-237`) always receives `BuildTimeSources` from the driver. `source_compilation.rs:795-805` passes `sources: Some(Arc::clone(&self.sources))` with bindings; extension at `:1140-1147` also uses `sources: Some`.

**Input that would hit the branch.** Only a caller that invokes normalization with `sources: None` while still needing scoped bindings or a retained base for correct rejection.

**Verdict.** CLOSED for product compilation: the observed driver paths always supply a source map, so the drop branch is not on the acceptance path for ordinary programs.

## A0962

**Trace (read).** In `collect_captured` (`product.rs:228-316`), `requirement_loaded` is true only when a product dependency file matches `declaration.requirement.source().path` with map/selection checks (`:242-249`). If `!requirement_loaded`, the loop pushes `None` to `resolved` and `continue`s (`:250-252`) without diagnostic. Caller search `collect_captured(` in `omega-rust`: 2 matches — definition and `psi-compiler/src/source_compilation.rs:486` (`admit_captured_native_bindings`). Same digest cluster (read): default span on unmatched remap (`:2456-2458`), ignored non-`select_provider` calls (`:3140-3141`), selections with no target matching every scope (`:1789-1790`).

**Smallest program (plain words).** A Build product that records a captured native binding pointing at a requirement source file that is not part of the loaded product closure; admission proceeds with a missing resolved binding instead of failing.

**Verdict.** REACHABLE.

## A0963

**Trace (read).** `callable_signature` (`machine_execution.rs:421-441`) returns `None` for invalid symbols, builtins/operators, and symbols with neither `machine_parameter_signature` nor `trait_machine_signature`. `callable_contract_violation` (`:449-496`) returns `None` immediately when `callable_signature` is `None` (`:455-456`), so no requires, linearity, or termination messages run. Caller: `machine_termination_violation` (`:364-374`) invokes it when `call.target_machine(program)` is `None`. `machine_termination_violation` when `program.machine(machine_symbol)?` fails returns `None` at `:262` (read: early `?` on missing machine). `walk_call_closure` (`:3401-3423`) at `:3410-3411` `continue`s when `program.machine(symbol)` is missing, omitting that symbol from suspend/block summary walks (`digest :3410-3412`). Search `callable_contract_violation` in `omega-rust`: 2 matches (definition + one call site).

**Smallest program (plain words).** A compile-time-evaluated call graph that reaches a call edge whose target is not a machine but a callable symbol with no parameter/trait signature, while that target would need a requires, linear, or termination check if it were recognized.

**Verdict.** REACHABLE.

## A0964

**Trace (read).** In `discover_applications` (`layouts.rs:3856+`), `canonical` requires toolchain `layout.omg` `Access` when a source map is available (`:3942-3954`). If `!canonical && (sources.is_some() || base_name != "Access")`, the loop `continue`s (`:3955-3956`); with `sources.is_none()` and `base_name == "Access"`, processing continues. When `sources.is_some()` and a closed operand lacks selection, diagnostics fire (`:4002-4009`); `None => {}` at `:4011` skips that when no map. `discover_applications(` search in `omega-rust`: 2 matches — call from `desugar_access_views` (`:3555`) and definition. `desugar_access_views` (`:3450-3458`) takes `context.sources.clone()`; product driver passes `Some` (`source_compilation.rs:799`).

**Verdict.** UNDECIDED: the permissive `Access` branch exists at `:3955-3956`, but this slice did not trace every caller that passes `sources: None` into `desugar_access_views` / `discover_applications`.

## A0965

**Trace (read).** `scoped_representation_requests` (`layouts.rs:5303-5360`) iterates layout storage occurrences (`:5309-5311`). For record types, if the policy is not closed, the symbol is a conformance parameter, it `continue`s (`:5338-5343`). If policy is not closed and `applicable_targets` is empty, it `continue`s (`:5346-5347`) without pushing a request — no policy machine evaluation. Caller search `scoped_representation_requests` in `omega-rust/psi/compilation/build-time-evaluation`: 3 matches (definition + `:5283`, `:5296` install paths). Related: open-policy evaluation can `continue 'application` on `StaticInputUnavailable` (`:5634-5637`, read).

**Smallest program (plain words).** A generic or open access-policy type used with layout storage where no build-time target applies; compilation skips representation layout work that a closed policy would have forced.

**Verdict.** REACHABLE.

## A0966

**Trace (read).** Lexer sets `integer_kind.empty_digits` for `0b`/`0o`/`0x` with no digits (`lexer.rs:63-76`) and `float_kind.empty_exponent` when `e`/`E` is not followed by digits (`:116`). Stage 01 `parse_integer_literal` rejects `kind.empty_digits` with `"invalid integer literal"` (`tokens_and_storage.rs:250-251`). `validate_float_literal` rejects `kind.empty_exponent` with `"invalid float literal"` (`:290-291`). Consumption: `take_parsed_integer` (`:682-683`) and `take_float_text` (`:740-741`) in the same file. Search `empty_digits` / `empty_exponent` in `omega-rust`: only `lexer.rs`, `token_kind.rs`, and these checks (read via grep). Digest area_07 (checked) agrees flags are rejected in stage 01 and nowhere else.

**Smallest program (plain words).** Source containing `0x` or a float literal with an empty exponent; lexer emits a token, stage 01 parse fails on the flag.

**Verdict.** CLOSED: every literal intake path runs the flag checks before acceptance.

## A0967

**Trace (read).** Prefix parse accepts contextual `move` (`expressions.rs:668-669`). `UnaryPrefix::Move` returns the operand unchanged (`:737`). Test `iterative_prefixes_preserve_borrows_literal_folding_and_call_acknowledgements` expects `&write move value` to parse as borrow of `value` (`tests.rs:4029-4034`). Search `\bmove\b` in `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src` (excluding tests): only `expressions.rs:668-669,737` and unrelated asm/register names (read). No downstream stage records a move flag on the syntax tree.

**Smallest program (plain words).** An expression where the author writes unary `move` before a name expecting the compiler to treat it differently from the bare name; the stored tree is the same as without `move`, and later checks cannot distinguish them.

**Verdict.** CLOSED for wrongful acceptance via a missing move marker: no checked stage consults the keyword after stage 01, so accept/reject does not hinge on erasure.

## What I did not read

- Full `target_observations::settle` and the syntax-side `const_initializers::evaluate` loop beyond cited regions (`const_initializers.rs` ~3300–4200+).
- Entire `machine_execution.rs` admission floor and effect-summary construction beyond cited call-closure sites.
- `validation/` ownership/custody pipelines for expression transfers (beyond grep showing no `move` keyword use).
- `product.rs` admission graph beyond cited spans (`:1789`, `:2458`, `:3140`).
- `layouts.rs` install path after `scoped_representation_requests` except `:5634-5638`.
- Commit `457adf4a11f` file snapshots (`git show` not run); symbols were re-located at current checkout lines cited above.

SLICE COMPLETE
