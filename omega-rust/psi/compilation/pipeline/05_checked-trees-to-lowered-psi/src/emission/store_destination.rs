//! Store destinations retain exact storage identity and operation authority.
//! Local field stores share root admission here; each operation separately
//! checks its replacement and the receiving frontier checks live custody.

use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{PlaceId, StructuralTypeId};

/// A local names its current storage declaration, which may be an operation
/// result or a parameter after an ownership-preserving continuation transfer.
/// The declaration's type and evidence survive that transfer; an operation
/// producer is not required to establish a parameter's storage a second time.
pub(crate) struct LocalStoreRoot<'a> {
    pub(crate) argument: &'a terminal_psi::StructuralArgument,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
    /// Type at the local alias's captured path within that root.
    pub(crate) selected_type: StructuralTypeId,
    pub(crate) qualifications: &'a [semantic_vocabulary::StructuralDomainId],
    pub(crate) projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
}

impl LocalStoreRoot<'_> {
    pub(crate) fn require_unqualified(&self) -> Result<(), LoweringError> {
        self.require_disjoint_from_write(&[])
    }

    /// Preserve only evidence whose original published-root region is disjoint
    /// from the complete normalized write. Overlapping predicates require an
    /// operation-specific restoration proof, including opaque root predicates.
    pub(crate) fn require_disjoint_from_write(
        &self,
        written: &[terminal_psi::StructuralPathSegment],
    ) -> Result<(), LoweringError> {
        if !terminal_semantics::structural_memberships_disjoint_from_write(
            self.qualifications,
            self.projected_qualifications,
            written,
        ) {
            return unsupported(
                "store local carries qualification evidence the replacement does not restore",
            );
        }
        Ok(())
    }
}

/// Resolve a local destination once, independently of the value being stored.
/// An owned home and an exclusive loan both permit mutation; neither grants
/// permission to drop predicates or claims. The operation still checks the
/// destination's shape and the current ownership frontier.
pub(crate) fn local_store_place<'a>(
    symbol: symbols::SymbolHandle,
    locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
    parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
    operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<LocalStoreRoot<'a>, LoweringError> {
    let crate::expression_preparation::bindings::StructuralLocalBinding {
        argument,
        structural_type: selected_type,
        ..
    } = crate::expression_preparation::bindings::structural_local_by_root(
        locals,
        facts::PlaceRoot::Symbol(symbol),
    )
    .ok_or(LoweringError::Unsupported(
        "store names an unknown structural local",
    ))?;
    if !matches!(
        argument.access,
        terminal_psi::mutating_access_pair_pattern!()
    ) {
        return unsupported("store local does not retain exclusive storage access");
    }
    let declaration = parameters
        .map(|parameter| {
            (
                parameter.place,
                parameter.structural_type,
                parameter.qualifications.as_slice(),
                parameter.projected_qualifications.as_slice(),
                parameter.multiplicity,
            )
        })
        .chain(operations.operations.iter().filter_map(|operation| {
            operation.result.structural().map(|result| {
                (
                    result.place,
                    result.structural_type,
                    result.qualifications.as_slice(),
                    result.projected_qualifications.as_slice(),
                    result.multiplicity,
                )
            })
        }))
        .find(|(place, _, _, _, _)| *place == argument.place)
        .ok_or(LoweringError::Unsupported(
            "store local has no declared storage root",
        ))?;
    Ok(LocalStoreRoot {
        argument,
        structural_type: declaration.1,
        multiplicity: declaration.4,
        selected_type: *selected_type,
        qualifications: declaration.2,
        projected_qualifications: declaration.3,
    })
}

/// Operations on a whole descriptor or owner require its complete home.
/// Field stores instead compose `local_store_place`'s captured alias prefix;
/// dropping that prefix would write another element of the enclosing carrier.
pub(crate) fn local_store_root<'a>(
    symbol: symbols::SymbolHandle,
    locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
    parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
    operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<LocalStoreRoot<'a>, LoweringError> {
    let root = local_store_place(symbol, locals, parameters, operations)?;
    if !root.argument.path.is_empty() {
        return unsupported("whole-home store names a projected local alias");
    }
    Ok(root)
}

/// Field replacement and restoration windows share the same root contract.
/// A store cannot preserve opaque root evidence merely by restoring a field.
/// Byte-owner writes instead retain their predicates in the byte operation.
/// Claims are current regional custody, not opaque type evidence: scalar
/// writes preserve them and structural moves/stores transport their exact
/// frontier. The original producer's claim roster cannot forbid all later
/// mutations of a home with a linear child.
pub(crate) fn unqualified_local_store_root<'a>(
    symbol: symbols::SymbolHandle,
    locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
    parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
    operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
    let root = local_store_root(symbol, locals, parameters, operations)?;
    root.require_unqualified()?;
    Ok((root.argument.place, root.structural_type))
}

/// The declared descriptor home a whole shared-view store replaces, and
/// whether it declares byte qualifications each store must prove. The home's
/// write authority is its own establishment: the binding's access describes
/// the referenced bytes, which a descriptor store never writes. Only a
/// declared local's home qualifies; a literal, subslice or call result names
/// immutable producer storage, not a replaceable slot.
pub(crate) fn descriptor_home(
    symbol: symbols::SymbolHandle,
    locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    operations: &crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<(PlaceId, StructuralTypeId, bool), LoweringError> {
    let crate::expression_preparation::bindings::StructuralLocalBinding { argument, .. } =
        crate::expression_preparation::bindings::structural_local_by_root(
            locals,
            facts::PlaceRoot::Symbol(symbol),
        )
        .ok_or(LoweringError::Unsupported(
            "descriptor store names an unknown structural local",
        ))?;
    if !argument.path.is_empty() {
        return unsupported("descriptor store local is not a whole home");
    }
    let producer = operations
        .operations
        .iter()
        .find(|operation| {
            operation
                .result
                .structural()
                .is_some_and(|result| result.place == argument.place)
        })
        .ok_or(LoweringError::Unsupported(
            "descriptor store local has no produced home",
        ))?;
    let Some(produced) = producer.result.structural() else {
        return unsupported("descriptor store local has no produced home");
    };
    if !matches!(
        producer.kind,
        terminal_psi::OperationKind::EstablishStructuralLocal {}
    ) || !matches!(
        produced.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) || !produced.projected_qualifications.is_empty()
        || !produced.claims.is_empty()
    {
        return unsupported("descriptor store local is not a declared descriptor home");
    }
    Ok((
        argument.place,
        produced.structural_type,
        !produced.qualifications.is_empty(),
    ))
}
