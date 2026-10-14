//! Data-only checks of explicit ownership validation, not native preparation
//! or a claim of independent PCC. The fixture is actual public CLI output.

use super::native::VerifiedNativeArtifactInput;
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_psi::{
    Operation, OperationKind, OperationResult, RecordFieldValue, StructuralAccess,
    TerminalAffineCleanupAction, TerminalModule, Terminator,
};

fn independently_validate_ownership(module: TerminalModule) -> Result<(), String> {
    let record = build_identity_optimization_execution_record(&module)
        .expect("foundation-valid module has an identity record");
    let artifact = CanonicalTerminalArtifact::from_parts(module, record, None)
        .expect("foundation-valid mutation can be encoded");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("mutant must reach semantic validation after canonical admission");
    let input = VerifiedNativeArtifactInput::prepare(
        &artifact,
        target::TargetProfile::LinuxX64,
        artifact.semantic_module().entry,
        &[],
    )
    .expect("native preparation converts data; it does not prove issuance")
    .into_optimization_input();
    let verified = crate::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("construct exact unit before explicit semantic validation");
    let (_, unit) = verified.into_parts();
    // These acyclic fixtures require no externally admitted cycle roster.
    let frontiers = optimization_unit_semantics::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .map_err(|error| format!("ownership reconstruction: {error:?}"))?;
    let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers)
        .map_err(|error| format!("ownership attachment: {error:?}"))?;
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        &unit,
        &[],
    )
    .map_err(|error| format!("semantic ownership validation: {error:?}"))
}

#[test]
#[ignore = "explicit qualification supplies OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT"]
fn qualified_owned_cleanup_replays_transport_and_single_disposal() {
    let path = std::env::var_os("OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT")
        .expect("supply actual public CLI canonical output");
    let bytes = std::fs::read(path).expect("read actual canonical bytes");
    let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("independent canonical admission of actual artifact");
    independently_validate_ownership(artifact.semantic_module().clone())
        .expect("intact artifact must pass the explicit semantic validator");

    // This ordinary issued fixture publishes carried memberships, not an
    // independent ordinary-machine issuer registry. Mutate only actual rows.
    for constructor_child in [true, false] {
        let mut module = artifact.semantic_module().clone();
        let selected = module
            .machines
            .iter()
            .enumerate()
            .find_map(|(machine_index, machine)| {
                let sources: Vec<_> = if constructor_child {
                    machine
                        .operations()
                        .flat_map(|operation| match &operation.kind {
                            OperationKind::EstablishRecord { fields }
                            | OperationKind::EstablishStructuralCase { fields, .. } => fields
                                .iter()
                                .filter_map(|field| match &field.value {
                                    RecordFieldValue::Structural(argument) => Some(argument.place),
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                            _ => Vec::new(),
                        })
                        .collect()
                } else {
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| match &block.terminator {
                            Terminator::Jump {
                                structural_arguments,
                                ..
                            } => structural_arguments
                                .iter()
                                .map(|argument| argument.place)
                                .collect::<Vec<_>>(),
                            _ => Vec::new(),
                        })
                        .collect()
                };
                sources.into_iter().find_map(|place| {
                    let (operation, _) = machine.producing_operation(place)?;
                    let result = operation.result.as_structural()?;
                    (!result.qualifications.is_empty()).then_some((machine_index, operation.id))
                })
            })
            .expect("fixture has an actual qualified source at this transport boundary");
        let producer = module.machines[selected.0]
            .operation(selected.1)
            .unwrap()
            .kind
            .direct_callee()
            .expect("actual qualified source is a call result");
        // Canonical call/result equality remains intact: weaken the actual
        // producer's result declaration and every matching call result together.
        // Qualified receiving constructor/arrival/formal declarations stay intact.
        let terminal_psi::TerminalMachineResult::Structural(result) = &mut module
            .machines
            .iter_mut()
            .find(|machine| machine.id == producer)
            .unwrap()
            .result
        else {
            unreachable!()
        };
        assert!(!result.qualifications.is_empty());
        result.qualifications.clear();
        for machine in &mut module.machines {
            for block in &mut machine.blocks {
                for operation in &mut block.operations {
                    if operation.kind.direct_callee() == Some(producer) {
                        let OperationResult::Structural(result) = &mut operation.result else {
                            unreachable!()
                        };
                        result.qualifications.clear();
                    }
                }
            }
        }
        let reason = independently_validate_ownership(module)
            .expect_err("deleted actual source membership cannot satisfy unchanged destinations");
        assert_eq!(
            reason,
            "ownership reconstruction: VerifiedOptimizationUnitProjectionMismatch"
        );
        println!(
            "weakened actual producer used by {} rejected at reported boundary: {reason}",
            if constructor_child {
                "constructor-child"
            } else {
                "owned-edge"
            }
        );
    }

    // A mutable unqualified callee cannot preserve the caller's opaque
    // membership merely because it writes back the scalar it just observed.
    let mut module = artifact.semantic_module().clone();
    let next_id = semantic_vocabulary::OperationId::new(
        module
            .machines
            .iter()
            .flat_map(|machine| machine.operations())
            .map(|operation| operation.id.get())
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    let (machine_index, parameter_index, block_index, read_index) = module.machines.iter()
        .enumerate().find_map(|(mi, machine)| machine.structural_parameters.iter()
            .enumerate().find_map(|(pi, parameter)| {
                if parameter.access != StructuralAccess::SharedBorrow
                    || parameter.qualifications.is_empty() { return None; }
                machine.blocks.iter().enumerate().find_map(|(bi, block)|
                    block.operations.iter().enumerate().find_map(|(oi, operation)|
                        matches!(&operation.kind, OperationKind::ScalarStructuralField { source, .. }
                            if *source == parameter.place).then_some((mi, pi, bi, oi))))
            })).expect("fixture has an actual qualified scalar observer");
    let observer = &mut module.machines[machine_index];
    let observer_id = observer.id;
    let parameter = &mut observer.structural_parameters[parameter_index];
    let position = parameter.position as usize;
    parameter.access = StructuralAccess::MutableBorrow;
    parameter.qualifications.clear();
    assert!(parameter.projected_qualifications.is_empty());
    let block = &mut observer.blocks[block_index];
    let read = &block.operations[read_index];
    let value = read
        .result
        .as_scalar()
        .expect("actual read has a scalar result")
        .id;
    let OperationKind::ScalarStructuralField {
        source,
        path,
        field,
    } = &read.kind
    else {
        unreachable!()
    };
    assert!(path.is_empty(), "actual observer reads a direct field");
    let store = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: next_id,
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: *source,
            path: Vec::new(),
            field: *field,
            value,
            range_obligation: None,
        },
    };
    block.operations.insert(read_index + 1, store);
    let mut calls = 0;
    for machine in &mut module.machines {
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if operation.kind.direct_callee() == Some(observer_id) {
                    let arguments = operation.kind.structural_arguments_mut().unwrap();
                    assert_eq!(arguments[position].access, StructuralAccess::SharedBorrow);
                    arguments[position].access = StructuralAccess::MutableBorrow;
                    calls += 1;
                }
            }
        }
    }
    assert!(calls > 0, "mutate real calls to the actual observer");
    let reason = independently_validate_ownership(module)
        .expect_err("unqualified mutable call must not retain opaque caller membership");
    assert_eq!(
        reason,
        "ownership reconstruction: StructuralCallContractMismatch { machine: MachineId(1), block: BlockId(9), node: 0 }"
    );
    println!("mutating unqualified observer rejected: {reason}");

    let mut module = artifact.semantic_module().clone();
    let mut doubled = false;
    for machine in &mut module.machines {
        for block in &mut machine.blocks {
            let rosters: Vec<_> = match &mut block.terminator {
                Terminator::Jump {
                    cleanup_actions, ..
                } => vec![cleanup_actions],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    vec![
                        &mut when_true.cleanup_actions,
                        &mut when_false.cleanup_actions,
                    ]
                }
                Terminator::StructuralCase { cases, .. } => cases
                    .iter_mut()
                    .map(|case| &mut case.cleanup_actions)
                    .collect(),
                _ => Vec::new(),
            };
            for actions in rosters {
                if let Some(action) = actions
                    .iter()
                    .find(|action| {
                        matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
                    })
                    .cloned()
                {
                    actions.push(action);
                    doubled = true;
                    break;
                }
            }
            if doubled {
                break;
            }
        }
        if doubled {
            break;
        }
    }
    assert!(
        doubled,
        "fixture must contain a real residual cleanup action"
    );
    let reason = build_identity_optimization_execution_record(&module).expect_err(
        "canonical uniqueness rejects duplicate actual residuals before semantic replay",
    );
    assert!(matches!(
        &reason,
        terminal_codec::PsiOptimizationExecutionRecordBuildError::Semantic(
            terminal_codec::CodecError::NonCanonicalOrder(
                "partial affine residual discards are unique"
            )
        )
    ));
    println!("double cleanup rejected by canonical foundation: {reason:?}");
}
