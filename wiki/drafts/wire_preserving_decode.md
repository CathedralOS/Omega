# Preserving decode and remainder custody — design

Board row: `TASKS.md` `**EXTERNAL-DATA-SCHEMA-CONVERSION.**` residual —
"no codec publishes preserving decode (`PreserveUnknown` demands are always
unsatisfiable); the preserving-decode mode and its remainder custody are the
next slice". This draft fixes the mode's shape; it authorizes the slices
enumerated in "Implementation surfaces" and nothing else.

Spec anchors:
[unknown members and remainder custody](../spec/layouts/codecs.md#unknown-members-and-remainder-custody),
[compact binary policy](../spec/layouts/codecs.md#compact-binary-policy),
[the codec contract file](../spec/layouts/codecs.md) generally.

Delete when the preserving-decode mode and its demand/report surface land,
or when the board retires the `PreserveUnknown` leg.

## Landed surface

- Carriers and requirements (`source/library/std/wire.omg`):
  `OpaqueWireRemainder { codec_identity: u64, bytes_and_ordering: &[u8] }`,
  `Relayed<T> { value: T, remainder: OpaqueWireRemainder }`,
  `PreservingDecode<Policy, Value>::decode_preserving(bytes: &[u8]) -> DecodeResult<Relayed<Value>>`,
  alongside `StrictDecode`, `ProjectingDecode`, `Encode`, `DecodeResult`.
  `compact_binary` itself stays strict; the legacy synthesized
  `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` out-param
  surface is transitional and remains strict-only.
- Admission detection
  (`build-evaluation/src/admission/wire_protocol.rs:428`
  `published_preserving_decode`): a `PreserveUnknown` demand is satisfied by
  any authored machine conformance `PreservingDecode<_, Value>::decode_preserving`
  whose `Value` argument is the local schema's value type; otherwise the
  demand reports `compact_binary publishes strict unknown-member behavior`
  or `codec {codec} publishes no preserving behavior`.
- Executable witness: pass canary
  `tests/omega/pass/wire/wire_preserving_decode_relay_exit` — an authored
  `preserving_decode` machine satisfying
  `PreservingDecode<PreservePolicy, LocalMessage>::decode_preserving`,
  binding the unknown tail into a zero-copy borrowed `OpaqueWireRemainder`
  under `codec_identity: 7`; runtime-verified (exit 70).

## Decode mode semantics

Preserving decode is a third normalized requirement, not a decoder flag
(spec: "distinct normalized requirements, not an unrecorded decoder
option"). The decode mode is therefore part of the codec policy's authored
surface — the schema selects it through the policy type parameter, and no
generated or selected codec may silently switch modes at runtime.

For a preserving realization under the `compact_binary` grammar:

- The frame still opens with the era discriminator varint, then fields in
  increasing stable-identity order. The decoder validates every member the
  selected era declares — malformed values, truncation, range violations,
  and noncanonical encodings still fail-closed to `DecodeResult::Invalid`,
  unchanged from strict decode's authority over what it validates.
- Members the local era does not declare (unknown tags, and members of
  unknown eras the policy elects to preserve rather than reject or
  negotiate) are not discarded: the decoder captures each unknown span's
  exact bytes plus its splice position relative to the known members —
  the relay-ordering sidecar — into `OpaqueWireRemainder.bytes_and_ordering`.
  The sidecar is codec-private; its layout is the realization's own choice
  and is never part of the schema report.
- `codec_identity` binds the producing realization's schema and decode
  policy jointly. For generated codecs the natural identity is the
  schema's `normalized_schema_report_identity` folded with the policy
  spelling: the fingerprint folds member identities, types, and retired
  rows but no policy (`typed-trees/.../names/identity.rs`), so two
  policies over one schema would otherwise collide. A remainder's splice
  positions are defined only relative to the known-member set the
  producing policy elected, so relaying it through the same schema under
  a different policy must reject.
- `Relayed<T>.value` carries the fully validated known value; ordinary
  facts about `T` describe only it. The remainder is semantically
  uninterpreted — opaque means uninterpreted, not confidential or
  unforgeable (wire.omg contract).

## Remainder custody

Two lawful carriers, matching the spec's custody rules:

- **Zero-copy** (the canary's form): `bytes_and_ordering` borrows the input
  buffer, so the `Relayed` value retains the input-buffer loan for its
  whole live extent. No allocation obligation; the remainder cannot
  outlive the source frame.
- **Owned copy**: the realization allocates and owns the remainder bytes;
  the allocation and its release are explicit resource obligations of the
  owning package, and the `Relayed` value carries no input loan.

A realization declares one carrier per conformance; mixing borrowed and
owned remainders across one `OpaqueWireRemainder` spelling is rejected —
the carrier's loan is part of the decoded value's custody, not a
per-call choice.

## Relay splice (encode side)

Preserving decode exists for relay. Re-encoding a `Relayed<T>` through the
same codec realization:

- Verifies `remainder.codec_identity` against the realization's own
  identity before consuming the remainder; a remainder produced by another
  codec realization rejects rather than emitting a corrupted frame.
- Re-inserts the retained unknown bytes at their recorded positions so the
  emitted frame keeps canonical increasing-identity order across the union
  of known and unknown members. Edits the consumer made to `value` apply
  normally; the remainder is spliced byte-exact.
- The encode entry that accepts `Relayed<T>` is a distinct requirement from
  `Encode<Policy, Value>` (which takes `&Value` and no remainder); a codec
  publishing preserving decode without the relay entry is a valid strict
  producer but cannot satisfy a demand that requires the splice.

## Demand/report surface

`PreserveUnknown` satisfaction already joins
`published_preserving_decode`. Two report fields stay strict-hardcoded and
become mode-aware when a preserving codec is selected:

- `unknown_member_behavior` (currently the literal `"strict"`) names the
  realized behavior — `"preserving"` for a conformance carrying a relay
  entry, `"projecting"`/`"strict"` otherwise.
- The synthesized row's `codec_requirement`/`encode_requirement` strings
  name `PreservingDecode<compact_binary, T>` / the relay-encode
  requirement when the generated codec publishes the mode, with the
  requirement report identities re-derivable from
  `normalized_schema_report_identity` as today.

## Rejections (fail-closed)

- Remainder with a foreign or zero `codec_identity` on encode.
- Remainder bytes/order mutated between decode and re-encode (detected at
  splice validation, not trusted).
- A preserve request for an era the policy did not authorize — "reject,
  preserve, negotiate" is an explicit decode-policy choice per era, and a
  wildcard match does not prove a migration route exists.
- `PreserveUnknown` on an edge whose local schema has no `PreservingDecode`
  conformance for its value type — the existing unsatisfiable verdict.

## Implementation surfaces (not this doc's edit scope)

- `psi/semantics/validation/src/value_custody/wire/decode_call.rs`:
  validation of the `decode_preserving` call shape (argument count,
  `Relayed` destination typing, borrowed-remainder loan binding).
- `psi/semantics/checked-interpreter/src/interpreter/evaluator/wire_codec.rs`:
  the evaluator decode path gains unknown-span capture + sidecar emission
  for the preserving mode.
- `build-evaluation/src/admission/wire_protocol.rs`: mode-aware
  `unknown_member_behavior` and requirement strings.
- `source/library/std/wire.omg`: only if the relay-encode requirement gets
  a spelled trait (e.g. `EncodeRelayed<Policy, Value>`) — today the splice
  contract rides inside the codec-private sidecar.

## Decisions and open questions

Resolved while drafting:

- `codec_identity` binds `policy ⊗ schema`, not the schema's report
  identity alone — `normalized_schema_report_fingerprint` folds member
  identities, types, and retired rows but no policy spelling, and a
  remainder's splice positions are defined only relative to the
  known-member set the producing policy elected. Under today's generated
  codec (single `compact_binary` spelling) the joint identity is
  observationally equal to the report identity; the fold exists so a
  second policy on one schema cannot silently share a remainder.
- The first preserving realization is an authored machine conformance:
  `published_preserving_decode` already joins it and the relay-exit
  canary exercises the form end to end. A synthesized
  `decode_preserving` entry on the generated `compact_binary` codec is a
  separable follow-up, not a prerequisite — it needs a deterministic
  per-schema sidecar layout (codec-private per realization today), a
  spelled relay-encode requirement, and the mode-aware report fields.
  It also shifts trust class: an authored or generated body checked
  against the public requirement is Derived, while
  generator-as-correct-by-construction is Admitted naming the compiler
  (codecs.md realization table).

Open:

- None blocking the authored route. The synthesized entry — if elected —
  must answer whether independently compiled producer/consumer pairs
  agree on the fixed sidecar layout across toolchain versions, or whether
  the splice is constrained to same-realization relays only.
