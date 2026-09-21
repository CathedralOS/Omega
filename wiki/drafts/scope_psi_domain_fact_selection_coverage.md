# Scope: PSI-DOMAIN-FACT-SELECTION-COVERAGE

Scope-verification record for the dispatched name
`PSI-DOMAIN-FACT-SELECTION-COVERAGE` (board stub at ~TASKS.md:13082, mined
unattributed). Audited at `891eb5c5844c` on linux x86-64 (cargo; `mbx`
absent on this host).

## Resolution

The name resolves to the checked-side leg that MODULE-NAMESPACE-RESOLUTION
records as its "(new-scope)" remaining-work item (~TASKS.md:4752):
*enforce predicate-domain establishment on local initializers* — selecting
the right domain/carrier is not proof of membership. The reproducer the
parent row pins is a predicate-only annotation on a `let`:

```
data Choice [copy] { case Empty; case Some(value: u32); }
domain Choice::NonEmpty requires self in Choice::Some;
machine read() -> u32 {
    let value: Choice in Choice::NonEmpty = Choice::Empty;
    value.value
}
```

which `compile_to_checked` incorrectly accepted when
`checks/contracts/writes.rs` only admitted destination facts where
`domain_requires_provenance` held — predicate-only annotations supplied
their own unproved facts. "Selection coverage" names the requirement that
the domain selected by the annotation be *covered* — its membership
obligations demanded and discharged by the initializer's evidence — before
destination facts may be admitted.

## Landed state

The leg landed at `e8bcd8812989` ("psi: enforce predicate-domain
establishment on local initializers"), present at the audit revision:

- `typed-trees-to-checked-trees/src/checks/contracts/writes.rs` — the
  local-initializer domain loop now computes `predicate_domain` from
  `domain.predicate_body.is_present()` alongside `requires_provenance`
  (~:61-103); an initializer that fails `value_proves_qualification` must
  satisfy `initializer_satisfies_predicate_domain` (~:974) or the
  statement rejects with "cannot prove initializer of `x` in `m` is in
  domain `D`; an annotation cannot establish routed qualification".
- The discharge decides `self in T::Case` from the construction's own
  selected case or live `AssignedCase`/guard evidence; member predicates
  read literal field initializers or place snapshots; mutation
  invalidation retires stale evidence; foreign cases/owners fail the exact
  variant-symbol comparison; domain-owned operators qualify their result
  by the selected candidate's `domain_symbol`.
- Pinned in `typed-trees-to-checked-trees/tests/contracts/main.rs`:
  `predicate_domain_initializer_rejects_wrong_case` (the parent's exact
  repro), `accepts_satisfying_case`, `union_membership` (accept member
  cases, reject `Quit`), `member_predicates` (+ nested-partially-true
  reject), `live_evidence` (parameter/call/`AssignedCase` routes),
  `stale_evidence_rejects`, `wrong_owner_rejects` (the parent row's
  "foreign qualified variant" gap — `Other::Some` under `Choice::NonEmpty`
  fails the owner check, and `Other::Occupied` rejects `Other::None`).

## Open legs

None under this name. Remaining predicate/domain work belongs to the
parent row's other legs (unmanaged source-map package commitments;
lexical/package selection for generic type-scoped constant attachment
heads and declared-domain case facts; constrained-constant discharge
extensions), all recorded on MODULE-NAMESPACE-RESOLUTION, and the
`crash_entry_values` shared-evidence leg on CRASH-CONTRACT.

## Slice verdict

No independent slice exists under this name — the mapped surface is
landed, test-pinned, and green at the audit revision. The stub should fold
into MODULE-NAMESPACE-RESOLUTION (or be retired) rather than dispatch a
new item.

## Fences observed at audit time

Live claims adjacent to — but not covering — this surface at
`891eb5c5844c`: `typed-trees-to-checked-trees/src/execution/unit` under
PROVIDER-ATTACHMENT-MACHINE-PLAN (~09:49Z) and
CANARY-RUNTIME-LITERAL-DISPATCH-EXIT (~14:32Z); `facts/field_domain.rs`
itself unfenced. Re-check `tools/claims.py status` before any code leg.
