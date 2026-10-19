# Aggregate Struct Return ABI Realization

// Fact: a four-word structural aggregate returned and passed by value preserves field refinement bounds summing to 100u64 across indirect return homes when subtracting 100u64.

Every place performs the same operation: construct and return a 32-byte (four-`u64`-field) structural record `Quartet` whose refinement clauses specify `first == 10u64`, `second == 20u64`, `third == 30u64`, and `fourth == 40u64` (summing to `100u64`). Because 32 bytes exceeds the 16-byte register return pair capacity on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`), returning `Quartet` by value exercises indirect aggregate return (`sret`) lowering into caller-allocated stack frame return homes via target-mandated hidden pointer registers (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, and `X8` on `macos_arm64`/`linux_arm64`). Each place transports `Quartet` by value across one of nine canonical program locations, sums its four fields (`q.first + q.second + q.third + q.fourth`), and performs exact subtraction `total - 100u64` returning 0. Only the transport mechanism of the aggregate operand changes across places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | aggregate returned by value is bound to a local before field projection and subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | aggregate returned by value is stored into an enclosing record field before projection |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | aggregate returned by value is passed by value as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | aggregate returned by value is returned directly from an indirect helper call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute indirect aggregate returns and join into an aggregate |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants define the field values returned in the four-word aggregate |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the returned aggregate by value and projects its fields |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine transports the returned aggregate by value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute indirect aggregate returns on each hosted target |

## Execution

Each positive place is an application that exits 0. All ten positive programs
(root `main.omg` and the nine places) bind `ProgramEntry` for the four hosted
targets, declare `Expected exit: 0`, and were observed to run: `omega run --both`
reports native `exit 0` together with Terminal `interp exit: 0` for every one of
them. The nine places additionally carry `expected_native_exit.txt` with `0`, and
the nine rows are pinned as `built ... exit:0` in the host native golden.

```sh
omega run --both samples/cli/facts/aggregate_struct_return_abi/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/local/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/field/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/parameter/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/call_result/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/match_join/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/const_name/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/other_state/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/other_machine/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/target_body/main.omg
```

Each place has an `invalid/` twin whose fourth field bound and value is `39u64` instead of `40u64` (summing to `99u64`), causing exact subtraction `total - 100u64` to fail proof checking with `may overflow \`u64\`` (`exact subtraction underflow`). Each twin carries a `sample.json` naming that rejection identity, so the negative controls are scored rather than unchecked. Check each twin with:

```sh
omega --check samples/cli/facts/aggregate_struct_return_abi/<place>/invalid/main.omg
```

## Observed evidence

Measured on `2d783d092d9d` (producer `target/debug/omega`, opt-level 1, host
`macos_arm64`):

- `python3 tools/sample_native_gate.py --filter cli/facts/aggregate_struct_return_abi`
  exits 0: `10/10 samples meet their documented oracle on macos_arm64` and
  `9/9 intended-invalid controls match their authored rejection`.
- `python3 tools/corpus_gate.py --filter facts/aggregate_struct_return_abi` reports
  `0 structural move(s)` over `18 fixtures ({'checked': 9, 'rejected': 9})`, with
  `0 fail fixtures did not reject`.
- `python3 tools/corpus_gate.py --native --record --filter facts/aggregate_struct_return_abi`
  pinned the nine positive places as `built ... exit:0`.

The oracle was shown to discriminate at the native boundary rather than merely
agreeing with itself. `omega run --both --keep --target macos_arm64` emits a
Mach-O arm64 image in which `make_quartet` writes the 32-byte aggregate through
the caller return home in `X8` (`mov x8, x8` / `str x1, [x8]` … `str x0, [x8, #0x18]`)
and `probe` sets `mov x8, sp` before the call, copies four words, and computes
`sum - 0x64`. Re-signing that image and patching only the single instruction that
loads the fourth field (`movz x3, #0x28` to `movz x3, #0x27`, 40 to 39) changes the
observed process outcome from `exit 0` to `exit 133` (SIGTRAP from the `brk #0`
crash arm). The recorded exit is therefore what the program actually produces
through the indirect aggregate return path.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `268c1b7311ca4420ac912427369c41dc`.
- **Requirement established:** End-to-end nine-place fact preservation and ABI realization for large structural aggregate returns exceeding register-return limits (`sret`). Specifically, a 32-byte structural record (`Quartet`) returned and passed by value preserves field refinement bounds summing to `100u64` across indirect return homes when subtracting `100u64`, with exact execution agreement between Terminal Psi and native code across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`).
- **Obligations accounted for:**
  - *Establishment:* Concrete aggregate field initializers satisfy `Quartet`'s declared `where` refinement clauses (`first == 10u64`, `second == 20u64`, `third == 30u64`, `fourth == 40u64`), establishing the arithmetic invariant that the field sum is identically `100u64`.
  - *Identity and lifetime:* The structural aggregate is an owned value type (`[copy]`). Returning and passing it by value preserves exact 32-byte layout geometry and individual field bounds through value movement, stack frame storage copy, and projections without clobbering, displacement, or lifetime aliasing.
  - *Composition:* Callee calls, indirect return home copy handoffs, match joins, state transitions, forwarding machines, and target-scoped bodies compose exact value transport and preserve the `100u64` invariant on all incoming control paths.
  - *Soundness boundaries:* Reject twins demonstrate that breaking the fourth refinement bound to `39u64` results in an unproved sum of `99u64`, which underflows exact subtraction `total - 100u64` and is strictly rejected during proof checking with `may overflow \`u64\``.
  - *Terminal Psi and carrier lowering:* The 32-byte record lowers to Terminal Psi using `ReturnStructural` with a distinct operation-result place and whole-value structural custody, cleanly separating argument transfer from result claim binding without loss of field refinement semantics.
  - *Native realization, frame/layout encoding, and image emission:* Realization assigns an indirect aggregate return pointer (`sret`) to the target-mandated register (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, `X8` on `macos_arm64`/`linux_arm64`), allocates caller return-home stack frame slots, emits multi-word memory copy instructions, and generates native PE/ELF/Mach-O images whose execution agrees identically with Terminal Psi (`omega run --both`) exiting 0.
- **What remains open:** Floating-point vector aggregate return classification (`HFA`/`HVX`), multi-return branch epilogue reflow with divergent aggregate return shapes, and dynamic slice view returns across indirect memory destinations.
