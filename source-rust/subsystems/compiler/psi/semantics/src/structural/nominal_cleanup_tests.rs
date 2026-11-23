//! A record with a selected cleanup may be taken apart by that cleanup or by
//! any machine of the record's own package, and by nothing else. The
//! carrier and the machine are compared by the package their nominal
//! identities name.
use super::nominal_cleanup::decomposition_authorized;
use semantic_vocabulary::MachineId;
use terminal_psi::{
    CheckedMachineDeclaration, NominalCleanupSelection, NominalDeclarationIdentity,
    TerminalIdentityTableBuilder,
};

const OWNER: &str = "4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d";
const FOREIGN: &str = "4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e";

fn machine(id: u64) -> MachineId {
    MachineId::new(id).unwrap()
}

/// Ask whether the machine named `release`, declared in `package`, may
/// take apart a `Lease` of package `OWNER` whose cleanup is machine 1.
fn release_may_decompose(package: &str) -> Result<(), &'static str> {
    let mut identities = TerminalIdentityTableBuilder::default();
    let carrier = identities.intern(&format!(
        "named(name(nominal\\(package-owner\\(32:{OWNER}\\)\\,path\\(Lease\\)\\)))"
    ));
    let release = identities.intern(&format!(
        "nominal(package-owner(32:{package}),path(release))"
    ));
    // The hermetic semantic identity every lowered machine carries. It is
    // not in the type identity grammar, so it cannot name the owner.
    let semantic = identities.intern(&format!(
        "package:{package}::declaration(path(release),named-callable())"
    ));
    let state = identities.intern(&format!(
        "nominal(package-owner(32:{package}),path(release::entry))"
    ));
    let declaration = CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(release),
        state: NominalDeclarationIdentity::new(state),
        semantic_declaration: semantic,
        parameter_types: vec![carrier],
        callable: semantic,
        result_type: carrier,
        contract: [1; 32],
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    };
    let selection = NominalCleanupSelection {
        declaration: NominalDeclarationIdentity::new(carrier),
        cleanup_machine: machine(1),
    };
    let identities = identities.build();
    decomposition_authorized(&identities, &selection, machine(2), Some(&declaration))
}

#[test]
fn a_machine_of_the_carriers_own_package_may_take_it_apart() {
    assert_eq!(release_may_decompose(OWNER), Ok(()));
}

#[test]
fn a_machine_of_another_package_may_not_take_it_apart() {
    assert_eq!(
        release_may_decompose(FOREIGN),
        Err("only the carrier's declaring package or selected cleanup may decompose it")
    );
}
