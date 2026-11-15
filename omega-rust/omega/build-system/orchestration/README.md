# Orchestration

Runs a whole build in order: prepare the inputs, run `build.omg` through
[build-settings](../build-settings/README.md), call the compiler, and write the
outputs. Package commands go through the same steps.

Install, update and resume start in [package_commands.rs](src/package/package_commands.rs).
It opens a package transaction, admits the command, runs every requested
target review, and returns all targets for decisions and publication. On an error, accepted files stay unchanged and the transaction
is released on return. One caller-owned sponsored session covers all targets
and automatic resume rounds. Discard only edits package files. Package management chooses the discovery and bound policy
passes and admits exact reuse.
[package_review.rs](src/package/package_review.rs) runs each admitted pass,
dependencies first: prepare Build, meet component demands, finish review,
and return the evidence to package policy. It keeps suspended consumer Builds
through cache-miss recovery. Install, update and read-only audit
share this code; the optional cache write happens only after policy admits
the finished review and the write.

[package_inspection.rs](src/package/package_inspection.rs) runs the read-only
audit: open the package, reserve one session, list the targets, review each,
finish the report and delete staging. An unavailable target stays in the
report and later targets still run. Package management keeps pinned
resolution, policy comparison, bounded reporting and the final unchanged-file
check; the application cannot publish an audit as package
acceptance.

Start at [build.rs](src/build.rs). One typed `BuildRequest` picks the host,
one explicit target or a batch, and the product: a check, a retained
Terminal artifact, a native program or a named Terminal inspection. Terminal
production keeps its canonical artifact for interpretation, rollback
comparison or explicit publication, with no native work. Inspection uses one
target and a private workspace and publishes nothing native. `build`
admits output directories, prepares the project, requests products through
package admission and the compiler's public interface, and publishes ordered
results. A refused target directory is reported beside that target and
does not hide successful siblings. Checks use throwaway
workspaces and never claim the requested product directory.

A Terminal consumer keeps the retained Build record as well as the artifact.
Its explicit entry receipt and provider selection reach the runtime
separately from host provisions. A deployment tag selects target
bodies only, not the program entry Build chose; without the receipt, the
artifact's default entry runs. Selecting the entry grants no backing storage
or host authority.

[products.rs](src/project/products.rs) translates package and standalone
product demands. [project_compilation.rs](src/project/project_compilation.rs)
shows ordinary package compilation: settle returned Builds, admit the target
batch, prepare products, reconcile package identities and delete staging.
Package management returns requests; it does not call this code. Its
completion step holds the acquired packages and one shared sponsor until
every target returns. An inadmissible target is an ordered refusal beside
successful siblings. A failed staging close is reported. SDK callers use the same `build(BuildRequest)`
function, including the `package_batch` counter probe. There is no separate
prepared-project check/native/inspection API or static graph resolver.

`package_batch --native-evidence --compare-single-targets --run` measures
the ordinary package build from before source preparation starts. With
`OMEGA_PHASE_PROFILE=1`, it records Omega stage and emission counts per
target, compares the native bytes with separate single-target requests, and
runs the host image and the reloaded Terminal artifact. Its positional expected
counts are explicit assertions; comparison requests start after that
measurement. The counters cover source completion,
Product Terminal publication, Build execution and settled checking, not each
numbered Psi stage or separate calling-policy publications.

[source_resolution.rs](src/compile/build_execution/source_resolution.rs)
discovers sources by running Build, for those consumers and for package
commands. Its
[readiness scheduler](src/compile/build_execution/source_resolution_readiness.rs)
keeps the suspended Build alive while it fetches the exact requested Build
dependency, settles that dependency's own graph, and asks Psi for
the named checked Terminal implementation. Installation checks the exact
requirement and its component closure before resuming. A normal return still
waits for every source request; an early helper finish neither replaces that
wait nor replays Build. Packages still own source acquisition,
pins, aliases and workspace selection.

The runnable [provider-readiness projects](../../../../samples/cli/probes/provider_readiness)
exercise this code through ordinary CLI review and native/Terminal
execution. Workspace observations keep the closed source graphs of helpers
that actually ran to compute their declarations. The final join
reconciles those graphs separately from Product routing; it does not fetch an
unselected member's pending Product requests. Package locks
keep these catalog execution sources for pinned offline recovery and
read-only audit, without adding Product imports.

[source_compilation.rs](src/compile/source_compilation.rs) handles one-shot
source requests from standalone products and compiler diagnostics. Its
`compile_sources`, `check_source` and `produce_source` functions all follow
the same steps: prepare an opaque compiler invocation, run its admitted Build
through [build_execution.rs](src/compile/build_execution.rs), and finish
from its exact return. The compiler no longer starts or steps
Build. An existing compiler session reuses sources and checks; these
functions do not fetch packages or replace the project-level `BuildRequest`.

[package_compilation.rs](src/package/package_compilation.rs) owns the
admitted package invocation. [build_execution.rs](src/compile/build_execution.rs)
works dependencies first: it collects generated dependency output, runs
ready Builds, and waits for required independent components before moving
on to consumers. Ready roots finish together through the
compiler's product interface. One worker thread with an explicit stack runs
the whole invocation; nested compiler calls run inline on it.

[native_completion.rs](src/compile/native_completion.rs) takes source-free pending
native products from both paths. It prepares native
work, settles the root and component scalar demands through the shared Build
evaluator, compiles that same preparation, and settles the report and
runtime provisions. A failure keeps the attempted evaluation's accounting.
Source admission comes first; native production needs no interpreter and
no evaluator callback. Check, Terminal, inspection and
component-publication requests never reach it.

The compiler returns an opaque `PackageBuild`. The application can read its
package inputs, generated bundle and required package identities, and join
component descriptions, but cannot extract its source continuation. Component executables stay in the compiler session.
`prepare_package` prepares a root Build for diagnostic callers.

[publication.rs](src/project/publication.rs) publishes admitted products;
execution uses it too. The CLI owns argument spelling, console output and
process exit codes, not these steps. Shared temporary directories belong to
`platform-custody`.

A native request names a retained or published destination. `omega run`
uses the same preparation and compilation, keeping its product for Terminal
replay before publication consumes it. Private entry RAM belongs to
that native request and reaches both package and standalone compilation.
Execution owns input replay, the native run and comparison, not a second
project compiler.

Source inspection follows the same steps and returns the compiler's
inspection product, not an invented compilation report. CLI inspection only
adapts the request and shows or adopts that artifact. Inspecting an
already-encoded artifact skips Build: it admits the supplied envelope and
neither fetches sources nor runs code.

This crate runs projects; it is not another compiler pipeline. It depends on
public interfaces and product contracts, not on intermediate source forms or
stage crates. Dependency scheduling for ordinary builds and reviews lives
here. Finishing Psi's public interface and replacing the source Build
evaluator with Terminal continuations remain open in the
[ownership migration](../../../pipeline.md#build-and-compiler-ownership-migration).
