# Fixed representation storage

Owning task: `LIBRARY-REPRESENTATION-FAMILIES`, supporting the layout-policy prerequisite of `BUILD-OWNED-TARGET-SELECTION`.

A selected layout must keep the established value of Pair.lo through storage transport. These programs test the same exact 42 constraint and observe the policy's chosen byte positions: lo at 8, hi at 0.

Every place carries the actual `LayoutStorage<Pair, PairLayout>` value, establishes `Delivered` with `42u64 as +Delivered`, forms `(value.lo as -Delivered) - 42u64`, and checks the authored byte placement (lo at 8, hi at 0). A twin changes only the establishing write from 42 to 41. The target-body place uses ordinary implementations selected by Build and separately observes provider markers 17 versus 29.

The explicit casts follow the current [named-domain rules](../../../../wiki/spec/language/domains.md#domain-casts). Adding `Delivered` still proves its exact predicate. Removing the displayed domain keeps the numerical fact and selects plain subtraction; it changes no bytes. The negative twin uses `41u64 as +Delivered` and must fail that same predicate. The constraints, input values, subtraction, transports, target bindings and byte checks are unchanged by this spelling migration. Current qualification remains in progress.

Canonical contracts: [layout applications](../../../../wiki/spec/layouts/plans.md#representation-applications), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), and [ownership](../../../../wiki/spec/terminal-psi/ownership.md). Policies receive authorized schema and explicit target facts; the returned plan supplies neither validation nor storage authority. Exact semantic type, selected implementation and field identities remain distinct from physical geometry.

The nine places and reject twins are local, field, parameter, call_result, match_join, [const_name](const_name/README.md), other_state (nested machine), other_machine and target_body. Each positive declares exit zero and binds all four required hosted targets plus macOS x86-64.

Historical observations below belong to their recorded source versions, before this named-cast migration; they are not fresh validation of the current files.

Exact optimized producer `bc022a133a9ab10d6ba88e5eb9fa893ee72e8ad2` observed all nine positives exiting zero in both native and Psi execution, with all nine Delivered=41 twins rejecting at their intended bound. At that revision, the generic and imported named-constant compositions also passed, and their policy-argument and private-policy controls rejected. The facts deduplication later removed those specimens. The final reduced milestone producer94 (`2ba727eefb59abae5e75b6b6f7e94a5ddcb58742`) freshly observed all nine positives at native/Psi exit zero and all nine intended twins. Exact products and Source controls are retained in the qualification evidence. The earlier frozen migration producer `8b4b6ec49266` refused the positives before checking the intended fact because pre-check policy evaluation lacked explicit target facts; those historical results remain baseline evidence.

The row does not close the parent tasks: lifetime/cleanup compositions, complete policy migration, foreign ABI agreement, alias and conversion families, portable artifact controls, independent review and verified publication remain open until demonstrated.

The match-join place separately executes both choices and observes caller-visible markers 17 and 29 after forming the unchanged subtraction. Identical returned representation bytes cannot conceal branch inversion. Its reject twin retains the same marker observations and breaks only the establishing field bound.

The [computed-policy composition](compositions/computed_const_policy/README.md)
supplies `Value + 1` to a layout policy under the caller's upper bound. Its
transparent-alias variation checks the same argument through `PolicyStorage`;
both unbounded controls retain the original arithmetic obligation.
