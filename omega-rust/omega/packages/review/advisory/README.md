# Omega package advisory tooling

This optional crate owns model-facing source-review instructions, its closed
response schema, bounded response custody, and reviewer invocation. It consumes
only deterministic review input from `omega-package-manager`; neither the
tool's availability nor its recommendation can change package acceptance,
capability conflicts, or compiler-owned audit policy.

The package manager intentionally has no dependency on this crate.

The lock records accepted review baselines and decisions under the authority
of whoever lands it. Advisory output is optional review assistance, never a
certificate that an audit occurred or that the lock was correctly accepted.
