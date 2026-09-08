//! Primitive locals retain their own operation-result place and scalar observations.

use super::*;

pub(crate) fn primitive_local_plan() -> AbstractOperationPlan {
    let mut plan = super::scalar_units::write_only_store_plan(false);
    let function = &mut plan.functions[0];
    let parameter = function.structural_parameters.remove(0);
    let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } = function.operations[1] else {
        panic!("fixture store");
    };
    let result = terminal_psi::StructuralOperationResult {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    function.operations[1] = AbstractOperation::EstablishPrimitiveLocal {
        psi_operation: id(57, OperationId::new),
        result,
        value,
    };
    let read = |operation, value_id| AbstractOperation::PrimitiveScalarRead {
        psi_operation: id(operation, OperationId::new),
        result: AbstractResult {
            value: id(value_id, ValueId::new),
            scalar_type: value.scalar_type,
        },
        source: parameter.place,
    };
    function.operations.splice(
        2..2,
        [
            read(59, 60),
            AbstractOperation::PrimitiveLocalStore {
                psi_operation: id(61, OperationId::new),
                destination: parameter.place,
                value,
            },
            read(62, 63),
        ],
    );
    plan
}

pub(crate) fn primitive_local_unit() -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &primitive_local_plan(),
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

pub(crate) fn primitive_local_call_plan() -> AbstractOperationPlan {
    let mut plan = primitive_local_plan();
    let mut callee = super::scalar_units::write_only_store_plan(false)
        .functions
        .remove(0);
    callee.machine = id(70, MachineId::new);
    callee.entry = id(71, BlockId::new);
    callee.block_entries[0].block = callee.entry;
    let parameter = &mut callee.structural_parameters[0];
    parameter.place = id(72, PlaceId::new);
    parameter.access = terminal_psi::StructuralAccess::MutableBorrow;
    let value = AbstractResult {
        value: id(73, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    };
    callee.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: id(74, OperationId::new),
            result: value.value,
            scalar_type: value.scalar_type,
            value: IntegerValue::Signed(0),
        },
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation: id(75, OperationId::new),
            destination: parameter.clone(),
            value,
        },
        AbstractOperation::ReturnUnit {
            psi_edge: id(76, EdgeId::new),
            cleanup_actions: Vec::new(),
        },
    ];
    plan.functions[0].operations.insert(
        4,
        AbstractOperation::CallUnit {
            psi_operation: id(77, OperationId::new),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: id(54, PlaceId::new),
                path: Vec::new(),
                access: parameter.access,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    plan.functions.push(callee);
    plan
}
