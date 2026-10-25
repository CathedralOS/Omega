# Case payload layout geometry

Querying `size_of` and `align_of` on a multi-variant `case` type establishes its exact discriminant-plus-payload layout geometry, discharging exact subtraction `size - (align * 2u64)`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the tagged `case` layout geometry through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | layout queries evaluate discriminant-plus-payload size and alignment directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | case payload size and alignment bound to local variables |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | case payload size and alignment stored in and projected from record fields |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | case payload size and alignment passed as explicit machine parameters |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | case payload size and alignment returned across machine call boundaries |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | case payload size and alignment preserved across match join convergence |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | case payload size and alignment evaluated in named compile-time constants |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | case payload size and alignment forwarded as arguments across state transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | case payload layout geometry computed in and delegated through an intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | case payload layout geometry computed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Measured geometry

The mechanism is a tag at offset 0 followed by the payload at the first offset
that satisfies the payload's own alignment. For a two-variant type whose payloads
are `Empty` and `Payload(T)`, that composes to
`size_of == 2 * max(4, size_of<T>)` and `align_of == max(4, align_of<T>)`.

| Payload `T` | `size_of` | `align_of` | `size - align * 2` |
| --- | --- | --- | --- |
| `u8` | 8 | 4 | 0 |
| `u16` | 8 | 4 | 0 |
| `u32` | 8 | 4 | 0 |
| `u64` | 16 | 8 | 0 |
| `[u64; 1]` | 16 | 8 | 0 |
| `[u64; 2]` | 24 | 8 | 8 |
| `[u64; 3]` | 32 | 8 | 16 |

The composing maximum is the discriminant slot's own width: a variant count small
enough for a 4-byte tag leaves the tag at 4 bytes. **A payload narrower than 4
bytes does not narrow the tag.** `u8` and `u16` payloads both report `size 8,
align 4` — the slot is widened to 4, not measured from the payload's width — so a
reject twin must not be authored on the belief that the tag tracks the payload.
The alignment row is what makes this a payload-packing fact rather than a tag
fact: `[u64; 2]` moves `size_of` because the payload is wider than the tag, while
`u32` leaves it at `max(4, 4) * 2`.

`size_of` responds to a real change in payload representation, and the oracle is
not constant-folded to its own expectation. Both engines agree on every row, and
corrupting the oracle falsifies the claim: asserting `size == 24` or `align == 4`
for the `u64` payload traps natively (signal 5) and in Terminal Psi (`Trap`),
while the measured values exit 0 in both. The `[u64; 2]` program traps on
`diff == 0` and exits 0 on `diff == 8`, so the computed difference — not the
comparison constant — selects the state.

## Requirements and Evidence

- **Parent outcome:** `LAYOUT-PLANS` / `COUNT-ADDRESS-GEOMETRY` / `TERMINAL-LAYOUT-WIRE-EVALUATION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** Tagged `case` discriminant plus payload layout geometry proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that querying compile-time layout geometry (`size_of<Packet>() == 16u64`, `align_of<Packet>() == 8u64`) on a multi-variant sum type (`case Packet { Empty, Payload(u64) }`) establishes exact layout geometry and proves non-negativity for exact subtraction `size - (align * 2u64) == 0u64` across all nine standard places.

Pinned fixtures: the nine positives are
[`tests/omega/pass/facts/case_payload_layout_geometry_through_<place>_exit`](../../../../tests/omega/pass/facts)
and the nine reject twins are
[`tests/omega/fail/facts/case_payload_layout_geometry_unproved_<place>`](../../../../tests/omega/fail/facts),
for `<place>` in `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, `target_body`. Each positive
carries its own `expected_native_exit.txt` (`0`); each twin authors both
`expected.txt` and `invalid`-style `sample.json` `rejection_contains`.
`<place>/main.omg` and `<place>/invalid/main.omg` differ only in the payload
variant — `Payload(value: u64)` against payloadless `Other` — never in a caller
guard.

### Pin re-verification (`9ff26a870215`, 2026-10-22)

Producer: `factsr1-producer` `target/debug/omega` built at `9ff26a870215`,
macOS arm64. Commands and numbers:

| Command | Result |
| --- | --- |
| `omega run --both` on root and all nine places (10 projects) | 10/10 `native exit 0` with `interp exit: 0` |
| `omega --check <place>/invalid/main.omg` (9 twins) | 9/9 reject, exit 1, `bounds [-4, -4] lie outside [0, 18446744073709551615]` |
| `omega --check tests/omega/fail/facts/case_payload_layout_geometry_unproved_<place>/main.omg` (9 fixture twins) | 9/9 reject, exit 1, same fragment |

Each of the nine pinned positives and nine pinned twins shares its program body
with its sample-side source (`<place>/main.omg`) or invalid twin
(`<place>/invalid/main.omg`); the fixture copies differ only in their
fact-sentence header comments.

Full 8-layer specification and lowering evidence:

- **Establishment:** Compile-time layout queries `size_of<Packet>()` and `align_of<Packet>()` evaluate the layout geometry of the tagged sum type `data Packet { case Empty; case Payload(value: u64); }`. The type comprises a discriminant tag field followed by the alignment-padded variant payload `value: u64` (alignment 8 bytes, size 16 bytes). The compiler proves the exact invariant `size == 16u64` and `align == 8u64`, statically establishing that `size >= align * 2u64` and discharging exact subtraction `size - (align * 2u64)` without runtime trapping or defensive caller guards.
- **Identity and lifetime:** Layout geometry values are compile-time immutable scalar constants tied to the canonical layout plan of the type definition. The static value bounds `size == 16u64` and `align == 8u64` persist unchanged across local variable bindings, record field projections, machine parameter passing, call returns, match join arms, compile-time constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The layout geometry bounds compose across all 9 standard places to statically discharge exact subtraction `size - (align * 2u64)`:
  - `local`: layout query results are bound to local variables `local_size` and `local_align` before subtracting.
  - `field`: layout query results are stored in `GeometryCarrier` record fields and projected out.
  - `parameter`: layout query results are passed as parameters to helper machines `receive_size` and `receive_align`.
  - `call_result`: helper machines `get_size()` and `get_align()` return layout query results across call boundaries.
  - `match_join`: layout query results converge through match arms into a unified value.
  - `const_name`: layout query results are bound to named constants `PACKET_SIZE` and `PACKET_ALIGN`.
  - `other_state`: layout query results are passed across transition edges into successor machine parameters.
  - `other_machine`: layout query computation is delegated through intermediate machine `runner()`.
  - `target_body`: layout query computation is executed across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in replacing the payload variant `Payload(value: u64)` with payloadless `Other`, changing the layout geometry (`size == 4u64`, `align == 4u64`) so that `size - (align * 2u64)` results in negative bounds `[-4, -4]` and fails compile-time check.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `exact arithmetic may overflow` (`bounds [-4, -4] lie outside [0, 18446744073709551615]`), proving that the layout and arithmetic proof systems soundly reject any invalid payload geometry variation. The refusal is **static**: it is decided at construction, before lowering, so no runtime trap is reachable on the positive path and none is authored. The trap route is exercised separately as a falsification control — asserting a false geometry (`size == 24`, `align == 4`) does execute and traps in both engines.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-variant `data` declarations lower into canonical `PSITERM\0` type definitions and layout plans. Layout queries resolve to target-neutral compile-time constants before Terminal Psi production. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Sum-type discriminant tags and variant payload offsets are resolved according to target layout geometry, validating that the 8-byte aligned `u64` payload requires an 8-byte discriminant slot and 16-byte total allocation across all hosted targets.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for 64-bit integer arithmetic and comparisons. Register homes and disjoint activation frames are allocated with entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under LAYOUT-PLANS: dynamically sized slice payloads in case variants, recursive sum types with indirection layout policies, and custom discriminant niche packing optimizations.
