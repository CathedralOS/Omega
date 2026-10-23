//! Resolve ingress and completed projection sources in their actual storage.
//! Copying and transfer share root evaluation and typed path preparation;
//! the consuming operation separately chooses its ownership consequence.

use super::super::super::super::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
};
use super::super::super::{
    LoweringError, PlaceId, StructuralMultiplicity, lookup_type_id, unsupported,
};
use super::{Emission, ProjectionSource};
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

impl Emission<'_, '_, '_> {
    /// Resolve a checked source plan to a whole-owned state parameter's
    /// ingress place. `StructuralLocal` names a parameter directly by symbol;
    /// `Parameter` counts the authored parameter list filtered to non-const,
    /// non-primitive entries, matching the checker's record-place producer.
    /// Returns `None` when the plan names no parameter at all.
    pub(super) fn parameter_source(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<PlaceId>, LoweringError> {
        let Some(declaration) = self.parameter_declaration(argument)? else {
            return Ok(None);
        };
        // A whole linear parameter moves its entry claim with it: the
        // establishment that receives it resolves that same identity.
        if declaration.multiplicity == StructuralMultiplicity::Unrestricted {
            return unsupported("owned selection parameter lacks whole owned custody");
        }
        Ok(Some(declaration.place))
    }

    /// Naming a whole `Unrestricted` parameter reads it: its value is a copy
    /// and the parameter stays usable, so it is never moved. Returns the
    /// parameter's ingress place to copy from.
    pub(super) fn copied_parameter(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<StructuralArgument>, LoweringError> {
        Ok(self
            .parameter_declaration(argument)?
            .filter(|declaration| declaration.multiplicity == StructuralMultiplicity::Unrestricted)
            .map(|declaration| StructuralArgument {
                place: declaration.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }))
    }

    /// Resolve the whole ingress place named by the checked source plan.
    /// Qualifications remain attached to that signature place through copies
    /// and transfers; their presence does not change its storage identity.
    /// `None` when the plan names no parameter.
    pub(super) fn parameter_declaration(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<&StructuralParameterDeclaration>, LoweringError> {
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let parameters = self.checked.state_parameters(authored);
        let (position, parameter) = match &argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let Some(entry) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == *symbol)
                else {
                    return Ok(None);
                };
                entry
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let Some(entry) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        !parameter.is_const
                            && self
                                .checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_none()
                    })
                    .nth(*parameter_index as usize)
                else {
                    return unsupported("owned selection parameter index is out of range");
                };
                entry
            }
            _ => return Ok(None),
        };
        if parameter.is_const {
            return unsupported("owned selection parameter is not runtime ingress");
        }
        let declaration = self
            .evaluation
            .structural_parameters
            .iter()
            .find(|(source_position, _)| *source_position as usize == position)
            .map(|(_, declaration)| declaration)
            .ok_or(LoweringError::Unsupported(
                "owned selection parameter has no signature place",
            ))?;
        // Mutability permits later writes; it does not turn an owned formal
        // into a loan. Source checking selected this occurrence's move, and
        // the emitter preserves its current custody rather than requiring the
        // binding to have been immutable since invocation.
        if declaration.access != StructuralAccess::Owned {
            return unsupported("owned selection parameter lacks owned ingress");
        }
        Ok(Some(declaration))
    }

    /// The once-evaluated root carrying a projection: an established local
    /// keeps its emitted place; an evaluated product completes through the
    /// ordinary value evaluator, including constructor and selected values.
    /// Returns the root place, the root's checked type identity and argument
    /// source identity for an observation or exact owned child transfer.
    pub(super) fn projection_source(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: CheckedStructuralValueHandle,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> Result<ProjectionSource, LoweringError> {
        let node = self.structural_value_node(source);
        let mut root_alias_path = Vec::new();
        let (root, root_type_identity, root_source) = match node.kind {
            CheckedStructuralValueKind::Place(argument) => {
                let root_source = argument.source.clone();
                if !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    return unsupported("projected selection root requires whole owned custody");
                }
                // The root is an established local place or a whole-owned
                // parameter's signature place; both arrive under
                // `StructuralLocal`/`Parameter` source plans that
                // `parameter_source` and the local namespace resolve exactly.
                let place = if let Some(place) = self.parameter_source(&argument)? {
                    place
                } else {
                    // All completed roots share this storage judgment,
                    // including expression captures and projected aliases.
                    // Retain their exact physical prefix instead of another
                    // source-family roster or a reconstructed initializer.
                    let source = self.structural_bindings().owned_argument_home(&argument)?;
                    root_alias_path = source.path;
                    source.place
                };
                (place, argument.type_identity, root_source)
            }
            _ => {
                let root_type_identity = validation::stored_structural_type_identity(
                    &self.checked.typed,
                    node.type_reference,
                    &[],
                    &[],
                );
                let root_type =
                    lookup_type_id(self.type_ids, &root_type_identity).map_err(|_| {
                        LoweringError::MissingUnitStructuralType(format!(
                            "projection source: {}",
                            self.checked
                                .type_identity_text(root_type_identity)
                                .unwrap_or_else(|| format!("{root_type_identity:?}")),
                        ))
                    })?;
                // The selected result's paths are relative to its leaf. The
                // producing owner needs its own full checked frontier,
                // including children left in the residual complement.
                let completed = self.establish_computed_operand(
                    identities,
                    source,
                    root_type,
                    node.type_reference,
                )?;
                if !completed.argument.path.is_empty()
                    || completed.argument.access != StructuralAccess::Owned
                {
                    return unsupported("projected selection requires a completed whole owner");
                }
                // The source remains a distinct established home when its
                // selected child copies. Scope-exit cleanup consumes this
                // exact expression binding, never the copied leaf's home.
                crate::expression_preparation::bindings::retain_completed_structural_homes(
                    &mut self.evaluation.structural_locals,
                    &[
                        crate::expression_preparation::bindings::StructuralLocalBinding {
                            result_binding_ordinal: None,
                            root: facts::PlaceRoot::Expression(node.expression),
                            argument: completed.argument.clone(),
                            structural_type: root_type,
                        },
                    ],
                )?;
                (
                    completed.argument.place,
                    root_type_identity,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression: node.expression,
                    },
                )
            }
        };
        Ok(ProjectionSource {
            root,
            root_path: root_alias_path,
            path,
            root_type_identity,
            root_source,
        })
    }
}
