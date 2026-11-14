# Returned write destination

A returned write-only loan permits replacing caller storage 7 with 42 while
preserving neighboring 19. Every place repeats that fact and exits 0 only
when the caller observes both values after the loan ends.

The original root witness `main.omg` with `build.omg` remains the starting witness.
The nine places are:

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | returned write destination loan replaced in caller storage |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | returned descriptor bound locally |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | returned descriptor stored in a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | returned descriptor passed as a parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | returned descriptor forwarded by another call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | either arm returns the same caller destination |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | named constant selects the caller array element |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | returned descriptor becomes a successor parameter |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | a forwarding machine calls the replacement machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | each target's Build row selects an ordinary implementation that returns the caller destination |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/returned_write_destination/main.omg
omega run --both samples/cli/facts/returned_write_destination/local/main.omg
omega run --both samples/cli/facts/returned_write_destination/field/main.omg
omega run --both samples/cli/facts/returned_write_destination/parameter/main.omg
omega run --both samples/cli/facts/returned_write_destination/call_result/main.omg
omega run --both samples/cli/facts/returned_write_destination/match_join/main.omg
omega run --both samples/cli/facts/returned_write_destination/const_name/main.omg
omega run --both samples/cli/facts/returned_write_destination/other_state/main.omg
omega run --both samples/cli/facts/returned_write_destination/other_machine/main.omg
omega run --both samples/cli/facts/returned_write_destination/target_body/main.omg
```

### The oracle discriminates

The exit code `0` is strictly discriminative. If `value` is not replaced with `42`, or if the neighboring variable `neighbor` (19) is corrupted, the transition guard `value == 42 && neighbor == 19` branches to `wrong()`, executing `crash Trap;`. Both native and Terminal Psi engines abort non-zero on divergence.

## Reject Twins

Each place has an `invalid/` reject twin (and corresponding `controls/read_destination/`) with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["write-only", "never observation"]`) and `expected.txt`. The twin changes only the store from `destination = 42` to `destination = destination` (the field case uses `destination.loan`), attempting prohibited observation of the write-only referent:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`invalid_local/main.omg`](invalid_local/main.omg) | `write-only` / `never observation` |
| field | [`invalid_field/main.omg`](invalid_field/main.omg) | `write-only` / `never observation` |
| parameter | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | `write-only` / `never observation` |
| call result | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | `write-only` / `never observation` |
| match join | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | `write-only` / `never observation` |
| const name | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | `write-only` / `never observation` |
| a nested machine | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | `write-only` / `never observation` |
| another machine | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | `write-only` / `never observation` |
| another target body | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | `write-only` / `never observation` |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/returned_write_destination_through_<place>_exit` (all 9 places pinned, each recording native exit 0).
- Negative controls: `tests/omega/fail/facts/returned_write_destination_through_<place>_rejected` (all 9 places pinned, rejecting with write-only observation).
- Related borrow controls: `tests/omega/fail/borrow/returned_write_destination_read`, `returned_write_destination_argument_read`, `returned_write_destination_backing_conflict`, `returned_write_destination_carrier_conflict`, and `returned_write_destination_readable_access`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).

### Specification and Lowering Layers

1. **Establishment:** Returning or forwarding a `&write` write-only destination loan from a selector/callee (`lend_write(destination: &write i32) -> &write i32 { &write destination }`) and executing `destination = 42` establishes initialized backing storage and satisfies the post-write fact `value == 42` directly in caller storage. Backing initialization and the post-write value are established directly by the definite write without any post-hoc caller equality guard (`transition x == 100`) after the operation completes.
2. **Identity and lifetime:** The returned `&write` loan preserves exact backing storage identity (`value`) across call boundaries and selection joins while maintaining disjointness from neighboring storage (`neighbor == 19`). Borrow rules forbid accessing the loaned backing storage (`value`) until the returned `&write` loan finishes, and write-only semantics strictly prohibit observing or reading the referent while the destination loan is active.
3. **Composition:** Returned `&write` destination loans compose across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming write `destination = 42` and subsequent verification of caller storage (`value == 42 && neighbor == 19`). Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in attempting to read the write-only destination (`destination = destination`), cleanly isolating the exact loan discipline under test without caller guard divergence.
4. **Soundness boundaries:** All 9 reject twins (`<place>/invalid/main.omg` and `<place>/controls/read_destination/main.omg`) fail closed during typechecking with exit code 1 and diagnostic: `error: machine `main` state `entry` reads write-only parameter `destination`; `&write` permits replacement or exact `&write` forwarding, never observation`. Reading uninitialized or write-only destinations, conflicting concurrent borrows, and alias leakage are strictly caught at source checking before backend emission.
5. **Checked-to-Terminal Psi lowering (`Psi 06–08`):** Returned `&write` destination loans and definite-initialization proofs lower into canonical `PSITERM\0` pointer returns and stores. Borrow restoration windows (`reborrow restored call uses`) track active loans across call sites and joins while compile-time write-only checks are preserved in typing and erased for execution. The Terminal Psi interpreter executes every positive place in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar types (`i32`, `u64`) and reference pointers (`&write T`) evaluate target layout geometry (`size_of` 8-byte pointer, 4-byte `i32`). `CallPlan` and `StatePlan` allocate `&write` destination pointer arguments and returns into standard ABI parameter/return registers according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for pointer passing, address calculation, indirect store (`mov [rdi], 42` or `str w0, [x1]`), and subsequent direct loads of caller storage (`value`, `neighbor`). Lowering assigns register homes and disjoint activation stack-frame slots for caller locals, inserting standard entry prologues and epilogues honoring `StatePlan` frame ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements for caller variables resolve in final binary instruction encodings, basic blocks relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct native PE/ELF/Mach-O emission produces functioning executables in complete agreement with Terminal Psi (`omega run --both` exit 0) across platforms.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps.

Base `4f87c9e08b1` refused the migrated positive: an opaque selected
implementation required an admitted write-only boundary claim, because the
write-only checker counted only machine-level `satisfies` clauses as
implementations. #1361 (`b7d254d952a`) counts named-implementation rows and
still requires each selected implementation to supply a checked body, so this
migration needs no compiler change. Observed with the optimized producer at
`11bdd2fe41f` on macOS x86-64: all nine positives and the root exit 0 under
`omega run --both` (native and Terminal Psi agree), and the nine twins plus
the `target_body` read control reject because `main` reads write-only
parameter `destination` (`&write` permits replacement or exact forwarding,
never observation). The other hosted targets keep their authored rows,
unexecuted here.
