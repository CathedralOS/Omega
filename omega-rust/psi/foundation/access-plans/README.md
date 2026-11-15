# Access and backing validation

This crate normalizes semantic access-policy data independently of layout and
backing. It also validates the physical placement and resource joins consumed by
compiler stages and the interpreter. Neither operation grants live caller authority.

- [plan.rs](src/plan.rs) owns semantic `AccessPlan` data and its normalization,
  separately from `ValidatedPlacement` and its physical snapshot. An absent
  caller row adds no restriction; an explicit empty caller list denies its region.
- [access_fields.rs](src/access_fields.rs) owns field descriptors and projections.
- [plan_policy.rs](src/plan_policy.rs) validates field geometry and operations.
- [resources.rs](src/resources.rs) validates provider profiles, restricted loans,
  initialization and relocation supply.

These validators do not create runtime views, issue backing authority or prove
that a schema describes a physical device. Their Terminal Psi and interpreter
consumers own execution and custody transitions.

Contract: [access policies](../../../../wiki/spec/resources/access.md).
