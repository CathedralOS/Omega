# Qualified field capacity

Fact: a qualified field supplies a split contract that preserves the exact remaining-capacity relation.

Parent: `BUMP-ALLOCATOR-CANARY`. This row repairs the proof transport needed to
bind a bump strategy's remaining count to its concrete tail length. Previously
the qualified-field split lost its postconditions, although the otherwise
identical unqualified model executed successfully.

Each place starts with 48 units, splits 16, reconstructs a strategy with 32
units remaining and observes both the residual and allocated lengths. Run
`omega run --both samples/cli/facts/qualified_field_capacity/<place>/main.omg`.
The nine places are `local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.
Every `invalid/main.omg` differs only by retaining the original remaining
count instead of subtracting the request; the default-domain obligation must
reject it. The same sources are pinned under
`tests/omega/{pass,fail}/facts/qualified_field_capacity_through_*`.

The distinct compiler mechanism is nominal field membership transport into a
selected call's prerequisites, followed by exact completed-result equations.
These are numeric models, not backing grants. Empty model domains establish no
physical-storage authority.

## Requirements and evidence

- Establishment: `Strategy` requires `remaining == tail.length`; construction
  must prove it. Qualifying the tail does not change its length.
- Identity and lifetime: admitted state parameters and completed call occurrences
  supply exact domain instances; local annotations and pending constructions
  cannot prove themselves. Existing stale-entry and returned-reference controls
  cover invalidated observations; this row uses owned fields.
- Composition: the same 48-to-16-to-32 split crosses all nine places. Both join
  arms execute. The request is a named constant in `const_name`.
- Soundness boundaries: all nine twins retain 48 instead of 32 and reject.
  Separate bundled-library controls reject an inflated counter and a split
  larger than its actual tail.
- Checked-to-Terminal Psi (Psi 05–07): the checked qualification and arithmetic
  relation reach ordinary record and call lowering, with proof erasure and
  actual Terminal interpretation. This model does not exercise affine cleanup,
  borrow restoration, dynamic dispatch, or a backing provider.
- Provider settlement, layout and ABI (Omega 00–02): each Build binds
  ProgramEntry for all four hosted targets. Record arguments and results use
  ordinary aggregate layouts and call plans; the host run verifies their
  observable field values.
- Instruction selection and activation frames (Omega 03–06): calls, record
  projections, subtraction, state edges and live results execute natively.
  No new target-specific realization path is introduced.
- Encoding and native emission (Omega 07–09): macOS arm64 native execution and
  Terminal interpretation both return zero for all nine positives. Other
  target bodies are source-checked; execution on those hosts is not claimed.

The bundled `alloc::bump` canary also checks the actual library contract:
remaining capacity equals tail length, and partition requests must fit that
length. Its abstract provider is not a real allocator demonstration.
Actual backing admission, initialization, placed reads, retirement and exact
vacant return remain unfinished parent acceptance.
