# Fixed representation storage

Owning task: `LIBRARY-REPRESENTATION-FAMILIES`, supporting the layout-policy prerequisite of `BUILD-OWNED-TARGET-SELECTION`.

Fact: An ordinary FixedLayout<Pair> application preserves the established Delivered fact on Pair.lo, so reading lo and subtracting 42u64 remains justified through each storage transport and execution uses the authored lo-at-8 representation.

Every place carries the actual `LayoutStorage<Pair, PairLayout>` value, reads its established field domain, forms `value.lo - 42u64`, and checks the authored byte placement (lo at 8, hi at 0). A twin changes only the establishing write from 42 to 41. The target-body place uses ordinary implementations selected by Build and separately observes provider markers 17 versus 29.

Canonical contracts: [layout applications](../../../../wiki/spec/layouts/plans.md#representation-applications), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), and [ownership](../../../../wiki/spec/terminal-psi/ownership.md). Policies receive authorized schema and explicit target facts; the returned plan supplies neither validation nor storage authority. Exact semantic type, selected implementation and field identities remain distinct from physical geometry.

The nine places and reject twins are local, field, parameter, call_result, match_join, const_name, other_state (nested machine), other_machine and target_body. They are mirrored under `tests/omega/{pass,fail}/facts/fixed_layout_storage_through_*`. Each positive declares exit zero and binds all four required hosted targets plus macOS x86-64.

Qualification is in progress. The fixed main baseline lacks the canonical API. A separately frozen usable migration producer at 8b4b6ec49266 wrongly refuses all nine positives because pre-check policy evaluation receives no explicit target facts; eight twins stop at that earlier refusal and only const_name reaches its intended bound error. Those are baseline observations, not acceptance.

The row does not close the parent tasks: lifetime/cleanup compositions, complete policy migration, foreign ABI agreement, alias and conversion families, portable artifact controls, independent review and verified publication remain open until demonstrated.
