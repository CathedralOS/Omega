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

## Protocol boundary

[src/protocol.rs](src/protocol.rs) owns the closed response envelope. Only its
canonical `recommend_audit` or `no_additional_audit` result is accepted, with no
prose. The runner selects no model and grants no ambient network access. Fixed
system instructions stay separate from bounded manager-rendered hostile input;
an owned streaming sink enforces the caller's output ceiling.

Advice is monotone: it can add an audit recommendation, never suppress compiler
recommendations, alter blockers, resolve decisions, admit evidence/packages, set
policy, or mutate accepted state. Bind a response to its exact rendered input
for stale-result detection, not as proof of an audit. Byte escaping protects the
packet grammar; it does not neutralize instructions embedded in reviewed code.
