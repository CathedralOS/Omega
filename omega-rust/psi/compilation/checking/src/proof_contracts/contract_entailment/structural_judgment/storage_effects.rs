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
        written: &[String],
    ) -> Option<()> {
        if written.is_empty() || !self.requires_value_preservation() {
            return Some(());
        }
        // Exact stored observations are live contents. Conservatively retire
        // all of them on any subsequent write; detached copies keep their own
        // value identities in the normal-return environment.
        std::sync::Arc::make_mut(&mut self.stored_values).clear();
        let program = self.program;
        let frames = self.frame_resolver()?;
        let mut invalidated = Vec::new();
        let mut consider = |symbol: SymbolHandle, reference| {
            let overlaps = crate::frame_coordinate_path(program, symbol, &[]).is_none_or(|path| {
                written
                    .iter()
                    .any(|written| crate::frame_paths_overlap(written, &path))
            });
            if overlaps || !frames.value_is_reference_free(reference) {
                invalidated.push(symbol);
            }
        };
        for parameter in program.state_parameters(state) {
            consider(parameter.symbol, parameter.type_reference);
        }
        // A named state may receive an earlier state's reference under a new
        // binder. The exit contract still names the original binder; retiring
        // only this state's spelling would revive its entry observation. As
        // with current reference-bearing roots, a write conservatively retires
        // reference-bearing parameters in the enclosing invocation as well.
        if let Some(machine) = program.machine(self.machine_symbol) {
            for earlier in program.machine_states(machine) {
                if earlier.symbol == state.symbol {
                    continue;
                }
                for parameter in program.state_parameters(earlier) {
                    if !frames.value_is_reference_free(parameter.type_reference) {
                        consider(parameter.symbol, parameter.type_reference);
                    }
                }
            }
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
        // These rows are source predicates, unlike the captured term equations.
        // A later branch must not reinterpret an old entry fact over new storage.
        self.premise_facts.retain(|fact| {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, *fact, &mut expressions);
            !expressions.iter().any(|expression| {
                crate::value_custody::places::expression_root_symbol(
                    *expression,
                    &program.expression_table,
                    self.machine_symbol,
                )
                .is_some_and(|symbol| storage.contains(&symbol))
            })
        });
        Some(())
    }

    fn ordinary_storage_path(&self, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Name(_) => true,
            typed_trees::expression::ExpressionNode::Member(member) => {
                self.ordinary_storage_path(member.receiver)
            }
            // Canonical places deliberately erase borrow recasts to their
            // backing coordinate. That coordinate does not authorize reusing
            // the captured value under a different representation.
            _ => false,
        }
    }

    pub(super) fn stored_value_observation(
        &self,
        expression: ExpressionHandle,
    ) -> Option<super::StructuralTerm> {
        self.ordinary_storage_path(expression).then_some(())?;
        let place = crate::value_custody::places::canonical_place_from_expression(
            self.program,
            expression,
        )?;
        self.stored_values
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, value)| value.clone())
    }

    pub(in crate::proof_contracts::contract_entailment) fn observe_exact_store(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
        value: super::StructuralTerm,
    ) -> Option<()> {
        let assignment = statement.as_assignment()?;
        self.ordinary_storage_path(assignment.target)
            .then_some(())?;
        let program = self.program;
        let place = crate::value_custody::places::canonical_place_from_expression(
            program,
            assignment.target,
        )?;
        let root = place.root.as_symbol()?;
        if !(program.state_parameter_by_symbol(state, root).is_some()
            || program.state_local_declaration(state, root).is_some())
            || !place.segments.iter().all(|segment| {
                matches!(segment,
                facts::PlaceSegment::Field { symbol } if symbol.is_valid())
            })
        {
            return None;
        }
        let reference = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            assignment.target,
        )?;
        let frames = self.frame_resolver()?;
        if !frames.value_is_reference_free(reference)
            || !frames
                .statement_value_write_frame(machine, statement)
                .into_complete_paths()?
                .is_empty()
        {
            return None;
        }
        let written = frames
            .assignment_write_frame(machine, statement)
            .into_complete_paths()?;
        let coordinate = crate::frame_coordinate_path(program, root, &place.segments)?;
        // No coarse region or rebased alias can stand in for this destination.
        if written.as_slice() != [coordinate] {
            return None;
        }
        self.observe_storage_writes(state, &written)?;
        std::sync::Arc::make_mut(&mut self.stored_values).push((place, value));
        Some(())
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
