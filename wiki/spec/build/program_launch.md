# Launching a program from a Build

A Build can run an executable its caller approved. The Build supplies arguments.
The caller binds a program identity to an executable and limits the resources
that executable can use. A stock invocation grants no program launches by default.

## Source interface

Core declares the memberless boundary trait `ProgramLaunch`. The following
declarations specify the interface of `{omega}/build/program.omg`. The host
installs one launcher provider for the activation. Its grant table binds
each demanded `Program` identity to its approved executable under the
[ordinary generic requirement rules](provider_selection.md#generic-selected-requirements). Build code calls the free machine through its file import.

```omega
public data RunResult {
    case NotRun;
    case Exited(status: u32);
    case StartFailed;
    case AbnormalExit;
    case TimedOut;
}

boundary trait Launcher {
    machine run<Program>(arguments: @[@[u8]]) -> RunResult
        reaches ProgramLaunch
        blocks
        crashes none
        ensures !(result in RunResult::NotRun);
}

public machine run<Program>(arguments: @[@[u8]]) -> RunResult
    reaches ProgramLaunch
    blocks
    crashes none
    ensures !(result in RunResult::NotRun)
{
    block Launcher::run::<Program>(arguments)
}
```

`Program` identifies an exact nominal declaration. Naming or constructing a
value of that type grants no authority. The ordered byte views are borrowed
for the call. Each is one UTF-8 argument with no embedded NUL. The host supplies
the program name separately and launches the bound executable directly.
Arguments retain their boundaries, including empty arguments and spaces.
Shell parsing, PATH lookup and file-association dispatch are outside this call.
If the host cannot represent the requested arguments under the bound program's
calling convention, it returns `StartFailed`.

## Grant and delegation

`omega build --allow ProgramLaunch` raises the root ceiling. Each
`--program <identity> <executable>` opens one grant. The following repeated
`--program-read <path>` and `--program-write <path>` options select readable
inputs and writable outputs for that grant. Paths use the activation's
[logical filesystem](scoped_execution.md#snapshot-and-staging-protocol).
`--program-timeout <milliseconds>` sets a positive elapsed-time limit within
the host's execution budget. When omitted, the host applies its finite limit.
The CLI resolves the qualified type name in the reviewed package graph.
An option for a program must follow its binding. Duplicate program bindings
and unresolved or ambiguous identities are errors. An embedding host can
supply the same grants directly.

The executable locator is resolved by the invoker. Admission binds the
reviewed executable and the runtime files it needs, preserving their identity
through launch. The process gets the granted paths within the caller's own
view and bounded private scratch. It receives no ambient working directory,
credentials, environment, inherited handles or network access. The host must
enforce the grant for the whole launched process tree and manage its lifetime
through completion, timeout or cancellation. Declaring the reach atom or
approving a path alone cannot provide that confinement.

A parent delegates a specific existing binding at registration with
`Build::dependency(build, source, ProgramLaunch::Inherit::<Program>())`.
The same selection applies to a configured dependency description. It
forwards at most the parent's approved executable, resources and limits.
A child receives no binding by default. Each further dependency requires
its own delegation. Registering a child performs no launch.

Before each Build starts, admission checks its reachable program demands
against the ceiling, actual bindings, host enforcement and
[restricted-build acceptance](../packages/acceptance.md#restricted-build-acceptance).
On refusal, identify the requesting Build, call path, program, requested
resources and missing approval, binding or host support. A user or agent can
audit the request and rerun. The same refusal applies in noninteractive use.

## Completion

The call waits for the launched program. A normal exit yields `Exited` with
the host exit status represented as a `u32`, including nonzero statuses.
Invalid argument encoding or an admitted launch that cannot start yields `StartFailed`. An
observed abnormal termination yields `AbnormalExit`. Expiration of the
launch deadline stops the launched process tree and yields `TimedOut`.
`crashes none` describes the Omega call. A crashed child does not crash its
caller. Cancellation of the Build follows ordinary activation interruption.

The first case, `NotRun`, is the [ordinary zero value](../language/data_and_literals.md).
Fresh result storage therefore records no attempt. Every normal return from
`run` establishes the stronger postcondition in its signature. A caller can construct result data, so `Exited(0)` alone cannot establish
that any output was written or sealed. Publishing a product still requires the Build's ordinary
[output completion](scoped_execution.md#staged-products-and-failure).
