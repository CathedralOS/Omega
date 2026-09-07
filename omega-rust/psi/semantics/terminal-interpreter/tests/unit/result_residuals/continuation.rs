use super::*;

fn module(ordinary: bool, nested: bool, complete_transfer: bool) -> TerminalModule {
    let mut module = produced_partial_module(ordinary, nested);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(2),
        identity: "observe_continuation".into(),
        attachment: None,
        scalar_parameters: vec![scalar_type],
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let caller = &mut module.machines[0];
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = caller.blocks[0].terminator.clone()
    else {
        unreachable!()
    };
    if complete_transfer {
        assert!(!nested);
        let mut last = caller.blocks[0].operations[1].clone();
        last.id = operation_id(3);
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut last.kind
        else {
            unreachable!()
        };
        structural_arguments[0].path = vec![field("left")];
        caller.blocks[0].operations.push(last);
    }
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(10),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(1),
                scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(17),
            },
        },
    );
    caller.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(1),
        target: block_id(4),
        arguments: vec![value_id(1)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: if complete_transfer {
            Vec::new()
        } else {
            residual_affine_discards
        },
    };
    caller.blocks.push(Block {
        id: block_id(4),
        parameters: vec![ValueDeclaration {
            id: value_id(2),
            scalar_type,
        }],
        operations: vec![Operation {
            id: operation_id(11),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary: boundary_id(2),
                arguments: vec![value_id(2)],
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }],
        terminator: Terminator::ReturnUnit {
            edge: edge_id(4),
            trivial_affine_discards: Vec::new(),
        },
    });
    module
}

#[derive(Default)]
struct Observe {
    factory: ProducePair,
    observed: Vec<TerminalScalarValue>,
}

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-aware boundary handler required")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            ..
        } = effect
            && *boundary == boundary_id(2)
        {
            self.observed.extend(arguments.iter().copied());
            Ok(TerminalEffectResult::Unit)
        } else {
            self.factory.handle_effect_result(effect)
        }
    }
}

#[test]
fn partial_continuation_charges_before_cleanup_and_carries_scalar_bindings() {
    for ordinary in [false, true] {
        for (nested, complete_transfer) in [(false, false), (true, false), (false, true)] {
            let module = module(ordinary, nested, complete_transfer);
            let Terminator::Jump {
                residual_affine_discards,
                ..
            } = &module.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            let expected = residual_affine_discards
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            let verified = verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap();
            let certificate =
                terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
            terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
            let mut reference = None;
            for incremental in [false, true] {
                let mut execution = start(&module, ordinary);
                let mut effects = Observe::default();
                let mut meter = if incremental {
                    TerminalFuelMeter::with_allowance(0)
                } else {
                    TerminalFuelMeter::unbounded()
                };
                let mut before_cleanup = false;
                let mut after_cleanup = false;
                let mut complete = false;
                for _ in 0..64 {
                    match execution
                        .resume_with_effect_handler(&mut meter, &mut effects)
                        .unwrap()
                    {
                        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                            assert!(incremental);
                            if exhaustion.site == FuelChargeSite::Edge(edge_id(1)) {
                                assert_eq!(
                                    execution
                                        .live_affine_frontier()
                                        .cloned()
                                        .collect::<std::collections::BTreeSet<_>>(),
                                    expected
                                );
                                assert!(effects.observed.is_empty());
                                assert!(meter.usage().at(exhaustion.site).is_none());
                                before_cleanup = true;
                            }
                            if exhaustion.site == FuelChargeSite::Operation(operation_id(11)) {
                                assert!(execution.live_affine_frontier().next().is_none());
                                assert!(effects.observed.is_empty());
                                after_cleanup = true;
                            }
                            meter.replenish(1).unwrap();
                        }
                        TerminalExecutionStatus::Complete(result) => {
                            assert_eq!(result, TerminalExecutionResult::Unit);
                            complete = true;
                            break;
                        }
                        status => panic!("unexpected {status:?}"),
                    }
                }
                assert!(complete);
                assert_eq!(before_cleanup, incremental);
                assert_eq!(after_cleanup, incremental);
                assert_eq!(effects.factory.calls, usize::from(!ordinary));
                assert_eq!(
                    effects.observed,
                    vec![TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                        value: IntegerValue::Unsigned(17),
                    }]
                );
                assert!(execution.live_affine_frontier().next().is_none());
                assert_eq!(certificate.ceiling_units(), meter.usage().total_units());
                let observed = (execution.effects().to_vec(), meter.usage().clone());
                if let Some(reference) = &reference {
                    assert_eq!(&observed, reference);
                } else {
                    reference = Some(observed);
                }
            }
        }
    }
}
