# Access admission

This crate checks complete access decisions against explicit layout and resource
inputs. It retains the private constructors for admitted placements, field keys,
descriptors and resource profiles. Passing these checks grants no live storage
authority.

- [plan.rs](src/plan.rs) owns semantic `AccessPlan` data and its normalization,
  separately from `ValidatedPlacement` and its physical snapshot. An absent
  caller row adds no restriction; an explicit empty caller list denies its region.
- [access_fields.rs](src/access_fields.rs) owns field descriptors and projections.
- [plan_policy.rs](src/plan_policy.rs) validates field geometry and layout identity.
- [resources.rs](src/resources.rs) validates provider profiles, interval restrictions,
  initialization and relocation supply.

Raw policy, operation and capability records belong to
[`access-vocabulary`](../representations/access-vocabulary/README.md).
[`storage-access`](../../runtime/storage-access/README.md) owns live grants and
checks operations against permissions, the current borrow and the source loan.
Region projection here checks the exact geometry before runtime authorization.

Contract: [access policies](../../../../wiki/spec/resources/access.md).
