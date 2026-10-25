# View extent after a content write

A view of at least 1024 bytes retains its extent after an indexed byte write
through a helper. Every place reads `view[view.len - 4]`, writes the first byte
through the helper, then restores the saved suffix byte using the same extent.
Each program exits 0 only when the first-byte change and suffix value reach the
caller. Its `invalid` twin weakens only the lower-bound fact to three bytes;
exact subtraction must still reject.

This witnesses the descriptor/content distinction needed by
FILESYSTEM-PLATFORM-SERVICES: Darwin directory enumeration needs to preserve its
private four-byte flags suffix around a native write into caller storage. It
does not authorize resizing, descriptor-slot replacement, stale content facts,
or unchecked indexing.

The owning requirement is preservation of an exact completed view descriptor
across referent writes, not preservation of the bytes it references. The
[subslice/state fixture](../../../../tests/omega/pass/facts/subslice_state_extent_after_write_exit/main.omg)
also constructs a clamped view before transferring it into the guarded state,
then reads and restores the same suffix around a write. Its
[wrong-view guard](../../../../tests/omega/fail/facts/subslice_state_extent_wrong_view_rejected/main.omg)
must not establish that view's bound. The nine places establish transport;
descriptor-slot replacement, contents invalidation and complete filesystem
acceptance keep their independent obligations.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness preserving view extent around a content write |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the view is bound to a local variable |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the view is stored in and read from a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the view arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the view is returned from an identity helper call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the view is joined across two arms of a match |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the minimum extent is named by `const minimum: u64 = 1024` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the view across a transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the view to the worker |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/view_extent_after_write/main.omg
omega run --both samples/cli/facts/view_extent_after_write/local/main.omg
omega run --both samples/cli/facts/view_extent_after_write/field/main.omg
omega run --both samples/cli/facts/view_extent_after_write/parameter/main.omg
omega run --both samples/cli/facts/view_extent_after_write/call_result/main.omg
omega run --both samples/cli/facts/view_extent_after_write/match_join/main.omg
omega run --both samples/cli/facts/view_extent_after_write/const_name/main.omg
omega run --both samples/cli/facts/view_extent_after_write/other_state/main.omg
omega run --both samples/cli/facts/view_extent_after_write/other_machine/main.omg
omega run --both samples/cli/facts/view_extent_after_write/target_body/main.omg
```

### The oracle discriminates

The exit code `0` is strictly discriminative. If `bytes[0]` is not modified to `41` by `write_first`, or if the suffix `bytes[1020]` is not restored to `73`, the guard `bytes[0] == 41 && bytes[1020] == 73` branches to `bad()`, executing `crash Trap;`. Both native and Terminal Psi engines abort non-zero on failure.

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["exact arithmetic", "may overflow `u64`"]`) and `expected.txt`. Weakening the lower bound from 1024 to 3 (`bytes.len >= 3` or `minimum: u64 = 3`) means `view.len - 4` is not provably in range (3 - 4 underflows in unsigned 64-bit integer arithmetic), failing closed during proof checking:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `exact arithmetic` / `may overflow \`u64\`` |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/view_extent_after_write_through_<place>_exit` (all 9 places pinned, each recording native exit 0).
- Negative controls: `tests/omega/fail/facts/view_extent_after_write_through_<place>_unproved` (all 9 places pinned, rejecting with `exact arithmetic`).
- Subslice state extent: `tests/omega/pass/facts/subslice_state_extent_after_write_exit` and `tests/omega/fail/facts/subslice_state_extent_wrong_view_rejected`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `FILESYSTEM-PLATFORM-SERVICES` (`TASKS.md`).

### Specification and Lowering Layers

1. **Establishment:** A mutable slice view `bytes: &mut [u8]` with precondition `requires bytes.len >= 1024` establishes an exact known minimum extent. Reading `view[view.len - 4]`, passing a sub-borrow `&mut view[0]` to `write_first(first)`, and subsequently indexing `view[view.len - 4] = saved` proves safe indexing directly from the preserved slice extent without requiring any post-hoc caller equality guard (`transition x == 100`) after the write helper returns.
2. **Identity and lifetime:** Mutating an indexed element through `write_first(&mut view[0])` mutates only the targeted byte `bytes[0]` and invalidates value facts about `view[0]`, while preserving the slice view descriptor's length invariant `view.len >= 1024` and all disjoint storage facts (such as the suffix at `view.len - 4`). Borrow scopes and reborrow restoration windows enforce that the sub-borrow `&mut view[0]` expires before `view` is re-accessed, guaranteeing exclusive access without alias confusion.
3. **Composition:** The preserved view extent fact composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) to prove safe array/slice element indexing both before and after the helper write. Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the extent lower bound (`bytes.len >= 1024` vs `bytes.len >= 3`), isolating the exact fact under test without caller guard divergence.
4. **Soundness boundaries:** All 9 reject twins fail closed during source proof checking when the lower bound is weakened to `bytes.len >= 3` (`minimum: u64 = 3`). Exact subtraction `view.len - 4` is not provably non-negative in unsigned 64-bit arithmetic (`3 - 4` underflows `u64`), and proof checking strictly rejects compilation with `error: exact arithmetic in machine preserve_suffix ... may overflow \`u64\``. Unproved or underflowing index bounds cannot escape to runtime.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Mutable slice element stores and subsequent reads lower into canonical `PSITERM\0` operations. Static length proof contracts are erased after verification while runtime `(ptr, len)` view descriptors and borrow restoration windows (`reborrow restored call uses`) are preserved. Both native and Terminal Psi interpreter engines execute identically in `omega run --both`, achieving exit code 0.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). The 1024-byte backing array `[u8; 1024]` and mutable slice view descriptor `&mut [u8]` validate target layout geometry (fat pointer of 16 bytes: 8-byte base pointer + 8-byte length). `CallPlan` and `StatePlan` allocate two-word slice parameters and references into standard target ABI calling convention registers and stack slots.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for indexed byte stores and loads (`base + index`), pointer offset arithmetic, and subroutine calls to `write_first`. Lowering assigns register homes and disjoint activation stack-frame slots for the `[u8; 1024]` local array, inserting standard function entry prologues and epilogues honoring `StatePlan` frame ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements to local stack frame activation storage resolve during form encoding, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct native PE/ELF/Mach-O emission produces functioning executables in complete agreement with Terminal Psi (`omega run --both` exit 0).
