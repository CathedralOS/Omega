# Delta request-boundary gate

Run from the repository root:

```sh
sh tests/delta/request-boundary/run.sh
```

The gate materializes the canonical Gamma entry and shared implementation from
their ordered source manifest, then executes them with the selected
Beta-authored Gamma evaluator. Python only frames requests, invokes that
evaluator, and compares observations; it does not parse or compile Delta.

The retained controls check:

- every fixed-header truncation point and each magic/version/reserved byte;
- complete-header admission before header contents, the first incorrect header
  byte before profile selection, and profile selection before source provision;
- profile 0, retired profile 2, and unknown multi-byte profile IDs;
- exact and adjacent declared source provision, including an unsigned maximum
  declaration with no body;
- the first missing or trailing body coordinate before source processing;
- strict refusal of raw Delta source at the canonical entry;
- exact 40-byte DCOUT failures, including status agreement, reserved zeros,
  little-endian fields, and unused-field normalization; and
- repeated byte-identical successful compilation followed by empty, textual,
  and binary input/output observations of the generated ConformanceBytesV1
  application.

The full exact-size source-body control deliberately begins with an invalid
source byte. Its evaluator-owned failure proves that DCREQ admission completed;
it does not claim complete frontend conformance at the 4-MiB source maximum.
The adjacent full-size body instead produces the exact source-provision frame.

Request failures use Reject code 1 `malformed_request`, Reject code 2
`unknown_profile`, or Incomplete resource 1 `source_bytes`, all in DCREQ
coordinate space 4. These follow the common frame and bounded order fixed by
[D30/D33](../../../wiki/architecture/bootstrap_chain/decisions.md#d33--dcout-admission-and-schema-diagnosis-are-bounded-and-total)
and the [current Delta boundary contract](../../../bootstrap/delta/LANGUAGE.md#compiler-boundary-family).
The compiler owns the serialized constants; the gate's expected bytes are
comparators, not a runtime metadata input.

Frontend and profile-schema failures are still unfinished DCOUT paths. Their
current status 249 with empty stdout remains an evaluator-owned failure, not a
canonical compiler rejection or a ConformanceBytesV1 application observation.
The gate pins that distinction without translating traps into guessed reasons.
It does not close the Delta compiler edge, its resource conformance, or later
failure publication.
