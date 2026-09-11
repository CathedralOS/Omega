//! Function count must not multiply the module's retained declaration payload.
use abstract_operations::{AbstractOperation, StructuralTypeCatalog};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, MachineId, ScalarType, StructuralTypeId,
};
use target_operations::TargetOperation;
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

#[test]
fn module_type_catalog_is_shared_across_functions_and_native_stages() {
    let native = target::NativeTarget::linux_x64();
    let (mut source, _, _) = crate::tests::fixtures::shared_unit_calls::fixture(native);
    for number in 2..=256 {
        source
            .structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: StructuralTypeId::new(number).unwrap(),
                identity: format!("test::Unused{number}"),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            });
    }
    let leaf = source.functions[2].clone();
    for number in 4..=64 {
        let mut function = leaf.clone();
        function.machine = MachineId::new(number).unwrap();
        function.entry = BlockId::new(number).unwrap();
        function.block_entries[0].block = function.entry;
        function.operations = vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(number).unwrap(),
            cleanup_actions: Vec::new(),
        }];
        source.functions.push(function);
    }
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    assert_eq!(target.functions.len(), 64);
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    assert!(
        source
            .structural_types
            .shares_storage_with(&unit.structural_types)
    );
    for function in &target.functions {
        let TargetOperation::UnitBody(body) = &function.operation else {
            panic!("Unit body");
        };
        assert!(
            source
                .structural_types
                .shares_storage_with(&body.structural_types)
        );
    }
    let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
    assert_eq!(legal.plan().scalar_functions.len(), 64);
    for function in &legal.plan().scalar_functions {
        let contract = function
            .structural
            .as_ref()
            .expect("borrowed structural parameter");
        assert!(
            source
                .structural_types
                .shares_storage_with(&contract.structural_types)
        );
    }
    crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();

    // Independent decoding has different storage, but identical content and identity.
    let copied: StructuralTypeCatalog = source.structural_types.to_vec().into();
    assert!(!copied.shares_storage_with(&source.structural_types));
    assert_eq!(copied, source.structural_types);
    let mut decoded = legal.plan().clone();
    for function in &mut decoded.scalar_functions {
        function.structural.as_mut().unwrap().structural_types = copied.clone();
    }
    assert_eq!(decoded, *legal.plan());
    assert_eq!(
        legalized_operations::legalized_operation_plan_identity(&decoded),
        legalized_operations::legalized_operation_plan_identity(legal.plan())
    );
    crate::validate_legalized_operations(&target, &source, &unit, decoded).unwrap();

    // An independently edited occurrence must still fail replay, without mutating
    // its siblings or the authoritative source catalog.
    let mut corrupt = legal.plan().clone();
    corrupt.scalar_functions[0]
        .structural
        .as_mut()
        .unwrap()
        .structural_types
        .make_mut()[0]
        .identity
        .push_str("::changed");
    assert_ne!(corrupt, *legal.plan());
    assert!(
        source.structural_types.shares_storage_with(
            &corrupt.scalar_functions[1]
                .structural
                .as_ref()
                .unwrap()
                .structural_types
        )
    );
    assert!(crate::validate_legalized_operations(&target, &source, &unit, corrupt).is_err());
}

#[test]
fn structural_call_return_retains_the_module_catalog() {
    let (source, target, _) =
        crate::tests::fixtures::projected_structural_call_return::projected_fixture(
            target::NativeTarget::linux_x64(),
        );
    let TargetOperation::ReturnStructuralCall {
        structural_types, ..
    } = &target.functions[0].operation
    else {
        panic!("structural call return");
    };
    assert!(
        source
            .structural_types
            .shares_storage_with(structural_types)
    );
}
