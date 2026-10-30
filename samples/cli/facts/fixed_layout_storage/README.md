# Fixed representation storage

Owning task: `LIBRARY-REPRESENTATION-FAMILIES`, supporting the layout-policy prerequisite of `BUILD-OWNED-TARGET-SELECTION`.

Fact: An ordinary FixedLayout<Pair> application preserves the established Delivered fact on Pair.lo, so reading lo and subtracting 42u64 remains justified through each storage transport and execution uses the authored lo-at-8 representation.

Every place carries the actual `LayoutStorage<Pair, PairLayout>` value, reads its established field domain, forms `value.lo - 42u64`, and checks the authored byte placement (lo at 8, hi at 0). A twin changes only the establishing write from 42 to 41. The target-body place uses ordinary implementations selected by Build and separately observes provider markers 17 versus 29.

Canonical contracts: [layout applications](../../../../wiki/spec/layouts/plans.md#representation-applications), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), and [ownership](../../../../wiki/spec/terminal-psi/ownership.md). Policies receive authorized schema and explicit target facts; the returned plan supplies neither validation nor storage authority. Exact semantic type, selected implementation and field identities remain distinct from physical geometry.

The nine places and reject twins are local, field, parameter, call_result, match_join, const_name, other_state (nested machine), other_machine and target_body. They are mirrored under `tests/omega/{pass,fail}/facts/fixed_layout_storage_through_*`. Each positive declares exit zero and binds all four required hosted targets plus macOS x86-64.

Exact optimized producer `bc022a133a9ab10d6ba88e5eb9fa893ee72e8ad2` observed all nine positives exiting zero in both native and Terminal execution, with all nine Delivered=41 twins rejecting at their intended bound. The generic and imported [named-constant compositions](const_name/README.md) also pass on their pinned paths, with exact policy-argument and private-policy controls rejecting. Final integrated qualification and publication remain in progress. The earlier frozen migration producer `8b4b6ec49266` refused the positives before checking the intended fact because pre-check policy evaluation lacked explicit target facts; those historical results remain baseline evidence.

The row does not close the parent tasks: lifetime/cleanup compositions, complete policy migration, foreign ABI agreement, alias and conversion families, portable artifact controls, independent review and verified publication remain open until demonstrated.
