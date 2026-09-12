//! Exact retained storage comparison for append-only typed continuations.
//!
//! Compare handles and payloads directly; diagnostic snapshots are not the
//! authority for preservation and need not allocate the generated suffix.

use super::{TypedTreeRoots, TypedTreeTables, TypedTrees};
use arena::Arena;

impl TypedTrees {
    /// Retain every authored root and its existing table storage unchanged.
    /// New roots and table entries may be appended. Semantic sidecars are
    /// checked separately by the continuation owner.
    pub fn retains_exact_root_storage(&self, base: &Self) -> bool {
        let TypedTreeRoots {
            const_declarations,
            data_definitions,
            domain_definitions,
            machines,
            measures,
            operators,
            propositions,
            traits,
            conformances,
            wire_schemas,
        } = &base.roots;
        root_start_is_retained(*const_declarations, self.roots.const_declarations)
            && root_start_is_retained(*data_definitions, self.roots.data_definitions)
            && root_start_is_retained(*domain_definitions, self.roots.domain_definitions)
            && root_start_is_retained(*machines, self.roots.machines)
            && root_start_is_retained(*measures, self.roots.measures)
            && root_start_is_retained(*operators, self.roots.operators)
            && root_start_is_retained(*propositions, self.roots.propositions)
            && root_start_is_retained(*traits, self.roots.traits)
            && root_start_is_retained(*conformances, self.roots.conformances)
            && root_start_is_retained(*wire_schemas, self.roots.wire_schemas)
            && base.proof_facts.iter().all(|(handle, _)| {
                self.proof_fact_source_span(handle) == base.proof_fact_source_span(handle)
            })
            && self.const_declarations().starts_with(
                base.tables
                    .const_declarations
                    .span_or_empty(*const_declarations),
            )
            && self.data_definitions().starts_with(
                base.tables
                    .data_definitions
                    .span_or_empty(*data_definitions),
            )
            && self.domain_definitions().starts_with(
                base.tables
                    .domain_definitions
                    .span_or_empty(*domain_definitions),
            )
            && self
                .machines()
                .starts_with(base.tables.machines.span_or_empty(*machines))
            && self
                .measures()
                .starts_with(base.tables.measures.span_or_empty(*measures))
            && self
                .operators()
                .starts_with(base.tables.operators.span_or_empty(*operators))
            && self
                .propositions()
                .starts_with(base.tables.propositions.span_or_empty(*propositions))
            && self
                .traits()
                .starts_with(base.tables.traits.span_or_empty(*traits))
            && self
                .conformances()
                .starts_with(base.tables.conformances.span_or_empty(*conformances))
            && self
                .wire_schemas()
                .starts_with(base.tables.wire_schemas.span_or_empty(*wire_schemas))
            && self.tables.retains_exact_prefix(&base.tables)
    }
}

fn root_start_is_retained<T>(base: arena::HandleSpan<T>, candidate: arena::HandleSpan<T>) -> bool {
    base.is_empty() || base.start() == candidate.start()
}

impl TypedTreeTables {
    fn retains_exact_prefix(&self, base: &Self) -> bool {
        // Exhaustive destructuring makes new storage require a preservation rule.
        let Self {
            const_declarations,
            data_definitions,
            data_type_parameters,
            data_members,
            data_payload_fields,
            domain_definitions,
            proof_facts,
            propositions,
            proposition_binders,
            domain_path_members,
            operator_path_members,
            machines,
            measures,
            measure_path_members,
            operators,
            machine_owned_data,
            machine_trait_conformances,
            machine_states,
            state_parameters,
            traits,
            conformances,
            trait_requirements,
            trait_machine_signatures,
            signature_invokes,
            signature_contracts,
            wire_schemas,
            wire_members,
            expression_table,
            statement_table,
            type_reference_table,
            authored_declaration_selections,
            proof_fact_source_spans,
        } = base;
        arena_is_exact_prefix(const_declarations, &self.const_declarations)
            && arena_is_exact_prefix(data_definitions, &self.data_definitions)
            && arena_is_exact_prefix(data_type_parameters, &self.data_type_parameters)
            && arena_is_exact_prefix(data_members, &self.data_members)
            && arena_is_exact_prefix(data_payload_fields, &self.data_payload_fields)
            && arena_is_exact_prefix(domain_definitions, &self.domain_definitions)
            && arena_is_exact_prefix(proof_facts, &self.proof_facts)
            && arena_is_exact_prefix(propositions, &self.propositions)
            && arena_is_exact_prefix(proposition_binders, &self.proposition_binders)
            && arena_is_exact_prefix(domain_path_members, &self.domain_path_members)
            && arena_is_exact_prefix(operator_path_members, &self.operator_path_members)
            && arena_is_exact_prefix(machines, &self.machines)
            && arena_is_exact_prefix(measures, &self.measures)
            && arena_is_exact_prefix(measure_path_members, &self.measure_path_members)
            && arena_is_exact_prefix(operators, &self.operators)
            && arena_is_exact_prefix(machine_owned_data, &self.machine_owned_data)
            && arena_is_exact_prefix(machine_trait_conformances, &self.machine_trait_conformances)
            && arena_is_exact_prefix(machine_states, &self.machine_states)
            && arena_is_exact_prefix(state_parameters, &self.state_parameters)
            && arena_is_exact_prefix(traits, &self.traits)
            && arena_is_exact_prefix(conformances, &self.conformances)
            && arena_is_exact_prefix(trait_requirements, &self.trait_requirements)
            && arena_is_exact_prefix(trait_machine_signatures, &self.trait_machine_signatures)
            && arena_is_exact_prefix(signature_invokes, &self.signature_invokes)
            && arena_is_exact_prefix(signature_contracts, &self.signature_contracts)
            && arena_is_exact_prefix(wire_schemas, &self.wire_schemas)
            && arena_is_exact_prefix(wire_members, &self.wire_members)
            && self.expression_table.retains_exact_prefix(expression_table)
            && self.statement_table.retains_exact_prefix(statement_table)
            && self
                .type_reference_table
                .retains_exact_prefix(type_reference_table)
            && self
                .authored_declaration_selections
                .as_slice()
                .starts_with(authored_declaration_selections.as_slice())
            && self
                .proof_fact_source_spans
                .starts_with(proof_fact_source_spans)
    }
}

pub(crate) fn arena_is_exact_prefix<T: Default + PartialEq>(
    base: &Arena<T>,
    candidate: &Arena<T>,
) -> bool {
    candidate.len() >= base.len() && candidate.iter().take(base.len()).eq(base.iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::ExpressionNode;
    use crate::machine::Machine;

    #[test]
    fn appended_storage_preserves_prefix_but_changed_children_do_not() {
        let mut base = TypedTrees::default();
        let expression = base.expression_table.insert(ExpressionNode::Boolean(false));
        base.push_machine(Machine::default());
        let mut candidate = base.clone();
        candidate
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        candidate.push_machine(Machine {
            is_public: true,
            ..Machine::default()
        });
        assert!(candidate.retains_exact_root_storage(&base));
        *candidate.expression_table.expression_mut(expression) = ExpressionNode::Boolean(true);
        assert!(!candidate.retains_exact_root_storage(&base));
    }

    #[test]
    fn changed_root_selection_rejects_even_with_unchanged_tables() {
        let mut base = TypedTrees::default();
        base.push_machine(Machine::default());
        let mut candidate = base.clone();
        candidate.roots.machines = arena::HandleSpan::empty();
        assert!(!candidate.retains_exact_root_storage(&base));
        candidate.roots.machines = candidate.tables.machines.insert_many([Machine::default()]);
        assert!(!candidate.retains_exact_root_storage(&base));
    }

    #[test]
    fn adding_previously_absent_metadata_to_a_retained_fact_rejects() {
        let mut base = TypedTrees::default();
        let fact = base.proof_facts.insert(Default::default());
        let mut candidate = base.clone();
        candidate.set_proof_fact_source_span(fact, source::SourceSpan::default());
        assert!(!candidate.retains_exact_root_storage(&base));
    }

    #[test]
    fn reclaimed_and_reused_slots_cannot_replace_retained_identity() {
        let mut base = Arena::new();
        let retained = base.insert(7u32);
        let mut candidate = base.clone();
        assert!(candidate.free(retained));
        let replacement = candidate.insert(7u32);
        assert_ne!(retained, replacement);
        assert!(!arena_is_exact_prefix(&base, &candidate));
    }

    #[test]
    fn comparisons_do_not_visit_the_generated_suffix() {
        use std::cell::Cell;
        use std::rc::Rc;
        #[derive(Clone, Default)]
        struct Counted(Rc<Cell<usize>>);
        impl PartialEq for Counted {
            fn eq(&self, _: &Self) -> bool {
                self.0.set(self.0.get() + 1);
                true
            }
        }
        let comparisons = Rc::new(Cell::new(0));
        let mut base = Arena::new();
        base.insert_many((0..3).map(|_| Counted(comparisons.clone())));
        let mut candidate = base.clone();
        candidate.insert_many((0..1000).map(|_| Counted(comparisons.clone())));
        assert!(arena_is_exact_prefix(&base, &candidate));
        assert_eq!(comparisons.get(), 3);
    }
}
