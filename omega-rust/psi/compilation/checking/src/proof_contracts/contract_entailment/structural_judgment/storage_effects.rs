//! Writes retire live storage observations, not completed value identities.
//!
//! The normal-return path owns effect order. Its complete caller-relative frame
//! may replace a binding's current value while earlier copies and equations
//! still describe their captured values. A later read of retired storage gets
//! a new unknown observation; unrelated owned bindings retain their facts.
//! Whole aggregate roots are retired on an overlapping field write. Reference-
//! bearing or opaque roots remain conservative until their exact preservation
//! is available: a disjoint spelling alone cannot preserve aliased contents.

use super::StructuralJudge;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn observe_storage_writes(
        &mut self,
        state: &State,
        frame: &facts::NormalizedWriteFrame,
    ) {
        if frame.complete_paths().is_some_and(<[String]>::is_empty)
            || !self.requires_value_preservation()
        {
            return;
        }
        let program = self.program;
        let frames = self.call_frames();
        let written = frame.complete_paths();
        let mut invalidated = Vec::new();
        let mut consider = |symbol: SymbolHandle, reference| {
            let overlaps = crate::frame_coordinate_path(program, symbol, &[]).is_none_or(|path| {
                written.is_none_or(|written| {
                    written
                        .iter()
                        .any(|written| crate::frame_paths_overlap(written, &path))
                })
            });
            if overlaps || frames.is_none_or(|frames| !frames.value_is_reference_free(reference)) {
                invalidated.push(symbol);
            }
        };
        for parameter in program.state_parameters(state) {
            consider(parameter.symbol, parameter.type_reference);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                consider(local.symbol, local.type_reference);
            }
        }
        let storage = std::sync::Arc::make_mut(&mut self.invalidated_storage);
        for symbol in invalidated {
            if !storage.contains(&symbol) {
                storage.push(symbol);
            }
        }
        // Entry predicates already live as produced path contributions.
        // The calling retirement step filters those same contributions; no
        // source predicate roster is retained or reread after this write.
    }

    pub(super) fn storage_observation_invalidated(&self, expression: ExpressionHandle) -> bool {
        crate::value_custody::places::expression_root_symbol(
            expression,
            &self.program.expression_table,
            self.machine_symbol,
        )
        .is_some_and(|symbol| self.invalidated_storage.contains(&symbol))
    }

    pub(super) fn storage_binding_invalidated(&self, symbol: SymbolHandle) -> bool {
        self.invalidated_storage.contains(&symbol)
    }
}
