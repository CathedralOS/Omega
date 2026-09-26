# Access plans and resident views

Contract: [placed access](../../../../wiki/spec/resources/placed_access.md).
[lib.rs](src/lib.rs) is the entry map. Layout owns geometry; access plans own
observation policy and permitted primitive operations over that geometry.

Owned Stable and Atomic resident lifecycles, borrowed resident views, projection,
specialization, and checked/runtime correspondence are separate owners. Every
transition rejoins exact admission, resource/profile, occurrence, claim, receipt,
and lifecycle data. Rejecting a transition returns its complete inputs.

These carriers remain below Terminal. The observing Atomic contract/runtime join
preserves a non-clonable specialized request and verifies its exact resident/result
shape; it does not issue an atomic operation or establish source-call, runtime
result, provider-selection, or native authority.

Do not turn a successful foundation join into a claim that Terminal lowering is
complete. The existing ACCESS-PLAN-AND-PLACED task owns the remaining
[source-to-Terminal integration](../../../../TASKS.md).
