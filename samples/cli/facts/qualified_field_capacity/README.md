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
`tests/omega/pass/facts/qualified_field_capacity_through_<place>_exit` and
`tests/omega/fail/facts/qualified_field_capacity_through_<place>_rejected`,
the corpus reject name the fact census resolves; the nine reject families were
renamed byte-identically from `_mismatch` and re-recorded under those names.

The distinct compiler mechanism is nominal field membership transport into a
selected call's prerequisites, followed by exact completed-result equations.
These are numeric models, not backing grants. Empty model domains establish no
physical-storage authority.

## Recorded native evidence (macos_arm64)

Measured on `macos_arm64` at revision `19265b33ea18` with the producer built
from that revision (`opt_level 1`):

```sh
python3 tools/corpus_gate.py --native --filter pass/facts/qualified_field_capacity_through
```

Before the record the gate reported all nine positives as `new fixture
(built)`, and the run reported `9 fixtures ({'built': 9})`,
`executed 9 (exit codes {'0': 9})`, 0 rejected without their expected fragment
and 0 fail fixtures that did not reject, so the missing piece was the durable
native row rather than a compiler refusal. The nine rows now read
`built ... exit:0 @19265b33ea18` in `tests/omega/corpus_native_macos_arm64.txt`,
spliced in as a pure addition: +9 lines, 0 deletions, no timeout rows, LOST 0,
and the golden's diagnostic-line count is unchanged at 79 — the native golden is
insertion-only and is not sorted, so a re-render would be a content loss a
key-based check cannot see.

The `--both` golden already carried this fact's nine agreement rows
(`interpreter:agrees`), so native-versus-interpreter agreement was recorded
before this row; what this record adds is the native leg its own acceptance
names. Only `target_body` is target-scoped and its `macos_arm64` body performs
the split and the strategy reconstruction, so the mechanism is exercised on
this host rather than merely compiled for it; the other eight places are
target-neutral.

The record discriminates in both directions. Corrupting an authored
observation in the local positive
(`transition first.strategy.remaining == 32` to `== 31`) says nothing
statically; the program builds and traps, and the gate reports `expected native
0, observed built {'exit': 'signal', 'signal': '5'}` with the structural move
`exit:0` to `exit:signal signal:5` (exit 3). Restoring the fixture
byte-for-byte returned the same filter to clean. Corrupting the capacity
relation instead — the twins' own single-line change, `remaining:
strategy.remaining - length` to `remaining: strategy.remaining` — is refused
before execution with the clause the nine twins pin:
`data `Strategy` literal cannot prove the default domain: a `where` fact is not
established by the values observed at this construction -- establish the
required bounds before constructing the value`.

The nine reject twins were re-verified on the check leg: `clean — 9 fixtures
({'rejected': 9})`, 0 rejected without their expected fragment, 0 fail
fixtures that did not reject.

Under the corpus reject names the nine reject rows were re-recorded at
`595562a958b` with the wave's shared opt-level 1 corpus runner (sha256
`fa7c9b282deaefd7f38eafd3b4ea07d960ad1eb7e679d23253fef05ed8ff30dd`): a
filtered re-run of the nine pins alone reads `clean — 9 fixtures
({'rejected': 9}), 0 rejected without their expected fragment, 0 fail fixtures
did not reject`, and the `target_body` twin keeps its four diagnostics, one per
checked target body. The rename rewrites the nine key lines in place — 9
insertions, 9 deletions, 0 continuation lines touched, diagnostic-line count
unchanged at 4191.

Remaining open on other hosts: `tests/omega/corpus_native_linux_x86_64.txt` and
`tests/omega/corpus_native_windows_x86_64.txt` carry no row for this fact and
need those hosts; `linux_arm64` has no native golden in this checkout.

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
