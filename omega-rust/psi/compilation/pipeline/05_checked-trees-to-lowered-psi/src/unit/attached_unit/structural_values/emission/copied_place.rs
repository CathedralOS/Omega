//! Readable copy sources keep their exact backing place. A literal offset
//! into a view still needs a live-length proof: normalize that coordinate to
//! the existing runtime-index operation contract, not a fixed-array extent.

use super::{Emission, LoweringError, StructuralAccess, StructuralArgument};
use crate::emission::operation_emission::expressions::{emit_element_length, emit_scalar_leaf};
use crate::expression_preparation::bindings::{ScalarBindings, StructuralScalarFieldBinding};
use checked_trees::{CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeShape};

impl Emission<'_, '_, '_> {
    /// Bind both a direct copy and a projected copy through the same exact
    /// authored root and ordered runtime-selector evaluation.
    pub(super) fn copied_place_argument(
        &mut self,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        let authored = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let bindings = ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals);
        let parameters = self.checked.state_parameters(authored.1);
        if !source
            .path
            .iter()
            .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
        {
            return bindings.shared_structural_argument(source, authored.0, parameters);
        }
        let place = bindings.shared_structural_place(source, authored.0, parameters)?;
        let fields = StructuralScalarFieldBinding::collect(
            &self.evaluation.structural_parameters,
            self.structural_types,
        );
        let mut selector_values = super::super::super::parameters::mint_runtime_index_selectors(
            std::slice::from_ref(source),
            &fields,
            self.values.as_slice(),
            self.next_value,
            self.operations,
        )?
        .into_iter();
        Ok(StructuralArgument {
            place,
            path: crate::expression_preparation::bindings::structural_paths::lower_argument_path(
                &source.path,
                Some(self.values.as_slice()),
                &mut selector_values,
                Some(&mut *self.calls),
            )?,
            access: StructuralAccess::SharedBorrow,
        })
    }

    /// Only root views have a length observation in the receiving projection
    /// contract. Fixed arrays retain their declared bounds and static paths;
    /// the suffix of a view projection is preserved without reinterpretation.
    pub(super) fn copied_place_path(
        &mut self,
        argument: &StructuralArgument,
    ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
        let mut path = argument.path.clone();
        if !matches!(
            path.first(),
            Some(StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. })
        ) {
            return Ok(path);
        }
        let root_type = self
            .evaluation
            .structural_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .chain(self.evaluation.block_structural_parameters.iter())
            .chain(
                self.evaluation
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == argument.place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                self.operations
                    .iter()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| result.place == argument.place)
                    .map(|result| result.structural_type)
            })
            .or_else(|| {
                self.evaluation
                    .view_locals
                    .iter()
                    .find(|local| local.place == argument.place)
                    .map(|local| local.structural_type)
            })
            .ok_or(LoweringError::Unsupported(
                "indexed copy has no established root type",
            ))?;
        let declaration = self
            .structural_types
            .iter()
            .find(|declaration| declaration.id == root_type)
            .ok_or(LoweringError::Unsupported(
                "indexed copy root type is absent from the catalog",
            ))?;
        if !matches!(declaration.shape, StructuralTypeShape::ElementView { .. }) {
            return Ok(path);
        }
        if let Some(StructuralPathSegment::FixedIndex(index)) = path.first() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| {
                LoweringError::Unsupported("view index has no unsigned count carrier")
            })?;
            let index = emit_scalar_leaf(
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(u128::from(*index)),
                },
                ScalarType::Integer(integer_type),
                self.next_value,
                self.operations,
            );
            path[0] = StructuralPathSegment::RuntimeIndex {
                index,
                obligation: self.calls.allocate_requirement()?,
            };
        }
        // The verifier rejoins this exact root's current, dominating length
        // to the index obligation. No capacity or source-array size is used.
        emit_element_length(argument.place, self.next_value, self.operations);
        Ok(path)
    }
}
