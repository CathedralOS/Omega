# Access and backing validation

This crate checks field-operation demands against layout geometry and supplied
backing capabilities. Compiler stages and the interpreter consume its normalized
plans, operation authorization, resource profiles and compatibility results.

- [plan.rs](src/plan.rs) owns the normalized layout/access/reach bundle and its
  validated snapshot. This internal bundle is not the source `AccessPlan` policy.
- [access_fields.rs](src/access_fields.rs) owns field descriptors and projections.
- [plan_policy.rs](src/plan_policy.rs) validates field geometry and operations.
- [resources.rs](src/resources.rs) validates provider profiles, restricted loans,
  initialization and relocation supply.

These validators do not create runtime views, issue backing authority or prove
that a schema describes a physical device. Execution and custody transitions
belong to their Terminal Psi and interpreter consumers.

Contract: [access policies](../../../../wiki/spec/resources/access.md).
