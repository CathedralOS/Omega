use super::*;

#[test]
fn structural_observations_read_only_the_active_case_at_the_selected_call() {
    use crate::boundary::structural_inputs::case_membership::StructuralCaseContents;
    use crate::boundary::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};
    use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralArgument, StructuralCaseDeclaration,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };

    let mut program = crate::engine::calls::frames::tests::empty_program();
    let module = Arc::get_mut(&mut program).unwrap().fixture_module_mut();
    let ty = StructuralTypeId::new(1).unwrap();
    let int = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let case = |n| StructuralCaseId::new(n).unwrap();
    let field = |n| StructuralFieldId::new(n).unwrap();
    let variant = |n, count| StructuralCaseDeclaration {
        declaration_identity: Default::default(),
        identity: Default::default(),
        id: case(n),
        fields: (1..=count)
            .map(|i| StructuralFieldDeclaration {
                declaration_identity: Default::default(),
                inline_carrier: None,
                id: field(i),
                identity: Default::default(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(int)),
                storage: None,
            })
            .collect(),
    };
    module.types.structural_types.push(StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty,
        identity: Default::default(),
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Sum {
            cases: vec![variant(1, 1), variant(2, 5), variant(3, 0)],
        },
        scalar_restriction: None,
        requires: vec![],
    });
    let caller = module.machines[0].id;
    let mut helper = module.machines[0].clone();
    helper.id = MachineId::new(2).unwrap();
    let place = PlaceId::new(1).unwrap();
    helper.parameter_order = vec![
        BoundaryParameterKind::Scalar,
        BoundaryParameterKind::Structural,
    ];
    helper
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type: ty,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: vec![],
            projected_qualifications: vec![],
            additional_input_qualifications: vec![],
        });
    let callee = helper.id;
    module.machines.push(helper);
    let operation = Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee,
            arguments: vec![ValueId::new(1).unwrap()],
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![StructuralArgument::whole(place, StructuralAccess::Owned)],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    );
    module.machines[0].blocks[0]
        .operations
        .push(operation.clone());
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(10_000, 1, 1, 1, 10_000, 100_000, 10_000, 100_000)
            .unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 1000)
        .unwrap();
    let mut execution = TerminalExecution::start_with_program(
        program,
        &[],
        Default::default(),
        None,
        Some(account),
    )
    .unwrap();
    let selector = TerminalCallObservation {
        caller,
        site: InvocationSite::Operation(operation.id),
        callee,
        argument: 1,
    };
    execution.select_call_observations(&[selector]).unwrap();
    let value = crate::TerminalStructuralValue::minted(1, ty, vec![]);
    let root = StructuralRuntimePlace::from(&value);
    execution.structural_values.insert(place, value).unwrap();
    let call = terminal_psi::direct_call_parts(&operation.kind).unwrap();
    for index in 1..=5 {
        let location = StructuralScalarRuntimeField::case_payload(root.clone(), field(index));
        execution
            .structural_scalar_fields
            .insert(
                location,
                crate::TerminalScalarValue::Integer {
                    scalar_type: int,
                    value: IntegerValue::Unsigned((index * 7).into()),
                },
            )
            .unwrap();
    }
    for (selected, fields) in [(2, 5), (1, 1), (3, 0)] {
        execution
            .structural_cases
            .insert(
                root.clone(),
                StructuralCaseContents {
                    structural_type: ty,
                    case: case(selected),
                },
            )
            .unwrap();
        execution.observe_direct_call(operation.id, call).unwrap();
        let records = &execution.call_observations.as_ref().unwrap().records;
        let last = records.last().unwrap();
        assert_eq!(last.call, selector);
        assert_eq!(last.values.len(), fields + 1);
        assert_eq!(
            last.values[0],
            TerminalBoundaryStorageValue {
                path: vec![],
                contents: crate::TerminalStorageContents::Case(case(selected))
            }
        );
        for (i, value) in last.values[1..].iter().enumerate() {
            assert_eq!(
                value.path,
                [
                    CanonicalStructuralPathSegment::Case(case(selected)),
                    CanonicalStructuralPathSegment::Field(field(i as u64 + 1))
                ]
            );
            assert_eq!(
                value.contents,
                scalar_contents(crate::TerminalScalarValue::Integer {
                    scalar_type: int,
                    value: IntegerValue::Unsigned(((i + 1) * 7) as u128)
                })
            );
        }
    }
    assert_eq!(
        execution.call_observations.as_ref().unwrap().records.len(),
        3
    );
    let mut changed = call.structural_arguments.to_vec();
    changed[0].access = StructuralAccess::SharedBorrow;
    assert!(
        execution
            .observe_direct_call(
                operation.id,
                DirectCallParts {
                    structural_arguments: &changed,
                    ..call
                }
            )
            .is_err()
    );
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .fixture_module_mut()
        .types.structural_types[0]
        .shape = StructuralTypeShape::Opaque;
    assert!(execution.observe_direct_call(operation.id, call).is_err());
    assert_eq!(
        execution.call_observations.as_ref().unwrap().records.len(),
        3
    );
}
use crate::embedding::instance::TerminalInstance;
use build_model::accounting::BuildEvaluationSponsor;
use build_model::contracts::{
    BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, ValueId};
use std::sync::Arc;
use terminal_psi::{Operation, OperationKind, OperationResult, Terminator, ValueDeclaration};

#[test]
fn exact_calls_retain_executed_integer_arguments_and_discard_crashed_invocations() {
    for crash in [false, true] {
        let mut program = crate::engine::calls::frames::tests::empty_program();
        let module = Arc::get_mut(&mut program).unwrap().fixture_module_mut();
        let mut callee = module.machines[0].clone();
        callee.id = MachineId::new(2).unwrap();
        callee.contract.id = ContractId::new(2).unwrap();
        let typed = |id| ValueDeclaration {
            id: ValueId::new(id).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            qualifications: Default::default(),
        };
        callee.parameters.push(typed(1));
        callee.parameter_order.push(BoundaryParameterKind::Scalar);
        let caller = &mut module.machines[0];
        let selector = |id| TerminalCallObservation {
            caller: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(id).unwrap()),
            callee: MachineId::new(2).unwrap(),
            argument: 0,
        };
        let block = &mut caller.blocks[0];
        for (id, n) in [(1, 19), (3, 41), (5, 87)] {
            let value = typed(id);
            block.operations.push(Operation::unsuspended(
                OperationId::new(id).unwrap(),
                OperationResult::Scalar(value),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(n),
                },
            ));
            block.operations.push(Operation::unsuspended(
                OperationId::new(id + 1).unwrap(),
                OperationResult::Unit,
                OperationKind::CallUnit {
                    callee: callee.id,
                    arguments: vec![value.id],
                    erased_arguments: vec![],
                    erased_proof_arguments: vec![],
                    structural_arguments: vec![],
                    claim_transfers: vec![],
                    requirement_obligations: vec![],
                    crash_continuations: vec![],
                },
            ));
        }
        if crash {
            block.terminator = Terminator::Crash {
                edge: EdgeId::new(1).unwrap(),
                cause: semantic_vocabulary::CrashCause::Trap,
                site_guard: vec![],
                frontier_lower_bound: vec![],
            };
        }
        module.machines.push(callee);
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(10_000, 1, 1, 1, 10_000, 100_000, 10_000, 100_000)
                .unwrap(),
            EvaluationMeasurementIdentity::TerminalPsi {
                work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            },
        )
        .unwrap();
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 1000)
            .unwrap();
        let mut execution = TerminalExecution::start_with_program(
            program.clone(),
            &[],
            Default::default(),
            None,
            Some(account.clone()),
        )
        .unwrap();
        assert!(execution.select_call_observations(&[selector(1)]).is_err());
        assert!(
            execution
                .select_call_observations(&[selector(2), selector(2)])
                .is_err()
        );
        // Deliberately reverse selector order; the values must follow execution.
        execution
            .select_call_observations(&[selector(6), selector(2)])
            .unwrap();
        let mut instance =
            TerminalInstance::new(crate::TerminalProgram { program }, execution, vec![]);
        assert!(instance.take_call_observations().is_empty());
        let mut meter = build_model::work::sponsored_meter(account.clone(), Some(1000)).unwrap();
        let status = instance.invoke(&mut meter).unwrap();
        let usage = account.snapshot().unwrap();
        assert_eq!(usage.record().work_units, meter.usage().total_units());
        let records = instance.take_call_observations();
        if crash {
            assert!(matches!(status, crate::TerminalExecutionStatus::Crashed(_)));
            assert!(records.is_empty());
        } else {
            assert!(matches!(
                status,
                crate::TerminalExecutionStatus::Complete(_)
            ));
            let values: Vec<_> = records
                .iter()
                .map(|record| {
                    let [
                        TerminalBoundaryStorageValue {
                            path,
                            contents:
                                crate::TerminalStorageContents::Scalar {
                                    value: crate::TerminalScalarValue::Integer { value, .. },
                                    ..
                                },
                        },
                    ] = &record.values[..]
                    else {
                        panic!("wrong values: {:?}", record.values)
                    };
                    assert!(path.is_empty());
                    (record.call, *value)
                })
                .collect();
            assert_eq!(
                values,
                [
                    (selector(2), IntegerValue::Unsigned(19)),
                    (selector(6), IntegerValue::Unsigned(87))
                ]
            );
            assert!(instance.take_call_observations().is_empty());
        }
    }
}
