# Using source packages

Omega installs source dependencies from Git or local paths and checks their
reachable authority and contracts. It does not need a package-hosting service.
The [acceptance contract](../spec/packages/acceptance.md) defines the guarantees;
this page explains the workflow.

## Declare the project

A library declares its own name in `build.omg`:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic_kernels");
}
```

An executable uses `builder.application("name")`; a workspace lists
`builder.member("path")` calls. Package and application names use lowercase
underscore-separated words. The default alias is the declared name unchanged;
use `--as` to choose another. See
[build declarations](../spec/build/declarations.md) for identity and projection.

Renaming a package changes its key, even when its directory and source locator
stay the same. Existing locks require a fresh update/review after a declaration
rename; old acceptance does not authorize the new identity. Hyphenated package
declarations are not accepted through an implicit spelling conversion.

Product dependencies use `depend`/`depend_as`; build-only helpers use
`build_depend`/`build_depend_as`. They are separate checked scopes, including
when the same library is used in both. The
[build dependency guide](chapter_15_modules_imports_visibility.md#build-and-product-dependencies)
explains target separation, helper imports, and explicit legacy migration.
Purpose-aware installation/lock support is implementation work; the current
command list below does not imply a new build-dependency CLI option.

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

### Build-time requests are part of the audit

Build code is confined by default to its admitted inputs, private staged outputs,
and bounded build facilities. Ordinary dependency generators using that baseline
need no unsafe-action approval.

Install/update also show what a dependency wants to do **during compilation**:
restricted host operations, the package requesting them, their resource scope,
and any change from accepted requests. Calls through another build helper or a
dependency's build activation do not hide those requirements. The review pauses
before an unaccepted restricted action; accepting it permits the build phase to
continue only if the host also supplies the required resources. Generated code
still goes through normal checking and review afterward.

`omega.lock` records accepted requests alongside package acceptance. It does not
store credentials, machine-specific grant paths, or temporary capabilities.
New or expanded authority needs explicit acceptance; unchanged accepted requests
do not prompt again. A resolution refresh cannot silently grant more access, and
changed source stays visible for audit even when its permissions are unchanged.
See [restricted-build acceptance](../spec/packages/acceptance.md#restricted-build-acceptance).

Ordinary compilation needs no policy from the OS that may eventually run the
binary. That ecosystem decides admission itself, optionally checking PCC evidence.
Project package acceptance and build-host grants do not authorize execution there.

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

The [command reference](../../omega-rust/omega/packages/manager/src/package_manager/README.md)
lists current options and limits; [inspection](../../omega-rust/omega/packages/manager/src/operations/inspect_packages/README.md)
describes reporting. [Package fixtures](../../tests/fixtures/packages/README.md)
and [remote pins](../../tests/fixtures/packages/REMOTE_PINS.md) document integration
test setup; local cases do not establish remote transport coverage.

## Live component replacement

Updating a source dependency, migrating stored data, and replacing running code
are different operations. A replaceable component is a selected realization's
closed code/state/resource graph, not intrinsically one package. Calls cross a
stable requirement; already-entered calls and returned era-dependent handles
retain their old era until their obligations end.

Publication of new routing therefore does not imply reclamation of old code.
Runtime packages coordinate admission, peak coexistence resources, state transfer,
quiescence, and recovery inside an owner-authorized envelope. A migration theorem
alone cannot discharge callbacks, borrows, or device claims. See the
[component publication contract](../spec/build/component_publication.md) and its
linked implementation note for the distinction between required protocol and
available source support.
