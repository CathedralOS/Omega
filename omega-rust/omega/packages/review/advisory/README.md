# Omega package advisory tooling

This optional crate owns model-facing source-review instructions, its closed
response schema, bounded response custody, and reviewer invocation. It consumes
only deterministic review input from `package-manager`; neither the
tool's availability nor its recommendation can change package acceptance,
capability conflicts, or compiler-owned audit policy.

The package manager intentionally has no dependency on this crate.

The lock records accepted review baselines and decisions under the authority
of whoever lands it. Advisory output is optional review assistance, never a
certificate that an audit occurred or that the lock was correctly accepted.

No production reviewer, provider configuration, or install/update invocation
is currently wired in. This is an optional adapter, not automatic LLM review.

If a concrete integration is requested, configuration and invocation belong
with CLI/tooling. The complete package workflow must still work with no model
configured and when invocation fails. Report unavailable advice without
suppressing compiler findings, resolving decisions, or claiming an audit.
Use the current lock-policy comparison and separately rendered source diffs;
do not introduce a second persisted baseline or certification requirement to
connect this adapter. No built-in model service is required.
