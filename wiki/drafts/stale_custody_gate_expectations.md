# Stale custody-gate expectations

Status: re-verification ledger for board item STALE-CUSTODY-GATE-EXPECTATIONS.
The item's question was whether the custody-gate expectation slice left by
TERMINAL-SOURCE-CUSTODY-GATE-ORDER went stale. It did not: the custody-named
fail fixtures still reject with their pinned diagnostic fragments. This note
authorizes no fixture or expectation change.

Affected subjects: [fail-canary corpus](../../tests/omega/fail/) and
`fail_canaries_reject_with_expected_diagnostic_fragment`
(`compiler/tests/canary_suite`).

## Re-verification

At `e8c29138ff` on linux x86-64:

```text
OMEGA_FAIL_CANARY_FILTER=content_retained_custody_from_borrow,placement_custody_wrong_arity_rejected,bump_allocator_cast_minted_resident,bump_allocator_cast_minted_vacant,quotient_routed_carrier_content_rejected \
  cargo nextest run -p compiler --test canary_suite \
    proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment
```

Result: PASS — all five custody-named fail fixtures reject with their pinned
fragments, matching the item's recorded verification at `10d93dd448d`. The
landed custody ordering repins no custody-gate expectation.

## Boundary of this row

The stale-expectation census the row measured (12 drifted canaries, 3 silent
acceptances including `calls/machine_self_call_recursion_rejected`) is
RC-DIAGNOSTICS-GATE's named lane — repinning drifted diagnostics and
investigating silent acceptances belongs to that fence, not this row. The
unrelated fail-fixture roster drift (6 unregistered fixtures, `roster.rs`
inventory check red at base) is likewise inventory work outside the
custody-gate question.

No custody-specific stale-expectation slice remains on this row.
