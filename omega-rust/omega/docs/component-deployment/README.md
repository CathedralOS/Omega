# Component deployment

Contract: [native products and component publication](../../../../wiki/spec/build/component_publication.md).

[component_deployment.rs](src/component_deployment.rs) is the starting point:
preflight and claim installation, seal providers, close progress, then finalize
the runnable. [flat_output.rs](src/flat_output.rs) separately owns filesystem
publication and receipt replay; [lib.rs](src/lib.rs) only wires the public API. A
`ComponentDeploymentSession` retains the candidate parts, real `InstalledCode`,
and claimed `InstalledRootLedger`. Provider/progress closure and finalization
consume typed sessions; failure preserves the current session and remaining inputs.

This is installation admission, not an OS update controller. Deployment journals,
transaction persistence, and restart recovery belong to the consuming runtime or
OS and are not prerequisites of this flow.

The output path stages/replays bytes and executable mode before returning a
publication receipt. Reports retain the resulting owned deployment rather than
reconstructing custody from a path. Keep the complete selected-plan set, including
unexecuted plans, through registry and installation replay.

## Remaining integration

This build owner is not connected to an ordinary production compiler caller.
Integration needs independently supplied installation, provider, progress, and
profile authority. A compiler-side adapter cannot stand in for those owners;
ordinary artifact-file publication does not establish an installed component.

The composer rejects installed external-root records while their code-borrowing
handles lack an owned teardown protocol. Keep this limitation explicit until the
complete retirement route can return or consume that custody correctly.
The existing component-substrate work on the [execution board](../../../../TASKS.md)
owns these gaps.
