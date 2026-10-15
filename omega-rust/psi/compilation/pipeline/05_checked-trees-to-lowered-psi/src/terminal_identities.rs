//! Fixed Terminal-Psi identity allocation and checked-to-lowered identity lookup.
//!
//! Producers allocate dense nonzero identities from these constructors and
//! rejoin checked sources to the lowered tables through the lookup functions.
//! Both lookup failures and namespace exhaustion fail closed.

use language_semantics::{Multiplicity, PermissionClaimIdentity, ServiceReachId};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId,
    PlaceId, PropositionId, ScalarType, ServiceId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId, ValueId,
};

use crate::lowering_error::LoweringError;

// Structural Unit call requirements occupy the upper half of the module-wide
// obligation namespace. Existing contract and cleanup producers allocate from
// the lower half, so composing their proof bundles cannot alias a call site.
pub(crate) const BYTE_VIEW_ELEMENT_TYPE_IDENTITY: &str = "named(name(u8))";

pub(crate) const TERMINAL_UNIT_CALL_OBLIGATION_BASE: u64 = 1_u64 << 63;

/// The Terminal tag for one checked custody multiplicity.
///
/// This mapping is total on purpose. A producer that re-spells it inline
/// tends to enumerate only the multiplicities its own result place can wear
/// and refuse `Linear`, which is wrong for a *contained* value: checking
/// records a transparent aggregate's linear members as independent
/// path-indexed claims on the aggregate's root rather than inventing a root
/// claim, so a linear member is an ordinary carrier of an affine record and
/// the tag is the member's own, not the root's. Call this instead of
/// matching; the refusal that belongs to a particular result place belongs
/// at that place, not in the conversion.
pub(crate) const fn structural_multiplicity(
    multiplicity: Multiplicity,
) -> terminal_psi::StructuralMultiplicity {
    match multiplicity {
        Multiplicity::Unrestricted => terminal_psi::StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => terminal_psi::StructuralMultiplicity::Affine,
        Multiplicity::Linear => terminal_psi::StructuralMultiplicity::Linear,
    }
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        pub(crate) fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);

/// Publish the resolved service declaration, including its package and module.
/// The service reach table's name is a diagnostic label: distinct modules may
/// each declare `Sink`. Executable catalogs and retained reach contracts must
/// use this same identity or an unused contract can silently select its sibling.
pub(crate) fn service_declaration_identity(
    checked: &checked_trees::CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    checked
        .typed
        .normalized_hermetic_symbol_identity(symbol)
        .map_err(LoweringError::InvalidServiceIdentity)
}

/// Find the `u8` scalar a borrowed `&[u8]` view establishes through the
/// element-view op family. The byte carrier keeps no element field, so the
/// catalog roots this row at that establishment and the emitter resolves the
/// op's element through it.
pub(crate) fn primitive_scalar_identity<'a>(
    mut plans: impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan>,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    plans
        .find(|plan| {
            matches!(
                plan.shape,
                checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(candidate)
                    if candidate == primitive
            )
        })
        .map(|plan| plan.identity)
}

pub(crate) fn byte_view_element_identity<'a>(
    plans: impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan>,
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    primitive_scalar_identity(plans, typed_trees::types::PrimitiveType::U8)
}

pub(crate) fn lookup_type_id(
    ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    identity: &typed_trees::type_identity::TypeIdentityId,
) -> Result<StructuralTypeId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (candidate == identity).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural type",
        ))
}

/// The structural declaration carrying one checked type identity. Lowering
/// joins checked values to the declaration roster through the identity's
/// retained text, so `type_identity` is any checked `TypeIdentityId`;
/// `absent_message` names the lost value when no declaration carries it.
pub(crate) fn structural_declaration_by_identity<'types>(
    types: &'types [terminal_psi::StructuralTypeDeclaration],
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    type_identity: typed_trees::type_identity::TypeIdentityId,
    absent_message: &'static str,
) -> Result<&'types terminal_psi::StructuralTypeDeclaration, LoweringError> {
    terminal_psi::structural_type_by_identity(
        types,
        identities.intern(
            &checked
                .type_identity_text(type_identity)
                .unwrap_or_default(),
        ),
    )
    .ok_or(LoweringError::Unsupported(absent_message))
}

/// A semantic domain family instantiated on one exact structural carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StructuralDomainBinding {
    /// Plain byte predicate from the lowered domain catalog; never route authority.
    pub byte_predicate: Option<terminal_psi::ByteSequencePredicate>,
    pub semantic_domain: language_semantics::SemanticDomainId,
    pub carrier: StructuralTypeId,
    pub domain: StructuralDomainId,
}

/// Diagnostic identity includes the authored carrier, never its allocation ID.
/// Always qualify it so pruning another instantiation cannot rename this one.
pub(crate) fn structural_domain_identity(family: &str, carrier_identity: &str) -> String {
    terminal_psi::StructuralDomainDeclaration::carrier_qualified_identity(family, carrier_identity)
}

pub(crate) fn lookup_domain_id(
    ids: &[StructuralDomainBinding],
    source: language_semantics::SemanticDomainId,
) -> Result<StructuralDomainId, LoweringError> {
    let mut matches = ids.iter().filter(|row| row.semantic_domain == source);
    let first = matches.next().ok_or(LoweringError::Unsupported(
        "Unit closure references an unlowered structural domain",
    ))?;
    if matches.next().is_some() {
        return Err(LoweringError::Unsupported(
            "Unit closure structural domain family requires an exact carrier",
        ));
    }
    Ok(first.domain)
}

pub(crate) fn lookup_domain_id_for_carrier(
    ids: &[StructuralDomainBinding],
    source: language_semantics::SemanticDomainId,
    carrier: StructuralTypeId,
) -> Result<StructuralDomainId, LoweringError> {
    let mut matches = ids
        .iter()
        .filter(|row| row.semantic_domain == source && row.carrier == carrier);
    let first = matches
        .next()
        .ok_or(LoweringError::MissingUnitStructuralDomainCarrier {
            domain: source,
            carrier,
        })?;
    if matches.next().is_some() {
        return Err(LoweringError::Unsupported(
            "Unit closure repeats a structural domain carrier binding",
        ));
    }
    Ok(first.domain)
}

pub(crate) fn lookup_service_id(
    ids: &[(ServiceReachId, ServiceId)],
    source: ServiceReachId,
) -> Result<ServiceId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered checked service",
        ))
}

/// Lower a checked service row into the sorted deduplicated ceiling shape the
/// lowered unit records.
pub(crate) fn lower_service_reach_sorted(
    source: &[ServiceReachId],
    ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(ids, *service))
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

pub(crate) fn lookup_machine_id(
    ids: &[(symbols::SymbolHandle, MachineId)],
    source: symbols::SymbolHandle,
) -> Result<MachineId, LoweringError> {
    lookup_machine_id_with(
        ids,
        source,
        "Unit call target is absent from the lowered closure",
    )
}

pub(crate) fn lookup_machine_id_with(
    ids: &[(symbols::SymbolHandle, MachineId)],
    source: symbols::SymbolHandle,
    absent_message: &'static str,
) -> Result<MachineId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(absent_message))
}

pub(crate) fn lookup_claim_id(
    ids: &[(PermissionClaimIdentity, ClaimId)],
    source: PermissionClaimIdentity,
) -> Result<ClaimId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit claim transfer references a non-entry caller claim",
        ))
}

pub(crate) fn dense_identity(index: usize) -> Result<u64, LoweringError> {
    u64::try_from(index)
        .map_err(|_| LoweringError::Unsupported("terminal Unit identity count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "terminal Unit identity count exceeds u64",
        ))
}

pub(crate) fn allocate_dense(next: &mut u64) -> Result<u64, LoweringError> {
    let current = *next;
    *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
        "terminal Unit identity count exceeds u64",
    ))?;
    Ok(current)
}

/// A fresh unqualified scalar `ValueDeclaration` on the next dense identity —
/// the literal every payload/case emission site mints.
pub(crate) fn fresh_scalar_declaration(
    next_value: &mut u64,
    scalar_type: ScalarType,
) -> Result<terminal_psi::ValueDeclaration, LoweringError> {
    Ok(terminal_psi::ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(allocate_dense(next_value)?),
        scalar_type,
    })
}
