# Area 11 open entries

Read-only trace of the "open" entries in `.agents/atlas/compiler-2026-11-10/digests/area_11.md`. Citations were checked against the working tree, which matches commit `457adf4a11f` for these paths (no diff in `omega-rust/psi/compilation/proof`, `psi-compiler`, or `representations/checked-trees`). Lines below were read unless a sentence says inferred.

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | `adm/integer_rules.rs:686-701` | UNDECIDED | Non-literal cast transport is accepted by the certificate checker, and no production code builds `IntegerCastBound`. |
| 2 | `adm/integer_affine.rs:2945-2964` | CLOSED | Every accepted conclusion is validated against the value context before the rule runs. |
| 3 | `adm/integer_affine.rs:2953-2964` | CLOSED | The shift-count evidence is required by the bound mapper on the only acceptance path. |
| 4 | `adm/integer_affine.rs:2491-2535` | CLOSED | The searched axioms are the already-validated semantic-axiom prefix. |
| 5 | `adm/mathematical_core.rs:3664-3666` | UNDECIDED | A missing declaration is omitted from the assumption set; not every body is known to be type-checked first. |
| 6 | `adm/mathematical_typing.rs:1375-1378` | CLOSED | Callers that compare types pass a universe as the shared type, so the strict shortcut does not apply. |
| 7 | `prf/checker.rs:342-344` | REACHABLE | A later assignment with the same displayed spelling skips the range check on a different place. |
| 8 | `prf/checker.rs:374-399` and the other `Uncovered` arms | UNDECIDED | The fallback helpers return false when they have no observation; the guard narrowers were not read. |
| 9 | `prf/checker.rs:5427-5473` | REACHABLE | The first machine with a same-named field decides unsigned, and a negative signed field is then treated as at least zero. |
| 10 | `prf/checker.rs:6522` | UNDECIDED | These two obligation kinds are never diagnosed; in-program argument checks cover only some of them. |
| 11 | `prf/lib.rs:845` | REACHABLE | A call nested in an array, struct, atomic, or range is treated as absent, so an overwrite scan can skip a bad assignment. |
| 12–56 | see digest | not reached | Stopped after entry 11. |

Paths: `adm/` = `omega-rust/psi/compilation/proof/admission/src/`, `prf/` = `omega-rust/psi/compilation/proof/src/`.

## 1. Cast bound, non-literal other side

Verdict: UNDECIDED.

Read `check_integer_cast_bound_conversion` in full (`integer_rules.rs:664-742`). The branch at `686-701` returns `Ok(())` when both propositions are integer-math orders of the same strictness, the moving sides are `MathValue` embeddings of the chain root and target (`669-680`), and the other sides are structurally equal. The other side is not required to be a literal. The digest's description matches.

Search `check_integer_cast_bound_conversion(` in `omega-rust`: 2 production hits plus tests. The production caller is `cast_bound_relation` (`proof.rs:733`), read at `713-744`. Search `cast_bound_relation(`: 2 hits, `proof.rs:778` (`check_integer_cited_bound`, read `746-791`) and `bounded_denotation.rs:8476` (read `8471-8509`). Both run only after `check_integer_cast_chain_witness` (`integer_rules.rs:522-616`, read). That witness allows only `IntegerExactCast` and `IntegerWiden` edges (`557-590`). `exact_cast_value_to` (`semantic-vocabulary/src/proposition.rs:371-391`, read) and `widen_value_to` (`436-451`, read) keep the same integer when they denote.

After the checker returns `Ok`, `accept_certificate_with_machine_parameters` (`proof.rs:171-218`, read) asks the kernel. `cast_bound_evidence` (`bounded_denotation.rs:3165-3326`, read) either derives the step or returns `None`. On `None`, `rule_instance` (`8640-8652`, read) still returns a term. The derivation that does succeed uses `cast_identity_law` (`3092-3114`, read), an assumed law that the cast equals its operand for every integer, not a check that the operand lies in the target range.

Search `IntegerCastBound {` in `omega-rust`: constructors are tests in `integer_rules.rs`, `bounded_denotation.rs`, and `proof.rs`, plus match arms. `checker.rs` only counts the node (`5770`). No production builder was found.

What would decide it: either a non-test producer of this rule, or a proof that an exact-cast equality axiom is admitted only when the two integers are equal. The digest is right that the other side need not be a literal.

## 2. Affine recognisers without `validate`

Verdict: CLOSED.

Read `check_integer_affine_witness` from `2873` through `2964`, `identity_ring_target` (`2829-2857`), and `trivial_affine_form` (`2859-2871`). The two arms return a form with coefficient 1, offset 0, and no endpoint steps, and neither calls `context.validate`. The digest is right about that.

Search `check_integer_affine_witness(` shows the production callers are `affine_bound_relation` (`proof.rs:482`, read `475-497`) and `exact_definition_bound_relation` (`proof.rs:627`, read `580-661`). Both then call `map_integer_affine_bound` / `check_integer_affine_bound_conversion` (`integer_affine.rs:372-396` and `608-671`, read). Empty steps make the route `DirectAdd`, `DirectSubtract`, `DirectMultiply`, or `DirectShift` (`428-449`). Those mappers were read (`910-1144` and `1372-1510`). Each one copies every `Value` leaf into the conclusion through `direct_math_leaf` (`1310-1327`) or, for wrapping multiply, clones `form.target()` (`1124-1134`). A leaf that is not a value or a typed literal makes the mapper return an error.

`check_node` (`proof.rs:2107-2171`, read) calls `context.validate` on every node conclusion (`2121-2123`) before the local rule check, and schedules the affine `root_bound` (`2300-2303`). `validate` rejects an unknown value id (`proposition.rs:2099-2101` and `2169-2171`, read). `accept_certificate` also validates the goal, the assumptions, and every semantic axiom first (`proof.rs:181-193`, read). A value that is not in the context cannot appear in a conclusion that already passed that check, and these recognisers' mappers put every accepted value into that conclusion.

Truth-root does not accept these forms: empty steps leave `saw_total_image` false and `integer_affine_truth_bounds` returns `TruthRootWithoutTotalImage` (`2097-2098`, read).

## 3. Shift count ignored at recognition

Verdict: CLOSED.

The arm at `2951-2964` binds `count: _` and returns `trivial_affine_form`. The digest is right about the ignore. The same form's route is `DirectShift` (`447`). `map_direct_shift_left_bound` (`1372-1510`, read) reads `count` again (`1385-1389`). A count that is not a typed literal must have equality or bound evidence (`1409-1465`); a missing lower bound returns `DirectShiftCountLowerMissing` (`1457-1464`). The truth-root path is the error in entry 2. `exact_definition_bound_relation` only builds add and subtract witnesses (`proof.rs:621-626`), so it does not accept this shift form. Both production entries of the witness checker go through that mapper.

## 4. Bitwise sibling searched, not cited

Verdict: CLOSED.

Read `bounded_bitwise_sibling` (`2491-2535`) and its two call sites (`2383-2384`, `2422`). The only caller of `apply_definition` that passes the slice is the chain loop at `3012-3020`, which passes `semantic_axioms[..index]`. Search `bounded_bitwise_sibling(`: those two sites only.

`accept_certificate` validates every semantic axiom before `check_node` (`proof.rs:189-193`). The elaborator does the same (`bounded_denotation.rs:15303-15307`, read). The sibling therefore only reads axioms that already passed `context.validate`. It uses an earlier axiom that the witness did not cite. Those axioms are hypotheses of the whole check, so a bound taken from one of them is a bound the check was already given. The digest is right that the lookup is a search; the "does not validate" part is closed by the two entry validations.

## 5. Assumption closure skips a missing index

Verdict: UNDECIDED.

Read `closure_over` (`mathematical_core.rs:3652-3678`). A seed whose `signature.get` fails is dropped (`3664-3666`) and is not recorded as an assumption. Read `assumption_closure` (`3680-3687`) and `judgment_assumption_closure` (`3689-3700`).

Search `judgment_assumption_closure(` and `certificate_assumption_closure(` outside tests. Production uses read: `mathematical_declarations.rs:5270` (after `check_type` at `5252`), `:5334` (schemas stored only after `check_type` at `6353`), `:5388` (after `check_type` at `5375`), `:6333` (`source_constructor_dependencies`, before `check_type` at `6353`), and `mathematical_proof_actual.rs:640` (`check_grants`, after `verify_mathematical_certificate` at `602`). Lines `6130` and `9931` are inside tests (asserts and `unwrap`).

`infer_leaf_type` rejects an unknown constant (`mathematical_typing.rs:312-321`, read). The typing driver sends every non-compound node there (`2504`, read). `check_signature` type-checks every declaration body (`mathematical_core.rs:3593-3617`, read). The theorem path at `5252` calls `check_type` on the theorem, not `check_signature` on every declaration. A constant buried in a declaration body that is not unfolded would be walked by the closure and skipped, without the error at `317`.

What would decide it: whether every `Term::Constant` index the elaborator emits is inside `signature` before a closure is trusted, including indexes that occur only in declaration bodies on the path at `5252-5274`.

## 6. Strict conversion returns true

Verdict: CLOSED.

The header states the rule (`mathematical_typing.rs:7-8`, read). `Work::resume` for `SharedSort` (`1371-1378`, read) returns true when `inferred_sort` of the shared type is `Strict`, without comparing the two terms. `inferred_sort` (`220-235`, read) and `Sort::is_strict` (`mathematical_core.rs:4264-4266`, read) were read. `convertible_retaining` (`mathematical_core.rs:1302-1311`, read) is the wrapper the digest names.

Search `Query::ordinary` and `Operation::Convert` in `mathematical_typing.rs`. The judgment builds the shared type as `Term::Sort(sort)` at `3158`, `3240`, `3292`, `3468`, `3580`, and `3601` (each read). The sort of a `Sort` term is `Type`, not `Strict` (`282-286`), so the shortcut does not fire for those comparisons. The exception is identity endpoint comparison, which passes `elimination.ty` (`2667-2670`, read). Identity formation rejects a strict carrier before either side is checked (`3482-3486`, read), so a type-checked identity does not reach that comparison with a strict carrier.

The production caller of `convertible_retaining` is `source_domains_correspond` (`mathematical_declarations.rs:4426-4458`, read). Its shared type is `infer_type` of a Pi domain (`4447-4458`), which is a sort, not a term of a strict sort. Other `convertible(` hits from `9172` upward in that file, and `bounded_denotation.rs:12355`, sit in tests (`unwrap` / `assert`). The digest is right that the branch itself does not check inhabitation. The paths that reach it from a program do not hand it a strict shared type.

## 7. Overwritten assignment matched by display name

Verdict: REACHABLE.

Read `check_bounded_assignment` (`checker.rs:332-418`) and `assignment_is_overwritten_before_consumption` in full (`420-512`). Search of that function name: one caller, line `342`. When it returns true, the assignment returns `None` and skips named constraints and the range check.

The original assignment is matched by expression handle (`444-446`). A later assignment skips the check when `display_name` strings are equal (`472-473`). `display_name` for a name is the path spellings joined by `::` (`typed-trees/src/values.rs:438-439`, read), not the symbol. `written_place_path` is the same spellings (`checker.rs:218-238`, read).

A state assigns an out-of-range integer to a local, then later assigns to a different place with the same spelling, such as a second local of that name, without a call or an aliasing read in between. The later assignment makes line `472` return true, and the first assignment is accepted with no range check. The duplicate-local diagnostic at `validation/src/value_custody_core.rs:5741-5753` (read) was not found under `pipeline/02` or `pipeline/03` (search `duplicate local` / `same name` there: no hits). That validation pass was not traced into the default compiler, which does not re-verify stage output.

## 8. `Uncovered` falls back to interval helpers

Verdict: UNDECIDED.

Read the `Uncovered` arms at `373-398` (assignment), `547-577` (initializer), `623-642` (return), `689-715` (call; the diagnostic push continues just past `705`), and `762-808` (transition). Read `integer_range_for_value` (`4991-4999`), the range macros (`5002-5027`), `neutral_range` (`5501-5506`, full `i64` span), `occurrence_integer_expression_fits_range` (`validation/src/arithmetic_domains.rs:10555-10604`), and `arrival_integer_expression_bounds` (`10392-10417`).

`occurrence_integer_expression_fits_range` returns false when there is no observation (`10570-10571`). `arrival_integer_expression_bounds` returns `None` when the statement is not the expected transition or expression (`10401-10408`). The transition arm then diagnoses (`800-806`). The interval side accepts when the value's declared constraint range fits the target (`374-380` via `4996-4998`). That part matches the digest's "stated" note at `4991-4998`.

Not read: `live_integer_constraints::proves_range` (called at `10595`), `expression_observation`, `apply_handle_guard`, and `guard_refined_binary_range`. A guard that narrows a range too far would accept on this path. What would decide it: those four functions, in particular whether a refinement can raise a lower bound or drop an upper bound without a proved condition.

## 9. Unsigned field taken from the first machine

Verdict: REACHABLE.

Read `operand_is_unsigned` and `operand_declared_primitive` (`5427-5474`). Search `operand_is_unsigned(`: two callers, `5221` and `5335`, both read (`5207-5228` and `5327-5338`). The obligation is discarded (`5432`). For `self.field`, the function walks `program.machines()` in order and returns the primitive of the first attached data type that has a field of that spelling (`5459-5472`). It does not use the current machine.

Both callers, when that result is unsigned, raise a negative lower bound to zero (`5220-5223`, `5334-5337`). The resulting range is what `guarded_integer_range_for_assignment_with_context` returns (`5190-5305`, read).

One machine, defined first, has an unsigned field. A later machine has a signed field with the same spelling. An assignment in the later machine uses that field as an operand of a checked computation. The checker treats the signed field as unsigned and lifts its lower bound to zero, so a negative value is accepted as fitting a non-negative target. The digest's description matches.

## 10. `BoundedValue` and `GuardedTransition` do nothing

Verdict: UNDECIDED.

Read the dispatch (`checker.rs:6486-6532`). Those two kinds match an empty arm (`6522`). `record_obligation` only increments `decided_elsewhere` (`5615-5617`, read). There is no comment at `5576`; that line is the field name. The digest's "measurement comment" is the counter, not a reason.

Read the collectors (`obligations.rs:42-168` and `275-289`, and `547-626`). `BoundedValue` is pushed for owned data, state parameters, and return types. Returns also get `BoundedStateReturn`, and initializers get `BoundedInitializer`; those are checked at `6525-6530`. A transition `When` guard pushes `GuardedTransition` (`156-168`) and, separately, `BoundedTransitionArgument` for each constrained parameter (`587-625`, read).

Search `ProofObligationKind::BoundedValue`, `ProofFactKind::BoundedValue`, and the `GuardedTransition` twins in `omega-rust`: the only production hits are the mapping in `pipeline/05_typed-trees-to-checked-trees/src/published.rs:171` and `:175`, and the fact builder at `4323-4349` (read). That builder stores a fact. It does not diagnose.

Not read: `collect_bounded_call_argument_obligations` (`809`). Not determined: whether a state parameter that is never supplied by an in-program transition or call, such as an entry parameter with a range constraint, has any other check. What would decide it: that call collector, and whether entry parameters must be proved.

## 11. Call search returns false for unlisted nodes

Verdict: REACHABLE.

Read `expression_contains_call_node` (`lib.rs:817-847`) and `ExpressionNode` (`typed-trees/src/values.rs:4199-4221`). The match walks `Match`, `Call`, `Binary`, `Unary`, `Cast`, `Borrow`, `Indexed`, and `Member`. `_ => false` (`845`) covers `ArrayLiteral`, `Atomic`, `Range`, `StructLiteral`, and `SavedSubject`, all of which can hold child expressions, plus leaves. The digest is right that unlisted kinds return false. The variants above are the ones that can hide a call.

Search `expression_contains_call_node(` outside the function: `obligations.rs:573` and `:1057`, `checker.rs:457`, `:481`, `:495`, and `lib.rs:706`. Read the checker three (inside `420-512`) and `dehoisted_initializer` (`695-712`). Read the use at `obligations.rs:1057-1068`: a hidden call keeps the transition guard instead of forcing `Always`. Read `594-619`: the same flag keeps the guard on a transition argument.

`collect_read_place_paths` (`checker.rs:241-277`, read) has the same hole: `_ => {}` does not enter an array or a struct.

A state assigns an out-of-range integer to a local, then assigns to that same local again with a right-hand side that is an array element or a struct field containing a call that reads the local. The call is invisible to `expression_contains_call_node`, the read is invisible to `collect_read_place_paths`, and the matching display name makes entry 7 return true. The first assignment is accepted with no range check while the call still observes the out-of-range value.

## Counts

- REACHABLE: 3 (entries 7, 9, 11)
- CLOSED: 4 (entries 2, 3, 4, 6)
- UNDECIDED: 4 (entries 1, 5, 8, 10)
- Not reached: 45 (entries 12 through 56)

## What I did not read

Entries 12 through 56 were not opened. Inside the entries above, these were not read: `judgments::drive` beyond the convert sites, the constant fallback, and identity formation; `proves_range`, `expression_observation`, `apply_handle_guard`, and `guard_refined_binary_range`; `collect_bounded_call_argument_obligations`; the compiler driver that would show whether `value_custody_core.rs` duplicate-local rejection runs on a normal compile; test bodies except where a line was identified as a test. No search was treated as a substitute for a function that was not opened.
