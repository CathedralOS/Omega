//! Executed scalar-field transfers retain exact place and carrier identities.
//! A stored value is captured before invalidation. Later reads use that value,
//! never the field initializer or a new observation of overwritten storage.
use super::{Binding, CurrentScalarValues, ScopedOccurrence, integer_carrier};
mod collection_values;
mod coordinates;
pub(super) use coordinates::{
    ScalarProjection, capture_path, entry_selector_value, projection_has_prefix, same_projection,
    source_path, storage_path,
};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticValue;

#[derive(Clone)]
pub(super) struct ScalarField {
    pub(super) owner: SymbolHandle,
    pub(super) path: Vec<ScalarProjection>,
    pub(super) carrier: PrimitiveType,
    pub(super) name: String,
    pub(super) value: Option<ScopedArithmeticValue>,
    pub(super) read_dependencies: Vec<String>,
}

pub(super) struct CapturedScalarField {
    pub(super) path: Vec<ScalarProjection>,
    pub(super) carrier: PrimitiveType,
    pub(super) value: ScopedArithmeticValue,
    pub(super) read_dependencies: Vec<String>,
    pub(super) source: Option<String>,
}

impl ScalarField {
    pub(super) fn for_owner(
        program: &dyn validation::CheckingInput,
        owner: SymbolHandle,
        field: CapturedScalarField,
    ) -> Self {
        let name = validation::frame_coordinate_path(program, owner, &source_path(&field.path))
            .expect("a captured scalar projection has a valid storage root");
        Self {
            owner,
            path: field.path,
            carrier: field.carrier,
            name,
            value: Some(field.value),
            read_dependencies: field.read_dependencies,
        }
    }
}

pub(super) fn scalar_field_occurrence(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ScopedArithmeticValue> {
    let (owner, path) = storage_path(occurrence, expression, bindings, fields)?;
    let ScopedOccurrence {
        program,
        machine,
        state,
        ..
    } = occurrence;
    if path.is_empty() {
        return None;
    }
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    let carrier = integer_carrier(program, reference)?;
    collection_values::selected_value(occurrence, owner, &path, carrier, fields)
}

impl CurrentScalarValues<'_> {
    pub(super) fn scalar_storage_path(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, Vec<ScalarProjection>)> {
        storage_path(self.occurrence(), expression, &self.current, &self.fields)
    }

    /// Syntax handles identify observations, while captured selectors identify
    /// the selected entry storage. Normalization may copy a selector into a
    /// fresh local without changing that storage coordinate.
    pub(crate) fn same_scalar_projection(
        &self,
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
    ) -> bool {
        let capture = |path: &[facts::PlaceSegment]| {
            capture_path(path, |selector| {
                coordinates::selector_value(
                    self.occurrence(),
                    selector,
                    &self.current,
                    &self.fields,
                )
            })
        };
        match (capture(left), capture(right)) {
            (Some(left), Some(right)) => same_projection(self.occurrence(), &left, &right),
            _ => false,
        }
    }

    /// Exact retained inline fields of one formal. The caller selects the
    /// frame: an entry frame supplies entry values; a later frame supplies
    /// only associations that survived its executed writes and calls.
    pub(crate) fn scalar_field_observations(
        &self,
        parameter: SymbolHandle,
    ) -> impl Iterator<
        Item = (
            Vec<facts::PlaceSegment>,
            PrimitiveType,
            &ScopedArithmeticValue,
            Vec<checked_trees::CheckedEntryIndexBound>,
        ),
    > {
        self.fields.iter().filter_map(move |field| {
            if field.owner != parameter {
                return None;
            }
            let value = field.value.as_ref()?;
            let indexes = self.scalar_projection_formation(field)?;
            Some((source_path(&field.path), field.carrier, value, indexes))
        })
    }

    fn scalar_projection_formation(
        &self,
        field: &ScalarField,
    ) -> Option<Vec<checked_trees::CheckedEntryIndexBound>> {
        // This arithmetic roster retains no case-membership judgment. A case
        // projection cannot become an unconditional entry observation here.
        if field
            .path
            .iter()
            .any(|step| matches!(step, ScalarProjection::Case(_)))
        {
            return None;
        }
        let geometry = validation::structural_scalar_coordinate(
            self.program,
            self.program.state_parameters(self.state),
            field.owner,
            &source_path(&field.path),
        )?;
        let mut indexes = Vec::new();
        for (path_step, step) in geometry.projections.iter().enumerate() {
            let validation::ScalarCoordinateProjection::Element {
                selector: expression,
                collection_type,
                element_type,
                collection_path,
                source_step,
                extent,
            } = step
            else {
                continue;
            };
            let ScalarProjection::Element { selector, .. } = field.path.get(*source_step)? else {
                return None;
            };
            let extent = extent.map(u64::try_from).transpose().ok()?;
            let length = if let Some(extent) = extent {
                ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(extent))
            } else {
                // The retained roster names this exact descriptor binding,
                // never its backing capacity or a same-typed sibling's count.
                self.coordinate_collection_length(field.owner, collection_path)?
            };
            if !self.proves_values_relation(
                self.program,
                self.machine,
                typed_trees::expression::BinaryOperator::GreaterOrEqual,
                selector,
                &ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero()),
            ) || !self.proves_values_relation(
                self.program,
                self.machine,
                typed_trees::expression::BinaryOperator::Less,
                selector,
                &length,
            ) {
                return None;
            }
            let primitive_type = match expression {
                checked_trees::CheckedEntryIndexSelector::Expression(expression) => {
                    let reference = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        *expression,
                    )?;
                    integer_carrier(self.program, reference)?
                }
                checked_trees::CheckedEntryIndexSelector::Constant(_) => PrimitiveType::U64,
            };
            indexes.push(checked_trees::CheckedEntryIndexBound {
                state: self.state.symbol,
                parameter: field.owner,
                path_step: u32::try_from(path_step).ok()?,
                selector: expression.clone(),
                collection_path: collection_path.clone(),
                collection_type: *collection_type,
                element_type: *element_type,
                extent,
                primitive_type,
            });
        }
        Some(indexes)
    }

    /// Instantiate the whole selector telescope before looking up a projected
    /// actual. A target's `slot` is the passed scalar value, not its old name
    /// in the source member of a mutual component.
    pub(crate) fn projected_scalar_value_in_call(
        &self,
        actual: ExpressionHandle,
        path: &[facts::PlaceSegment],
        callee: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
    ) -> Option<ScopedArithmeticValue> {
        if path.is_empty() {
            return self.capture(self.program, self.machine, self.state, actual);
        }
        if parameters.len() != arguments.len() {
            return None;
        }
        let bindings = parameters
            .iter()
            .zip(arguments)
            .filter(|(parameter, _)| {
                integer_carrier(self.program, parameter.type_reference).is_some()
            })
            .map(|(parameter, argument)| Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: self.capture(self.program, self.machine, self.state, *argument),
                alternatives: Vec::new(),
                immutable_coordinate: true,
                read_dependencies: Some(Vec::new()),
            })
            .collect::<Vec<_>>();
        let occurrence = self.occurrence_at(self.program, callee, state, &[]);
        let path = capture_path(path, |selector| {
            coordinates::selector_value(occurrence, selector, &bindings, &[])
        })?;
        self.captured_scalar_fields(actual, 0)
            .into_iter()
            .find(|field| same_projection(self.occurrence(), &field.path, &path))
            .map(|field| field.value)
    }

    /// Transfer only values from this executed expression or the live roster.
    /// Unmodeled producers and unequal joins supply no field equality.
    pub(super) fn captured_scalar_fields(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Vec<CapturedScalarField> {
        if depth >= 64 {
            return Vec::new();
        }
        let mut captured = match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.captured_scalar_fields(borrow.target, depth + 1),
            ExpressionNode::ArrayLiteral(elements) => self.capture_array_elements(*elements, depth),
            ExpressionNode::Indexed(indexed)
                if validation::has_builtin_subslice_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                ) =>
            {
                self.capture_window_elements(expression, indexed, depth)
                    .unwrap_or_default()
            }
            ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                let mut captured = Vec::new();
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    if !field.field_symbol.is_valid() {
                        continue;
                    }
                    for mut nested in self.captured_scalar_fields(field.value, depth + 1) {
                        nested
                            .path
                            .insert(0, ScalarProjection::Field(field.field_symbol));
                        captured.push(nested);
                    }
                    let carrier = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        field.value,
                    )
                    .and_then(|reference| integer_carrier(self.program, reference));
                    if let Some(carrier) = carrier
                        && let Some(value) =
                            self.capture(self.program, self.machine, self.state, field.value)
                    {
                        captured.push(CapturedScalarField {
                            path: vec![ScalarProjection::Field(field.field_symbol)],
                            carrier,
                            value,
                            read_dependencies: Vec::new(),
                            source: None,
                        });
                    }
                }
                captured
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                let Some((owner, prefix)) = self.scalar_storage_path(expression) else {
                    return Vec::new();
                };
                self.fields
                    .iter()
                    .filter_map(|field| {
                        if field.owner != owner
                            || !projection_has_prefix(self.occurrence(), &field.path, &prefix)
                            || field.path.len() <= prefix.len()
                        {
                            return None;
                        }
                        Some(CapturedScalarField {
                            path: field.path[prefix.len()..].to_vec(),
                            carrier: field.carrier,
                            value: field.value.clone()?,
                            read_dependencies: field.read_dependencies.clone(),
                            source: Some(field.name.clone()),
                        })
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let borrowed = matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Borrow(_)
        ) || validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            expression,
        )
        .and_then(|reference| validation::reference_shell(self.program, reference))
        .is_some();
        if borrowed {
            for field in &mut captured {
                // Copies of owned values become independent snapshots; a
                // descriptor only copies a binding to live backing. Retain
                // that original coordinate before window paths are rebased.
                if let Some(source) = &field.source {
                    crate::collections::push_unique(&mut field.read_dependencies, source.clone());
                }
            }
        }
        captured
    }

    pub(super) fn replace_scalar_fields(
        &mut self,
        owner: SymbolHandle,
        prefix: &[ScalarProjection],
        fields: Vec<CapturedScalarField>,
    ) {
        let occurrence = ScopedOccurrence {
            program: self.program,
            machine: self.machine,
            state: self.state,
            lengths: &self.lengths,
            hypotheses: &self.hypotheses,
        };
        self.fields.retain(|field| {
            field.owner != owner
                || !(projection_has_prefix(occurrence, &field.path, prefix)
                    || projection_has_prefix(occurrence, prefix, &field.path))
        });
        for mut field in fields {
            let mut path = prefix.to_vec();
            path.extend(field.path);
            field.path = path;
            self.fields
                .push(ScalarField::for_owner(self.program, owner, field));
        }
    }
}
