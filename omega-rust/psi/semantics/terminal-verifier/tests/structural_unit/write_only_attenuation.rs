//! Projected attenuation preserves write-only access and the bounded leaf shape.

use super::*;

fn indexed_attenuation_module() -> TerminalModule {
    let mut module = projected_unit_call_module();
    module.structural_domains.clear();
    module.boundary_machines.clear();
    module.services.clear();
    module.root_service_reach = Default::default();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    module.machines[1].blocks[0].operations.clear();
    for machine in &mut module.machines {
        machine.entry_claims.clear();
        machine.published_service_ceiling.clear();
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        machine.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    }
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    claim_transfers.clear();
    module
}

#[test]
fn indexed_mutable_root_can_lend_a_write_only_primitive() {
    let module = indexed_attenuation_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an exact primitive subloan may attenuate mutable authority");
}

#[test]
fn indexed_write_only_attenuation_does_not_admit_aggregate_leaves() {
    let mut module = indexed_attenuation_module();
    module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
    ));
}

#[test]
fn indexed_write_only_attenuation_rejects_shared_and_unserved_owned_roots() {
    let mut module = indexed_attenuation_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentAccessExceedsSource { .. })
    ));

    // Owned projected loans have a separate custody contract; this change only
    // removes observation authority from an already exclusive borrowed root.
    module.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
    ));
}

#[test]
fn indexed_write_only_attenuation_keeps_exact_access_and_bounds() {
    let module = indexed_attenuation_module();
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let mut changed = module.clone();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        assert!(matches!(
            validate_module(&changed),
            Err(ModuleError::StructuralArgumentAccessMismatch { .. })
        ));
    }
    let mut changed = module;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut changed.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
}
