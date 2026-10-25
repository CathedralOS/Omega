# Dynamic Trait Dispatch

Parent task: `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`
Ticket: `e5f8ec0912314fa78f865ec38c7c5b2d`

## Governing requirement

Dispatching inline conformance members through a runtime `dyn` trait selector over explicit data arguments without an implicit `Self` or receiver attachment, preserving exact contracts, bounds, and values across all nine places.

**Fact: a runtime dyn trait selector dispatches an inline conformance member over explicit data arguments to establish 100u64 when subtracting 100u64.**

Every place performs the same operation: dispatch the inline `combine` member of named implementation `UnitCombine satisfies WeightedCombine` via a runtime trait selector `selector: dyn WeightedCombine = UnitCombine as dyn WeightedCombine;` over explicit `u64` arguments `(60u64, 40u64)` under precondition bounds `left >= 60u64 && left <= 60u64` and `right >= 40u64 && right <= 40u64`, returning the sum under postcondition guarantee `ensures result >= 100u64 && result <= 100u64`. Exact subtraction `total - 100u64` evaluates to 0, establishing exit code 0 under native and Terminal Psi execution. Only the transport of the `dyn` selector or dispatched result changes across the nine places.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness creating dyn WeightedCombine selector and dispatching combine over explicit operands | [`main.omg`](main.omg) | - |
| local | `local/` | the dyn trait selector is bound to a local before dispatching combine | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the dyn trait selector is stored in a record field before dispatching combine | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | the dispatching probe reads its dyn trait selector as an explicit parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | the dyn trait selector is obtained from a call result before dispatching combine | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both arms of a match join yield the dyn trait selector before dispatching combine | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant defines the exact target value for subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | a successor machine receives the dyn trait selector and bounded operands to dispatch combine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | a forwarding machine transports the dyn trait selector and bounded operands to dispatch combine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each hosted target body dispatches combine through the dyn trait selector | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Verification commands

Each positive place is an application that exits 0 in both native and Terminal Psi execution:

```sh
omega run --both samples/cli/facts/dynamic_trait_dispatch/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/local/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/field/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/parameter/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/call_result/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/match_join/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/const_name/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/other_state/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/other_machine/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place has an `invalid/` twin whose transported right operand violates the combine precondition contract `right >= 40u64 && right <= 40u64`. The invalid control must reject with `cannot prove requires contract for call dyn WeightedCombine::combine`. Check each twin with:

```sh
omega --check samples/cli/facts/dynamic_trait_dispatch/local/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/field/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/parameter/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/call_result/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/match_join/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/const_name/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/other_state/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/other_machine/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION` (`TASKS.md`).
- **Ticket:** `e5f8ec0912314fa78f865ec38c7c5b2d`.
- **Requirement established:** End-to-end nine-place fact preservation and dynamic trait object dispatch (`dyn Trait`). Dispatching inline conformance members through a runtime `dyn` trait selector over explicit data arguments without an implicit `Self` or receiver attachment, preserving exact contracts, bounds, and values across all nine places with differential agreement between native and Terminal Psi execution.
- **Pinned fixtures:**
  - Pass: `tests/omega/pass/facts/dynamic_trait_dispatch_through_*_exit`.
  - Fail: `tests/omega/fail/facts/dynamic_trait_dispatch_through_<place>_rejected`.
- **Authority:** [Dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md) and [Conformance](wiki/spec/language/conformance.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** The trait `WeightedCombine` defines inline member `combine(left: u64, right: u64) -> u64` with precondition `requires left >= 60u64 && left <= 60u64 && right >= 40u64 && right <= 40u64` and postcondition `ensures result >= 100u64 && result <= 100u64`. Named implementation `UnitCombine satisfies WeightedCombine` establishes conformance satisfaction through its verified inline body `left + right`. Coercing the named implementation `UnitCombine as dyn WeightedCombine` constructs a runtime dynamic trait selector referencing `UnitCombine`'s closed requirement map. Invoking the trait method `WeightedCombine::combine(selector, a, b)` with satisfied actual bounds directly establishes the postcondition guarantee `result >= 100u64 && result <= 100u64` at the call edge without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The dynamic selector `dyn WeightedCombine` carries package-scoped implementation selection identity without an implicit `Self` data receiver or hidden heap allocation. In existential trait objects with data payloads, the 16-byte `(data_ptr, selector)` carrier retains exact pointer-to-data and selector coherence across all 9 places. Here, the explicit arguments `60u64` and `40u64` are immutable scalar values transported independently with distinct lifetimes and disjoint value facts, maintaining their identity across variable bindings and call boundaries.
- **Composition:** Dynamic calls `WeightedCombine::combine(selector, left, right)` substitute exact actuals, verify preconditions strictly at every call site, and rejoin the guaranteed postcondition `result >= 100u64 && result <= 100u64` on every incoming path across all nine standard places: local variable binding (`local`), nominal struct field projection (`field`), explicit parameter passing (`parameter`), function call return value (`call_result`), match branch convergence join (`match_join`), named constant value definition (`const_name`), state transition arrival (`other_state`), forwarding machine routing (`other_machine`), and target-scoped body candidates (`target_body`). The established guarantee composes directly into exact subtraction `total - 100u64 == 0u64`. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in the right argument violating the required precondition bound (`right >= 40u64 && right <= 40u64`), never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call dyn WeightedCombine::combine: right >= 40u64 && right <= 40u64`. When a dynamic dispatch call violates the trait's `requires` contract or when the trait `guarantees` bound is insufficient for the consuming operation, the compiler refuses to lower the program to Terminal Psi or emit native code.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source coercion `UnitCombine as dyn WeightedCombine` lowers to `SelectImplementation`, producing an explicit implementation table reference. Dynamic invocation `WeightedCombine::combine(selector, a, b)` lowers directly to canonical target-neutral `PSITERM\0` as `CallDynamic`, cleanly separating the explicit `dyn` selector lane from the explicit data argument lanes without synthetic receiver parameters or existential type packing. Proof-only obligations and preconditions are erased after verification. The resulting Terminal Psi executes identically in the reference interpreter (`omega run --both`), observing exit code 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for 16-byte `(data_ptr, selector/vtable)` `dyn` trait carriers (or bare implementation selectors) is evaluated and validated. `CallPlan` and `StatePlan` lower dynamic calls and indirect dispatch-table placements across target ABI registers (passing selector and arguments in standard integer registers: `rcx`/`rdx`/`r8` on Windows x86-64, `rdi`/`rsi`/`rdx` on Linux x86-64, and `x0`-`x2` on ARM64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for dispatch-table / selector resolution, indirect function call execution (`call [rax]` on x86-64 / `blr x8` on ARM64), and integer arithmetic (`add`, `sub`). The register allocator assigns register homes and disjoint stack-frame activation storage for locals and spilled call parameters, computing worst-case stack usage (WCSU) and inserting entry prologues and callee-saved epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Immutable `dyn` dispatch tables are emitted with symbol relocations (`physical children`) pointing to `UnitCombine::combine`. Relocations and symbolic stack-frame displacements resolve, basic blocks and branch offsets relax with reflow, machine-state footprint evidence validates (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted that execute and exit with code 0 on `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Remaining open scope under `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`: multi-method dynamic vtable dispatch, finite generic method families with canonical value tuples, and cross-package dynamic conformance visibility export.

### Recorded measurements

Producer `2d783d092d9d`, target `macos_arm64`, CLI opt-level 1.

| Check | Command | Observed |
| --- | --- | --- |
| nine places plus root run | `omega run --both <place>/main.omg` | native exit 0 and Terminal exit 0, 10/10 |
| nine reject twins | `omega --check <place>/invalid/main.omg` | `cannot prove requires contract for call dyn WeightedCombine::combine`, 9/9 |
| sample gate | `python3 tools/sample_native_gate.py --facts --filter dynamic_trait_dispatch` | 10/10 meet the oracle, 9/9 intended-invalid matched |
| fixture native leg | `python3 tools/corpus_gate.py --native --filter dynamic_trait_dispatch` | clean; 9 built, 9 executed, exit codes `{0: 9}` |

Every sample place and the root carry `expected_native_exit.txt` of `0`
beside the `Expected exit: 0` header. The executed oracle for the row is the
one the corpus gate reads: `tests/omega/pass/facts/dynamic_trait_dispatch_through_<place>_exit/expected_native_exit.txt`,
whose nine rows are recorded in `tests/omega/corpus_native_macos_arm64.txt`.

**Canonical fixture names.** The nine fail fixtures were originally pinned as
`dynamic_trait_dispatch_unproved_<place>`, a shape a fact-row census that counts
fail fixtures only as `<fact>_through_<place>_rejected` reads as zero. They were
renamed to `dynamic_trait_dispatch_through_<place>_rejected` with the nine
golden keys rewritten in place (9 insertions, 9 deletions, 0 continuation
lines touched), and all three golden legs were then re-recorded at
`114906953e1` with the corpus runner built at that base (sha256
`f14e17c7521b980e193ba6929baec279dd843ec8144214f42e19e4e94cacfe5a`):
`python3 tools/corpus_gate.py --filter facts/dynamic_trait_dispatch` reads 18
fixtures — nine `checked` with `check-target:macos_arm64` and nine `rejected`
with their authored fragment — with 0 structural moves, and the native and
both legs re-record each `_exit` fixture as a host success
(`built ... exit:0`, `built ... exit:0 interpreter-exit:0 interpreter:agrees`).

**Oracle discrimination.** The recorded `0` is measured, not assumed. The
selector is the only input that decides the value: holding the trait contract
and both data arguments fixed, `UnitCombine as dyn WeightedCombine` gives
`native exit 0` / `interp exit: 0`, while swapping in a second conformance
`AltCombine satisfies WeightedCombine` — same declared contract, body
`left + right + 1u64` — gives `native signal 5` / `interp crash: Trap
(host presentation agrees)`. Both engines move together with the selector, and
neither agrees with the recorded `0` for the wrong selection. The compiler also
refuses to declare a realization that does not guarantee the requirement's
`ensures`, so a conforming implementation cannot silently return the wrong
value.

**Fixture oracle enforcement.** Corrupting one recorded expectation to `7`
makes the gate refuse with `expected native 7, observed built {'exit': '0'}`,
so the pinned rows are enforced rather than inert.
