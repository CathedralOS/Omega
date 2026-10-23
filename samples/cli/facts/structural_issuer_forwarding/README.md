# Named requirement issuance through a forwarding wrapper

An exact issuer establishes Token::Issued, and forwarding preserves it for consumption.

This row belongs to **DOMAIN-ISSUER-ROUTES**. An explicitly selected named
implementation supplies `Issuer::issue`; a generic evidence binder, forwarding
wrapper and consumer carry its structural result. The consumer requires
`Issued` before reading the payload. Its later equality guard checks the runtime
oracle and supplies no provenance.

The original [fixed witness](requirement_probe/main.omg) has SHA-256
`494fd0ec6a3330b184ece45a9a25bc271be889b21a012f960a6b476eab7efc33`.
Baseline `d6cf554fd1cbf20551983d29bae26f5cf3b81664` rejects its named issuer's
return and construction, and the generic wrapper's call-result qualification.
The witness and its exit-zero oracle remain unchanged through the repair.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each has an
`invalid/main.omg` changing only the authorized requirement from `Issuer::issue`
to `OtherIssuer::issue`. The same member spelling and carrier signature grant
no authority. Both match alternatives execute. The constant place names the
payload, not a reusable authority token. Every positive binds all four hosted
targets. The row is pinned under
`tests/omega/{pass,fail}/facts/structural_issuer_forwarding_<place>`.

[Selection](selection/main.omg) additionally exercises a generic named map's
captured body and two explicitly selected implementations, observing 17 and 23.
Focused controls under `tests/omega/fail/domains/named_issuer_*` challenge
equal-field reconstruction, independent predicates, a different requirement of
the same trait, and the issuer body's own call preconditions.

Run with a freshly built compiler on the actual host:

```text
python3 samples/cli/facts/structural_issuer_forwarding/validate.py --cli target/debug/omega --output build/issuer-row --controls
```

The validator requires native and Terminal exit 0 for every positive and the
authored diagnostic for every negative. It retains raw logs and hashes the
generated native image, canonical Terminal product and companion outputs.

## Requirements

- Establishment: exact requirement identity authorizes the selected named
  realization's result boundary. Carrier validity, predicates and custody remain
  independent obligations; the result annotation grants nothing to the body.
- Identity and lifetime: exact declaration handles and retained invocation/capture
  ownership identify the issuer. An equal reconstructed token lacks provenance.
- Composition: fields, parameters, calls, joins and state transfers preserve the
  same qualification. Generic calls rejoin the selected public contract rather
  than treating the rewritten executable target as the public declaration.
- Soundness boundaries: wrong-trait and wrong-member controls reject, and neither
  result establishment nor forwarding proves an independent predicate or the
  body's call precondition. Boundary admission remains separate.
- Psi 05–07: ordinary structural calls, results and state edges preserve checked
  membership into canonical Terminal Psi and its interpreter. No special issuer
  operation or runtime tag is introduced.
- Omega 00–02: hosted entry selection, record layout, and argument/result calling
  plans carry the token and observed payload. No provider backing or geometry
  claim is made.
- Omega 03–06: ordinary instruction selection, value homes and activation frames
  transport the payload through calls and both control alternatives.
- Omega 07–09 and emission: the emitted native image must agree with Terminal
  execution. Execution evidence is restricted to the actual Linux x86-64 host;
  binding other targets does not claim their execution.

## Validation frontier

All nine places, the unchanged original and the two-selection variation executed
at native exit 0 and Terminal exit 0 on Linux x86-64. All nine wrong-trait twins
and four focused controls rejected for their authored reasons. This bounded row covers private
same-project ordinary requirements and named implementations. It does not close
the parent capability: cross-package private routes, admitted provider issuance,
protected Vacant/Resident backing, selected payload grants and other hosted
execution remain outside this advance.
