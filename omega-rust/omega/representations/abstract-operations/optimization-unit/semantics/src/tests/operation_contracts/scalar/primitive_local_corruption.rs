//! Hostile current-IR roots, SSA observations, and occurrence metadata.
use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::fixtures::primitive_locals::primitive_local_call_plan;
use crate::tests::support::id;
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation as O;
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use semantic_vocabulary::ValueId;

#[test]
fn reads_cannot_reuse_initializer_or_pre_call_observation_identity() {
    for replacement in [53, 60] {
        let plan = primitive_local_call_plan();
        let mut unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        let O::PrimitiveScalarRead { result, .. } =
            &mut unit.functions[0].blocks[0].nodes[5].operation
        else {
            panic!("post-call read")
        };
        result.value = id(replacement, ValueId::new);
        refresh_function_derivatives(&mut unit, 0);
        assert!(validate_psi_optimization_unit(&unit).is_err());
    }
}
