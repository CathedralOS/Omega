# build_component_assumptions

This check-only root still uses the retired
`Build::accept_component_assumption(builder, "<64-hex digest>")` approval call.
The current evaluator discards the digest; it does not record an accepted
assumption. The ratified
[consumer acceptance and Build attenuation](../../../../wiki/spec/build/component_publication.md#consumer-acceptance-and-build-attenuation)
rule puts dependency approval in the existing consumer policy/lock route;
optional Build policy may only narrow that accepted ceiling.

**COMPONENT-SUBSTRATE** in [TASKS.md](../../../../TASKS.md) retains migration
of this root, the one-argument control, and its observations to consumer-policy
and attenuation controls. No executable files are changed by the ratification.
The runnable customer's authored exit oracle is retained in
[`component_assumption_recording`](../../independent/component_assumption_recording/README.md).
