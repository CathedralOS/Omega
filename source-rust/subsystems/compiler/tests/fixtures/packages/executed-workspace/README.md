# Executed workspace source acquisition

Owning acceptance: `PIPELINE-ARCHITECTURE-ENFORCEMENT` and `TERMINAL-BUILD-SEED`.

The Git SDK publishes these authored sources into a temporary real repository.
The root's helper emits three members. The selected member computes its name,
then requests a declared sibling through a relative Path. The client selects
two different names from the same repository and aliases one member twice.
Its authored result is native exit 0 and Terminal Unit/exit 0.

The unselected member's first Build registers its Git library and selects
the library's provider for its successor, `name.omg`. The successor asks that
provider for the member name. Its missing Product dependency stays unopened,
and the invalid Product body stays unchecked.

The package-manager test supplies two Git references to the same helper. A
fresh catalog review must keep the Git source used for each registration,
including after a new commit returns the same name. The test then changes
the provider result and checks that the catalog rejects the duplicate name.
Run `cargo test -p package-manager --lib catalog_review_keeps_the_provider`.
