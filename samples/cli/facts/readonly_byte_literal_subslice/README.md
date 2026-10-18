# Read-only byte literal subslice window preservation

// Fact: a checked subslice of an immutable byte-view literal preserves its exact window length of 10u64 when multiplied by 10u64 and subtracting 100u64.

Every place establishes an immutable byte-view literal of 14 bytes (`"0123456789ABCD"`), takes a checked 10-byte subslice window `[2u64..12u64]` satisfying ordered proof legs `start <= end` and `end <= length`, observes its exact `.len` (`10u64`), transports that subslice or window length across the place, scales by `10u64` to `100u64`, and executes exact subtraction `total - 100u64` returning `0`.

| Place | Transport |
| --- | --- |
| `local` | Local variable binding |
| `field` | Record field storage |
| `parameter` | Parameter passing with required contract |
| `call_result` | Callee invocation returning subslice length |
| `match_join` | Pattern match branch convergence |
| `const_name` | Named constants defining subslice window bounds |
| `other_state` | State machine transition forwarding length |
| `other_machine` | Forwarding machine evaluating the product |
| `target_body` | Target-scoped machine bodies on hosted targets |

Run each positive place with:

```sh
omega run --both samples/cli/facts/readonly_byte_literal_subslice/<place>/main.omg
```

Each place has an `invalid/` twin that takes a 9-byte subslice (`[2u64..11u64]`), causing the product `90u64` to underflow unsigned exact subtraction `total - 100u64`. Each invalid twin rejects during proof checking with `may overflow \`u64\``. Check each twin with:

```sh
omega --check samples/cli/facts/readonly_byte_literal_subslice/<place>/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `607450e1cd264d4f836091eeaf41f295`.
- **Requirement established:** End-to-end nine-place fact row preservation for immutable byte-view literal subslice window bounds, length folding, and exact arithmetic subtraction.
- **Obligations accounted for:**
  1. **Establishment:** The 14-byte literal `"0123456789ABCD"` establishes an immutable byte-view backing with exact length 14. The subslice operation `bytes[2u64..12u64]` proves both range-formation obligations `2u64 <= 12u64` and `12u64 <= 14u64`, producing a valid 10-byte window descriptor.
  2. **Identity and lifetime:** The byte literal resides in the immutable read-only image literal pool; its descriptor borrows immutable backing that outlives every function activation. Window length `10u64` is an invariant property of the validated subslice and is conserved across all 9 places.
  3. **Composition:** Callee calls, record field loads, match branch joins, named constants, state transitions, machine forwarding, and target-scoped bodies compose the exact 10-byte window length and discharge caller exact arithmetic obligations without loss of evidence.
  4. **Soundness boundaries:** Breaking the subslice length in the invalid twins (`[2u64..11u64]`, length 9) fails the exact subtraction obligation `90u64 - 100u64` and strictly rejects with `may overflow \`u64\`` across all 9 places.
  5. **Terminal Psi and carrier lowering:** Source constructs lower into Terminal Psi `ByteSequenceLength` and `ByteSequenceSubslice` with explicit proof legs `start <= end` and `end <= length`. Descriptors carry base pointer and extent through the structural lane.
  6. **Native realization, frame/layout encoding, and image emission:** The backend `image-emission` crate places the addressed byte literal in the read-only byte pool following `.text`, resolves PC-relative relocations across PE, ELF, and Mach-O image formats, performs subslice pointer/length arithmetic, and achieves bit-for-bit semantic agreement between Terminal Psi and native execution (`omega run --both`) exiting 0.
- **What remains open:** Dynamic runtime-indexed byte subslice mutable projections, non-literal slice views under content mutation, and inter-package dynamic slice provider composition.
