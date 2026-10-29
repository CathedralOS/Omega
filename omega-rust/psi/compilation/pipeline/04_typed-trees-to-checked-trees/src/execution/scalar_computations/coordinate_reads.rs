//! Entry coordinates are pure observations over an immutable formal telescope.
//! The source owner proves selector formation before admitting that coordinate.
//! Bind its pure scalar recipe directly to captured entry formals: scheduling an
//! ordinary effectful read chain would put control before the rank exists.
//! Geometry and scalar read templates remain shared with ordinary observations;
//! no Indexed AST or later authored occurrence supplies entry authority.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
};
use checked_trees::{
    CheckedEntryIndexBound, CheckedScalarExpression, CheckedScalarReadProjection,
    CheckedScalarSelector, CheckedStorageRoot, CheckedStructuralPredicatePathSegment as Predicate,
    CheckedUnitStructuralPathSegment as Storage,
};
use typed_trees::types::PrimitiveType;

impl Builder<'_, '_> {
    pub(super) fn scalar_coordinate(
        &mut self,
        expression: ExpressionHandle,
        coordinate: &validation::CanonicalPlace,
        expected_type: PrimitiveType,
        entry_indexes: &[CheckedEntryIndexBound],
    ) -> Option<CheckedScalarComputationHandle> {
        let facts::PlaceRoot::Symbol(symbol) = coordinate.root else {
            return None;
        };
        if coordinate.segments.is_empty() {
            let position = self
                .parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)?;
            if self.parameter_types.get(position) != Some(&expected_type) {
                return None;
            }
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: expected_type,
                }),
            ));
        }
        let geometry = validation::structural_scalar_coordinate(
            self.program,
            self.authored_parameters,
            symbol,
            &coordinate.segments,
        )?;
        if geometry.primitive_type != expected_type {
            return None;
        }
        let parameter_position = self
            .authored_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        let mut path = Vec::new();
        for (path_step, step) in geometry.projections.into_iter().enumerate() {
            match step {
                validation::ScalarCoordinateProjection::Storage(step) => {
                    let step = match step {
                        Storage::Referent => Predicate::Referent,
                        Storage::Field(field) => Predicate::Field(field),
                        Storage::Case(case) => Predicate::Case(case),
                        Storage::FixedIndex(index) => Predicate::FixedIndex(index),
                        _ => return None,
                    };
                    path.push(CheckedScalarReadProjection::Static(step));
                }
                validation::ScalarCoordinateProjection::Element {
                    selector,
                    collection_type,
                    element_type,
                    extent,
                } => {
                    let path_step = u32::try_from(path_step).ok()?;
                    let mut bounds = entry_indexes.iter().filter(|bound| {
                        bound.state == self.state
                            && bound.parameter == symbol
                            && bound.path_step == path_step
                            && bound.selector == selector
                            && bound.collection_type == collection_type
                            && bound.element_type == element_type
                            && Some(bound.extent) == u64::try_from(extent).ok()
                    });
                    let bound = bounds.next()?;
                    if bounds.next().is_some() {
                        return None;
                    }
                    let selector_value = crate::values::scalar::scalar_lowering::pure_selector(
                        self.program,
                        self.operators,
                        selector,
                        self.parameters,
                        self.authored_parameters,
                        self.parameter_types,
                        self.locals,
                        self.exact_integer_casts,
                    )?;
                    if crate::values::scalar::expression_facts::scalar_expression_type(
                        &selector_value,
                    ) != Some(bound.primitive_type)
                    {
                        return None;
                    }
                    path.push(CheckedScalarReadProjection::RuntimeIndex(
                        CheckedScalarSelector {
                            expression: selector_value,
                            occurrence: ExpressionHandle::invalid(),
                            entry_bound: Some(bound.clone()),
                        },
                    ));
                }
            }
        }
        let (template, _) = crate::values::scalar::scalar_lowering::scalar_storage_read_projection(
            self.program,
            expression,
            self.authored_parameters,
            crate::values::scalar::scalar_lowering::ScalarStorageProjection {
                root: CheckedStorageRoot::Parameter {
                    index: u32::try_from(parameter_position).ok()?,
                },
                path,
                selected_type: geometry.result_type_reference,
            },
        )?;
        Some(self.insert(expected_type, CheckedScalarComputationKind::Value(template)))
    }
}
