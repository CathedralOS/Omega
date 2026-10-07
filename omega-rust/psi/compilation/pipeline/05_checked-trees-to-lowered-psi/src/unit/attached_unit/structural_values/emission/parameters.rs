//! Parameter ingress places: resolving a checked source plan to a whole
//! parameter declaration, and projecting a parameter subtree to its moved
//! root with the alias path and leaf/root type identities the operation
//! result carries.

use super::super::super::super::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
};
use super::super::super::{
    LoweringError, PlaceId, StructuralMultiplicity, StructuralPlaceKind, unsupported,
};
use super::{Emission, ProjectedMove};
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
        if declaration.multiplicity != StructuralMultiplicity::Affine {
            return unsupported("owned selection parameter lacks whole plain-affine custody");
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
        if parameter.is_const || parameter.is_mutable {
            return unsupported("owned selection parameter is not immutable owned ingress");
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
        Ok(Some(declaration))
    }

    /// The once-evaluated root carrying a projected move: an established local
    /// keeps its emitted place; a structural product emits its producing call.
    /// Returns the root place, the root's checked type identity for residual
    /// reconstruction, and its argument source plan for stamped evidence.
    pub(super) fn projected_root(
        &mut self,
        source: CheckedStructuralValueHandle,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        leaf_type_identity: String,
    ) -> Result<ProjectedMove, LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(source)
            .clone();
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
                let place = match &root_source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => match self
                        .evaluation
                        .structural_locals
                        .iter()
                        .find(|local| local.root == facts::PlaceRoot::Symbol(*symbol))
                        .map(|local| &local.argument)
                    {
                        // A root that is itself a projected alias contributes
                        // its own path: the composed walk names the leaf under
                        // the place that actually holds the storage. Every
                        // other established local occupies its whole place and
                        // contributes nothing here.
                        Some(source) => {
                            root_alias_path = source.path.clone();
                            source.place
                        }
                        None => {
                            self.parameter_source(&argument)?
                                .ok_or(LoweringError::Unsupported(
                                    "projected selection root place missing",
                                ))?
                        }
                    },
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        ..
                    } => self
                        .parameter_source(&argument)?
                        .ok_or(LoweringError::Unsupported(
                            "projected selection root place missing",
                        ))?,
                    _ => {
                        return unsupported("projected selection root is not a structural local");
                    }
                };
                (place, argument.type_identity, root_source)
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self
                    .operand_calls
                    .iter()
                    .filter(|call| call.value == source);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "projected selection operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("projected selection operand call is duplicated");
                }
                let Some(result) = call
                    .operation()
                    .call()
                    .filter(|planned| planned.is_module())
                    .and_then(checked_trees::CheckedUnitCallPlan::structural_result)
                else {
                    return unsupported(
                        "projected selection call drifted from its structural producer",
                    );
                };
                let root_type_identity = result.type_identity.clone();
                let binding_ordinal = result.binding_ordinal;
                let declaration = self.emit_operand_call(call)?;
                let StructuralPlaceKind::OperationResult { .. } = declaration.kind else {
                    return unsupported("projected selection call has no result place");
                };
                (
                    declaration.id,
                    root_type_identity,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    },
                )
            }
            _ => {
                return unsupported("projected selection requires a place or call root");
            }
        };
        Ok(ProjectedMove {
            root,
            root_path: root_alias_path,
            path,
            leaf_type_identity,
            root_type_identity,
            root_source,
        })
    }
}
