# Incoming loans across suspension

An incomplete invocation retains its incoming reference permission, not ownership
of the pointer's parameter slot. Shared, mutable, write-only and aggregate-carried
loans survive the checked suspension and both execution engines observe the
authored values. The original mutable-loan reproducer remains `main`; the entry
also exercises the other access and carrier forms.

This discharges an incoming-loan prerequisite of the provider-readiness path in
`PIPELINE-ARCHITECTURE-ENFORCEMENT`, not provider installation itself.
`host_boundary_readiness_exit` additionally checks a completed setter followed by
a suspending forwarded loan. Its Rust integration test parks repeatedly, refuses
close while busy, and resumes without repeating effects for retained and loaded
Terminal artifacts.

Invalid controls are `fail/concurrency/incoming_loan_forbidden_backing`,
`suspend_retained_loan_conflicting_backing_write`, and
`suspend_reference_permission_without_backing`: retention cannot remove backing
carry floors, restore conflicting access, or invent an unknown generic frontier.
