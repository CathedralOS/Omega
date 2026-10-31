# Access plans and resident views

Contract: [access policies](../../../../wiki/spec/resources/access.md).
[lib.rs](src/lib.rs) is the entry map. `AccessPlan` contains a `LayoutPlan`,
field-operation decisions, and required reach. Layout owns representation and
geometry without access rights. `Access<P, T>` is the complete view;
`Accessor<T>` is a projected operation handle, not an unrestricted lvalue.

Owned Stable and Atomic resident lifecycles, borrowed resident views, projection,
specialization, and checked/runtime correspondence are separate owners. Every
transition rejoins exact admission, resource/profile, occurrence, claim, receipt,
and lifecycle data. Rejecting a transition returns its complete inputs.

These carriers remain below Terminal. The observing Atomic contract/runtime join
preserves a non-clonable specialized request and verifies its exact resident/result
shape; it does not issue an atomic operation or establish source-call, runtime
result, provider-selection, or native authority.

The policy and compatibility checks consume the backing's actual authority and
capabilities. They do not issue a grant, invent resident custody, or establish
that a schema corresponds to the physical device.
