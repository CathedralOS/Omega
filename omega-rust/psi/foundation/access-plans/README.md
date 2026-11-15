# Access and backing validation

This crate checks access rules separately from memory layout. It also checks
where values fit and which storage resources compiler stages and the interpreter
need. Passing these checks alone grants no permission to access storage.

- [plan.rs](src/plan.rs) owns semantic `AccessPlan` data and its normalization,
  separately from `ValidatedPlacement` and its physical snapshot. An absent
  caller row adds no restriction; an explicit empty caller list denies its region.
- [access_fields.rs](src/access_fields.rs) owns field descriptors and projections.
- [plan_policy.rs](src/plan_policy.rs) validates field geometry and operations.
- [resources.rs](src/resources.rs) validates provider profiles, restricted loans,
  initialization and relocation supply.

These checks do not create runtime views or prove that a type description
matches a physical device. The compiler and interpreter still control execution,
storage ownership and borrowing.

Contract: [access policies](../../../../wiki/spec/resources/access.md).
