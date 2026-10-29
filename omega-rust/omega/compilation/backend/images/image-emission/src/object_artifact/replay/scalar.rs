//! Scalar-body replay: stack depth, conditional and division regions,
//! control-flow graphs, cleanup preservation, and scalar field stores.

pub(crate) mod call_stack;
pub(crate) mod cleanup_preservation;
pub(crate) mod conditional_call_paths;
pub(crate) mod conditional_regions;
pub(crate) mod conditional_stack;
pub(crate) mod control_cleanup;
pub(crate) mod control_flow;
pub(crate) mod division_stack;
pub(crate) mod shared_convergence;
pub(crate) mod stack;
pub(crate) mod stack_mutation;
pub(crate) mod stack_regions;
pub(crate) mod structural_scalar_field_store;

#[cfg(test)]
fn stack_mutation(
    offset: usize,
    byte_count: usize,
    kind: machine_code::ScalarStackMutationKind,
) -> machine_code::ScalarStackMutation {
    machine_code::ScalarStackMutation {
        offset,
        byte_count,
        kind,
    }
}
