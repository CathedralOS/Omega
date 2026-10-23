# Computed subslice extent from a predicate

Given `can_window(items, low) == true`, the captured view
`items[low + 0 .. low + 2]` has extent 2, and reading offset 1 observes original
backing `items[low + 1]`.

`can_window` is an ordinary pure total predicate whose checked body states
`items.len == 4 && low <= 2`. Its name alone grants nothing. Its exact
application supplies the premises for the checked endpoint arithmetic and the
independent ordered range-formation obligation. The subsequent offset-1 read
still owes its own bound against the resulting descriptor. The target-family
head retains the whole predicate requirement: publishing interval observations
requires checked equivalence in both directions, rather than only necessary
consequences. Disconnected truth sets cannot be replaced by their interval hull.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each retains
that same fact, bound and read. Every runnable entry checks the view's measured
extent and reads from `[11, 23, 37, 53]` at both `low = 1` and the allowed
endpoint `low = 2`, expecting 37 and 53 respectively. The match place executes
both alternatives at both values. All four target bodies are checked; native
execution observes Windows x86-64 and Linux x86-64 on matching hosts.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [source](main.omg) | — | root entry subslice extent and offset read |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | the computed subslice view is bound in a local |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | the computed subslice view is stored in a record field |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | the computed subslice view arrives as an explicit parameter |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | a helper call returns the computed subslice view with `result.len == 2` |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | both match arms produce the computed subslice view |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | named constants `ZERO` and `SPAN` supply the subslice endpoint offsets |
| other state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | a successor state receives the computed subslice view |
| other machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | another machine receives and inspects the computed subslice view |
| target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | all four target-scoped machine bodies construct and read the subslice view |

Each place's `invalid/` (and `controls/insufficient_bound`) changes only the predicate's bound
from `low <= 2` to `low <= 3`. The original range and read remain intact. The
twin must reject the range call's `end <= items.len` obligation, because the
weaker premise permits an endpoint of 5 in backing of extent 4.

## Requirements and Evidence

- **Parent outcome:** `BORROW-PROOF-CONVERGENCE` (`TASKS.md`).
- **Establishment:** Checked pure predicate `can_window(items, low) == true` (whose body states `items.len == 4 && low <= 2`) supplies the exact collection length and index upper bound needed to prove exact `u64` endpoint additions `low + 0u64` and `low + 2u64`, ordered range formation `start <= end`, and backing inclusion `end <= items.len`, establishing that the resulting subslice descriptor `items[low + 0u64..low + 2u64]` has exact extent 2 without any post-hoc caller guard.
- **Identity and lifetime:** The borrowed subslice view `&[u8]` retains its provenance, backing loan, and exact extent fact (`view.len == 2`) across immutable local bindings, borrowed record fields (`Held<'a>`), parameters, helper return lifetimes, match joins, constant offset names, state transitions, and cross-machine calls while remaining tied to the live backing array `items`. Storage versions and distinct element identities are preserved without invalidation.
- **Composition:** Call contracts (`requires can_window(items, low) == true`, `requires view.len == 2`, `ensures result.len == 2`), match-arm joins, state transitions (`inspect(view)`), and target-qualified machine bodies compose the predicate premise with subslice construction so the consuming offset read `view[1]` is proved directly from the transported extent fact (`1 < 2`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` (and `<place>/controls/insufficient_bound/main.omg`) differs only in the predicate bound (`low <= 2` vs `low <= 3`).
- **Soundness boundaries:** Each place's reject twin weakens only the predicate bound from `low <= 2` to `low <= 3` and is rejected during proof checking with `cannot prove requires contract for call range ... end <= items.len`. Additional corpus controls reject effectful predicate actuals, saved stale predicates across mutations, and interval hull approximations of disconnected truth sets.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Subslice range formation `items[low + 0u64..low + 2u64]`, descriptor length projection `view.len`, and indexed element read `view[1]` lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing borrowed two-word slice descriptors (base pointer + element count) and restoration windows while erasing proof-only obligation tags (`proof-term-tags`). The Terminal Psi interpreter evaluates the subslice expressions and executes in agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Backing byte-array `[u8; 4]` (`size_of = 4`, align 1) and two-word slice descriptors (`size_of = 16`, align 8) validate. Target `CallPlan`/`StatePlan` lower slice descriptor pointers and counts across registers (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit addition for subslice base pointer calculation (`base + low`), offset-1 byte loads (`movzx` or `ldrb`), integer comparisons, assigns register homes and disjoint stack-frame activation storage for `items`, and inserts standard entry prologues and epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for local array storage and slice views, basic blocks lay out with branch relaxation and offset reflow around conditional branches, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## Run record: the oracle discriminates

Recorded on `macos_arm64`, producer `@bcd7c58d3a26`, `opt-level=1`. All ten
runnable entries (root plus nine places) report `native exit 0` and
`interp exit: 0` under `omega run --both`. Nine pass fixtures are pinned in
`tests/omega/corpus_native_macos_arm64.txt` as `built ... exit:0`; nine reject
twins plus three further controls are pinned in
`tests/omega/corpus_outcomes.txt` as `rejected ... expected`.

The measured exit is only meaningful if the oracle can fail. Two corruptions
separate cleanly, and the distinction is the finding:

- **Refused statically (checker, before native production).** Replacing the
  captured extent with a literal one — `items[low + 0u64..low + 1u64]` while the
  offset-1 read remains — is rejected during proof checking with
  `cannot prove requires contract for call index from read: 1 < view.len`, and the
  out-of-domain call `read(&items, 3u64)` with
  `cannot prove requires contract for call read from main: can_window(items, 3) == true`.
  Purely static corruption of the construct therefore yields a proof error, not a
  runtime trap.
- **Reached runtime and trapped (both engines).** A runtime discrimination proof
  needs a value the prover cannot constant-fold, so the base index is seeded from
  a boundary contract admitting a range: `boundary trait RequiredLow { machine
  get() -> u64 ensures result <= 2; }` joined by `Build::assume_binding_contract`
  to a provider returning `1`. The baseline
  `read(&items, RequiredLow::get()) == 37u8` exits 0 in both engines. Corrupting
  only the expected observation to `== 53u8` traps both engines identically —
  `native signal 5` / `interp crash: Trap (host presentation agrees)`, exit 255 —
  and a provider returning `3u64`, outside its own asserted range, gives
  `native signal 5` with the interpreter refusing at
  `cause: VerifiedOperationMalformed` (exit 201). The exact captured length is
  therefore load-bearing at runtime, not folded away.

```sh
python3 tools/sample_native_gate.py --filter cli/facts/computed_subslice_extent
python3 tools/corpus_gate.py --both --filter computed_subslice_extent
```

### Re-pin at the corpus fixture names (macOS arm64)

The row's fixtures first landed as `computed_subslice_extent_<place>_exit` and
`computed_subslice_extent_<place>_insufficient_bound`. The corpus census
(`tools/swarm/fact_status.sh`) matches fixture families as
`<fact>_through_<place>_{exit,rejected}` on exact path components, so it read
zero for this row; on 2026-10-22 both families moved to
`tests/omega/{pass,fail}/facts/computed_subslice_extent_through_<place>_`
`{exit,rejected}` with contents unchanged (`git mv`; each `build.omg`'s
`Build::application` label renamed to name the behavior it pins). Every entry
was re-measured on macOS arm64: root and nine places `omega run --both` exit 0
with `native exit 0` / `interp exit: 0` and the nine twins exit 1 at
`cannot prove requires contract for call range ... end <= items.len` in state
`entry` (shared opt-level-1 producer at base `9ff26a870215`), and the golden
records below were taken with the corpus runner built from the landing-base
tree `a93651a39273` (`--build-profile` reports `opt_level: 1`, digest
`36b7269cd30c3582`). The three goldens carry this row at the anchored base
(`@a93651a39273+wt`): nine `checked check-target:macos_arm64` rows and twelve
`rejected expected` rows in `tests/omega/corpus_outcomes.txt`, nine
`built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and
nine `built ... exit:0 interpreter:agrees` rows in
`tests/omega/corpus_both_macos_arm64.txt`.

Authority comes from [state contracts](../../../../wiki/spec/language/state_contracts.md),
[checked predicate consequences](../../../../wiki/spec/language/dependent_values.md#collection-invariants),
[exact subslice descriptors](../../../../wiki/spec/terminal-psi/byte_views.md), and
[live fact identity](../../../../wiki/spec/terminal-psi/fact_validity.md).
