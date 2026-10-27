# Nested machine invocation

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../TASKS.md).

Fact: a nested machine returns its explicitly supplied value under the enclosing
static type. The call must retain the exact enclosing type binder without
capturing the parent's runtime argument or inheriting its entry facts.

The [generic-context witness](generic_context/main.omg) exercises three-level
nesting with enclosing and independently declared static type parameters.
Baseline `02e7d90a49aee` refuses this shape in resolution with `nested machines
retaining an enclosing static telescope are not yet supported`. After removing
that refusal, the unchanged witness exposed the missing specialization context.
The repair lifts the original static binder identities into ordinary generic
declarations and operands; it adds no runtime environment.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [source](local/main.omg) | [wrong result](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [wrong result](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [wrong result](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [wrong result](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [wrong result](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [wrong result](const_name/invalid/main.omg) |
| Nested machine | [source](other_state/main.omg) | [wrong result](other_state/invalid/main.omg) |
| Other machine | [source](other_machine/main.omg) | [wrong result](other_machine/invalid/main.omg) |
| Target body | [source](target_body/main.omg) | [wrong result](target_body/invalid/main.omg) |

Every positive returns its authored exit 0 in Terminal Psi and Windows x86-64
native execution. Every twin changes only the nested helper's result from its
supplied `T` to `bool` and rejects at that result. Copies and oracles are pinned
under `tests/omega/{pass,fail}/facts/nested_machine_invocation/`.

Additional witnesses exercise [static constants](const_context/main.omg),
[explicit borrowed arguments](borrow_context/main.omg),
[shadowed static names and a bare result before declarations](shadowing/main.omg),
[static callable parameters](callable_context/main.omg), and
[a nested helper selected as a callable](selected_helper/main.omg).
[Runtime capture](capture_rejected/main.omg) rejects an unsupplied parent value.
[Ancestor cycles](ancestor_cycle/main.omg) execute 100,000 terminal cycles while
retaining a caller-owned mutable borrow, then return to the ordinary caller.

The row establishes static binder identity, explicit argument composition,
result checking, continuation, and end-to-end erasure/realization for these
calls. All four hosted target bindings are authored; executable observations
are Windows x86-64, not runs on the other three hosts. The corpus long-cycle
witness supplies ten million units of interpreter sponsor fuel.

The parent outcome remains open: complete call-contract/custody acceptance,
joint ranking and physical stack-bound evidence, product-compiler support,
and migration/removal of all retired source-state paths are not established
by this row. Existing unrelated baseline corpus regressions are not reclassified.
