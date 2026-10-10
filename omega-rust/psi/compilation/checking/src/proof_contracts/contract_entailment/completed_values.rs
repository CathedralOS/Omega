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
pub(super) struct CompletedValueProjection {
    pub path: Vec<PlaceSegment>,
    pub sources: Vec<CompletedValueSource>,
    /// A whole unknown aggregate is an obligation too. Its omitted children
    /// cannot establish absence merely because a few demanded leaves are known.
    pub terminal: bool,
}

/// Authenticate term field names against the current declared owner. These are
/// already captured term projections, not source selectors to replay.
fn field_projections(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    name: &str,
) -> Vec<(Vec<PlaceSegment>, TypeReferenceHandle)> {
    let peeled = program.type_reference_table.unconstrained_type_reference(
        program
            .type_reference_table
            .dereferenced_type_reference(reference),
    );
    let Some(owner) = crate::value_custody::type_shapes::head_data_symbol(program, peeled) else {
        return Vec::new();
    };
    let Some(data) = program.data_definition_by_symbol(owner) else {
        return Vec::new();
    };
    program
        .data_members(data)
        .iter()
        .flat_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            typed_trees::data::DataMember::Variant(variant) => program.data_payload_fields(variant),
            _ => &[],
        })
        .filter(|field| field.name.as_str() == name)
        .filter_map(|field| {
            let mut path = Vec::new();
            crate::value_custody::places::push_field_place_segments(
                program,
                &mut path,
                field.symbol,
            );
            let projected = crate::value_custody::places::project_type_reference_from_segments(
                program, reference, &path,
            )?;
            Some((path, projected))
        })
        .collect()
}

fn term_field_paths(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    value: &StructuralTerm,
    names: &[&str],
) -> Vec<Vec<PlaceSegment>> {
    let Some((name, remaining)) = names.split_first() else {
        return vec![Vec::new()];
    };
    let mut paths = Vec::new();
    for (prefix, projected_type) in field_projections(program, reference, name) {
        let Some(projected) = project_observed_value(program, reference, value.clone(), &prefix)
        else {
            continue;
        };
        for suffix in term_field_paths(program, projected_type, &projected, remaining) {
            let mut path = prefix.clone();
            path.extend(suffix);
            paths.push(path);
        }
    }
    paths
}

fn append_projection(
    projections: &mut Vec<CompletedValueProjection>,
    path: Vec<PlaceSegment>,
    terminal: bool,
) {
    if let Some(existing) = projections
        .iter_mut()
        .find(|existing| existing.path == path)
    {
        existing.terminal |= terminal;
    } else {
        projections.push(CompletedValueProjection {
            path,
            sources: Vec::new(),
            terminal,
        });
    }
}

/// Observed constructors supply their finite actual child roster. A collection
/// descriptor or opaque aggregate remains a whole-value obligation; declaration
/// extent does not manufacture observations of its children.
fn observed_projections(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    value: &StructuralTerm,
    path: &mut Vec<PlaceSegment>,
    projections: &mut Vec<CompletedValueProjection>,
) {
    let mut children = Vec::new();
    let mut complete_constructor = false;
    if let StructuralTerm::Constructor {
        data: name,
        case,
        fields,
    } = value.semantic_value()
    {
        let peeled = program.type_reference_table.unconstrained_type_reference(
            program
                .type_reference_table
                .dereferenced_type_reference(reference),
        );
        if let Some(owner) = crate::value_custody::type_shapes::head_data_symbol(program, peeled)
            && let Some(data) = program.data_definition_by_symbol(owner)
            && data.name.as_str() == name.as_str()
        {
            let declared = program
                .data_members(data)
                .iter()
                .flat_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case.as_str() =>
                    {
                        program.data_payload_fields(variant)
                    }
                    _ => &[],
                })
                .collect::<Vec<_>>();
            complete_constructor = declared.len() == fields.len()
                && declared.iter().all(|field| {
                    fields
                        .iter()
                        .any(|(name, _)| name.as_str() == field.name.as_str())
                });
            for (name, _) in fields {
                for (prefix, projected_type) in field_projections(program, reference, name) {
                    if let Some(projected) =
                        project_observed_value(program, reference, value.clone(), &prefix)
                    {
                        children.push((prefix, projected_type, projected));
                    }
                }
            }
            complete_constructor &= children.len() == fields.len();
        }
    }
    // Each observed constructor edge supplies a distinct declared selector.
    // Explicit/public demands below perform their own union with these rows.
    projections.push(CompletedValueProjection {
        path: path.clone(),
        sources: Vec::new(),
        terminal: !complete_constructor || children.is_empty(),
    });
    for (prefix, projected_type, projected) in children {
        let previous = path.len();
        path.extend(prefix);
        observed_projections(program, projected_type, &projected, path, projections);
        path.truncate(previous);
    }
}

/// Locate only projections carried by the requested term. The final identity
/// check uses the same normal-result owner as completion; declared field lookup
/// merely authenticates selectors and does not donate an equality.
fn parameter_projection_paths(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    reference: TypeReferenceHandle,
    value: &StructuralTerm,
    observed: &StructuralTerm,
) -> Vec<Vec<PlaceSegment>> {
    let mut paths = Vec::new();
    if value == observed {
        paths.push(Vec::new());
    }
    let suffix = match (value.semantic_value(), observed.semantic_value()) {
        (
            StructuralTerm::Variable(base) | StructuralTerm::Opaque(base),
            StructuralTerm::Opaque(projected),
        ) => projected
            .strip_prefix(base)
            .and_then(|suffix| suffix.strip_prefix('.')),
        (StructuralTerm::BoundValue(base), StructuralTerm::BoundProjection { subject, path })
            if base == subject =>
        {
            Some(path.as_str())
        }
        (
            StructuralTerm::BoundProjection {
                subject: base,
                path: prefix,
            },
            StructuralTerm::BoundProjection { subject, path },
        ) if base == subject => path
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_prefix('.')),
        (_, StructuralTerm::Projection { subject, path }) if subject.as_ref() == value => {
            Some(path.as_str())
        }
        _ => None,
    };
    if let Some(suffix) = suffix {
        for path in term_field_paths(
            program,
            reference,
            value,
            &suffix.split('.').collect::<Vec<_>>(),
        ) {
            if project_observed_value(program, reference, value.clone(), &path)
                .is_some_and(|projected| judge.completed_value_identity(projected) == *observed)
                && !paths.contains(&path)
            {
                paths.push(path);
            }
        }
    }
    if let StructuralTerm::Constructor { fields, .. } = value.semantic_value() {
        for (name, _) in fields {
            for (prefix, projected_type) in field_projections(program, reference, name) {
                let Some(projected) =
                    project_observed_value(program, reference, value.clone(), &prefix)
                else {
                    continue;
                };
                let projected = judge.completed_value_identity(projected);
                for suffix in
                    parameter_projection_paths(program, judge, projected_type, &projected, observed)
                {
                    let mut path = prefix.clone();
                    path.extend(suffix);
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths
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
    crate::value_custody::places::project_type_reference_from_segments(program, reference, path)?;
    completed_value_observations(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        &[path.to_vec()],
    )?
    .into_iter()
    .find(|projection| projection.path == path)
    .map(|projection| projection.sources)
    .filter(|sources| !sources.is_empty())
}

/// Execute the normal-return owner once. Explicit demands and paths actually
/// observed by constructors/public result equations share one query. Unknown
/// aggregate roots remain terminal obligations beside their known leaves.
pub(super) fn completed_value_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    demands: &[Vec<PlaceSegment>],
) -> Option<Vec<CompletedValueProjection>> {
    let mut visitor = CompletedProjectionVisitor {
        state: state.symbol,
        statement_index,
        expression,
        reference,
        demands,
        observations: None,
        recognized: true,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor)?;
    visitor.recognized.then_some(visitor.observations?)
}

struct CompletedProjectionVisitor<'path> {
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    demands: &'path [Vec<PlaceSegment>],
    observations: Option<Vec<CompletedValueProjection>>,
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
            let candidates = outcome
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
            let formals = program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    let (_, value) = environment
                        .iter()
                        .find(|(name, _)| name == parameter.name.as_str())?;
                    Some((
                        parameter,
                        outcome.judge.completed_value_identity(value.clone()),
                    ))
                })
                .collect::<Vec<_>>();
            let value = outcome
                .judge
                .completed_value_identity(outcome.value.clone());
            let mut projections = Vec::new();
            observed_projections(
                program,
                self.reference,
                &value,
                &mut Vec::new(),
                &mut projections,
            );
            for path in self.demands {
                if crate::value_custody::places::project_type_reference_from_segments(
                    program,
                    self.reference,
                    path,
                )
                .is_some()
                {
                    append_projection(&mut projections, path.clone(), false);
                }
            }
            for path in outcome
                .judge
                .completed_value_projection_paths(&outcome.value)
            {
                for path in term_field_paths(
                    program,
                    self.reference,
                    &value,
                    &path.split('.').collect::<Vec<_>>(),
                ) {
                    if crate::value_custody::places::project_type_reference_from_segments(
                        program,
                        self.reference,
                        &path,
                    )
                    .is_some()
                    {
                        append_projection(&mut projections, path, false);
                    }
                }
            }
            for projection in &mut projections {
                let Some(projected) = project_observed_value(
                    program,
                    self.reference,
                    value.clone(),
                    &projection.path,
                ) else {
                    continue;
                };
                let projected = outcome.judge.completed_value_identity(projected);
                projection.sources.extend(
                    candidates.iter().filter_map(|(source, value)| {
                        (projected == *value).then_some(source.clone())
                    }),
                );
                for (parameter, value) in &formals {
                    for path in parameter_projection_paths(
                        program,
                        &outcome.judge,
                        parameter.type_reference,
                        value,
                        &projected,
                    ) {
                        let source = CompletedValueSource::Parameter {
                            symbol: parameter.symbol,
                            path,
                        };
                        if !projection.sources.contains(&source) {
                            projection.sources.push(source);
                        }
                    }
                }
            }
            if let Some(retained) = &mut self.observations {
                for previous in retained.iter_mut() {
                    if let Some(current) = projections
                        .iter()
                        .find(|current| current.path == previous.path)
                    {
                        previous
                            .sources
                            .retain(|source| current.sources.contains(source));
                        previous.terminal |= current.terminal;
                    } else {
                        previous.sources.clear();
                    }
                }
                for mut projection in projections {
                    if !retained
                        .iter()
                        .any(|previous| previous.path == projection.path)
                    {
                        projection.sources.clear();
                        retained.push(projection);
                    }
                }
            } else {
                self.observations = Some(projections);
            }
        }
    }
}
