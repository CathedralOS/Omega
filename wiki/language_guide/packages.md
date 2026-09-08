# Using source packages

Omega installs source dependencies from Git or local paths and checks their
reachable authority and contracts. It does not need a package-hosting service.
The [acceptance contract](../spec/packages/acceptance.md) defines the guarantees;
this page explains the workflow.

## Declare the project

A library declares its own name in `build.omg`:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

An executable uses `builder.application("name")`; a workspace lists
`builder.member("path")` calls. The default alias for `arithmetic-kernels` is
`arithmetic_kernels`; use `--as` to choose another. See
[build declarations](../spec/build/declarations.md) for identity and projection.

## Install and update

```text
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega install --resume
omega update --resume
omega <install|update> --discard-review
```

The command resolves and checks the candidate, then compares it with `omega.lock`.
Blocking changes appear in a compact review document with `pending` decision
tokens. Edit those to `accept` or `reject`, then resume the same operation.
Accepted changes publish the declaration and lock together. A stale candidate
must be checked again; rejection keeps accepted files unchanged.

No blocking findings means no approval step. A missing lock starts fresh review.
Missing old source still permits policy comparison, but limits the code diff and
calls for auditing the candidate directly. Keep the lock under the project's
normal review controls: it records your decisions, not a certificate of safety.

## Inspect without accepting

```text
omega audit packages [--project <dir>] [--target <name>]... [--details]
```

Inspection shows fresh graph, API, authority, and assumption findings beside
accepted policy. It does not accept changes. `--details` expands normalized
policy; source diffs are separate from editable decisions. Audit recommendations
may remain after permissions are accepted because new code can misuse old powers.

`--offline` restricts acquisition to local sources and cached recorded Git pins,
including resume and historical-source diagnostics. It does not skip checking
or authorize reuse of a cached branch when a new selection was requested.
Missing required content fails without publishing a candidate.

The [command reference](../../omega-rust/omega/packages/manager/src/operations/package_commands/README.md)
lists current options and limits; [inspection](../../omega-rust/omega/packages/manager/src/operations/inspect_packages/README.md)
describes reporting. [Package fixtures](../../tests/fixtures/packages/README.md)
and [remote pins](../../tests/fixtures/packages/REMOTE_PINS.md) document integration
test setup; local cases do not establish remote transport coverage.
