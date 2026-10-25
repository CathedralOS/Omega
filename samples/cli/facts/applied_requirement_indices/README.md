# Applied requirement indices

An explicitly applied boundary requirement preserves its complete type-index
tuple through helper arguments and results: its outer trait telescope and its
member telescope remain independent, and two calls to the same member do not
share result facts merely because they share a declaration.

Every place copies the sentence `Applied requirements preserve exact type-indexed contracts.`
and executes the same four applied requirement paths (`Identity<Word>::identity`,
`Identity<Word>::stated`, `Identity<Word>::compound`, and `Identity<Word>::paired`
instantiated with `Other` and `Word`), exiting `0` only when all four runtime
payloads (`29`, `37`, `43`, and `47`) survive intact. Only the transport place
of the type-indexed contract changes.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness forwarding applied requirement type-index tuples |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the applied requirement result is stored in a typed local (`Payload in Tagged<Value>` vs. `Payload in Tagged<Alt>`) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the type-indexed payload is projected from a generic record field (`Holder<Value, Alt>::value`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the type-indexed payload arrives as an explicit machine parameter (`value: Payload in Tagged<Value>` vs. `Tagged<Alt>`) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a nested applied requirement call result feeds an outer applied requirement call (`paired<Inner>` vs. `paired<Alt>`) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a `match` expression join into a type-indexed local (`let joined: Payload in Tagged<Value>` vs. `Tagged<Alt>`) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named `const` (`const INITIAL: Payload in Tagged<Word>` vs. `Tagged<Other>`) supplies the initial indexed payload |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the indexed payload into `machine selected(value: Payload in Tagged<Value>)` vs. `Tagged<Alt>` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate `relay` machine forwards the applied requirement result (`-> Payload in Tagged<Value>` vs. `Tagged<Alt>`) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target-qualified `forward` bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) forwards the indexed contract |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/applied_requirement_indices/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/local/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/field/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/parameter/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/call_result/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/match_join/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/const_name/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/other_state/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/other_machine/main.omg
omega run --both samples/cli/facts/applied_requirement_indices/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/applied_requirement_indices
```

### The oracle discriminates

The exit code `0` is strictly discriminative. Perturbing a single payload token in `main` (`let value: Payload in Tagged<Word> = Payload { value: 29 };` to `30`) preserves all static type-indexed domain contracts (`Tagged<Word>`) so the program still compiles cleanly, but changes `from_state.value` at runtime from `29` to `30`. The guard `from_state.value == 29 && from_compound.value == 37 && from_pair.value == 43 && from_other_pair.value == 47` then takes the `false -> bad()` branch and executes `crash Trap;`, causing both native and Terminal Psi execution to abort non-zero (`native signal 5` and `interp crash: Trap` in agreement).

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["index compatibility condition"]`). In each twin, `<place>/main.omg` and `<place>/invalid/main.omg` differ by a single type-index token on that place's construct (`Value` -> `Alt`, `Inner` -> `Alt`, or `Word` -> `Other`), breaking exact normalized index compatibility while keeping caller guards and control flow identical:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `index compatibility condition` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `index compatibility condition` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `index compatibility condition` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `index compatibility condition` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `index compatibility condition` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `index compatibility condition` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `index compatibility condition` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `index compatibility condition` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `index compatibility condition` |

## Pinned Fixtures

The nine-place row is pinned under `tests/omega/pass/facts/applied_requirement_indices_through_<place>_exit` and `tests/omega/fail/facts/applied_requirement_indices_through_<place>_rejected`:

- Pass fixtures (`expected_native_exit.txt` = `0`):
  - `tests/omega/pass/facts/applied_requirement_indices_through_local_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_field_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_parameter_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_call_result_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_match_join_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_const_name_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_other_state_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_other_machine_exit`
  - `tests/omega/pass/facts/applied_requirement_indices_through_target_body_exit`
- Fail fixtures (`expected.txt` = `index compatibility condition`):
  - `tests/omega/fail/facts/applied_requirement_indices_through_local_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_field_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_parameter_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_call_result_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_match_join_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_const_name_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_other_state_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_other_machine_rejected`
  - `tests/omega/fail/facts/applied_requirement_indices_through_target_body_rejected`

Adjacent provider fixtures (`tests/omega/pass/providers/applied_type_index_forwarding_exit`, `tests/omega/pass/providers/applied_routed_index_forwarding_exit`, `tests/omega/pass/providers/applied_requirement_computed_const_exit`, and their fail controls `applied_type_index_wrong_actual`, `applied_type_index_wrong_field`, `applied_type_index_nested_wrong_actual`, `applied_type_index_wrong_result`, and `applied_routed_parameter_mint`) cover compound indices, computed const index expressions, and issuer-routed domain qualifications.

## Compiler Repairs Landed

Completing the nine-place row exposed and fixed three checking gaps in `omega-rust/`:

1. **Vacuous indexed domains on `@const-qualification-*` probe returns (`domain_implications.rs`):** Removed the duplicate `typed_trees::domain::has_symbol_only_proof(program, domain)` pre-filter before calling `value_satisfies_domain_predicates_in_contexts` on `TransitionTargetNode::Value` exits so `const` initializers qualified with predicate-free indexed marker domains (`const INITIAL: Payload in Tagged<Word> = Payload { value: 29 };`) discharge their vacuous conjunction (`proof_facts(domain).is_empty()`) cleanly.
2. **Declared type reference preservation on substituted `const` expressions (`member_resolution.rs`):** Updated `expression_type_position` to return `MemberPosition::Reference(declaration.declared_type)` when an expression originates from a `const` declaration (`program.expression_const_declaration(expression)`), matching `result_type.rs` so `expression_type_reference_in_state` and `index_compatibility` retain the `const` binding's declared domain-constrained type reference rather than falling back to the bare `StructLiteral` carrier.
3. **Match-arm recursion in `append_expression_compatibilities` (`index_compatibility.rs`):** Added `ExpressionNode::Match(dispatch)` recursion in `append_expression_compatibilities` so `match` join expressions check and record `IndexCompatibilityFact` conditions for every result arm against the destination `target_type`.

## Requirements and Evidence

- **Parent outcome:** Cathedral backing-lifecycle prerequisite (`TASKS.md`) — applied boundary requirement contracts preserve exact type-index tuples across all 9 transport places.

### Specification and Lowering Layers

1. **Establishment:** Type-indexed domain memberships (`Payload in Tagged<Value>`, `Payload in Tagged<Box<Value>>`, `Payload in Pair<Outer, Inner>`) are established by explicit qualification on predicate-free domain families (`let value: Payload in Tagged<Word> = Payload { value: 29 };`, `const INITIAL: Payload in Tagged<Word> = ...`) and by applied boundary requirement return types and `ensures result in Tagged<Value>` postconditions ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md), [dependent values](../../../../wiki/spec/language/dependent_values.md)). No post-hoc caller equality guard establishes any static index fact; the final `transition` checks runtime `u64` payload values only after all static requirement calls have completed.
2. **Identity and lifetime:** Each value's normalized domain instance (`Tagged<Value>`, `Tagged<Box<Word>>`, `Pair<Word, Other>`, `Pair<Word, Word>`) remains attached to its exact storage or value version across local bindings, generic record fields (`Holder<Value>`), parameters, nested call results, `match` arm joins, `const` initializers, state transitions (`selected`), cross-machine forwarding (`Relay::relay`), and target-qualified machine bodies ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md), [content custody](../../../../wiki/spec/resources/content_custody.md)). Two distinct calls to `Identity<Outer>::paired<Inner>` in the same expression maintain distinct invocation namespaces without conflating result facts.
3. **Composition:** Applied requirement calls substitute both the outer trait telescope (`Identity<Outer>`) and the member telescope (`paired<Inner>`) into parameter types, return types, `requires`, and `ensures` clauses ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)). For each place, `<place>/main.omg` and `<place>/invalid/main.omg` differ by a single type-index token on that place's construct (`Value` -> `Alt`, `Inner` -> `Alt`, or `Word` -> `Other`).
4. **Soundness boundaries:** Every `<place>/invalid/main.omg` twin fails closed during compile-time checking with `error: index compatibility condition ... is not established: actual ... and expected ... are distinct normalized instances` when an unproved or mismatched type index is transported into a local, field, parameter, nested call argument, match join, const initializer, state parameter, machine return type, or target body ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md), [quotients](../../../../wiki/spec/proofs/quotients.md), [inductive profile](../../../../wiki/spec/proofs/inductive_profile.md), [termination](../../../../wiki/spec/language/termination.md), [authority](../../../../wiki/spec/resources/authority.md)).
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Checked trees lower applied boundary requirement calls, specialized generic helpers, state transitions, and match dispatches into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing static domain index tags after discharging `IndexCompatibilityFacts` (`ClosedEvaluation` and `LicensedNormalization`) while retaining boundary-application demands and target-scoped body candidates ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md), [ownership](../../../../wiki/spec/terminal-psi/ownership.md), [loans](../../../../wiki/spec/terminal-psi/loans.md), [boundary calls](../../../../wiki/spec/terminal-psi/boundary_calls.md), [observations](../../../../wiki/spec/terminal-psi/observations.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows (`ProgramEntry` for all four hosted targets, `accept_boundary<Identity>`, `select_provider<Identity, Provider>`) join each closed boundary requirement (`Identity<Word>::identity`, `compound`, `paired<Other>`, `paired<Word>`, `stated`) to `Provider`'s concrete specializations without reopening source, evaluate `Payload` and `Holder` layouts, and lower calls into validated `CallPlan` and `StatePlan` placements ([provider selection](../../../../wiki/spec/build/provider_selection.md), [assumed binding contracts](../../../../wiki/spec/build/assumed_binding_contracts.md), [foreign bindings](../../../../wiki/spec/build/foreign_bindings.md), [private callbacks](../../../../wiki/spec/build/private_callbacks.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [recasts](../../../../wiki/spec/layouts/recasts.md), [codecs](../../../../wiki/spec/layouts/codecs.md), [calling plans](../../../../wiki/spec/build/calling_plans.md), [byte views](../../../../wiki/spec/terminal-psi/byte_views.md)).
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for 64-bit payload transport, equality comparisons, and conditional branches across `forward`, `field`, `forward_pair`, `later`, `Relay::relay`, and `Provider` states, assigns register homes and spill slots, and composes bounded worst-case stack usage (WCSU) ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [checked assembly](../../../../wiki/spec/language/assembly.md)).
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves stack displacements and basic-block branch offsets, validates machine-state footprint evidence, and emits native executables that exit `0` and agree with Terminal Psi (`omega run --both`) across the root witness and all nine transport places ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)).
