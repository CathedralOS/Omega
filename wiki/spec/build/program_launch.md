# Launching a program from a Build

A Build launches a tool by its path in that Build's namespace. The invoker
binds a logical path to a particular executable and sets its limits. The
stock invocation starts with an empty launch table.

## Source interface

Core declares the memberless boundary trait `ProgramLaunch` for launch reach. The host installs
one launcher for the activation. The Build library exposes the following
operation through `{omega}/build/program.omg`.

```omega
public data RunResult {
    case NotRun;
    case Exited(status: u32);
    case StartFailed;
    case AbnormalExit;
    case TimedOut;
}

boundary trait Launcher {
    machine launch_program(path: @[u8], arguments: @[@[u8]]) -> RunResult
        reaches ProgramLaunch
        blocks
        crashes none
        ensures !(result in RunResult::NotRun);
}

public machine launch_program(path: @[u8], arguments: @[@[u8]]) -> RunResult
    reaches ProgramLaunch
    blocks
    crashes none
    ensures !(result in RunResult::NotRun)
{
    block Launcher::launch_program(path, arguments)
}
```

The launcher borrows the path and argument views for the call. Each argument
is UTF-8 with no embedded NUL. The host supplies
the program name separately and launches the bound executable directly.
Arguments retain their boundaries, including empty arguments and spaces.
Shell parsing, PATH lookup and file-association dispatch are outside this call.
If the host cannot represent the requested arguments under the bound program's
calling convention, it returns `StartFailed`.

## Grant and delegation

`omega build --allow ProgramLaunch` raises the root ceiling. Each
`--program <logical-path> <executable>` opens one grant. For example,
`--program /tools/packer ./tools/linux/packer` binds the Build-visible name
`/tools/packer` to the executable chosen by the invoker. The host resolves
the external locator at grant creation and retains the executable's identity
and required runtime files through launch. Replacing a file at that spelling
cannot authorize the replacement. Duplicate logical bindings reject.
An embedding host can supply the same table directly.

Repeated `--program-read <path>` and `--program-write <path>` select the
readable inputs and writable outputs for the preceding program grant in the
activation's [logical filesystem](scoped_execution.md#snapshot-and-staging-protocol).
`--program-timeout <milliseconds>` sets a positive elapsed-time limit. The
host applies a finite limit when omitted. The process gets the granted paths within
the caller's own view and bounded private scratch. It receives no ambient working directory,
credentials, environment, inherited handles or network access. The host must
enforce the grant for the whole launched process tree and manage its lifetime
through completion, timeout or cancellation. Declaring the reach atom or
approving a path alone cannot provide that confinement.

At registration a parent can delegate a list of program paths from its
own launch table. An omitted or empty list grants none. Reject a selection
the parent does not hold. A child receives only the selected bindings, with
the same executable identities and at most the parent's resources and limits.
For instance, a parent holding packer and shader compiler grants can pass
just `["/tools/packer"]` to its textures dependency. Apply the child's
directory remaps to the delegated names. For example, if
`/tools` appears as `/sdk` in the child, its delegated packer is
`/sdk/packer`. A read-only mapping alone conveys no launch grant. Each
subsequent handoff requires its own delegation.

The explicit delegation is the consuming project's approval for that child
within those bounds. Before entry, check the Build's reachable
`ProgramLaunch` effect against its ceiling. At each executed
`launch_program` call, look up the supplied path in that activation's launch
table. A missing grant fails the call at runtime and reports the Build and
logical path before any process starts. Literal and computed paths follow
the same rule. An untaken branch performs no lookup. The host must be able
to enforce the entire grant before it starts the process. Earlier authorized
effects remain observable. Launch never searches the external filesystem
for an equivalent path. [Staged approval](../packages/acceptance.md#restricted-build-acceptance)

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
`launch_program` establishes the stronger postcondition in its signature. A caller can construct result data, so `Exited(0)` alone cannot establish
that any output was written or sealed. Publishing a product still requires the Build's ordinary
[output completion](scoped_execution.md#staged-products-and-failure).
