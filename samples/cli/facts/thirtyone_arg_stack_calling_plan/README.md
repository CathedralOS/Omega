# 31-Argument Calling Plan Across Register and Stack Slots

## Fact Sentence
Fact: a machine transition call passing thirty-one u64 arguments across register and stack ABI slots preserves the thirty-first stack-passed argument's static bound 40u64 <= a31 && a31 <= 80u64 across all nine places so a31 - 40u64 and 80u64 - a31 are proved safe without underflow.

## Specification and Lowering Layers

1. **Establishment**:
   The transition machine `pick31(a1..a31)` receives thirty-one `u64` arguments. Arguments `a1..a30` are exact integers `1..30`, and the thirty-first argument is bounded `40u64 <= a31 && a31 <= 80u64`. Across register-to-stack parameter transitions, static bounds are established and verified at entry contracts.

2. **Identity and Lifetime**:
   Each argument value has unique scalar storage identity across register homes and caller-allocated activation stack frame homes. The 31st argument's interval is preserved across value lifetimes without storage aliasing or clobbering.

3. **Composition**:
   The caller passes evaluated expressions into `pick31`, and the resulting value satisfies `40u64 <= res && res <= 80u64`. In all nine places, this bound directly justifies exact subtractions `80u64 - res` and `res - 40u64` without underflow.

4. **Soundness Boundaries**:
   Proof rules ensure that integer range assertions are checked before subtraction operations occur. Widening `a31 <= 81u64` in the invalid twins causes `80u64 - res` to fail closed at compile time with `may overflow \`u64\``.

5. **Checked-to-Terminal Psi Lowering (Psi 05–07)**:
   The high-level Omega calling expression lowers into canonical Terminal Psi (`PSITERM\0`) call signatures carrying 31 typed argument lanes, correctly preserving the static bound intervals into integer range constraints without losing argument indices or types.

6. **Provider Settlement, Layout Geometry, and ABI Calling Plans (Omega 00–02)**:
   Calling plans evaluate target ABI layout geometries for all four supported targets:
   - `windows_x86_64`: args 1..4 in registers (`RCX, RDX, R8, R9`), args 5..31 in stack slots (27 stack arguments).
   - `linux_x86_64`: args 1..6 in registers (`RDI, RSI, RDX, RCX, R8, R9`), args 7..31 in stack slots (25 stack arguments).
   - `macos_arm64` / `linux_arm64`: args 1..8 in registers (`X0..X7`), args 9..31 in stack slots (23 stack arguments).
   In all four hosted targets, the 31st argument `a31` is guaranteed to be homed in a caller-allocated activation frame stack slot.

7. **Instruction Selection, Register Homes, and Activation Frame Realization (Omega 03–06)**:
   The backend emits instructions placing arguments 1..k in target registers and storing arguments (k+1)..31 into stack frame slots at negative/positive offsets relative to the stack frame pointer, updating worst-case stack usage (WCSU) bounds.

8. **Form Encoding, Branch Relaxation, Relocations, and Native Image Emission (Omega 07–09 + emission)**:
   Final image emission lays out stack frame setup/teardown epilogues, branch relaxation, relocations, and machine code bytes matching native target execution with Terminal Psi interpretation (`omega run --both` exit code 0).

## Nine Places Table

| Place | Directory | Purpose | Expected Exit | Rejection Twin |
|---|---|---|---|---|
| local | `local/` | Arguments held in local variables before invocation | 0 | `may overflow \`u64\`` |
| field | `field/` | Result stored in struct field before exact subtraction | 0 | `may overflow \`u64\`` |
| parameter | `parameter/` | Result validated after passing as parameter to consumer | 0 | `may overflow \`u64\`` |
| call_result | `call_result/` | Arguments returned from individual helper calls | 0 | `may overflow \`u64\`` |
| match_join | `match_join/` | Call invoked across boolean match arms and joined | 0 | `may overflow \`u64\`` |
| const_name | `const_name/` | Arguments passed as top-level constants | 0 | `may overflow \`u64\`` |
| other_state | `other_state/` | Subtractions evaluated in another machine state reached via transition | 0 | `may overflow \`u64\`` |
| other_machine | `other_machine/` | Bound validated inside a secondary machine | 0 | `may overflow \`u64\`` |
| target_body | `target_body/` | Explicit target-scoped bodies for all 4 hosted targets | 0 | `may overflow \`u64\`` |

## Owning Item

Owning task: `CALLING-PLANS` / `STORAGE` / `ACTIVATION-STORAGE` ([TASKS.md](../../../../TASKS.md)), governing specifications at [wiki/spec/build/calling_plans.md](../../../../wiki/spec/build/calling_plans.md) and [wiki/spec/resources/activation_storage.md](../../../../wiki/spec/resources/activation_storage.md). The requirement this row establishes is that the thirty-first argument's static interval bound survives argument evaluation, register/stack placement on all four hosted ABIs and the callee's own entry contract, so an exact subtraction on it is proved without a runtime check.

## Pinned Corpus Fixtures

All 18 corpus fixtures are pinned under `tests/omega/`; the nine pass fixtures are byte-identical copies of their place programs, so the sample and the fixture cannot drift apart, and each fail fixture is a byte-identical copy of its place's `invalid/` twin.

### Pass Fixtures (`tests/omega/pass/facts/`)

- `thirtyone_arg_stack_calling_plan_through_local_exit`
- `thirtyone_arg_stack_calling_plan_through_field_exit`
- `thirtyone_arg_stack_calling_plan_through_parameter_exit`
- `thirtyone_arg_stack_calling_plan_through_call_result_exit`
- `thirtyone_arg_stack_calling_plan_through_match_join_exit`
- `thirtyone_arg_stack_calling_plan_through_const_name_exit`
- `thirtyone_arg_stack_calling_plan_through_other_state_exit`
- `thirtyone_arg_stack_calling_plan_through_other_machine_exit`
- `thirtyone_arg_stack_calling_plan_through_target_body_exit`

### Fail Fixtures (`tests/omega/fail/facts/`)

- `thirtyone_arg_stack_calling_plan_through_local_rejected`
- `thirtyone_arg_stack_calling_plan_through_field_rejected`
- `thirtyone_arg_stack_calling_plan_through_parameter_rejected`
- `thirtyone_arg_stack_calling_plan_through_call_result_rejected`
- `thirtyone_arg_stack_calling_plan_through_match_join_rejected`
- `thirtyone_arg_stack_calling_plan_through_const_name_rejected`
- `thirtyone_arg_stack_calling_plan_through_other_state_rejected`
- `thirtyone_arg_stack_calling_plan_through_other_machine_rejected`
- `thirtyone_arg_stack_calling_plan_through_target_body_rejected`

## Recorded Evidence

The nine places and nine reject twins were compiled and executed with the shared generation producer at base `9b001b8f6af` (`omega` sha256 `6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, `corpus_runner` sha256 `cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`) and recorded on three legs:

- `tests/omega/corpus_outcomes.txt` — 9 `checked … check-target:macos_arm64` pass rows and 9 `rejected … expected` fail rows; each twin rejects with its own verbatim diagnostic (`exact arithmetic in machine `probe` state `entry` local `headroom` may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation) …`);
- `tests/omega/corpus_native_macos_arm64.txt` — 9 `built … exit:0` rows;
- `tests/omega/corpus_both_macos_arm64.txt` — 9 `built … exit:0 interpreter-exit:0 interpreter:agrees` rows.

36 recorded rows in total, no timeout rows, additions only. Each twin was also compiled on its own with the producer's check product (18 invocations, all exit 1) and that observation is the text its `expected.txt` carries; the `invalid/` controls keep their authored `sample.json` fragments beside it.
