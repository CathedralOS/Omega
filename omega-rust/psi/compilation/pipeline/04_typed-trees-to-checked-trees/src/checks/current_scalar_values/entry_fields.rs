//! Invocation entry observations use a finite coordinate demand. Owned storage
//! supplies its ordinary field frontier; observing beyond it requires a matched
//! frozen reference load from the frame owner. Never expand pointer-type graphs:
//! recursive types do not define a finite set of observable field paths.
use super::CurrentScalarValues;
use super::scalar_fields::{ScalarProjection, capture_path};
use validation::ScopedArithmeticValue;

/// Paths are declaration identities, shared across a call component so a private
/// forwarder need not repeat the caller's rank expression. Suffixes let an actual
/// projected from a larger record retain the same selected declaration path.
pub(crate) fn parameter_projection_paths(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Vec<Vec<facts::PlaceSegment>> {
    let mut pending = Vec::new();
    for (_, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        pending.extend(
            program
                .tables
                .expression_roots(statement)
                .into_iter()
                .map(|(_, expression)| expression),
        );
    }
    for contract in program
        .state_contracts(state)
        .iter()
        .chain(program.machine_contracts(machine))
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let Some(expression) = fact.as_expression() {
                pending.push(*expression);
            }
        }
    }
    let mut seen = Vec::new();
    let mut paths = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || seen.contains(&expression) {
            continue;
        }
        seen.push(expression);
        if let Some(place) = validation::canonical_place_from_expression(program, expression)
            && let facts::PlaceRoot::Symbol(root) = place.root
            && program.state_parameter_by_symbol(state, root).is_some()
        {
            let path = place.segments;
            for start in 0..path.len() {
                let suffix = path[start..].to_vec();
                if !paths.contains(&suffix) {
                    paths.push(suffix);
                }
            }
        }
        validation::expression_traversal::children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
    paths
}

impl CurrentScalarValues<'_> {
    /// Owned records and readable borrowed records expose typed entry fields.
    /// A borrowed root requires its own stable reference binding; it does not
    /// freeze the referent. These current rows still expire on overlapping
    /// writes, while a selected entry value is saved independently by ranking.
    /// Only the formal's authenticated boundary is crossed here: reference
    /// leaves inside the record need separate load evidence and are not walked.
    pub(super) fn seed_parameter_fields(
        &mut self,
        requested: &[Vec<facts::PlaceSegment>],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) {
        for parameter in self.program.state_parameters(self.state) {
            if parameter.is_const || parameter.relevance.is_erased() {
                continue;
            }
            let reference = if let Some(shell) =
                validation::reference_shell(self.program, parameter.type_reference)
            {
                if !shell.access.is_readable()
                    || !validation::state_reference_parameter_binding_is_stable(
                        self.program,
                        self.machine,
                        self.state,
                        parameter.symbol,
                    )
                {
                    continue;
                }
                shell.referee
            } else {
                parameter.type_reference
            };
            let mut pending = vec![(reference, Vec::new(), 0usize)];
            while let Some((reference, path, depth)) = pending.pop() {
                if depth >= 64 {
                    continue;
                }
                let declared = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                // Atomic/placed carriers require their observation operation;
                // equal scalar representation supplies no ordinary field read.
                if self
                    .program
                    .placed_view_plan_for_type_reference(declared)
                    .is_some()
                    || self
                        .program
                        .placed_view_field_plan_for_type_reference(declared)
                        .is_some()
                    || matches!(self.program.type_reference_table.type_reference(declared),
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                            if self.program.symbols.builtin_type_atom(*symbol)
                                .is_some_and(|atom| atom.is_atomic()))
                {
                    continue;
                }
                let identity = format!(
                    "state:{:?}:parameter:{:?}:field:{path:?}",
                    self.state.symbol, parameter.symbol
                );
                if let Some(value) =
                    super::fixed_length(self.program, reference, || format!("{identity}:length"))
                {
                    if !path.is_empty() {
                        let coordinate = path
                            .iter()
                            .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                            .collect::<Vec<_>>();
                        let Some(name) = validation::frame_length_coordinate_path(
                            self.program,
                            parameter.symbol,
                            &coordinate,
                        ) else {
                            continue;
                        };
                        self.lengths.push(super::Length {
                            subject: super::LengthSubject::Field {
                                owner: parameter.symbol,
                                path,
                            },
                            name,
                            stable: false,
                            type_reference: reference,
                            value: Some(value),
                        });
                    }
                    continue;
                }
                if let Some(carrier) = super::integer_carrier(self.program, reference) {
                    if !path.is_empty() {
                        self.fields
                            .push(super::scalar_fields::ScalarField::for_owner(
                                self.program,
                                parameter.symbol,
                                super::scalar_fields::CapturedScalarField {
                                    path: path
                                        .iter()
                                        .map(|symbol| ScalarProjection::Field(*symbol))
                                        .collect(),
                                    carrier,
                                    read_dependencies: Vec::new(),
                                    value: ScopedArithmeticValue::Atom {
                                        identity,
                                        unsigned: super::unsigned(carrier),
                                    },
                                },
                            ));
                    }
                    continue;
                }
                let reference = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                if matches!(
                    self.program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    continue;
                }
                let Some(data) =
                    self.program
                        .data_application_definition(reference)
                        .or_else(|| {
                            self.program
                                .type_reference_table
                                .type_head_symbol(reference)
                                .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
                        })
                else {
                    continue;
                };
                for member in self.program.data_members(data) {
                    if let typed_trees::data::DataMember::Field(field) = member
                        && !field.relevance.is_erased()
                    {
                        let mut child = path.clone();
                        child.push(field.symbol);
                        pending.push((field.type_reference, child, depth + 1));
                    }
                }
            }
            let Some(frames) = frames else { continue };
            let order = self
                .program
                .statement_table
                .admitted_execution_order(self.state.statement_nodes);
            let Some(&first) = order.indices().first() else {
                continue;
            };
            for coordinate in requested {
                let Some(path) = capture_path(coordinate, |selector| {
                    super::scalar_fields::entry_selector_value(
                        self.occurrence(),
                        selector,
                        &self.current,
                        &self.fields,
                    )
                }) else {
                    continue;
                };
                if self.fields.iter().any(|field| {
                    field.owner == parameter.symbol
                        && super::scalar_fields::same_projection(
                            self.occurrence(),
                            &field.path,
                            &path,
                        )
                }) {
                    continue;
                }
                let Some(field) = validation::structural_scalar_coordinate(
                    self.program,
                    self.program.state_parameters(self.state),
                    parameter.symbol,
                    &coordinate,
                ) else {
                    continue;
                };
                let Some(sources) = field
                    .source_requirements
                    .iter()
                    .map(|requirement| {
                        frames
                            .coordinate_sources_before_statement(
                                self.machine,
                                self.state,
                                first,
                                parameter.symbol,
                                requirement,
                            )
                            .filter(|sources| !sources.is_empty())
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let read_dependencies = sources
                    .into_iter()
                    .flatten()
                    .map(|source| source.path)
                    .collect();
                self.fields
                    .push(super::scalar_fields::ScalarField::for_owner(
                        self.program,
                        parameter.symbol,
                        super::scalar_fields::CapturedScalarField {
                            path,
                            carrier: field.primitive_type,
                            read_dependencies,
                            value: ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "state:{:?}:parameter:{:?}:field:{coordinate:?}",
                                    self.state.symbol, parameter.symbol
                                ),
                                unsigned: super::unsigned(field.primitive_type),
                            },
                        },
                    ));
            }
        }
    }
}
