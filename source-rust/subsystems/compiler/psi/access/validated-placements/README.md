# Access admission

The [entry](admission.rs) checks carried policies against an explicit layout and resource profile. The resulting placement, field keys and validated resource profile retain their exact identities.

- [Access fields](access_fields.rs) bind field keys to descriptors and projections.
- [Plans](plan.rs) normalize semantic restrictions and issue validated placements. An explicit empty caller list denies its region.
- [Placement checks](plan_policy.rs) compare field geometry and layout identity.
- [Resource checks](resources.rs) validate interval coverage, initialization and relocation supply.

The [policy records](../access.rs) describe the request. The [interpreter](../../../../interpreter/memory/storage-access/) checks live operations against the grant, borrow and source loan. [Access policies and views](../../../../../../wiki/spec/resources/access.md)
