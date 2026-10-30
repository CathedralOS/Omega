# Canonical owned content at a selected boundary

The program creates an existing-input `Region in Owned` under an unrestricted
establishment domain and invokes the explicitly selected `RegionPort` provider.
The projection uses the compiler-owned core `Content` and `CountedQuantity`
declarations. Provider selection grants no content or issuance authority.

The provider observes the region before its owner cleanup consumes it, then
returns scalar status through a branch. A second machine moves an independent
owned input through a local before the same observation, consumption and scalar
completion. Both paths must return 47. A third path conditionally consumes an
owned input through a scalar call: the selected arm consumes it in the callee,
while the skipped arm retains its exact normal-exit cleanup obligation. The
entry checks both outcomes and a shared qualified observation before consuming
its owner. Native and Terminal execution must both exit 70.

This covers existing unrouted input custody, not establishment from a protected
issuer or a new routed result. Incompatible-unit and local-projection-lookalike
controls remain separate failures in `fail/core`.
