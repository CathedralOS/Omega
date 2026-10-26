# Project Build

Install/update/resume enter [package_commands.rs](src/package_commands.rs):
open package transaction custody, admit the command, execute every requested
target review, then return the complete roster for decisions and publication.
An error leaves accepted files unchanged and releases the transaction on return.
One caller-owned sponsored session spans all targets and automatic resume rounds;
discard stays a package-file operation and does not compile source. Package
management selects discovery/bound policy passes and admits exact reuse.
[package_review.rs](src/package_review.rs) executes each admitted pass dependency-first:
prepare Build, satisfy component demands, complete review, then return evidence
to package policy. It retains suspended consumer Builds through cache-miss
recovery. Install/update and read-only audit share this executor; optional cache
publication occurs only after policy admits the completed review and cache write.

[package_inspection.rs](src/package_inspection.rs) owns the read-only audit
lifecycle: open package custody, reserve one session, prepare the target roster,
review each target, finish the report and dispose staging. An unavailable target
remains in the report while later targets continue. Package management retains
pinned resolution, policy comparison, bounded reporting and the final unchanged-file
check; the application cannot publish an audit as package acceptance.

Start at [build.rs](src/build.rs). One typed `BuildRequest` selects a host,
single explicit target or batch, and a check, native or named Terminal inspection
product. Inspection has one target context and uses a private workspace without
native publication. The entrance
admits output directories, prepares the project, requests products through
package admission and compiler facades, and publishes ordered outcomes. A target's directory
refusal stays beside that target; it does not hide successful siblings. Checks
use disposable workspaces and do not claim the requested product directory.

[products.rs](src/products.rs) translates package and standalone product demands.
[project_compilation.rs](src/project_compilation.rs) shows ordinary package
compilation: settle returned Builds, admit the target batch and prepare products, then reconcile
package identities and dispose staging. Package management returns requests; it
does not invoke this route. Its completion owner retains acquisition custody and
one aggregate sponsor until every target returns. An inadmissible target remains
an ordered refusal beside successful siblings. A failed staging close is reported.
SDK callers use the same `build(BuildRequest)` entrance, including the
`package_batch` counter probe and CLI `qualify_inputs` replay example. There is
no separate prepared-project check/native/inspection API or static graph resolver.

[package_compilation.rs](src/package_compilation.rs) owns the admitted package
invocation. [build_execution.rs](src/build_execution.rs) advances dependency-first
frontiers, joins generated dependency output, executes ready Builds, and waits for
demanded independent components before advancing consumers. Ready roots complete
together through the compiler's product facade. One explicit stack worker spans
the invocation; nested compiler calls execute inline on that worker.

The compiler returns an opaque `PackageBuild`: the application can inspect its
package inputs, generated bundle and demanded package identities, and join
component descriptions, but cannot extract its source continuation. Component
executable custody stays in the compiler session. `prepare_package` prepares a
root Build for diagnostic consumers; `join_dependency_builds` leaves the root
unexecuted and returns its joined inputs. The corpus uses these same operations,
not a test-only dependency scheduler.

[publication.rs](src/publication.rs) publishes admitted products and is also used
by execution. The CLI owns argument spelling, console output and process exits,
not this lifecycle. Shared temporary directories belong to `platform-custody`.

Native requests name a retained or published destination. `omega run` uses the
same preparation and compilation route, retaining its product for Terminal
replay before publication consumes it. Private entry RAM belongs to that native
request and reaches both package and standalone compilation. Execution owns
input replay, native invocation and comparison, not a second project compiler.

Source inspection uses this same lifecycle and returns the compiler's explicit
inspection product, not a fabricated compilation report. CLI inspection only
adapts the request and presents/adopts that artifact. Already-encoded artifact
inspection remains independent of Build: it admits the supplied envelope and
does not acquire sources or execute code.

This is the project application owner, not another compiler pipeline. It depends
on facades and product contracts, not intermediate source representations or
stage crates. Ordinary and review dependency scheduling are now here.
Completing Psi's facade and replacing the checked-tree Build evaluator with Terminal
continuations remain explicit obligations of the
[ownership migration](../../../pipeline.md#build-and-compiler-ownership-migration).
