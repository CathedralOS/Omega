# Omega D outcome machinery

This gate exercises the OCOUT outcome side of the
[standalone compiler request](../../../wiki/spec/build/compiler_request.md)
inside the complete manifested Epsilon-written compiler D. The
[customer](main.epsilon) is appended to the packed closure and interpreted
through the selected chain:

```text
Gamma-written evaluator -> Delta compiler -> Epsilon evaluator -> D + customer
```

It verifies, with exact bytes, that D's embedded contract projections and
canonical failure-frame encoder behave as the request contract assigns:

- every scalar-resource code's selected limit and coordinate space, the
  parser-resource projection onto wire codes 3..11, the lexical-diagnostic
  projection onto `Reject` codes 2..9, and every assigned
  `Reject`/`InternalFailure` code-to-space pair;
- exact 40-byte and 48-byte canonical-source OCOUT frames for one Reject, two
  resource `Incomplete`, and one `InternalFailure` outcome;
- refusal of unassigned tags, unassigned codes, illegal code/coordinate-space
  pairs, negative or trailing fields, nonzero scalar fields on non-resource
  outcomes, non-table limits, and nonzero ordinals outside space 4 — with the
  record left unpublished after each refusal;
- the bounded publication sum, including exact, saturated, and defect
  results;
- the phase-1 declared-extent provision check on the framed request envelope;
- the outcome tuples the scalar compilation paths record and their canonical
  publication: a source-anchored `duplicate_name` `Reject`, a literal
  `integer_literal_out_of_range` `Reject`, an unanchored `missing_entry`
  `Reject`, `unterminated_string_literal` and `invalid_utf8` lexical
  `Reject`s — each encoded to and emitted as its canonical frame — plus a
  source-anchored coverage `Incomplete` that stays unpublishable and the
  untouched tuple after `Complete`, which likewise cannot encode a frame;
- the OCREQ V1 subject/invocation field/tag shape pass (phases 0, 2, and 6)
  over a canonical single-package request, and its `malformed_request` tuple
  for an unassigned product tag — encoded successfully by the frame writer,
  proving the decoder's produced tuple is one the encoder accepts.

The expected observation is [expected.hex](expected.hex), computed from the
assigned tables rather than captured output. This is a development observation
over the private Epsilon execution envelope, not the sealed request edge; no
Rust compiler, host parser, host typechecker, or host code generator supplies
the program's meaning.

From the repository root on macOS arm64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/omega-outcome/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed, and the
existing shell tools; macOS also requires `codesign` for the materialized
evaluator. Outputs live in ignored `build/omega-outcome/`.
`OMEGA_OUTCOME_OBSERVATION_SECONDS` overrides the default 14,400-second
customer watchdog, `OMEGA_OUTCOME_RECEIPT_SECONDS` overrides the default
1,800-second receipt-reconstruction watchdog, and `OMEGA_OUTCOME_BUILD_DIR`
selects a different output directory; these are host controls, not language
semantics.

## Bound customer entry

The [customer](main.epsilon) entry is bound at 18,230 bytes, SHA-256
`c92fdbd62f7933859922481c021b951ffb01baec7efee5d4ee8f72c9f3d8ca4d`, and packs
on top of the bound member closure to 576,295 bytes, SHA-256
`517bee1ce9b180a993eb1a90e6d32e997afb08e8d4ba8c945d7637ceb355b919`.
`tools/bootstrap/omega/compiler_env.sh` checks the entry identity before every
packing and `tests/bootstrap/omega-identity.sh` covers the refusals. The same
pins stand inline in `gate.py`; they are records of this one subject, not
independent identities.
