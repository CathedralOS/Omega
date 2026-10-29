//! One completed scalar store updates its physical storage associations.
//! Capture RHS first, authenticate exact locations at the same effect cut,
//! retire overwritten observations, then publish the new value and dependencies.
//! Typed origins establish equality; invalidation paths only expire observations.
use super::scalar_fields::{CapturedScalarField, ScalarProjection, source_path};
use super::{CurrentScalarValues, integer_carrier, unsigned};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticValue;

/// A current storage association, refreshed at the store's observation cut.
/// The read paths serve later invalidation; they are never equality evidence.
pub(super) struct ScalarStoreAlias {
    owner: SymbolHandle,
    path: Vec<ScalarProjection>,
    read_dependencies: Vec<String>,
}

impl CurrentScalarValues<'_> {
    /// Capture a scalar store's completed value before its target write retires
    /// associations. An opaque result gets one fresh store identity, no bits.
    pub(super) fn captured_scalar_store(
        &self,
        target: ExpressionHandle,
        value: Option<ScopedArithmeticValue>,
        ordinal: usize,
    ) -> Option<CapturedScalarField> {
        let (owner, path) = self.scalar_storage_path(target)?;
        let reference = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            target,
        )?;
        let reference = validation::reference_shell(self.program, reference)
            .map_or(reference, |shell| shell.referee);
        let carrier = integer_carrier(self.program, reference)?;
        let value = value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
            identity: format!(
                "state:{:?}:store:{ordinal}:field:{owner:?}:{path:?}:{carrier:?}",
                self.state.symbol
            ),
            unsigned: unsigned(carrier),
        });
        Some(CapturedScalarField {
            path: Vec::new(),
            carrier,
            source: None,
            read_dependencies: self
                .fields
                .iter()
                .find(|field| {
                    field.owner == owner
                        && super::scalar_fields::same_projection(
                            self.occurrence(),
                            &field.path,
                            &path,
                        )
                })
                .map_or_else(Vec::new, |field| field.read_dependencies.clone()),
            value,
        })
    }

    /// Capture the current scalar behind a newly formed reference. The source
    /// owner supplies exact load evidence; no initializer is replayed later.
    pub(super) fn reference_scalar_initializer(
        &self,
        statement: &typed_trees::statement::StatementNode,
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> (Option<ScopedArithmeticValue>, Option<Vec<String>>) {
        let Some(frames) = frames else {
            return (None, None);
        };
        if !frames
            .expression_write_frame(self.machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
        {
            return (None, None);
        }
        let Some(sources) = frames
            .reference_expression_origins_before_statement(self.machine, statement, expression)
            .filter(|sources| !sources.is_empty())
        else {
            return (None, None);
        };
        let dependencies = sources
            .iter()
            .map(|(root, path)| validation::frame_coordinate_path(self.program, *root, path))
            .collect::<Option<Vec<_>>>();
        let values = sources
            .iter()
            .map(|(root, path)| {
                if path.is_empty() {
                    return self.current_symbol(*root);
                }
                let path = super::scalar_fields::capture_path(path, |selector| {
                    self.capture(self.program, self.machine, self.state, selector)
                })?;
                self.fields
                    .iter()
                    .find(|field| {
                        field.owner == *root
                            && super::scalar_fields::same_projection(
                                self.occurrence(),
                                &field.path,
                                &path,
                            )
                    })?
                    .value
                    .clone()
            })
            .collect::<Option<Vec<_>>>();
        let value = values.and_then(|values| {
            let first = values.first()?;
            values
                .iter()
                .all(|value| value == first)
                .then(|| first.clone())
        });
        (value, dependencies)
    }

    /// A store publishes one completed scalar to every proven identical live
    /// location. Invalidation footprints cannot establish this equality. Use
    /// exact prefix sources, and keep owned copies at their distinct homes.
    /// Effectful operands have a later observation cut and cannot use this
    /// pre-statement correspondence; their ordinary invalidation still applies.
    pub(super) fn scalar_store_aliases(
        &self,
        statement: &typed_trees::statement::StatementNode,
        ordinal: usize,
        carrier: PrimitiveType,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Vec<ScalarStoreAlias> {
        let Some(frames) = frames else {
            return Vec::new();
        };
        let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
            return Vec::new();
        };
        if [assignment.target, assignment.value]
            .iter()
            .any(|expression| {
                !frames
                    .expression_write_frame(self.machine, *expression)
                    .into_complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        {
            return Vec::new();
        }
        let Some(origins) = frames.assignment_storage_origins(self.machine, statement) else {
            return Vec::new();
        };
        let [destination] = origins.destinations.as_slice() else {
            return Vec::new();
        };
        if destination.collection_coarse
            || !destination.root.is_valid()
            || destination.invocation_output.is_some()
            || destination.is_image_storage
        {
            return Vec::new();
        }
        let candidates = self
            .fields
            .iter()
            .filter(|field| field.carrier == carrier)
            .map(|field| (field.owner, field.path.clone()))
            .chain(
                self.current
                    .iter()
                    .filter(|binding| !binding.immutable_coordinate)
                    .map(|binding| (binding.symbol, Vec::new())),
            );
        candidates
            .filter_map(|(owner, path)| {
                let coordinate = source_path(&path);
                let sources = frames.coordinate_sources_before_statement(
                    self.machine,
                    self.state,
                    ordinal,
                    owner,
                    &coordinate,
                )?;
                let [source] = sources.as_slice() else {
                    return None;
                };
                (!source.collection_coarse
                    && source.root == destination.root
                    && source.segments == destination.segments
                    && source.recasts == destination.recasts
                    && source.representation == destination.representation
                    && source.invocation_output == destination.invocation_output
                    && source.is_image_storage == destination.is_image_storage)
                    .then(|| ScalarStoreAlias {
                        owner,
                        path,
                        read_dependencies: vec![source.path.clone()],
                    })
            })
            .collect()
    }

    pub(super) fn publish_scalar_store(
        &mut self,
        aliases: &[ScalarStoreAlias],
        value: &ScopedArithmeticValue,
    ) {
        for binding in &mut self.current {
            if let Some(alias) = aliases
                .iter()
                .find(|alias| alias.owner == binding.symbol && alias.path.is_empty())
            {
                binding.value = Some(value.clone());
                binding.alternatives.clear();
                binding.read_dependencies = Some(alias.read_dependencies.clone());
            }
        }
        let occurrence = super::ScopedOccurrence {
            program: self.program,
            machine: self.machine,
            state: self.state,
            lengths: &self.lengths,
            hypotheses: &self.hypotheses,
        };
        for field in &mut self.fields {
            if let Some(alias) = aliases.iter().find(|alias| {
                alias.owner == field.owner
                    && super::scalar_fields::same_projection(occurrence, &alias.path, &field.path)
            }) {
                field.value = Some(value.clone());
                field.read_dependencies.clone_from(&alias.read_dependencies);
            }
        }
    }
}
