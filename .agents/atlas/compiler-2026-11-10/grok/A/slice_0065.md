# slice_0065

Digest preamble (read): `digests/area_14.md` lines 1–24 — Area 14 is validation’s second half; `mpb` is `omega-rust/psi/compilation/validation/src/machine_parameters_and_bindings.rs`, `ops` is `operators.rs` in the same directory; citations at commit `457adf4a11f`, re-read at current checkout (line numbers matched).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0497 | mpb:4941-4943 | REACHABLE | Machine-parameter slot whose resolved symbol equals the binder symbol skips nominal and callable-shape validation. |
| A0498 | mpb:4028-4033 | REACHABLE | Invalid interrupt registrar application on a machine with open parameters returns with no error. |
| A0499 | mpb:7679,7923 | REACHABLE | Incomplete or empty static binding tuples skip conformance checking on the acceptance path. |
| A0500 | mpb:7396-7401 | REACHABLE | Undeclared type/const operands make `is_declared_source_tuple` return true and admit the tuple. |
| A0501 | mpb:6092-6094 | REACHABLE | Missing rebuilt applied signature ends requirement-predicate validation with no tuple check. |
| A0502 | mpb:6912-6947 | REACHABLE | `validate_arguments` can `continue` past ordinals with no declaration, forward, or spelling and no matching error branch. |
| A0503 | ops:84-86,107-109 | REACHABLE | `Ok(None)` for open operator static applications is treated as success with no closed application recorded. |
| A0504 | ops:1753-1756 | UNDECIDED | Builtin fallback keys on primitive spelling via `type_is_primitive`; user `Named` types that shadow primitive names were not traced. |

## A0497

**Cited:** `machine_parameters_and_bindings.rs:4941-4943` (read).

**Enclosing function (read):** `form_call_selection` (`mpb:4716-4982+`) loops machine-parameter requirements zipped with `machine_arguments`. After nested-application diagnostics (`mpb:4924-4939`), if `selected.symbol == parameter.symbol` it `continue`s (`mpb:4941-4943`) before `validate_nominal_machine_selection` (`mpb:4951`) and `validate_selected_callable_shape` (`mpb:4968`).

**Callers:** ripgrep `\bform_call_selection\(` under `omega-rust` → count **4** (all in `machine_parameters_and_bindings.rs`: `mpb:1290`, `mpb:4061`, `mpb:4691`, plus definition at `mpb:4716`).

**Upward (read):** `form_static_machine_selections` (`mpb:4453+`) calls `validate_call_selection` / `installed_interrupts::form`, which call `form_call_selection` (`mpb:4691`, `mpb:4500`). `program_validation.rs:4138` runs `form_static_machine_selections` after static-argument/binding validators (`mpb:4125-4136`); errors block compilation.

**Input reaching branch:** A nominal static machine call where the argument for a `Machine` parameter resolves to the same symbol as that parameter’s binder (self-naming slot).

**Verdict REACHABLE:** Such a program should still prove the selected callable matches the contract, but the slot is skipped entirely.

## A0498

**Cited:** `machine_parameters_and_bindings.rs:4028-4033` (read).

**Enclosing function (read):** `installed_interrupts::form` (`mpb:3963-4077+`). When `application` or `registrar_target` is invalid (`mpb:4009`), it resolves lexical owner of the registrar application (`mpb:4010-4027`). If that owner machine has open parameters (`callable_body_has_open_parameters`, `mpb:4030`), it `return`s (`mpb:4032`) without pushing the error at `mpb:4034-4036`.

**Callers:** ripgrep `\binstalled_interrupts::form\(` under `omega-rust` → count **2** (`mpb:4500`, `mpb:4553` from `form_static_machine_selections`).

**Upward (read):** `form_static_machine_selections` during program validation formation (`program_validation.rs:4138`).

**Input reaching branch:** Interrupt-install call with incomplete/invalid registrar application while the owning machine’s body still has uninstantiated type parameters.

**Verdict REACHABLE:** Should require complete registrar application; instead validation exits silently.

## A0499

**Cited:** `machine_parameters_and_bindings.rs:7679`, `mpb:7923` (read).

**Enclosing function (read):** `validate_static_machine_bindings` (`mpb:7560-7936`). Per authored application in the loop from `mpb:7665`: if `bindings.is_empty() && lifetimes.is_empty()` → `continue` (`mpb:7679-7680`), skipping trait-tuple resolution, lifetime validation, and `validate_conformance_bindings`. Later, `complete` is computed (`mpb:7898-7910`); `validate_conformance_bindings` runs only `if complete` (`mpb:7923-7932`).

**Callers:** ripgrep `\bvalidate_static_machine_bindings\b` under `omega-rust` → **3** non-definition refs: `program_validation.rs:4131`, `preparation.rs:2098`, `immutable_checking_input.rs:97` (string label).

**Upward (read):** `program_validation.rs:4131` in formation prologue; errors from `finish_diagnostics` (`mpb:7935`) reject the program only when diagnostics are pushed—skipped paths push none.

**Input reaching branch:** (1) Static machine application handle with no bindings and no lifetimes recorded. (2) Application with some bindings but `complete == false` (missing non-implementation/non-proposition parameters or conformance slots).

**Verdict REACHABLE:** Incomplete tuples should be conformance-checked or rejected; both gaps allow acceptance without `validate_conformance_bindings`.

## A0500

**Cited:** `machine_parameters_and_bindings.rs:7396-7401` (read).

**Enclosing function (read):** `is_declared_source_tuple` (`mpb:7302-7462`). Collects type/const operand references from bindings (`mpb:7387-7394`). If `require_declared_application_operands(&operands, owner)` is `Err` → `return true` (`mpb:7396-7400`) before binding walk and before `open` is finalized.

**Callers:** ripgrep `\bis_declared_source_tuple\(` under `omega-rust` → count **2** (definition + use at `mpb:7712` inside `validate_static_machine_bindings`).

**Upward (read):** In `validate_static_machine_bindings`, when trait requirement template cannot be closed (`closed_requirement_application` is `None`, `mpb:7709`), if `is_declared_source_tuple(...)` is true, `selected = Some(application.binding_declaration)` (`mpb:7712-7721`) and processing continues without the error path at `mpb:7734-7740`.

**Input reaching branch:** Retained requirement-tuple application whose type or const binding operands fail declared-application operand rules (`callable_applications.rs:537-542`, read signature only).

**Verdict REACHABLE:** Should not count as an exact declared source tuple; early `true` admits the binding.

## A0501

**Cited:** `machine_parameters_and_bindings.rs:6092-6094` (read); related early exit `mpb:6085-6087` (read, same function).

**Enclosing function (read):** `requirement_predicates::validate` (`mpb:5994-6105`). Builds `scope`, optionally returns on `closed_non_member` (`mpb:6079-6087`), then `signature = scope.as_ref().and_then(|scope| program.applied_callable_signature(scope, target))`; `let Some(signature) = signature else { return; }` (`mpb:6089-6094`) skips `validate_tuple_requirements` (`mpb:6095-6104`).

**Callers:** `requirement_predicates::validate` invoked from `validate_call` (`mpb:6504`); `validate_call` from `validate_authored_static_arguments` (`mpb:6222`, `mpb:6237`); that from `program_validation.rs:4125`.

**Upward (read):** Program validation prologue walks all expression/statement calls with static arguments; silent `return` leaves no diagnostic.

**Input reaching branch:** Requirement-tagged call with generic tuple requirements where applied trait scope exists but `applied_callable_signature` cannot be rebuilt for the executable target (or, separately, dynamic finite family member that is closed but not in the family—`mpb:6085-6087`).

**Verdict REACHABLE:** Tuple requirement predicates should run or fail closed; path accepts without `validate_tuple_requirements`.

## A0502

**Cited:** `machine_parameters_and_bindings.rs:6912-6947` (read).

**Enclosing function (read):** `validate_arguments` (`mpb:6726-7023`), per-ordinal loop (`mpb:6743+`). Block at `mpb:6912-6947`: when no const declaration, no forwarded type, and no spelling, it may error for type-only/value mismatches (`mpb:6922-6944`) or `continue` (`mpb:6946`).

**Callers:** `validate_arguments` call sites in `machine_parameters_and_bindings.rs` — ripgrep `\bvalidate_arguments\b` in that file → uses at `mpb:5823`, `mpb:6305`, `mpb:6536` plus definition `mpb:6726` (and `traits.rs` has a separate homonym, not read).

**Upward (read):** `validate_call` (`mpb:6536-6543`) pushes `Err` only when returned; `continue` inside `validate_arguments` yields `Ok(())` for the whole call. `validate_authored_static_arguments` → `program_validation.rs:4125`.

**Input reaching branch:** Extra static argument ordinal with no declaration/forward/spelling, not caught by the inner type-shaped or value-parameter errors (e.g. surplus argument beyond telescope, or unrecognized const/name shape).

**Verdict REACHABLE:** Should report extra or invalid static argument; `continue` accepts the call fragment.

## A0503

**Cited:** `operators.rs:84-86`, `ops:107-109` (read).

**Enclosing function (read):** `validate_named_operator_application` (`ops:67-129+`). Returns `Ok(None)` when operator has unsupported telescope kinds and `static_arguments.is_empty()` (`ops:76-86`), or when `closed_operator_application_for_operands` is `None` and `static_arguments.is_empty()` (`ops:102-109`).

**Callers:** ripgrep `\bvalidate_named_operator_application\(` under `omega-rust` → count **4** invocation sites: `operators.rs:294`, `calls.rs:3255`, `monomorphization.rs:4672`, plus definition `ops:67`.

**Upward (read):** In `calls.rs:3255-3292`, `Ok(_)` including `Ok(None)` falls through without error (`calls.rs:3291`); only `Err` pushes diagnostics. Statement path `operators.rs:294-312` likewise `Ok(_) => {}`.

**Input reaching branch:** Named operator use with lifetime/machine/proposition telescope and no explicit static arguments, or operand types too open to close the application, with empty `machine_arguments` on the call.

**Verdict REACHABLE:** Should reject or retain an obligation; `Ok(None)` is treated as success with no closed operator application recorded.

## A0504

**Cited:** `operators.rs:1753-1756` with `ops:1816-1817` (read).

**Enclosing function (read):** `select_scoped_candidates` (`ops:1728-1770`) returns `CheckedOperatorResolutionStatus::BuiltinFallback` when no domain-active candidates and no root candidates but `builtin` is true (`ops:1753-1756`). `builtin_meaning_exists` (`ops:1788-1805`) uses `type_is_primitive`, which for `TypeReferenceNode::Named` calls `PrimitiveType::from_name(name.as_str())` only (`ops:1816-1817`, read).

**Callers:** ripgrep `\bselect_scoped_candidates\(` under `omega-rust` → count **3** (all `operators.rs`: `ops:1460`, `ops:1556`, definition `ops:1728`).

**Upward (read):** Scoped resolution in operator validation sets `status` on `CheckedOperatorUseFact`; pending uses finalized via `select_pending_domain_operator_meanings` (`ops:1523+`).

**Undecided gap:** Area digest notes shadowing of primitive names by user `Named` types is “not traced” (`digests/area_14.md:273`, read). Without reading whether a user data type can appear as `Named` with a primitive spelling while `symbol` points at user data, cannot state a concrete accepting program.

**What would decide:** Whether typed trees emit user types as `Named` with primitive spellings and whether `type_is_primitive` must consult `symbol`/`SymbolKind::Data` instead of `from_name` alone.

**Verdict UNDECIDED.**

## What I did not read

- Full bodies of `machine_parameters_and_bindings.rs`, `operators.rs`, `program_validation.rs`, and `callable_applications.rs` beyond cited functions.
- `require_application_operand_bindings` implementation (only `require_declared_application_operands` signature at `callable_applications.rs:537-542`).
- `validate_conformance_bindings` body (`mpb` second site).
- Omega samples or corpus programs that witness each branch.
- `git show 457adf4a11f:` variants of cited lines (current checkout matched digest line numbers).

SLICE COMPLETE
