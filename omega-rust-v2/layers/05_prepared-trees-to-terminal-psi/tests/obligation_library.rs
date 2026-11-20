//! `VerifyLibraryImplementation` reaches L5 in every source package, and L5
//! lowers those packages, but no Psi operation or edge corresponds to it.
//!
//! L3 records the obligation on the standard library's own declaration of
//! `access\new_plan` (`toolchain_declarations/core_interface.rs`, `configure_roles`)
//! and copies that library row into each importing package (`import`). The row's
//! site lies in the library tree, its subject is a library machine, and Psi
//! lowers only local declarations: every `CallTarget` names a local machine, a
//! local member selection or a runtime descriptor. The consumer is whoever
//! verifies and links the library implementation, not this package's Psi.
//!
//! The twins below change the user program. Their Psi differs; the imported row
//! does not. This pins the absence of a Psi receiver. It is not a witness and is
//! not registered in `tools/spec-corpus/l5_output_checks.tsv`.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_typed::ObligationKind;

#[test]
fn a_library_implementation_row_is_imported_unchanged_and_names_no_local_psi_declaration() {
    let selected = omega_toolchain::core_interface().id("access\\new_plan");
    let mut rows = Vec::new();
    for source in [
        "machine idle(){}",
        "machine leaf(x:u8)->u8 {x} machine caller(y:u8)->u8 {leaf(y)}",
    ] {
        let lowered = pipeline::lower(source);
        let found = lowered.obligations("VerifyLibraryImplementation");
        let [row] = found.as_slice() else {
            panic!("{source}: expected the one imported library row, found {found:?}")
        };
        let ObligationKind::VerifyLibraryImplementation(machine) = &row.kind else {
            unreachable!()
        };
        assert_eq!(machine, selected, "{source}: the row names the library's new_plan");
        assert_ne!(
            row.site.package.as_str(),
            pipeline::PACKAGE,
            "{source}: the row's site is the library declaration"
        );
        assert_eq!(row.site.package.as_str(), machine.package.as_str(), "{source}");

        let package = &lowered.package;
        let names = |owner: &str, path: &str| owner == machine.package.as_str() && path == machine.path.as_str();
        assert!(
            !package
                .machines
                .iter()
                .any(|m| names(m.declaration.package.as_str(), m.declaration.path.as_str())),
            "{source}: no lowered machine is the library implementation"
        );
        assert!(
            !package
                .machine_templates
                .iter()
                .any(|t| names(t.declaration.package.as_str(), t.declaration.path.as_str())),
            "{source}: no machine template is the library implementation"
        );
        assert!(
            package
                .machines
                .iter()
                .all(|m| m.declaration.package.as_str() == pipeline::PACKAGE),
            "{source}: Psi holds only this package's machines"
        );
        rows.push(((*row).clone(), package.machines.len()));
    }
    let [(first, idle), (second, calling)] = rows.as_slice() else {
        unreachable!()
    };
    assert_ne!(idle, calling, "the twins lower different programs");
    assert_eq!(first, second, "the imported row does not depend on the importing program");
}
