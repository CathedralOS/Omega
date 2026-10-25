# Project Build

Start at [build.rs](src/build.rs). One typed `BuildRequest` selects a host,
single explicit target or batch, and a check or native product. The entrance
admits output directories, prepares the project, requests products through
package/compiler facades, and publishes ordered outcomes. A target's directory
refusal stays beside that target; it does not hide successful siblings. Checks
use disposable workspaces and do not claim the requested product directory.

[products.rs](src/products.rs) translates package and standalone product demands.
[publication.rs](src/publication.rs) publishes admitted products and is also used
by execution. The CLI owns argument spelling, console output and process exits,
not this lifecycle. Shared temporary directories belong to `platform-custody`.

This is the project application owner, not another compiler pipeline. It depends
on facades and product contracts, not intermediate source representations or
stage crates. The deeper dependency/Build activation scheduler still lives in
package management and the old compiler coordinator. Moving it here, completing
Psi's facade, and replacing the checked-tree Build evaluator with Terminal
continuations remain explicit obligations of the
[ownership migration](../../../pipeline.md#build-and-compiler-ownership-migration).
