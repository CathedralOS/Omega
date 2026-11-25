# Using source packages

Omega installs source dependencies from Git or local paths and checks their
reachable authority and contracts. It does not need a package-hosting service.
The [acceptance contract](../spec/packages/acceptance.md) defines the guarantees;
this page explains the workflow.

## Declare the project

A library declares its own name in `build.omg`:

```omega
machine build(builder: @mutable Build) {
    Build::package(builder, "arithmetic_kernels");
}
```

An executable uses `Build::application(builder, "name")`; a workspace lists
`Build::member(builder, "path")` calls. Package and application names use lowercase
underscore-separated words. The default alias is the declared name unchanged;
use `--as` to choose another. See
[build declarations](../spec/build/declarations.md) for identity and execution.

Renaming a package changes its key, even when its directory and source locator
stay the same. Existing locks require a fresh update/review after a declaration
rename; old acceptance does not authorize the new identity. Hyphenated package
declarations are not accepted through an implicit spelling conversion.

The caller supplies the libraries that Build code imports. The Build selects
the libraries its product can import. In
`use {geometry}/math/vector.omg as vec;`, `geometry` identifies the registered
dependency and `vec` names one file inside the importing source. [Chapter 15](chapter_15_modules_imports_visibility.md#build-helpers)
shows the two dependency kinds.

## Giving one dependency a filesystem

Suppose a firmware package's Build reads `expected.sha256`. The parent reads
the checksum file from its own filesystem, creates an empty memory filesystem,
and writes a copy of those bytes there as `expected.sha256`. The original
file stays in place. The parent can then give the firmware package a read-only
view of that tree. If the child needs writable scratch too, the parent mounts
a writable directory in the view it supplies.

This parent helper receives the prepared view by value and transfers it to the
firmware dependency. `Files` is the filesystem implementation the caller
selected for this backing.

```omega
machine register_firmware<Backing, const Files: Filesystem<Backing>>(
    build: @mutable Build,
    child_filesystem: Backing
) -> DependencyRegistration {
    let source: Source = Source::Path { location: "deps/firmware" };
    let firmware: DependencyDescription = |dependency|::from_source(source);
    let configured: DependencyDescription = |dependency|::with_filesystem::<Backing, Files>(
        firmware, child_filesystem
    );
    Build::dependency(build, configured)
}
```

The driver installs the read-only view for the firmware Build before entry.
When that Build reads `expected.sha256` through its filesystem library, it
reads the copy. Its entry is still `machine build(build: @mutable Build)`.
[Child Build filesystems and borrowing](../spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build)

## Giving one dependency network access

Suppose only the board package needs to fetch firmware. The caller runs
`omega build --allow Network release/build.omg`. The release Build chooses
which child gets its network grant when it registers the dependencies.

```omega
machine build(build: @mutable Build) {
    Build::package(build, "release");
    Build::dependency(build, Source::Path { location: "board" }, Networking::Inherit);
    Build::dependency(build, Source::Path { location: "assets" });
}
```

The board Build can use the parent's admitted live network provider. The
assets Build stays offline. If the caller omits the flag, both stay offline
and a board Build requiring a live fetch fails before entry. To keep it
offline, an earlier stage can fetch the firmware and supply it in the board
Build's filesystem view. [Build network grants](../spec/build/scoped_execution.md#live-networking-in-a-build)

## Running a program in a Build

Suppose the board package declares a program identity and calls its vendor's
packer. This helper returns the outcome to the board Build, which can inspect
the status before using the output.

```omega
use {omega}/build/program.omg as program;

public data VendorPacker {}

machine pack_firmware() -> |program|::RunResult
    reaches ProgramLaunch
    blocks
    crashes none
{
    let arguments: [@[u8]; 2] = ["firmware.bin", "boot.img"];
    block |program|::run::<VendorPacker>(@arguments[..])
}
```

For a board Build with readable `firmware.bin` and writable `boot.img` in
its view, the caller can bind the packer and grant those two paths:

```sh
omega build --allow ProgramLaunch \
  --program board::VendorPacker /opt/vendor/packer \
  --program-read firmware.bin --program-write boot.img \
  --program-timeout 30000 board/build.omg
```

The host resolves `board::VendorPacker` to the same type the helper uses.
A missing approval prevents entry. An admitted packer that exits with status
1 returns that status to the helper. [Program launch contract](../spec/build/program_launch.md)

## Packaging several products

An ordinary release program can run `omega build` for each producer and pass
the finished artifacts to a packer. For example, a release can place two
separately built executables in an installer. A release tool that links the
project-build library can request the same builds directly. Each producer's
own `build.omg` still describes that one project.
[Releases with independent products](../spec/build/declarations.md#releases-with-independent-products)

## Building from a revision in source

In the proposed staged workflow a project pins each direct Git dependency in
its own Omega source. Building the project alone or as a dependency uses the
same entry. A cached copy of that exact revision can satisfy an offline build.
Use an extra stage when the Build itself imports the library. A product-only
dependency needs the ordinary registration.

For example, a Git declaration for `board` can follow the `stable` branch for
updates and still build from yesterday's exact commit. Installing `board`
again keeps that commit. Updating it proposes changing the commit literal.
A custom revision calculation stays editable by the author.
[Complete board example and command behavior](../spec/packages/staged_inputs.md)

## Version 4 install and update

```text
omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega packages update [package-or-alias...] [--to <revision>]
omega packages install --resume
omega packages update --resume
omega packages <install|update> --discard-review
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

### Assumed binding contracts are extremely high risk

**Reject these in ordinary applications.** A library asking you to approve one
to work around a borrow error, missing proof, broken contract or compiler
limitation is asking you to inherit its implementation problem as a trust
assumption. Repair that problem instead. Exceptional use must identify a
deliberate requirement outside the checked model; a failing build is not one.

`Build::assume_binding_contract::<Required::operation, Supplied::operation>(builder)`
asks the root to accept a semantic relationship that ordinary conformance
checking has not proved. Audit shows both contracts, the actual implementation,
the missing guarantees/authority and the transitive dependency path. Install and
update require explicit acceptance for that exact relationship and implementation;
an unchanged public signature does not preserve approval of changed selected code.
The lock retains a compact readable risk row, not a full provider-plan dump.

The assumed connection can be demonstrably false. Audit shows detected
contradictions and the facts refuting the exact relationship; acceptance records
your decision to trust it anyway, not a proof or promise of safe execution.
Neither body escapes its own checks, and structural/operational mismatches
cannot be approved through this semantic assumption.

This runtime permission cannot authorize build-time execution or use of those
assumptions, even through a helper or automatic test. The build may register
the relationship for its output only. See
[root-assumed bindings](chapter_19_capabilities_effects_boundaries.md#root-assumed-binding-contracts).

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
binary. That ecosystem decides admission itself.
Project package acceptance and build-host grants do not authorize execution there.

## Inspect without accepting

```text
omega packages audit [--project <dir>] [--target <name>]... [--details]
```

Inspection shows fresh graph, API, authority, and assumption findings beside
accepted policy. It does not accept changes. `--details` expands normalized
policy; source diffs are separate from editable decisions. Audit recommendations
may remain after permissions are accepted because new code can misuse old powers.

`--offline` restricts acquisition to local sources and cached recorded Git pins,
including resume and historical-source diagnostics. It does not skip checking
or authorize reuse of a cached branch when a new selection was requested.
Missing required content fails without publishing a candidate.

The [command reference](../spec/build/command_line.md) lists the full option set.

## Compile without a build file

A loose root uses default application configuration. It still needs target
entry and provider selections. For repeated use, write a build file naming
your application and dependencies and pass that file to the command.
[Build-first entry selection](../spec/build/command_line.md#build-first-entry-selection)

Unnamed source gets an owner identity derived from its source, dependencies,
and semantic build inputs. Importing the same artifact twice retains the same
declarations; editing and rebuilding anonymous source creates a new owner.
Use a named package when you need continuity across updates.

## Live component replacement

A fixed service and a loaded provider can coexist. The editor's Build may
select the backing of `Console::write_line`. A plugin's Build exports a
satisfaction of the same Console requirement:

```omega
// editor/build.omg
Build::select_provider::<Console, TerminalConsole>(b);

// quiet/build.omg
Build::export_provider::<Console, QuietConsole>(b);
```

The editor owns the plugin as data. Given the image and component description
read from disk, the successful load path looks like this (`?` denotes returning
on failure):

```omega
let preview: Provider<Console> = load_provider::<Console>(image, description)?;
let ops: @Console = Provider::get(@preview);
ops.write_line("preview");        // calls the loaded export
Console::write_line("editor");    // calls the Build-selected terminal
```

To change the preview, use a mutable local and assign a successfully loaded
replacement after the last use of its borrowed operations. A helper can take
the borrowed runtime record. A generic requiring a static implementation still
needs a compile-time selection. No Build declaration is needed for the preview
variable. The [editor example](../drafts/designs/component_hot_swap_authorship.md)
shows replacement and a rejected use after replacement. [Owned provider contract](../spec/build/component_publication.md#owned-runtime-providers)
