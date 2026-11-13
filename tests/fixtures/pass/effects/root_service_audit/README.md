# Consuming-Root Service Audit

The root delegates its original mutable Build to `configure`, which registers
the driver's exact `MotorControl` boundary twice. The union is idempotent.
The unselected state must not register `Untaken`. The driver's own Build
registers `DependencyOnly`, which must not choose its consumer's audit policy.
The root's unrelated `FilesystemContentWrite` spelling is not core-owned.

`omega audit packages --project <this folder> --details` must highlight exactly
`ConsumerAudit "audit_driver"::"MotorControl"` in the driver's findings, never
`DependencyOnly`, `Untaken`, or the root's filesystem lookalike. Registration
itself remains an acceptance-visible policy row, even without root exposure.

The ordinary-trait reject control is
`tests/omega/fail/build/service_audit_requires_boundary`.