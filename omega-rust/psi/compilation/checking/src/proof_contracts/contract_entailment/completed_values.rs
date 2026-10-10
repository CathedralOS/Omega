//! Source observations of demanded completed-value projections.
//!
//! Execute the shared normal-return path owner at the actual statement prefix,
//! then join a declared projection with already captured operands or exact
//! current-state formal projections. Only identities proved on every normal
//! path survive. Published equations can describe a produced value; private
//! callee bodies and a matching carrier cannot donate a source observation.

use super::citations::CitationFacts;
use super::observed_projections::project_observed_value;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{StructuralPrefixVisitor, observe_runtime_state_prefix};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State, types::TypeReferenceHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CompletedValueSource {
    Expression(ExpressionHandle),
    Parameter {
        symbol: SymbolHandle,
        path: Vec<PlaceSegment>,
    },
}

#[derive(Clone)]
pub(super) struct DeclaredValueProjection {
    pub path: Vec<PlaceSegment>,
    pub type_reference: TypeReferenceHandle,
}

/// Enumerate finite declaration geometry, independently of the expression
/// which supplies it. Recursive referents and collections are retained at
/// their root; this record-field demand does not invent element observations.
pub(super) fn declared_value_projections(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<DeclaredValueProjection> {
    fn append(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        path: &mut Vec<PlaceSegment>,
        owners: &mut Vec<SymbolHandle>,
        projections: &mut Vec<DeclaredValueProjection>,
    ) {
        projections.push(DeclaredValueProjection {
            path: path.clone(),
            type_reference: reference,
        });
        let peeled = program.type_reference_table.unconstrained_type_reference(
            program
                .type_reference_table
                .dereferenced_type_reference(reference),
        );
        if let typed_trees::types::TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(peeled)
        {
            for index in 0..*length {
                let segment = PlaceSegment::FixedIndex { index };
                let Some(projected) =
                    crate::value_custody::places::project_type_reference_from_segments(
                        program,
                        reference,
                        std::slice::from_ref(&segment),
                    )
                else {
                    continue;
                };
                path.push(segment);
                append(program, projected, path, owners, projections);
                path.pop();
            }
            return;
        }
        let Some(owner) = crate::value_custody::type_shapes::head_data_symbol(program, peeled)
        else {
            return;
        };
        if owners.contains(&owner) {
            return;
        }
        let Some(data) = program.data_definition_by_symbol(owner) else {
            return;
        };
        owners.push(owner);
        let fields = program
            .data_members(data)
            .iter()
            .flat_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                typed_trees::data::DataMember::Variant(variant) => {
                    program.data_payload_fields(variant)
                }
            });
        for field in fields {
            let previous = path.len();
            crate::value_custody::places::push_field_place_segments(program, path, field.symbol);
            if let Some(projected) =
                crate::value_custody::places::project_type_reference_from_segments(
                    program,
                    reference,
                    &path[previous..],
                )
            {
                append(program, projected, path, owners, projections);
            }
            path.truncate(previous);
        }
        owners.pop();
    }
    let mut projections = Vec::new();
    append(
        program,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut projections,
    );
    projections
}

pub(super) fn expression_statement_index(
    program: &dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<usize> {
    let mut owners = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .filter(|(_, statement)| {
            crate::machine_calls::calls::statement_value_expression_roots(program, statement)
                .into_iter()
                .any(|root| {
                    crate::expression_owners::expression_contains(program, root, expression)
                })
        })
        .map(|(index, _)| index);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

pub(super) fn completed_value_projection_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<CompletedValueSource>> {
    let type_reference = crate::value_custody::places::project_type_reference_from_segments(
        program, reference, path,
    )?;
    let projection = DeclaredValueProjection {
        path: path.to_vec(),
        type_reference,
    };
    completed_value_projections_sources(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        std::slice::from_ref(&projection),
    )?
    .into_iter()
    .next()
    .filter(|sources| !sources.is_empty())
}

/// Evaluate the prefix and producing expression once for all demanded paths.
/// Unsupported projections keep an empty observation row, not an empty
/// dependency inventory. Consumers retain known rows beside those unknowns.
pub(super) fn completed_value_projections_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    projections: &[DeclaredValueProjection],
) -> Option<Vec<Vec<CompletedValueSource>>> {
    let mut visitor = CompletedProjectionVisitor {
        state: state.symbol,
        statement_index,
        expression,
        reference,
        projections,
        sources: None,
        recognized: true,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor)?;
    visitor.recognized.then_some(visitor.sources?)
}

struct CompletedProjectionVisitor<'path> {
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    projections: &'path [DeclaredValueProjection],
    sources: Option<Vec<Vec<CompletedValueSource>>>,
    recognized: bool,
}

impl StructuralPrefixVisitor for CompletedProjectionVisitor<'_> {
    fn site(&self) -> super::structural_prefix::StructuralPrefixSite {
        // Projection identity is unconditional. The arithmetic edge owner
        // independently keeps this transition's guard for range and descent.
        super::structural_prefix::StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: false,
        }
    }

    fn visit(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'_>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.state) else {
            self.recognized = false;
            return;
        };
        let Some(outcomes) = judge.normal_return_paths(
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.recognized = false;
            return;
        };
        for outcome in outcomes {
            let mut candidates = outcome
                .judge
                .completed_expression_values()
                .iter()
                .filter(|(expression, _)| {
                    *expression != self.expression
                        && crate::expression_owners::expression_contains(
                            program,
                            self.expression,
                            *expression,
                        )
                })
                .map(|(expression, value)| {
                    (
                        CompletedValueSource::Expression(*expression),
                        outcome.judge.completed_value_identity(value.clone()),
                    )
                })
                .collect::<Vec<_>>();
            for parameter in program.state_parameters(state) {
                let Some((_, value)) = environment
                    .iter()
                    .find(|(name, _)| name == parameter.name.as_str())
                else {
                    continue;
                };
                for candidate in declared_value_projections(program, parameter.type_reference) {
                    if let Some(value) = project_observed_value(
                        program,
                        parameter.type_reference,
                        outcome.judge.completed_value_identity(value.clone()),
                        &candidate.path,
                    ) {
                        candidates.push((
                            CompletedValueSource::Parameter {
                                symbol: parameter.symbol,
                                path: candidate.path,
                            },
                            outcome.judge.completed_value_identity(value),
                        ));
                    }
                }
            }
            let observations = self
                .projections
                .iter()
                .map(|projection| {
                    let Some(projected) = project_observed_value(
                        program,
                        self.reference,
                        outcome
                            .judge
                            .completed_value_identity(outcome.value.clone()),
                        &projection.path,
                    ) else {
                        return Vec::new();
                    };
                    let projected = outcome.judge.completed_value_identity(projected);
                    candidates
                        .iter()
                        .filter_map(|(source, value)| {
                            (projected == *value).then_some(source.clone())
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            if let Some(retained) = &mut self.sources {
                for (retained, observations) in retained.iter_mut().zip(observations) {
                    retained.retain(|source| observations.contains(source));
                }
            } else {
                self.sources = Some(observations);
            }
        }
    }
}
