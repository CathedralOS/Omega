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

For a firmware application, the Build can bind
`Build::bind(build, uefi_x86_64::ProgramEntry, |app|::start)`. Selecting
`uefi_x86_64` selects the target package's entry contract, including its
package-authored calling policy. The bind selects your machine. A dependency
with its own `UefiApplication` declaration cannot change the firmware ABI.
[UEFI handoff](../spec/build/uefi_entry.md#physical-arrival-and-semantic-roots)

## Using the same console in a Build and an application

In the staged design, `omega build build.omg` starts at the named file. Here
the first entry prepares an interpreted Build. That Build prints a message
and describes a Linux editor. Both programs import the same helper.

```omega
// build.omg
use {omega}/language/core/process_authority.omg as process;
use {omega}/build/dependency.omg;
use {system}/console.omg;

machine build(build: @mutable Build) {
    let system: DependencyDescription =
        |dependency|::from_source(Source::Path { location: "vendor/system" });
    let allowed: [BoundaryServiceReference; 1] = [
        Build::service::<|process|::StandardOutput>(build)
    ];
    _ = |dependency|::add(build, system, @allowed[..]);
    Build::build_entry(build, "build_0.omg");
    Build::select_provider::<|console|::Console, |console|::InterpreterConsole>(
        build, interpreter
    );
}
```

Here `vendor/system` declares the package name `system`. The console references
in the selection are designated product operands. The preparing entry never
calls the console. Its caller supplies the child's admitted output sink.

```omega
// build_0.omg
use {omega}/language/core/process_authority.omg as process;
use {omega}/build/dependency.omg;
use generated.omg;
use main.omg as editor;

machine build(build: @mutable Build) {
    |generated|::announce("configuring editor");
    Build::application(build, "editor");
    let system: PackageBinding = |dependency|::input(build, "system");
    let allowed: [BoundaryServiceReference; 1] = [
        Build::service::<|process|::StandardOutput>(build)
    ];
    _ = |dependency|::add(build, system, @allowed[..]);
    Build::bind(build, linux_x86_64::ProgramEntry, |editor|::main);
}
```

```omega
// generated.omg, supplied in the frozen source view of both programs
use {omega}/language/core/process_authority.omg as process;
use {system}/console.omg;

public machine announce(message: @[u8]) reaches |process|::StandardOutput {
    |console|::Console::write_line(message);
}
```

```omega
// main.omg
use {omega}/language/core/process_authority.omg as process;
use generated.omg;

machine main() reaches |process|::StandardOutput {
    |generated|::announce("editor started");
}
```

The first message reaches the interpreter's diagnostic sink. When the editor
runs, its message reaches Linux standard output through the provider selected
by the `system` package for that target. The intermediate Build forwards the
package once. Both entries read `generated.omg` from the supplied source view.
[Stage admission and acceptance cases](../spec/build/scoped_execution.md#providers-for-an-interpreted-successor)

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

In the staged workflow a project pins each direct Git dependency in
its own Omega source. Building the project alone or as a dependency uses the
same entry. A cached copy of that exact revision can satisfy an offline build.
Use an extra stage when the Build itself imports the library. A product-only
dependency needs the ordinary registration.

A Build can use `{omega}/build/dependencies/providers/git.omg` to declare an alias,
repository, tracking reference and selected revision. After an online build has
acquired that exact commit, `omega build --offline build.omg` can reconstruct
the product from the cached commit and its tree. A missing object fails the build.

For example, a Git declaration for `board` can follow the `stable` branch for
updates and still build from yesterday's exact commit. The add and update guides explain how to register that package or change
its commit literal.
A custom revision calculation stays editable by the author.
[Complete board example and command behavior](../spec/packages/staged_inputs.md)

### Unsafe in a dependency

Enable each reachable unsafe atom with `Build::enable_unsafe::<Atom>(build)`
and include that atom in the consuming dependency edge’s ceiling. If a helper starts upgrading shared borrows, the error identifies
the originating operation and the dependency edge missing admission. Treat
any use of `Unsafe` in a library as malicious.
[Admission example](../spec/resources/native_borrows.md#build-admission-and-diagnostics)

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
