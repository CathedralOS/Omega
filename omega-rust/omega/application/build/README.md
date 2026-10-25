# Project Build

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
compilation: admit the target batch, schedule its Builds and products, then reconcile
package identities and dispose staging. Package management returns requests; it
does not invoke this route. Its completion owner retains acquisition custody and
one aggregate sponsor until every target returns. An inadmissible target remains
an ordered refusal beside successful siblings. A failed staging close is reported.
The prepared-project native/check/inspection APIs here also serve SDK callers;
package-management compilation exports have been removed.

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
stage crates. Candidate review still invokes checking in package management;
ordinary dependency scheduling is now here. Moving review scheduling, completing
Psi's facade, and replacing the checked-tree Build evaluator with Terminal
continuations remain explicit obligations of the
[ownership migration](../../../pipeline.md#build-and-compiler-ownership-migration).
