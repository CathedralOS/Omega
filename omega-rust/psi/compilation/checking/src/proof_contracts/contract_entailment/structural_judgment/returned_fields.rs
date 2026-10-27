//! Payload correspondence of completed owned results.
//!
//! A checked construction preserves its initializer values, including through
//! locals, nested calls and state transfers. Infer that correspondence under
//! declaration entry inputs, intersect every normal exit, then substitute the
//! caller's completed operands. Never specialize a callee path using caller
//! facts or export a private expression as a new source observation.
//!
//! Only integer/Boolean leaf equations leave this owner. The result root stays
//! its exact captured occurrence: payload equality creates neither nominal
//! membership nor ownership/lineage equality. Reference crossings, mutable
//! entry inputs and callee-local observation epochs cannot be transported by
//! this value correspondence. See calls_and_outcomes.md's complete record
//! construction and content_custody.md's transparent forwarding rules.

use super::{StructuralJudge, StructuralTerm};
use crate::proof_contracts::contract_entailment::{
    citations::machine_requires_facts,
    completed_values::{CompletedValueProjection, observed_projections, term_field_paths},
    observed_projections::project_observed_value,
    structural_case_arms::recognize_structural_case_arms,
};
use facts::PlaceSegment;
use typed_trees::{
    machine::{Machine, MachineBodyStatus},
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceHandle},
};

impl StructuralJudge<'_> {
    pub(super) fn returned_field_equations(
        &self,
        callee: &Machine,
        selected_state: &typed_trees::state::State,
        parameters: &[StateParameter],
        actuals: &[StructuralTerm],
        captured: &StructuralTerm,
    ) -> Vec<(StructuralTerm, StructuralTerm)> {
        let decline = |reason: &str| {
            if super::super::struct_trace() {
                eprintln!(
                    "STRUCT returned-fields owner={:?} declined={reason}",
                    callee.symbol
                );
            }
            Vec::new()
        };
        let program = self.program;
        if !callee.body_is_present
            || callee.body_status != MachineBodyStatus::Complete
            || !callee.supply_mode.is_checked_body()
            || callee.structural_type_equations_pending
            || callee.has_specialization_parameters()
        {
            return decline("no complete selected checked body with a closed telescope");
        }
        let Some(entry) = program.machine_states(callee).first() else {
            return decline("no body entry");
        };
        // Matching telescopes alone do not select the body: two states may
        // both take no arguments and return different values. Entry traversal
        // proves only the exact selected entry's normal-return correspondence.
        if selected_state.symbol != entry.symbol {
            return decline("selected state is not this body's entry");
        }
        if parameters.len() != actuals.len()
            || !program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.symbol)
                .eq(parameters.iter().map(|parameter| parameter.symbol))
        {
            return decline("selected operands do not bind this body's entry telescope");
        }
        if self.returned_field_owners.contains(&callee.symbol) {
            return decline("recursive field correspondence");
        }
        // Scalar results have no construction fields. This is also the common
        // cheap path: ordinary calls need no second traversal for this query.
        if program
            .primitive_type_reference(entry.return_type)
            .is_some()
            || crate::value_custody::places::data_definition_for_type(program, entry.return_type)
                .is_none()
            || crate::reference_shell(program, entry.return_type).is_some()
        {
            return decline("no owned structural result");
        }
        let substitution = parameters
            .iter()
            .zip(actuals)
            .map(|(parameter, value)| (parameter.name.as_str().to_owned(), value.clone()))
            .collect::<Vec<_>>();
        let can_memoize = self.returned_field_owners.is_empty()
            && program
                .machine(callee.symbol)
                .is_some_and(|stored| std::ptr::eq(stored, callee));
        if can_memoize
            && let Some(memos) = crate::frozen_program::frozen_program_memos(program)
            && let Ok(cached) = memos.returned_field_projections.lock()
            && let Some(template) = cached.get(&callee.symbol).cloned()
        {
            drop(cached);
            return template
                .iter()
                .filter_map(|(path, value)| {
                    let subject =
                        project_observed_value(program, entry.return_type, captured.clone(), path)?;
                    Some((subject, Self::substitute_term(value, &substitution)))
                })
                .collect();
        }
        let store_template = |template: Vec<(Vec<PlaceSegment>, StructuralTerm)>| {
            if can_memoize
                && let Some(memos) = crate::frozen_program::frozen_program_memos(program)
                && let Ok(mut cached) = memos.returned_field_projections.lock()
            {
                cached.insert(callee.symbol, std::sync::Arc::new(template));
            }
        };
        let requires = machine_requires_facts(program, callee);
        let mut judge = Self::from_runtime_state_requires(program, callee, entry, &requires);
        judge.returned_field_owners = std::sync::Arc::clone(&self.returned_field_owners);
        std::sync::Arc::make_mut(&mut judge.returned_field_owners).push(callee.symbol);
        let mut diagnostics = Vec::new();
        let Some(leaves) = recognize_structural_case_arms(
            program,
            callee,
            &judge,
            &crate::proof_only_classification(program),
            &mut diagnostics,
            None,
        ) else {
            store_template(Vec::new());
            return decline("normal-return paths are incomplete or recurrent");
        };
        let Some(first) = leaves.first().filter(|_| diagnostics.is_empty()) else {
            store_template(Vec::new());
            return decline("no independently checked normal-return leaves");
        };
        let first_value = first
            .normal_judge
            .completed_value_identity(first.value.clone());
        let mut projections = Vec::new();
        observed_projections(
            program,
            entry.return_type,
            &first_value,
            &mut Vec::new(),
            &mut projections,
        );
        // A forwarded completed call has opaque root identity. Its retained
        // typed equations supply demands without reconstructing its record.
        for names in first
            .normal_judge
            .completed_value_projection_paths(&first_value)
        {
            for path in term_field_paths(
                program,
                entry.return_type,
                &first_value,
                &names.split('.').collect::<Vec<_>>(),
            ) {
                if !projections.iter().any(|projection| projection.path == path) {
                    projections.push(CompletedValueProjection {
                        path,
                        sources: Vec::new(),
                        terminal: true,
                    });
                }
            }
        }
        let template: Vec<(Vec<PlaceSegment>, StructuralTerm)> = projections
            .into_iter()
            .filter_map(|projection| {
                if projection.path.is_empty() {
                    return None;
                }
                let reference =
                    owned_projection_type(program, entry.return_type, &projection.path)?;
                let primitive = program.primitive_type_reference(reference)?;
                if !primitive.is_fixed_integer() && primitive != PrimitiveType::Bool {
                    return None;
                }
                let values = leaves
                    .iter()
                    .map(|leaf| {
                        let value = leaf
                            .normal_judge
                            .completed_value_identity(leaf.value.clone());
                        let projected = project_observed_value(
                            program,
                            entry.return_type,
                            value,
                            &projection.path,
                        )?;
                        let projected = leaf.normal_judge.completed_value_identity(projected);
                        // Preserve a shared input identity before using a
                        // branch-specific equality. Both paths may forward x
                        // even when just one path independently knows x == 17.
                        let projected = if declaration_input_value(&judge, parameters, &projected) {
                            projected
                        } else {
                            leaf.normal_judge.resolve(projected)
                        };
                        declaration_input_value(&judge, parameters, &projected).then_some(projected)
                    })
                    .collect::<Option<Vec<_>>>()?;
                let value = values.first()?.clone();
                if values.iter().any(|other| *other != value) {
                    return None;
                }
                Some((projection.path, value))
            })
            .collect();
        let equations = template
            .iter()
            .filter_map(|(path, value)| {
                let subject =
                    project_observed_value(program, entry.return_type, captured.clone(), path)?;
                Some((subject, Self::substitute_term(value, &substitution)))
            })
            .collect();
        store_template(template);
        equations
    }
}

/// Inventory lookup may peel a reference to discover its declaration. That is
/// navigation, not observation authority: independently reject every such
/// crossing before transporting either an input or a result leaf.
fn owned_projection_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    for segment in path {
        if crate::reference_shell(program, reference).is_some()
            || crate::slice_view_shell(program, reference, &[]).is_some()
        {
            return None;
        }
        reference = crate::value_custody::places::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
    }
    (crate::reference_shell(program, reference).is_none()
        && crate::slice_view_shell(program, reference, &[]).is_none())
    .then_some(reference)
}

/// A callee-local captured event has no caller identity. Only exact immutable
/// declaration inputs and already selected scalar value operations may leave
/// the declaration query. In particular, no source expression is replayed.
fn declaration_input_value(
    judge: &StructuralJudge<'_>,
    parameters: &[StateParameter],
    value: &StructuralTerm,
) -> bool {
    let input = |name: &str| {
        parameters.iter().find(|parameter| {
            parameter.name.as_str() == name
                && !parameter.is_mutable
                && judge.frame_resolver().is_some_and(|frames| {
                    frames.proof_value_is_caller_isolated(parameter.type_reference)
                })
        })
    };
    match value {
        StructuralTerm::Integer(_) => true,
        StructuralTerm::Constructor { data, case, fields } => {
            data == "bool" && matches!(case.as_str(), "true" | "false") && fields.is_empty()
        }
        StructuralTerm::Variable(name) => input(name).is_some(),
        StructuralTerm::Projection { subject, path } => {
            let StructuralTerm::Variable(name) = subject.as_ref() else {
                return false;
            };
            let Some(parameter) = input(name) else {
                return false;
            };
            term_field_paths(
                judge.program,
                parameter.type_reference,
                subject,
                &path.split('.').collect::<Vec<_>>(),
            )
            .iter()
            .any(|path| {
                owned_projection_type(judge.program, parameter.type_reference, path).is_some()
            })
        }
        StructuralTerm::ScalarBinary {
            references,
            meaning,
            left,
            right,
            ..
        } => {
            meaning.iter().all(|(primitive, _)| {
                primitive.is_fixed_integer() || *primitive == PrimitiveType::Bool
            }) && [left, right]
                .into_iter()
                .zip(references)
                .all(|(operand, reference)| {
                    super::super::structural_terms::scalar_operand_is_observed(
                        judge.program,
                        *reference,
                        operand,
                    ) && declaration_input_value(judge, parameters, operand)
                })
        }
        _ => false,
    }
}
