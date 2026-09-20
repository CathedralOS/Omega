# Backend vocabulary rejection audit

Sweep of every wire-decode point under `omega-rust/omega/backend/` at
`22e6066e9f` where a byte-level marker, kind, tag, or schema field maps onto a
closed vocabulary. For each surface the audit asks two questions: does every
non-admitted value reject with a named diagnostic (no `unreachable!`, no
silent acceptance), and does a test exercise that rejection?

## Method

Enumerate each decoder in scope: every `match`/`from_tag` on wire bytes, every
fixed-magic or version check, every closed-enum reconstruction in
`decode_*`/`from_bytes`/`from_decoded_*` paths. Then locate the test that
proves rejection: a byte-mutation case, a field-substitution matrix, or a
named negative test. Gaps are closed either by a new negative test (when the
rejection already exists) or by adding the rejection itself.

## Catalog

### `component-description` — `component_description.rs`

`decode_component_description` enforces a fully canonical frame: magic,
length-bounded embedded Terminal artifact, then ordered rosters. Closed
vocabularies and their rejection labels:

| Wire element | Rejection | Coverage |
| --- | --- | --- |
| `DESCRIPTION_MAGIC` | `InvalidMagic` | `component_verification/tests.rs` `wire::magic` |
| overall length > `MAX_COMPONENT_DESCRIPTION_BYTES` | `RosterBoundExceeded("description byte length")` | **added** (`oversized_descriptions_reject_before_decoding`) |
| schema `u32` | deferred by design: `verify_component` rejects via `IncompatibleSchema` | `rejects_an_incompatible_schema` |
| frontier tag `u8` (1–3) | `UnsupportedTag("frontier tag")` | `wire::frontier-tag` |
| embedded artifact | `Corrupt("embedded artifact did not decode")` | `embedded artifact` case + terminal-codec's own `every_noncurrent_format_and_vocabulary_marker_rejects` |
| roster counts (every roster) | `RosterBoundExceeded(name)` | `wire::import-roster-bound` + shared `roster_len` |
| identity strings (len + UTF-8) | `IdentityInvalid("too long" / "not utf-8")` | "too long" existing; **"not utf-8" added** (`non_utf8_identities_reject`) |
| roster order (9 sites) | `NonCanonicalOrder(name)` | existing per-roster battery |
| entry kind `u8` (1–7) | `UnsupportedTag("entry kind")` | `wire::entry-kind-tag` |
| entry evidence `u8` (0/1) | `UnsupportedTag("entry evidence")` | `wire::entry-evidence-tag` |
| authority class `u8` (1–3) | `UnsupportedTag("authority class")` | **added** |
| authority evidence `u8` (0–3) | `UnsupportedTag("authority evidence")` | **added** |
| service coordinate 0 | `Corrupt("zero service identity")` | **added** (`zero_service_identity_in_a_bound_rejects`) |
| custody kind `u8` (1–8) | `UnsupportedTag("custody kind")` | **added** |
| custody evidence `u8` (0/1) | `UnsupportedTag("custody evidence")` | **added** |
| obligation kind `u8` (1–5) | `UnsupportedTag("obligation kind")` | **added** |
| realization presence `u8` (0/1) | `UnsupportedTag("realization presence")` | **added** |
| trailing bytes / truncation | `Corrupt("trailing bytes" / "truncated …")` | `wire::trailing` / `wire::truncated` |

New coverage lives in `component_description/tests.rs`. The
`every_closed_wire_vocabulary_rejects_a_non_admitted_tag` sweep mutates each
envelope byte of a populated canonical fixture to a non-admitted tag value and
asserts the complete label set — a future tag field that forgets its
rejection fails the test by absence, not by a pinned position.

### `executable-installation` — `container.rs`, `container_bytes/decoding.rs`

`decode_executable_container` covers the full closed vocabulary: fixed magic,
reserved-zero fields, `format_marker` `u16` (v1/v2 only), canonical header
length, declared-vs-actual total length, bounded section count, canonical
directory offset, `architecture` tag (1/2), section `flags` (bit 0 only),
section `kind` `u16` with required/optional rules per kind and per format
marker, unique-kind tiling, informational-identity recomputation, per-kind
payload records (relocation `kind` 1–5, relocation `target_kind` 1/2,
placement `phase` 1–3, presence flags 0/1 for range/regime/scope), entry
alignment, and relocation/entry count bounds.

Coverage: `container_bytes/tests.rs`
`executable_container_wire_rejects_every_one_field_substitution` plus targeted
cases (`stale_container_marker_bytes_reject`,
`malformed_counts_unknown_required_and_identity_drift_reject`, truncation /
overlap / reserved-bit cases) hit every one of these labels. **No gap.**

### `native-artifact` — `callable_entry/codec.rs` + `model.rs`

`OptimizedOrdinaryCallableEntry{Record,Manifest}::decode` cover magic,
version, stage tag, hardening tag, architecture, object format, scalar/integer
type tags, calling-policy, register (tag,index) pairs, exit policy, entry
assumption, disposition, unavailable status, lengths, UTF-8, trailing bytes,
and recomputed identity.

Coverage: `compiler/tests/callable_entry_custody.rs` exercises every decode
error variant in a one-field-substitution matrix, including the retired
leaf-only exit-policy tags. **No gap.**

### `machine-emission` — `function_realization/codec/`

`FunctionRelativeOptimizationRealizationManifest::decode` covers magic,
version (12), stage, selected-lowering completion status, x86 branch
relaxation status, post-allocation optimization status and kind, action-count
overflow, architecture, object format, layout policy, scope, frame
disposition, unavailable status, trailing bytes, and recomputed identity.

Coverage: `compiler/tests/realization_custody.rs` maps every
`ManifestDecodeError` variant to a mutation case. **No gap.**

### Out of scope / no wire decode

- `native-artifact` `native_artifact.rs` and `dynamic_elf.rs`: post-decode
  validators and producers — no byte-vocabulary admission of their own.
- `component-candidate`: thin producer calling `decode_component_description`.
- `images/image-*`: encoders and in-memory model validation, not byte
  decoders; origin vocabularies are compile-time enums.
- `layout`, `register-environment`, `plans/*`, `machine-services`, ABI and
  calling-convention crates: no wire decoders found.
- ISA `selected_form_encoding` decoders, `object/object-file` codecs,
  `external-roots`, and `component_verification.rs` sit under sibling claims
  at audit time; each already routes unknown values through named diagnostics.

## Findings

1. **component-description tag coverage gap (fixed here):** six vocabulary
   tags plus the UTF-8 identity check, the zero-service coordinate, and the
   description byte-length bound had no decode-level rejection test. All
   rejections already existed; the new `component_description/tests.rs`
   sweep pins them.
2. **Adjacent observation (not fixed, not a backend surface):** a single-byte
   corruption inside the *embedded Terminal artifact* can make the
   `terminal-codec` decoder attempt a ~400 GB allocation (a roster count is
   trusted for `Vec::with_capacity` before bounds validation). The Psi-side
   codec is outside this audit's claimed paths; worth a follow-up board item
   for allocation-before-validation in terminal-codec readers.

## Evidence

- `cargo nextest run -p component-description --lib` — 23/23 pass.
- `cargo clippy -p component-description --all-targets` — clean.
- `cargo fmt --check` — clean.
