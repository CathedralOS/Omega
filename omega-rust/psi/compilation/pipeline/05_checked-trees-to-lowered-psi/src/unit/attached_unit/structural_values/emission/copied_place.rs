//! Readable copy sources keep their exact backing place. A literal offset
//! into a view still needs a live-length proof: normalize that coordinate to
//! the existing runtime-index operation contract, not a fixed-array extent.

use super::{Emission, LoweringError, StructuralAccess, StructuralArgument};
use crate::emission::operation_emission::expressions::{emit_element_length, emit_scalar_leaf};
use crate::expression_preparation::bindings::StructuralScalarFieldBinding;
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
        let mut selectors = self
            .capture_projection_selectors(identities, expression, source)?
            .into_iter();
        let place = home.place;
        let prepared = home.path.into_iter().map(
            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static,
        ).chain(prepared).collect::<Vec<_>>();
        let path =
            crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                &prepared,
                place,
                Some(self.values.as_slice()),
                &mut selectors,
                Some(&mut *self.calls),
                Some((&mut *self.next_value, &mut *self.operations)),
            )?;
        Ok(StructuralArgument {
            place,
            path,
            access: StructuralAccess::SharedBorrow,
        })
    }

    /// Copies and reference formation evaluate the same retained projection
    /// operands in order. Later selector control flow transports earlier saved
    /// values; no consumer replays their source expressions after capture.
    pub(super) fn capture_projection_selectors(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: checked_trees::expression::ExpressionHandle,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<Vec<semantic_vocabulary::ValueId>, LoweringError> {
        let fields = StructuralScalarFieldBinding::collect(
            &self.evaluation.structural_parameters,
            self.structural_types,
        );
        let source_count = self.values.len();
        let mut computation_ordinal = 0u32;
        for segment in &source.path {
            match segment {
                checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(value) => {
                    let selector = self.scalar(
                        identities,
                        checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                            expression,
                            selector_ordinal: computation_ordinal,
                        },
                        *value,
                        source_count,
                    )?;
                    computation_ordinal =
                        computation_ordinal
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "projection selector ordinal overflow",
                            ))?;
                    self.values.push(selector);
                }
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StructuralField(_)
                    | checked_trees::CheckedRuntimeIndex::StorageRead { .. },
                ) => {
                    let selected = super::super::super::parameters::mint_runtime_index_selectors(
                        identities,
                        &[checked_trees::CheckedUnitStructuralArgumentPlan {
                            path: vec![segment.clone()],
                            ..source.clone()
                        }],
                        &fields,
                        &self.evaluation.contextual_bindings(),
                        self.values.as_slice(),
                        self.next_value,
                        self.operations,
                    )?;
                    for id in selected {
                        let value = self
                            .operations
                            .iter()
                            .rev()
                            .filter_map(|operation| operation.result.scalar())
                            .find(|value| value.id == id)
                            .or_else(|| self.values.iter().find(|value| value.id == id).copied())
                            .ok_or(LoweringError::Unsupported(
                                "projection field selector lost its evaluated value",
                            ))?;
                        self.values.push(value);
                    }
                }
                _ => {}
            }
        }
        // Private selector control flow transports the earlier saved
        // values. Read their final identities after all evaluations.
        let captured = self.values[source_count..]
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>();
        self.values.truncate(source_count);
        Ok(captured)
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
            .chain(self.evaluation.block_structural_parameters.iter())
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
