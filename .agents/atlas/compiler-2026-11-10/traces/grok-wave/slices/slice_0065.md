slice_0065

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0497 | mpb:4941-4943 | REACHABLE | Machine static arg whose symbol equals the Machine binder skips nominal/shape checks and records no selection. |
| A0498 | mpb:4028-4033 | REACHABLE | Invalid/missing registrar on interrupt install is ignored without diagnostic when lexical owner has open parameters. |
| A0499 | mpb:7679; mpb:7923 | REACHABLE | Empty or incomplete binding tuples skip retention/conformance validation that runs only on complete tuples. |
| A0500 | mpb:7396-7401 | REACHABLE | Undeclared application operands make `is_declared_source_tuple` return true and can excuse retained-target mismatch. |
| A0501 | mpb:6092-6094 | REACHABLE | Missing rebuilt callable signature ends `requirement_predicates::validate` without tuple-requirement diagnostics. |
| A0502 | mpb:6912-6947 | REACHABLE | Extra or unclassified static args can `continue` past the extra-argument error at mpb:6948-6953. |
| A0503 | ops:84-86; ops:107-109 | CLOSED | `Ok(None)` is swallowed only where later call validation still runs; nonempty unsupported telescope still errors. |
| A0504 | ops:1753-1756; ops:1816 | UNDECIDED | `BuiltinFallback` keys off `Named` spellings via `PrimitiveType::from_name`, not whether a user type shadows that name. |

### A0497

1. Read `form_call_selection` in full at mpb:4716–5055 (read); read `machine_selection_arguments` / `is_machine_selection_symbol` at mpb:4623–4640 (read).
2. Search `form_call_selection\(` under `omega-rust/`: **4 hits** (all in `machine_parameters_and_bindings.rs`: definition plus call sites at mpb:1290, 4061, 4691).
3. Call chain to program accept/reject (read): `program_validation::validate` mpb:4138 → `form_static_machine_selections` mpb:4453 → `validate_call_selection` mpb:4659 → `form_call_selection` mpb:4691; formation errors abort at mpb:4139–4143.
4. Input on accepting branch (read): a machine static argument drawn from `machine_selection_arguments` (State or MachineParameter symbol) where `selected.symbol == parameter.symbol` for the zipped machine requirement binder at mpb:4919–4942.
5. **Verdict: REACHABLE** — A generic registration call can supply the machine-parameter binder symbol itself as the static machine argument; the loop `continue`s before `validate_nominal_machine_selection` (mpb:4951) and `validate_selected_callable_shape` (mpb:4968), so a non-machine self-reference is not rejected at this site.
6. Unsure: whether a later obligation always requires a `ValidatedRequirementCallMachineSelection` row for every machine slot (did not read all effect-admission consumers).

### A0498

1. Read `installed_interrupts::form` in full at mpb:3963–4096 (read); did not read `registrar_identities` body beyond its call at mpb:4053.
2. Search `installed_interrupts::form` / module references: callers at mpb:4500 and mpb:4553 inside `form_static_machine_selections` (read).
3. Call chain (read): same formation path as A0497; silent `return` at mpb:4032 emits no diagnostic; closed owners get mpb:4034–4036.
4. Input (read): `!application.is_valid() || !request.registrar_target.is_valid()` at mpb:4009, registrar lexical owner from static application at mpb:4022–4027, and `program.callable_body_has_open_parameters(machine)` true at mpb:4030–4031 (`declaration_tables_and_index.rs` mpb:2503–2512, read definition only).
5. **Verdict: REACHABLE** — Interrupt installation with an incomplete registrar application on a caller whose body still has open specialization parameters returns without the “complete registrar application” error that closed owners receive (mpb:4034–4036).
6. Unsure: whether lowering or another validator always rejects programs that retain `installed_interrupt_operation` without a formed `PreparedInstalledInterruptRegistration` (only read that early return skips mpb:4079–4095).

### A0499

1. Read `validate_static_machine_bindings` loop at mpb:7560–7936 (read); read `complete` predicate mpb:7898–7910 and `validate_conformance_bindings` header mpb:8055–8063 (read).
2. Call chain (read): `program_validation::validate` → `static_bindings::validate_static_machine_bindings` mpb:4131–4134; `finish_diagnostics` mpb:7935.
3. Accepting branches (read): mpb:7679–7680 continues when both bindings and lifetimes are empty (skips mpb:7682–7933 block); mpb:7923–7932 runs `validate_conformance_bindings` only when `complete` is true.
4. Input (read): static machine applications with missing binders (failing `complete` at mpb:7898–7910) or wholly empty binding/lifetime rows at mpb:7679.
5. **Verdict: REACHABLE** — A partial specialization tuple can pass type/recipe checks yet never receive conformance-binding validation guarded by `if complete` at mpb:7923.
6. Unsure: whether `forwarded_evidence::validate` at mpb:7915–7922 alone is sufficient for all incomplete-tuple failure modes (read header at mpb:7145 only).

### A0500

1. Read `is_declared_source_tuple` in full at mpb:7302–7462 (read); read caller loop at mpb:7709–7724 (read).
2. Search `is_declared_source_tuple`: **2 hits** (definition and sole call at mpb:7712).
3. Call chain (read): inside `validate_static_machine_bindings` retained-target reconciliation mpb:7688–7740; failure would push mpb:7735–7739 unless `is_declared_source_tuple` succeeds.
4. Input on accepting branch (read): type/const binding operands collected mpb:7387–7394; `program.require_declared_application_operands(&operands, owner).is_err()` at mpb:7396–7400 forces `return true` before further operand closure checks mpb:7402–7404.
5. **Verdict: REACHABLE** — Open or not-yet-declared application operands classify the tuple as a “declared source,” allowing retained-target reconciliation to proceed without the mpb:7735–7739 error.
6. Unsure: full behavior of `require_application_operand_bindings` with `allow_declared_binders: true` in `callable_applications.rs` mpb:537–543 (read signature only, not full walk).

### A0501

1. Read `requirement_predicates::validate` in full at mpb:5994–6105 (read); did not read every helper in `source_requirement` / dynamic selector paths beyond mpb:6032–6070.
2. Search `requirement_predicates::validate(`: **1 hit** (mpb:6504 from static argument validation, read call site mpb:6470–6504).
3. Call chain (read): `validate_authored_static_arguments` mpb:6195 → `validate_call` mpb:6222 → `requirement_predicates::validate` mpb:6504; prologue errors can abort program validation before bodies (mpb:4089+).
4. Input (read): call with generic tuple requirements and resolved `scope`, but `program.applied_callable_signature(scope, target)` is `None` at mpb:6089–6093 → early `return` without `validate_tuple_requirements` mpb:6095–6104.
5. **Verdict: REACHABLE** — A requirement call with generic tuple predicates can reach the validator and exit silently when the applied signature cannot be rebuilt, even though mpb:6107+ would refute or require-proof on a complete signature.
6. Unsure: whether `closed_non_member` return mpb:6085–6087 is a separate accepting path for the same entry (also silent); not fully traced to an alternate diagnostic.

### A0502

1. Read `validate_arguments` in full at mpb:6726–6970+ (read through the `continue` at mpb:6946); callers at mpb:5823, 6305, 6536 read at call sites only.
2. Search `validate_arguments(` in validation crate: **mpb:6726** definition plus three internal call sites (read grep via file).
3. Call chain (read): static machine/product/closed-requirement telescopes; errors become diagnostics in callers (e.g. mpb:6312).
4. Input on accepting branch (read): at ordinal `parameter == None` with no const declaration, forwarded type, or spelling match, and failing to hit error branches mpb:6922–6944 → `continue` mpb:6946 skips mpb:6948–6953 extra-argument error.
5. **Verdict: REACHABLE** — Supernumerary static arguments whose shape does not match the earlier error patterns are dropped by `continue` instead of rejected as extra arguments.
6. Unsure: whether any caller always sizes `arguments` to `parameters.len()` so the branch is dead (did not read all argument producers).

### A0503

1. Read `validate_named_operator_application` in full at ops:67–125 (read).
2. Search `validate_named_operator_application(`: **4 matches in 3 files** (definition ops:67; ops:294; `calls.rs:3255`; `monomorphization.rs:4672`).
3. Call chain (read): expression calls — `calls.rs:3255` match arm `Ok(_) => {}` at mpb:3291 then `validate_call_arguments` mpb:3294; statement path ops:294–312 similar pattern (read).
4. Input (read): `Ok(None)` when `static_arguments.is_empty()` and unsupported lifetime/machine/proposition telescope ops:76–86, or when `closed_operator_application_for_operands` is `None` with empty static args ops:106–109.
5. **Verdict: CLOSED** — Empty static args with open operand typing return no closed application but callers still run full call-operand validation (`calls.rs:3294`, read); nonempty unsupported telescope returns `Err` ops:87–89 (read).
6. Unsure: monomorphization-only path at `monomorphization.rs:4672` (not read in full).

### A0504

1. Read `select_scoped_candidates` at ops:1728–1769 (read), `builtin_meaning_exists` ops:1788–1805 (read), `type_is_primitive` ops:1807–1826 (read); read `finalize_pending_use` caller ops:1547–1560 (read).
2. Search `select_scoped_candidates(`: **3 hits** in `operators.rs` (definition plus ops:1460, ops:1556).
3. Call chain (read): operator fact formation → `BuiltinFallback` status → later checked-tree operator passes treat builtin fallback as builtin meaning (`checks_operators.rs:2762`, read one line).
4. Input (read): no admissible domain-rooted candidates, `builtin` true from operand builtin spelling/carrier checks ops:1436–1444 / ops:1559, and `[] if builtin => BuiltinFallback` ops:1753–1756; primitive test uses `TypeReferenceNode::Named { name, .. }` + `PrimitiveType::from_name(name.as_str())` ops:1816–1817 without inspecting defining symbol.
5. **Verdict: UNDECIDED** — Whether a user-defined `Named` type can carry a primitive spelling while lacking a domain operator candidate (shadowing) was not traced through name resolution (`PrimitiveType::from_name` definition not read).
6. Entry description matches code at cited lines (read).

### What I did not read

- Full bodies of `validate_nominal_machine_selection`, `validate_selected_callable_shape`, `require_application_operand_bindings`, and `PrimitiveType::from_name`.
- Complete `admit_static_machine_effects` consumer graph beyond mpb:4569–4607.
- Pipeline lowering paths for interrupts without `PreparedInstalledInterruptRegistration`.
- Entire `validate_call_arguments` in `calls.rs` (only entry at mpb:3294).
- Monomorphization caller of `validate_named_operator_application`.

SLICE COMPLETE
