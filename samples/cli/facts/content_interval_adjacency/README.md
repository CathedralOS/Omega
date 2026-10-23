# Adjacent interval content

Adjacent intervals preserve separated content when forwarded. The callable
requires `embed(left.base) + embed(left.length) == embed(right.base)` and
returns both owned claims beneath a pair; its postcondition conserves their
separated interval content. This is the numeric prerequisite exposed by
Cathedral's vacant-memory accounting, not a grant of physical storage or
permission to merge unrelated roots.

Every place copies that sentence and performs the same forwarding call with the
same content equation and the same runtime oracle: all four returned fields are
compared, both match arms are exercised, an empty interval appears on either
side, and one endpoint is `u64::MAX`. Only the transport of the two owned claims
changes. The captured equality must not erase the sum's nonnegative operands
before the interval ordering proof consumes them.

The root `main.omg` is byte-identical to the local place program
(`5a76e50b18a55fbc1c7f2cf9b5863d49f21f00a5`), so all ten programs carry the
same first-line fact sentence, the same adjacency `requires`, the same
separated-content `ensures`, the same `Held::content` projection and the same
`Expected exit: 0`; the nine places differ only in the transport mechanism. The
root also carries the folder's `build.omg`, which binds `ProgramEntry` for the
four hosted targets.

| Place | Positive | Reject twin | Transport mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness forwarding two adjacent owned claims |
| Local | [local](local/main.omg) | [overlap allowed](local/invalid/main.omg) | the adjacent claims are locals forwarded in one construction |
| Field | [field](field/main.omg) | [overlap allowed](field/invalid/main.omg) | the pair is built inside a record field and projected back out |
| Parameter | [parameter](parameter/main.omg) | [overlap allowed](parameter/invalid/main.omg) | the adjacency premise arrives as the callable's own parameter |
| Call result | [call result](call_result/main.omg) | [overlap allowed](call_result/invalid/main.omg) | a wrapper returns the completed call's pair |
| Match join | [match join](match_join/main.omg) | [overlap allowed](match_join/invalid/main.omg) | both match arms build the same pair |
| Const name | [const name](const_name/main.omg) | [overlap allowed](const_name/invalid/main.omg) | a named constant joins the sum |
| Another state | [state](other_state/main.omg) | [overlap allowed](other_state/invalid/main.omg) | a successor state receives both claims |
| Another machine | [machine](other_machine/main.omg) | [overlap allowed](other_machine/invalid/main.omg) | a forwarding machine supplies the pair |
| Target body | [target body](target_body/main.omg) | [overlap allowed](target_body/invalid/main.omg) | each of four target bodies supplies the pair |

Each twin changes only the adjacency requirement from `==` to `>=`, allowing
overlap. It must reject the separated-content postcondition. The named state's
arrival contract repeats the machine's requirement; states do not have result
postconditions.

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
in its own `build.omg` and declares `Expected exit: 0`. The authored acceptance
command is

```sh
omega run samples/cli/facts/content_interval_adjacency/main.omg --both
omega run samples/cli/facts/content_interval_adjacency/<place>/main.omg --both
```

### Measured on an Intel macOS host

Producer: revision `40e0f28bfbedfbcedd325586c02e288d3cf9af69`, `omega` built
with `python3 tools/build_slot.py -- mbx build -p omega` (`--build-profile`
reports `{"opt_level":"1"}`, binary digest
`24cea28fa0907253ff13aadd1ffae2c16aff54cc`), host target `macos_x86_64`
(`x86_64-apple-darwin`). This host executes only `macos_x86_64`. Every
observation below was re-measured on that revision with that binary; the
candidate adds no Rust. The same measurements on the earlier producers
`591336bdc9e`, `94cfa03cda2`, `b558cbfd1583` and `592f6958a1c9` agreed.

| Place | Terminal Psi interpreter | Host native `macos_x86_64` | linux_x86_64 image in a linux/amd64 container |
| --- | --- | --- | --- |
| root | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| local | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| field | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| parameter | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| call result | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| match join | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| const name | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| another state | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| another machine | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |
| target body | `interp exit: 0` | `native exit 0` (agrees) | ELF x86-64 static, exit 0 |

The host leg runs the scratch copy with no `--target`, which realizes and
executes the host target and then replays the same product in Terminal Psi.
Replay caution: `omega run --target <name>` reaches
`ExecutionOutcome::TargetOnly`, so it publishes the product and neither
executes nor interprets it. A `compiled for target ... OK` line is therefore
not an execution or agreement observation, and a probe that reads one as
agreement measures nothing; execute through `omega run --both <root.omg>` with
no `--target` (host target), or run the published image on its own target. The
scratch copy adds a `macos_x86_64::ProgramEntry` binding — and, for the target
body place, the matching `macos_x86_64 machine forward` body — because the four
authored bindings deliberately name the four hosted targets. The fact, the
bound and the operation are unchanged.

The linux leg compiles the authored sources unmodified
(`omega --target linux_x86_64 --build-dir <dir> <main.omg>`) and executes the
published image in an `alpine` linux/amd64 container, so the authored program
runs on a real target it binds.

The three targets this host cannot execute were realized without execution,
one image per program, every realization status 0: `windows_x86_64` PE32+
x86-64 console executable, `linux_arm64` statically linked ELF aarch64, and
`macos_arm64` Mach-O arm64. No execution is claimed for those targets, so no
native golden row was recorded for this row on this host. `tests/omega/corpus_both_macos_arm64.txt` carries
`interpreter:agrees` rows for all nine place fixtures recorded on an
Apple-silicon host at `f77f2aa2a1c2`; that evidence is retained, not re-measured
here.

On this Intel host the authored command with no `--target` refuses at entry
settlement, because `macos_x86_64` is not one of the four hosted targets:

```text
error: selected target `macos_x86_64` has no bound required root slot `macos_x86_64::ProgramEntry`
```

That is host pinning, not a compiler defect, and the four-target binding is not
changed to work around it.

### Integration record

The full measurement above was produced by one compiler build, and the row
landed on a later integration base; this record keeps the two apart.

- **Full-measurement producer:** revision
  `40e0f28bfbedfbcedd325586c02e288d3cf9af69`, `omega` digest
  `24cea28fa0907253ff13aadd1ffae2c16aff54cc`, opt-level 1. All of the
  observations above (host leg, linux leg, realization, discrimination,
  reject twins, corpus filter, sample gate) were taken on that producer.
- **Fresh post-rebase checks:** after rebasing onto the integration base below,
  the exercised checks were re-run: the host leg root plus nine places with
  bare `--both`, the nine reject twins, the discrimination probe, and the
  filtered corpus pass. Where the integration base carries newer compiler
  commits than the full-measurement producer, those checks and the named delta
  below are the applicability argument, and the results stay bound to the
  producer that produced them.
- **Retained evidence:** the realization formats and the linux container run
  are recorded on the full-measurement producer; they are not re-claimed for a
  later base.

<!-- integration-delta:start -->
Integration base at landing: `54d2215ed5f899a2dc67c6de3345588955714d83`, 3 commit(s) newer than the
full-measurement producer `40e0f28bfbedfbcedd325586c02e288d3cf9af69`. Exercised-surface `.rs` files newer
than that producer: 44.

Commits:

- `54d2215ed5f psi: prove open indexed constructors across erased calls`
- `705856dc542 library: declare blocking in portable filesystem contracts`
- `2611add3fd6 psi: prove closed construction operands without prefix replay`

Rust files:

- `omega-rust/omega/compilation/driver/tests/support/mathematical_actuals.rs`
- `omega-rust/omega/compilation/driver/tests/support/terminal_inspection.rs`
- `omega-rust/psi/compilation/checking/src/lib.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/call_cycles/rank_relevance.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/call_cycles/runtime_ranking.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/call_cycles/runtime_ranking/occurrences.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/calls.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/calls/occurrences.rs`
- `omega-rust/psi/compilation/checking/src/machine_calls/calls/unit_returns.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/ordered_values.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/construction_predicates.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/structural_terms/binary.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/mathematical_applications.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/mathematical_applications/prepared.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/relevance/runtime_uses.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/current_values.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/current_scalar_values.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/current_scalar_values/call_contracts.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/runtime_indices.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/termination/ranking/current_values.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/calls/argument_paths.rs`
- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/values/scalar/call_lowering.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/emission/continuation_identities/proofs.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/scalar_lowering/contracts.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/admission.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/composed_control/state_graph/admission.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/ordinary_machine.rs`
- `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/ordinary_machine/proof_scope.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature/scope.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature/source_evidence.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature/source_families.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature/source_families/construction.rs`
- `omega-rust/psi/compilation/proof/src/mathematical_declarations/mathematical_signature/source_families/terms.rs`
- `omega-rust/psi/compilation/representations/checked-trees/src/checked_trees/values/proof_terms.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/mathematical_proof_actual.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/mathematical_proof_actual/applications.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/mathematical_proof_actual/slots.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/proof_term_wire.rs`
- `omega-rust/psi/foundation/semantic-vocabulary/src/lib.rs`
- `omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs`
- `omega-rust/psi/foundation/semantic-vocabulary/src/proposition/proof_terms.rs`
<!-- integration-delta:end -->

### The oracle discriminates

The runtime oracle is the four-field content comparison in `exercise`. A corrupt
probe that leaves the adjacency requirement, the separated-content
postcondition and the forwarded `base`/`length` extents untouched, and changes
only what the program observes of the returned content (`pair.right.length ==
tail` to `pair.right.length == pair.left.length`), makes the first call's
observation disagree. Both engines trap and the host presentation agrees:

```text
native signal 4
interp crash: Trap (host presentation agrees)
```

Restoring the authored comparison re-verifies `native exit 0` with
`interp exit: 0`. A probe that corrupts the arithmetic instead
(`tail + 1`) never reaches execution: the exact arithmetic obligation refuses it
with `exact arithmetic in machine `exercise` state `entry` local `correct` may
overflow `u64``, so it is a checking observation, not a runtime discrimination.

## Reject Twins

Each place has an `invalid/` reject twin with authored `expected.txt`
(`cannot prove ensures contract`). In each twin the adjacency requirement is
relaxed from `==` to `>=` at the sites where it is declared, and nothing else
changes; the separated-content postcondition is then no longer provable. All
nine reject at checking with status 1:

| Place | Twin | Rejected obligation |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove ensures contract for exit from carry` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove ensures contract for exit from Forwarder::forward` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove ensures contract for exit from forward` |

Every diagnostic names the same obligation, and each prints
`separate(Held.content(old(&left)), Held.content(old(&right))) ==
separate(Held.content(&result.left), Held.content(&result.right))`. The call
result twin therefore weakens the inner `carry` requirement too: a wrapper that
only relaxed its own requirement would fail at the call's requirement
discharge instead, which is a different obligation.

## Pinned Fixtures

Every positive place is pinned under `tests/omega/pass/facts/` as
`content_interval_adjacency_through_<place>_exit` (byte-identical to its place
program, with `expected_native_exit.txt` `0`) and every reject twin under
`tests/omega/fail/facts/` as `content_interval_adjacency_through_<place>_rejected`
(carrying the authored `expected.txt` fragment). The families were moved on
2026-10-22 from the original nested `content_interval_adjacency/<place>/`
layout to these names — contents unchanged — because the corpus census
(`tools/swarm/fact_status.sh`) matches fixture families on exact path
components and read the row as unpinned. Re-measured on macOS arm64: root and
nine places `omega run --both` exit 0 (`native exit 0`, `interp exit: 0`), and
the nine twins reject at exit 1 with `cannot prove ensures contract ...` under
the authored fragment (shared opt-level-1 producer at base `9ff26a870215`). The
recorded goldens carry `checked check-target:macos_arm64` and
`rejected expected` rows for all eighteen in `tests/omega/corpus_outcomes.txt`
(records taken with the runner built from the landing-base tree `a93651a39273`,
opt-level 1, digest `36b7269cd30c3582`; stamps `@a93651a39273+wt`), plus
`built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt` (this
macOS arm64 host executes the host target, so the row now carries its own
native rows as well as the retained `interpreter:agrees` evidence at
`f77f2aa2a1c2` in `tests/omega/corpus_both_macos_arm64.txt`, re-recorded at
`a93651a39273` in the same pass).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS`
  (`TASKS.md`), under
  [content conservation](../../../../wiki/spec/resources/content_custody.md)
  with canonical `source/library/core/content.omg` identities. This row
  establishes the numeric adjacency prerequisite: a callable's postcondition
  conserves the separated interval content of two adjacent owned claims, and
  that fact survives all nine transport places.
- **What this landing adds:** the folder's missing canonical root witness
  (`main.omg` with `build.omg`) and this evidence record. The nine places and
  their twins already passed on the parent revision; this is row completion,
  not a new compiler repair, and it changes no `omega-rust/` source. The owning
  item's remaining acceptance is named under "Remaining customer work" and is
  untouched by this row.

### Specification and Lowering Layers

1. **Establishment:** the callable's own `requires` clause
   (`embed(left.base) + embed(left.length) == embed(right.base)`) admits the two
   claims, and its `ensures` clause states the separated-content equation over
   the result. The nonnegative embedded widths order the intervals before the
   separation operation, and the sum's operands are consumed by that proof
   rather than erased by the captured equality. No caller re-establishes the
   fact after the call with a post-hoc equality guard; `exercise` passes its own
   parameter equation into the call and consumes the returned pair.
2. **Identity and lifetime:** two linear claims move into distinct returned
   fields and are each dropped exactly once (`let { left, right } = pair`) after
   the observed comparison. The returned pair names the same two owned values it
   received. No backing, Stable access or new authority follows from the numeric
   geometry.
3. **Composition:** the same content equation is carried through a local, a
   record field, an explicit parameter, a call result, both match arms, a const
   name, a successor state, a forwarding machine and four target bodies. In each
   place `<place>/main.omg` and `<place>/invalid/main.omg` differ only in the
   adjacency requirement's relation; the consuming operation is the machine's
   `ensures`, proved directly from the fact carried through that place.
4. **Soundness boundaries:** overlap-permitting twins reject at the
   separated-content obligation, so numeric adjacency is what authorizes the
   equation — not equal sizes, and not scalar totals. Ownership controls remain
   under `tests/omega/fail/ownership/selected_constructor_*`. Returns through a
   wrapper and two simultaneously live results check that caller-local
   identities do not alias callee-local births. Physical storage, Stable access
   and the source-level compatible-lineage merge are not modelled here.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the checked ownership
   transfers and erased proof facts lower into canonical target-neutral Terminal
   Psi (`PSITERM\0`), and the interpreter replays the product that native
   execution ran (`interp exit: 0`, `native exit 0` for root and all nine
   places). These programs make no claim about `dyn` dispatch, boundary calls or
   borrowed-storage restoration windows.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   the Build companion selects the entry and ordinary structural argument/result
   layout and calling plans transport both regions; the four hosted images
   realize, and the linux/amd64 image executes. There is no foreign provider or
   physical memory acquisition here.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
   selected constructors and nested carrier calls exercise instruction
   selection, activation storage and return transport on x86_64 (host and
   linux/amd64). No standalone performance or bounded-stack guarantee is
   asserted.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
   the emitted host Mach-O and linux/amd64 ELF execute with the same zero result
   as Terminal Psi; branch placement, relocations and image emission are
   exercised only as needed by these programs. `windows_x86_64`, `linux_arm64`
   and `macos_arm64` realize to their own image formats but were not executed on
   this host.

## Remaining customer work

Cathedral's verified handoff retains its image/context and two vacant children.
Stable access, resident initialization, readback, retirement and an allocator
remain unfinished. A separately checked generic merge additionally needs the
source-level compatible-lineage contract raised in
[OWNER_QUESTIONS Q1](../../../../OWNER_QUESTIONS.md#q1-how-does-a-checked-callable-require-compatible-extent-lineage).
Numeric adjacency alone must never authorize that merge.

The owning item's wider acceptance stays open: checked wrapper theorem
propagation (a wrapper returning a match-selected completed partition call
proving its authored conservation guarantee), canonical
`core/content_conservation_contract` execution, the first authorized claim of
`content_retained_custody_round_trip` (**BOUNDARY-ISSUANCE**), and the fail
fixtures for overlap, gaps, wrong projection/unit/lineage, stale invocation and
substituted theorem arguments named on that item.