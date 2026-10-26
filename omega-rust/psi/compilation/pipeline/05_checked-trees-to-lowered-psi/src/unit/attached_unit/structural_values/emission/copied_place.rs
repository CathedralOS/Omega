//! Readable copy sources keep their exact backing place. A literal offset
//! into a view still needs a live-length proof: normalize that coordinate to
//! the existing runtime-index operation contract, not a fixed-array extent.

use super::{Emission, LoweringError, StructuralAccess, StructuralArgument};
use crate::emission::operation_emission::expressions::{emit_element_length, emit_scalar_leaf};
use checked_trees::CheckedUnitStructuralArgumentPlan;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeShape};

impl Emission<'_, '_, '_> {
    /// Bind both a direct copy and a projected copy through the same exact
    /// authored root and ordered runtime-selector evaluation.
    pub(super) fn copied_place_argument(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: checked_trees::expression::ExpressionHandle,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        if source.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return super::unsupported("copied value requires readable source custody");
        }
        let authored = crate::declarations::state_declaration(self.checked, self.state)?;
        let bindings = self.structural_bindings();
        let parameters = self.checked.state_parameters(authored.1);
        let home = bindings.borrowed_structural_home(source, parameters)?;
        let root = bindings
            .structural_argument_type(&home)
            .ok_or(LoweringError::Unsupported(
                "indexed copy lost its published root type",
            ))?;
        let prepared =
            crate::expression_preparation::bindings::structural_paths::prepare_structural_argument_path(
                identities,
                self.checked,
                source,
                root,
                bindings.structural_declarations(),
            )?;
        let prepared = home.path.into_iter().map(
            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static,
        ).chain(prepared).collect::<Vec<_>>();
        self.complete_projection(
            identities,
            expression,
            home.place,
            prepared,
            StructuralAccess::SharedBorrow,
        )
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
        let root_type = self.projection_root_type(argument.place)?;
        let declaration = terminal_psi::find_structural_type(&self.structural_types[..], root_type)
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
        emit_element_length(
            terminal_psi::StructuralArgument::whole(
                argument.place,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
            self.next_value,
            self.operations,
        );
        Ok(path)
    }
}
