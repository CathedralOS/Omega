use super::*;

pub(super) fn parameter_return_plan(parameter_count: usize) -> AbstractOperationPlan {
    let machine = MachineId::new(10).expect("machine");
    let result = ValueId::new(100).expect("result");
    let integer = IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(10 + index as u64).expect("parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let returned = parameters.last().expect("fixture has parameters").value;
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(10).expect("block"),
            parameters,
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![AbstractOperation::Return {
                psi_edge: EdgeId::new(10).expect("edge"),
                result,
                value: returned,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

pub(super) fn direct_call_plan(parameter_count: usize) -> AbstractOperationPlan {
    let caller = MachineId::new(1).expect("caller");
    let callee = MachineId::new(2).expect("callee");
    let integer = IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let caller_parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(10 + index as u64).expect("caller parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let callee_parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(30 + index as u64).expect("callee parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let caller_result = ValueId::new(100).expect("caller result");
    let callee_result = ValueId::new(101).expect("callee result");
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(1).expect("caller block"),
                parameters: caller_parameters.clone(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: caller_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(1).expect("call"),
                        result: caller_result,
                        scalar_type,
                        callee,
                        arguments: caller_parameters
                            .iter()
                            .map(|parameter| parameter.value)
                            .collect(),
                        requirement_obligations: vec![ObligationId::new(700).unwrap()],
                        crash_continuations: vec![CrashRouteBucket {
                            cause: CrashCause::Trap,
                            alternatives: vec![CrashRouteGuard::Truth],
                        }],
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(1).expect("caller return"),
                        result: caller_result,
                        value: caller_result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: BlockId::new(2).expect("callee block"),
                parameters: callee_parameters.clone(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: callee_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![AbstractOperation::Return {
                    psi_edge: EdgeId::new(2).expect("callee return"),
                    result: callee_result,
                    value: callee_parameters.last().expect("parameter").value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}
