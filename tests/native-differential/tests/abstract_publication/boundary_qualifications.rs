//! Boundary qualification consumption through optimizer projection custody.

use super::*;
use optimization_validation::{
    OptimizedAbstractPlanProjectionError, validate_optimized_abstract_plan_projection,
    validate_transformed_psi_optimization_unit,
};

#[test]
fn whole_root_boundary_requirement_consumes_carried_qualification_through_projection() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized =
        publish_optimization_run(run(boundary_qualification_verified(), selections)).unwrap();
    let unit = optimized.unit();
    let required = semantic_vocabulary::StructuralDomainId::new(1_905).unwrap();
    let unrelated = semantic_vocabulary::StructuralDomainId::new(1_906).unwrap();

    assert!(optimized.commits().is_empty());
    assert!(
        unit.proof_questions.is_empty(),
        "D47 must mint no obligation"
    );
    assert_eq!(unit.boundary_machines[0].requires[0].domain, required);
    assert_eq!(
        unit.functions[0].structural_parameters[0].qualifications,
        [required]
    );
    let AbstractOperation::BoundaryCall {
        boundary,
        structural_arguments,
        ..
    } = &unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture must retain one boundary call")
    };
    assert_eq!(
        *boundary,
        semantic_vocabulary::BoundaryMachineId::new(1_903).unwrap()
    );
    assert!(structural_arguments[0].path.is_empty());
    assert_eq!(optimized.validation().final_unit(), unit.identity);
    assert_eq!(
        optimized.pre_physical_manifest().record().final_unit,
        unit.identity,
    );
    assert_eq!(
        optimized.pre_physical_manifest().record().projection,
        optimized.validation().identity(),
    );

    let mut missing = unit.clone();
    missing.functions[0].structural_parameters[0]
        .qualifications
        .clear();
    missing.identity = optimization_unit::recompute_psi_optimization_unit_identity(&missing);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &missing),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut widened = unit.clone();
    widened.functions[0].structural_parameters[0]
        .qualifications
        .push(unrelated);
    widened.identity = optimization_unit::recompute_psi_optimization_unit_identity(&widened);
    assert_eq!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &widened),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch),
        "a transformation cannot mint even a catalog-declared qualification",
    );

    let mut erased_requirement = unit.clone();
    erased_requirement.boundary_machines[0].requires.clear();
    erased_requirement.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&erased_requirement);
    assert_eq!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &erased_requirement,),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch),
    );

    let mut substituted_boundary = unit.clone();
    let AbstractOperation::BoundaryCall { boundary, .. } =
        &mut substituted_boundary.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *boundary = semantic_vocabulary::BoundaryMachineId::new(1_999).unwrap();
    substituted_boundary.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&substituted_boundary);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(
            optimized.verified_input(),
            &substituted_boundary,
        ),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));

    let mut projected_path = unit.clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut projected_path.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(terminal_psi::StructuralPathSegment::Field("missing".into()));
    projected_path.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&projected_path);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &projected_path),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    for corrupt in [
        |plan: &mut abstract_operations::AbstractOperationPlan| {
            plan.functions[0].structural_parameters[0]
                .qualifications
                .clear();
        },
        |plan: &mut abstract_operations::AbstractOperationPlan| {
            plan.boundary_machines[0].requires.clear();
        },
    ] {
        let mut plan = optimized.plan().clone();
        corrupt(&mut plan);
        assert_eq!(
            validate_optimized_abstract_plan_projection(
                optimized.verified_input(),
                unit,
                &plan,
                optimized.selections(),
                optimized.psi_selections(),
                optimized.identity_bundle().rule_set(),
                abstract_operations_to_abstract_operations::baseline_psi_cost_model_identity(),
                optimized.decisions(),
                optimized.pass_manifests(),
                optimized.transformation_ledger(),
                optimized.identity_bundle(),
            ),
            Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch),
            "projection replay must reject detached qualification custody",
        );
    }

    let mut wrong_path = optimized.plan().clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut wrong_path.functions[0].operations[0]
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(terminal_psi::StructuralPathSegment::Field("missing".into()));
    assert_eq!(
        validate_optimized_abstract_plan_projection(
            optimized.verified_input(),
            unit,
            &wrong_path,
            optimized.selections(),
            optimized.psi_selections(),
            optimized.identity_bundle().rule_set(),
            abstract_operations_to_abstract_operations::baseline_psi_cost_model_identity(),
            optimized.decisions(),
            optimized.pass_manifests(),
            optimized.transformation_ledger(),
            optimized.identity_bundle(),
        ),
        Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch),
    );
}

#[test]
fn projected_boundary_requirement_retains_exact_path_custody_through_prephysical_publication() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized = publish_optimization_run(run(
        partial_path_boundary_qualification_verified(),
        selections,
    ))
    .unwrap();
    let unit = optimized.unit();
    let required = semantic_vocabulary::StructuralDomainId::new(1_926).unwrap();
    let unrelated = semantic_vocabulary::StructuralDomainId::new(1_927).unwrap();
    let expected_path = vec![terminal_psi::StructuralPathSegment::Field("left".into())];

    assert!(optimized.commits().is_empty());
    assert!(unit.proof_questions.is_empty(), "D47 mints no obligation");
    assert_eq!(
        unit.functions[0].structural_parameters[0].projected_qualifications,
        [terminal_psi::StructuralPathQualification {
            path: expected_path.clone(),
            domain: required,
        }]
    );
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture must retain one boundary call")
    };
    assert_eq!(structural_arguments[0].path, expected_path);
    assert_eq!(optimized.validation().final_unit(), unit.identity);
    assert_eq!(
        optimized.pre_physical_manifest().record().final_unit,
        unit.identity,
    );

    let mut missing = unit.clone();
    missing.functions[0].structural_parameters[0]
        .projected_qualifications
        .clear();
    missing.identity = optimization_unit::recompute_psi_optimization_unit_identity(&missing);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &missing),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
    ));

    let mut widened = unit.clone();
    widened.functions[0].structural_parameters[0]
        .projected_qualifications
        .push(terminal_psi::StructuralPathQualification {
            path: vec![terminal_psi::StructuralPathSegment::Field("right".into())],
            domain: unrelated,
        });
    widened.identity = optimization_unit::recompute_psi_optimization_unit_identity(&widened);
    assert_eq!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &widened),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch),
    );

    let mut wrong_path = unit.clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut wrong_path.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::Field("right".into())];
    wrong_path.identity = optimization_unit::recompute_psi_optimization_unit_identity(&wrong_path);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(optimized.verified_input(), &wrong_path),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
    ));

    let mut detached = optimized.plan().clone();
    detached.functions[0].structural_parameters[0]
        .projected_qualifications
        .clear();
    assert_eq!(
        validate_optimized_abstract_plan_projection(
            optimized.verified_input(),
            unit,
            &detached,
            optimized.selections(),
            optimized.psi_selections(),
            optimized.identity_bundle().rule_set(),
            abstract_operations_to_abstract_operations::baseline_psi_cost_model_identity(),
            optimized.decisions(),
            optimized.pass_manifests(),
            optimized.transformation_ledger(),
            optimized.identity_bundle(),
        ),
        Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch),
    );

    assert_eq!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            optimized.plan(),
            target::NativeTarget::linux_x64(),
        ),
        Err(
            abstract_operations_to_target_operations::LoweringError::UnsupportedProjectedStructuralQualifications,
        ),
    );
}

#[test]
fn projected_function_and_call_results_cross_replay_abstract_and_prephysical_custody() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized =
        publish_optimization_run(run(projected_structural_result_verified(), selections))
            .expect("projected structural results cross a no-rewrite optimizer run");
    let expected = [terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field("payload".into())],
        domain: semantic_vocabulary::StructuralDomainId::new(1_946).unwrap(),
    }];
    let unit = optimized.unit();
    let function_result = unit.functions[0]
        .result
        .structural()
        .expect("caller returns a structural result");
    assert_eq!(function_result.projected_qualifications, expected);
    let AbstractOperation::CallStructural { result, .. } =
        &unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("caller retains one structural call")
    };
    assert_eq!(result.projected_qualifications, expected);
    assert_eq!(
        optimized.plan().functions[0]
            .result
            .structural()
            .unwrap()
            .projected_qualifications,
        expected,
    );
    let AbstractOperation::CallStructural { result, .. } =
        &optimized.plan().functions[0].operations[0]
    else {
        unreachable!()
    };
    assert_eq!(result.projected_qualifications, expected);
    assert_eq!(optimized.validation().final_unit(), unit.identity);
    assert_eq!(
        optimized.pre_physical_manifest().record().final_unit,
        unit.identity,
    );

    let mut detached = optimized.plan().clone();
    let AbstractOperation::CallStructural { result, .. } = &mut detached.functions[0].operations[0]
    else {
        unreachable!()
    };
    result.projected_qualifications.clear();
    assert_eq!(
        validate_optimized_abstract_plan_projection(
            optimized.verified_input(),
            unit,
            &detached,
            optimized.selections(),
            optimized.psi_selections(),
            optimized.identity_bundle().rule_set(),
            abstract_operations_to_abstract_operations::baseline_psi_cost_model_identity(),
            optimized.decisions(),
            optimized.pass_manifests(),
            optimized.transformation_ledger(),
            optimized.identity_bundle(),
        ),
        Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch),
    );
    let lowered = abstract_operations_to_target_operations::lower_to_target_operations(
        optimized.plan(),
        target::NativeTarget::linux_x64(),
    )
    .expect("the exact projected structural call/return closure reaches target IR");
    let target_operations::TargetOperation::ReturnStructuralCall {
        structural_parameters,
        operation_result,
        result,
        ..
    } = &lowered.functions[0].operation
    else {
        panic!("the caller retains its exact structural call/return carrier")
    };
    assert_eq!(structural_parameters[0].projected_qualifications, expected);
    assert_eq!(operation_result.projected_qualifications, expected);
    assert_eq!(result.projected_qualifications, expected);
}
