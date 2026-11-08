//! Which call result types can be a checked structural result, decided once
//! per value-plan build for every call the flow facts record.
//!
//! Planning a structural call value asks whether its substituted result type
//! admits a checked structural result binding. Each ask builds a fresh shape
//! collector and shapes the whole type, about 45 µs, and the product program
//! asks 466 times about 231 distinct types. The answer is a pure function of
//! the typed program and the type reference: the asking site always starts
//! from an empty collector and passes no binders. So the build answers every
//! distinct result type of every flow call up front, on the checking pool,
//! and the site reads the answer here. A site whose type is not in the table
//! (a call the flow facts do not record) shapes it as before.

use checked_trees::FlowFacts;
use typed_trees::types::TypeReferenceHandle;

use crate::execution::terminal_unit::control::checked_structural_result_type;
use crate::execution::terminal_unit::types::{CarrierIndex, ShapeCollector};

/// Every flow call's result type, ascending by handle, with whether it admits
/// a checked structural result.
#[derive(Clone, Default)]
pub(super) struct CallResultTypes(std::sync::Arc<[(TypeReferenceHandle, bool)]>);

impl CallResultTypes {
    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        carriers: &CarrierIndex,
    ) -> Self {
        use rayon::prelude::*;
        let mut result_types = flow
            .control
            .states
            .iter()
            .filter_map(|(_, state_flow)| {
                let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
                let state = program.state_by_symbol(state_flow.state_symbol)?;
                Some(
                    crate::flow::call_flows(&flow.control, state_flow.calls)
                        .iter()
                        .filter_map(move |call| {
                            validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                call.authored_expression,
                            )
                        }),
                )
            })
            .flatten()
            .collect::<Vec<_>>();
        result_types.sort_unstable_by_key(|reference| handle_order(*reference));
        result_types.dedup();
        let answers = validation::checking_thread_pool().install(|| {
            result_types
                .par_iter()
                .map(|&reference| (reference, admits(program, carriers, reference)))
                .collect::<Vec<_>>()
        });
        Self(answers.into())
    }

    /// Whether `result_type` admits a checked structural result, from the
    /// table when the build answered it, else by shaping it now.
    pub(super) fn admits(
        &self,
        program: &dyn ::validation::CheckingInput,
        carriers: &CarrierIndex,
        result_type: TypeReferenceHandle,
    ) -> bool {
        match self
            .0
            .binary_search_by_key(&handle_order(result_type), |(reference, _)| {
                handle_order(*reference)
            }) {
            Ok(position) => self.0[position].1,
            Err(_) => admits(program, carriers, result_type),
        }
    }
}

fn admits(
    program: &dyn ::validation::CheckingInput,
    carriers: &CarrierIndex,
    result_type: TypeReferenceHandle,
) -> bool {
    checked_structural_result_type(
        program,
        &mut ShapeCollector::with_carrier_index(program, carriers.clone()),
        result_type,
        &[],
    )
    .is_some()
}

fn handle_order(reference: TypeReferenceHandle) -> (u32, u32) {
    (reference.arena_index(), reference.generation())
}
