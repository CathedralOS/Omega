# Archived native derivation records

The V1 native producer wrote `OMGNTI41` alongside a canonical Psi publication.
Its reader accepted the authorized publication and decoded the selected entry,
provider plans, installation assumptions, package permissions and operation
evidence needed to reconstruct that producer's retained native proposal.
[Codec and format tests](saved_inputs.rs) · [Record vocabulary](vocabulary.rs)

The V1 package-review cases retain the Windows proposal round trip and the macOS
descriptor callback execution, including its native image checks. The V1
installation-assumption review and independent component reader also consumed
the record. These files preserve the calls and assertions for future migration.
[Package review cases](v1-tests/package_review.rs) · [Image checks](v1-tests/native_entry_controls.rs) · [V1 installation example](../../../../compiler/v1-build-orchestration/examples/installation_assumption_review/saved_project.rs) · [V1 component reader](../../../../compiler/v1-products/src/independent_component.rs)

The current compiler accepts saved canonical `.psi` through its native entry
point. The receiver authorizes the manifest and attaches an installation for that
program and target. Explicit object validation reads the stage outputs retained
by the current compilation.
[Saved Psi entry point](../../../../../../source-rust/subsystems/compiler/psi-to-native/psi_to_native.rs) · [Published-object verification](../../../../../../source-rust/subsystems/compiler/psi-to-native/verification/published-objects/) · [Saved-program regression tests](../../../../../../source-rust/subsystems/compiler/psi-to-native/tests/saved_linux_program.rs)
