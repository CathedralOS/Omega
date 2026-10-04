//! Bind source field identities to the exact emitted receiver declaration.
//!
//! Selected structural payloads use the same field-path resolver as ordinary
//! records, carrying a case-leaf copy to the observation's emission point. Retain the
//! complete canonical carrier path: a leaf declaration may occur below several
//! siblings, and its identity alone cannot select the observed value. Case
//! membership and ownership remain obligations of the selecting edge and replay.
//! Primitive elements below a case use a checked carrier copy followed by the
//! ordinary primitive read. This keeps case authority at the copy operation;
//! the primitive path does not acquire an unchecked case projection grammar.

use super::super::{
    CheckedUnitStructuralPathSegment, StructuralFieldId, StructuralFieldType, StructuralTypeId,
    StructuralTypeShape,
};
use super::{
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
use semantic_vocabulary::StructuralCaseId;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct StructuralScalarFieldBinding {
    source_position: u32,
    source: PlaceId,
    structural_type: StructuralTypeId,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
    case_payloads: CasePayloadAccess,
}

/// Observation roots share one declaration catalog, including when the machine
/// has no structural parameters. A later local observation must not depend on
/// an unrelated formal supplying its type declarations.
#[derive(Clone, Default)]
pub(crate) struct StructuralFieldBindings {
    roots: Vec<StructuralScalarFieldBinding>,
    locals: Vec<(symbols::SymbolHandle, PlaceId, StructuralTypeId)>,
    established_payloads: Vec<EstablishedCasePayload>,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
}

impl std::ops::Deref for StructuralFieldBindings {
    type Target = [StructuralScalarFieldBinding];
    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}
impl std::ops::DerefMut for StructuralFieldBindings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}

impl StructuralFieldBindings {
    pub(crate) fn clear_parameters(&mut self) {
        self.roots.clear();
    }

    pub(crate) fn register_local(
        &mut self,
        symbol: symbols::SymbolHandle,
        place: PlaceId,
        structural_type: StructuralTypeId,
        declarations: &[StructuralTypeDeclaration],
    ) {
        if !self
            .declarations
            .iter()
            .any(|row| row.id == structural_type)
        {
            self.declarations = declarations.into();
        }
        self.locals.push((symbol, place, structural_type));
    }

    pub(crate) fn observation_source(
        &self,
        root: checked_trees::CheckedStorageRoot,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
        let candidates: Vec<_> = match root {
            checked_trees::CheckedStorageRoot::Parameter { index } => self
                .roots
                .iter()
                .filter(|binding| binding.source_position == index)
                .map(|binding| (binding.source, binding.structural_type))
                .collect(),
            checked_trees::CheckedStorageRoot::ViewLocal { symbol }
            | checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => self
                .locals
                .iter()
                .filter(|(candidate, _, _)| *candidate == symbol)
                .map(|(_, place, structural_type)| (*place, *structural_type))
                .collect(),
        };
        match candidates.as_slice() {
            [source] => Ok(*source),
            _ => unsupported("field observation lost its unique established root"),
        }
    }

    pub(crate) fn established_payload(
        &self,
        source: PlaceId,
        case: StructuralCaseId,
        field: StructuralFieldId,
    ) -> Result<Option<usize>, LoweringError> {
        let mut rows = self
            .established_payloads
            .iter()
            .filter(|row| row.source == source && row.case == case && row.field == field);
        let position = rows.next().map(|row| row.position);
        if rows.next().is_some() {
            return unsupported("case payload has duplicate established values");
        }
        Ok(position)
    }

    pub(crate) fn declarations(&self) -> &[StructuralTypeDeclaration] {
        &self.declarations
    }
}

/// Whether a read through a case segment may observe this root's payload.
///
/// A whole root's scalar payload is read as a parameter of the block its
/// `StructuralCase` dispatch selected for that case, so the verifier's own tag
/// dispatch is the case knowledge. A read of a root case payload therefore
/// resolves to an established payload value, is deferred for the guard
/// decision lowering that will introduce the dispatch, or is refused. A sum
/// below record fields has no dispatch terminator; `resolve_typed` reads its
/// payload in place through a carrier ending in the `Case` step instead.
/// Structural payload members carry their own observation source, independent
/// of this parameter-binding mode: a case-leaf copy is emitted at the read and
/// owes the active-case fact there. No successor scanner precomputes the read.
#[derive(Clone, Default)]
enum CasePayloadAccess {
    #[default]
    Unavailable,
    Deferred,
    Established(Vec<EstablishedCasePayload>),
}

/// One payload field bound by a dominating `StructuralCase` successor, at its
/// dense position in the scalar namespace of the code it dominates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EstablishedCasePayload {
    pub(crate) source: PlaceId,
    pub(crate) case: StructuralCaseId,
    pub(crate) field: StructuralFieldId,
    pub(crate) position: usize,
}

/// A resolved case-qualified payload read.
pub(crate) enum CasePayloadRead {
    /// A declaration-resolved read, not an active-case assertion. Scalar
    /// payloads may bind through a dispatch; structural carriers are copied
    /// at the observation point under independent active-case validation.
    Observation {
        source: crate::emission::operation_emission::expressions::PrimitiveReadSource,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    /// The dominating dispatch's payload parameter.
    Established { position: usize },
}

/// Let guard lowering keep case-qualified reads for its dispatch to bind.
pub(crate) fn defer_case_payloads(fields: &mut [StructuralScalarFieldBinding]) {
    for field in fields {
        field.case_payloads = CasePayloadAccess::Deferred;
    }
}

/// Bind reads of payloads a dominating dispatch selected on every incoming path.
pub(crate) fn establish_case_payloads(
    fields: &mut StructuralFieldBindings,
    established: &[EstablishedCasePayload],
) {
    // Locals and parameters share the selected edge's value namespace. A
    // temporary subject need not have a parameter-root binding to retain it.
    fields
        .established_payloads
        .retain(|prior| !established.iter().any(|row| row.source == prior.source));
    fields.established_payloads.extend_from_slice(established);
    for field in &mut fields.roots {
        let rows = established
            .iter()
            .filter(|row| row.source == field.source)
            .copied()
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            field.case_payloads = CasePayloadAccess::Established(rows);
        }
    }
}

impl StructuralScalarFieldBinding {
    pub(crate) fn collect(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> StructuralFieldBindings {
        // Share declarations between roots and resolve only observed paths. A
        // recursive leaf inventory would expand unused nested record contents.
        let declarations: std::sync::Arc<[StructuralTypeDeclaration]> = types.into();
        let mut bindings = Vec::new();
        for (position, parameter) in parameters {
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            ) {
                continue;
            }
            bindings.push(StructuralScalarFieldBinding {
                source_position: *position,
                source: parameter.place,
                structural_type: parameter.structural_type,
                declarations: std::sync::Arc::clone(&declarations),
                case_payloads: CasePayloadAccess::Unavailable,
            });
        }
        StructuralFieldBindings {
            roots: bindings,
            locals: Vec::new(),
            established_payloads: Vec::new(),
            declarations,
        }
    }
}

pub(crate) fn resolve(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<
    (
        PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        StructuralFieldId,
    ),
    LoweringError,
> {
    let (source, carrier_path, field, declared) = resolve_typed(fields, position, path)?;
    if declared != scalar_type {
        return unsupported("runtime field observation changes its declared scalar type");
    }
    Ok((source, carrier_path, field))
}

/// The same field-path resolution returning the leaf's declared scalar type
/// for a caller that must mint the read before it knows the leaf type — a
/// runtime-index selector spelled `self.<fields>` reads whatever the path
/// names and emits it as the operand.
pub(crate) fn resolve_typed(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    (
        PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        StructuralFieldId,
        ScalarType,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "runtime field observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("runtime field observation has ambiguous bindings");
    }
    let mut structural_type = binding.structural_type;
    let mut carrier_path = Vec::with_capacity(path.len().saturating_sub(1));
    let mut visited = Vec::new();
    for (ordinal, segment) in path.iter().enumerate() {
        if visited.contains(&structural_type) {
            return unsupported("runtime field observation has a cyclic carrier");
        }
        visited.push(structural_type);
        let mut declarations = binding
            .declarations
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "runtime field observation lost its carrier declaration",
        ))?;
        if declarations.next().is_some() {
            return unsupported("runtime field observation has ambiguous carrier declarations");
        }
        if let checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index) =
            segment
        {
            // Indexing selects the exact element carrier. Its nested fields
            // continue through declaration resolution below; the final scalar
            // field and any case authority remain separate obligations.
            let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                return unsupported("runtime field observation indexes a non-array carrier");
            };
            if *element_index >= *length {
                return unsupported("runtime field observation fixed index is out of bounds");
            }
            carrier_path.push(
                semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(*element_index),
            );
            structural_type = *element;
            continue;
        }
        if let checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity) = segment {
            // A case of a sum reached through record fields: its payload is
            // read in place as an ordinary scalar observation whose carrier
            // ends in the `Case` step. Terminal verification admits that read
            // only where dominating control flow proved this case of this
            // exact place active (`case_hops`), so a guard's membership test
            // is the read's authority and a write to the sum retires it. A
            // root case keeps its dispatch route (`resolve_case_payload`),
            // whose selected block binds the payload as parameters.
            if ordinal == 0 {
                return unsupported(
                    "runtime field observation reads a root case payload outside its dispatch",
                );
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return unsupported(
                    "runtime field observation selects a case of a non-sum carrier",
                );
            };
            let [checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity)] =
                &path[ordinal + 1..]
            else {
                return unsupported(
                    "runtime field observation requires one payload field after its case",
                );
            };
            let mut selected = cases.iter().filter(|case| case.identity == *case_identity);
            let (Some(case), None) = (selected.next(), selected.next()) else {
                return unsupported("runtime field observation lost its declared case");
            };
            let mut payload = case
                .fields
                .iter()
                .filter(|field| field.identity == *field_identity);
            let (Some(field), None) = (payload.next(), payload.next()) else {
                return unsupported("runtime field observation lost its declared payload field");
            };
            if field.relevance.is_erased() {
                return unsupported("runtime field observation has an erased or ambiguous field");
            }
            let declared = match field.field_type {
                StructuralFieldType::Scalar(scalar) | StructuralFieldType::Atomic(scalar) => scalar,
                StructuralFieldType::BoundedInteger(integer) => {
                    ScalarType::Integer(integer.integer_type())
                }
                StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                _ => return unsupported("runtime field observation requires a scalar leaf"),
            };
            carrier_path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                case.id,
            ));
            return Ok((binding.source, carrier_path, field.id, declared));
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            unreachable!("fixed indices and cases return above");
        };
        // A mixed shape's common fields sit directly on the carrier beside the
        // tag, addressed exactly like a record's; its payload fields need the
        // `Case` step above.
        let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
            &declaration.shape
        else {
            return unsupported("runtime field observation requires a record carrier");
        };
        let mut selected = fields.iter().filter(|field| field.identity == *identity);
        let field = selected.next().ok_or(LoweringError::Unsupported(
            "runtime field observation lost its declared field",
        ))?;
        if selected.next().is_some()
            || field.relevance.is_erased()
            || fields
                .iter()
                .filter(|candidate| candidate.id == field.id)
                .count()
                != 1
        {
            return unsupported("runtime field observation has an erased or ambiguous field");
        }
        if ordinal + 1 == path.len() {
            let declared = match field.field_type {
                StructuralFieldType::Scalar(scalar) | StructuralFieldType::Atomic(scalar) => scalar,
                StructuralFieldType::BoundedInteger(integer) => {
                    ScalarType::Integer(integer.integer_type())
                }
                StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                _ => return unsupported("runtime field observation requires a scalar leaf"),
            };
            return Ok((binding.source, carrier_path, field.id, declared));
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return unsupported("runtime field observation requires a structural carrier field");
        };
        carrier_path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            field.id,
        ));
        structural_type = child;
    }
    unsupported("runtime field observation requires a nonempty field path")
}

/// Resolve a scalar read below one sum root's selected payload. The selected case
/// must belong to the root's exact declared sum and the leaf must be a
/// relevant scalar leaf of the requested carrier. Scalar payloads retain their
/// dispatch binding; structural payload members retain a checked copy source.
/// Resolving a declaration is not evidence that its case is active.
pub(crate) fn resolve_case_payload(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<CasePayloadRead, LoweringError> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "case payload observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("case payload observation has ambiguous bindings");
    }
    let Some((checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity), tail)) =
        path.split_first()
    else {
        return unsupported("case payload observation requires one case and one payload field");
    };
    let mut declarations = binding
        .declarations
        .iter()
        .filter(|declaration| declaration.id == binding.structural_type);
    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
        "case payload observation lost its sum declaration",
    ))?;
    if declarations.next().is_some() {
        return unsupported("case payload observation has ambiguous sum declarations");
    }
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return unsupported("case payload observation requires a sum root");
    };
    let mut selected = cases.iter().filter(|case| case.identity == *case_identity);
    let case = selected.next().ok_or(LoweringError::Unsupported(
        "case payload observation lost its declared case",
    ))?;
    if selected.next().is_some() {
        return unsupported("case payload observation has an erased or ambiguous payload");
    }
    let field = match tail {
        [checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity)] => {
            let mut payload = case
                .fields
                .iter()
                .filter(|field| field.identity == *field_identity);
            let field = payload.next().ok_or(LoweringError::Unsupported(
                "case payload observation lost its declared payload field",
            ))?;
            if payload.next().is_some() || field.relevance.is_erased() {
                return unsupported("case payload observation has an erased or ambiguous payload");
            }
            let declared = match field.field_type {
                StructuralFieldType::Scalar(scalar) => scalar,
                StructuralFieldType::BoundedInteger(integer) => {
                    ScalarType::Integer(integer.integer_type())
                }
                StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                _ => return unsupported("case payload observation requires a scalar payload"),
            };
            if declared != scalar_type {
                return unsupported("case payload observation changes its declared scalar type");
            }
            field.id
        }
        [
            checked_trees::CheckedStructuralPredicatePathSegment::Field(_),
            projection @ ..,
        ] if !projection.is_empty() => {
            let payload = plan_case_payload_leaf(fields, position, path).ok_or(
                LoweringError::Unsupported(
                    "case payload observation has no exact scalar projection",
                ),
            )?;
            if payload.scalar_type != scalar_type {
                return unsupported("case payload observation changes its declared scalar type");
            }
            // The observation owns preparation, not the successor argument
            // scanner. Its carrier copy is emitted only where this expression
            // executes, and Terminal replay checks the selected case there.
            return Ok(CasePayloadRead::Observation {
                source: crate::emission::operation_emission::expressions::PrimitiveReadSource::CaseLeaf {
                    source: payload.source,
                    path: vec![
                        semantic_vocabulary::CanonicalStructuralPathSegment::Case(payload.case),
                        semantic_vocabulary::CanonicalStructuralPathSegment::Field(payload.member),
                    ],
                    structural_type: payload.member_type,
                },
                path: payload.path,
                field: payload.field,
            });
        }
        _ => {
            return unsupported("case payload observation requires one case and one payload field");
        }
    };
    match &binding.case_payloads {
        CasePayloadAccess::Deferred => Ok(CasePayloadRead::Observation {
            source: binding.source.into(),
            path: vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                case.id,
            )],
            field,
        }),
        CasePayloadAccess::Established(rows) => rows
            .iter()
            .find(|row| row.case == case.id && row.field == field)
            .map(|row| CasePayloadRead::Established {
                position: row.position,
            })
            .ok_or(LoweringError::Unsupported(
                "case payload observation requires an established case",
            )),
        CasePayloadAccess::Unavailable => {
            unsupported("case payload observation requires an established case")
        }
    }
}

/// One planned leaf read below a case payload's record member: `member` is
/// copied where the selecting case edge dominates and `field` is read as an
/// ordinary scalar leaf of the copy. The shared operation emitter allocates
/// both at the expression's actual execution point.
#[derive(Debug, Clone)]
pub(crate) struct PlannedCasePayloadLeaf {
    pub(crate) source: PlaceId,
    pub(crate) case: StructuralCaseId,
    pub(crate) member: StructuralFieldId,
    pub(crate) member_type: StructuralTypeId,
    pub(crate) path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    pub(crate) field: StructuralFieldId,
    pub(crate) scalar_type: ScalarType,
}

/// Resolve the selected payload member, then use ordinary field-path resolution
/// below it. The copy is still established only on the selecting edge. Keeping
/// the full canonical carrier path makes arbitrary record nesting compositional
/// and distinguishes sibling occurrences of the same declared leaf field.
pub(crate) fn plan_case_payload_leaf(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Option<PlannedCasePayloadLeaf> {
    let [
        checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity),
        checked_trees::CheckedStructuralPredicatePathSegment::Field(member_identity),
        projection @ ..,
    ] = path
    else {
        return None;
    };
    if projection.is_empty() {
        return None;
    }
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let mut declarations = binding
        .declarations
        .iter()
        .filter(|declaration| declaration.id == binding.structural_type);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return None;
    };
    let mut selected = cases.iter().filter(|case| case.identity == *case_identity);
    let case = selected.next()?;
    if selected.next().is_some() {
        return None;
    }
    let mut members = case
        .fields
        .iter()
        .filter(|field| field.identity == *member_identity);
    let member = members.next()?;
    if members.next().is_some() || member.relevance.is_erased() {
        return None;
    }
    let StructuralFieldType::Structural(member_type) = member.field_type else {
        return None;
    };
    let projected = StructuralScalarFieldBinding {
        source_position: position,
        source: binding.source,
        structural_type: member_type,
        declarations: std::sync::Arc::clone(&binding.declarations),
        case_payloads: CasePayloadAccess::Unavailable,
    };
    let (_, path, field, scalar_type) =
        resolve_typed(std::slice::from_ref(&projected), position, projection).ok()?;
    Some(PlannedCasePayloadLeaf {
        source: binding.source,
        case: case.id,
        member: member.id,
        member_type,
        path,
        field,
        scalar_type,
    })
}

pub(crate) fn resolve_byte_projection(
    source: PlaceId,
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    Option<(
        PlaceId,
        Vec<crate::emission::runtime_elements::ProjectionStep>,
        StructuralFieldId,
    )>,
    LoweringError,
> {
    let Some((checked_trees::CheckedStructuralPredicatePathSegment::Field(identity), carrier)) =
        path.split_last()
    else {
        return unsupported("byte field length requires an exact field endpoint");
    };
    let carrier = carrier
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(index) => Ok(
                CheckedUnitStructuralPathSegment::RuntimeIndex(index.clone()),
            ),
            _ => unsupported("byte field length has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_steps(
        structural_type,
        &carrier,
        identity,
        declarations,
    )?;
    // Raw fixed arrays have static capacity, not live metadata; a borrowed
    // view leaf carries its extent on the view descriptor instead. The
    // element view behind a shared-borrow slice field keeps its element count
    // on the same descriptor slot a byte carrier keeps its byte count.
    let leaf_is_view = match field.field_type {
        StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
        )
        | StructuralFieldType::PredicateByteSequence {
            carrier:
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                | terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            ..
        } => true,
        StructuralFieldType::Structural(view) => declarations.iter().any(|declaration| {
            declaration.id == view
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        }),
        _ => false,
    };
    if !leaf_is_view {
        return Ok(None);
    }
    Ok(Some((source, path, field.id)))
}

/// Resolve an indexed read that lands on a fixed-array record field: the path
/// reaches the array itself and carries its declared element scalar. A leaf
/// that is not a fixed array returns `None`, so byte-sequence carriers keep
/// `resolve_byte_length`'s read family.
pub(crate) fn resolve_indexed_array(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    Option<(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        ScalarType,
    )>,
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "indexed field read has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("indexed field read has ambiguous bindings");
    }
    let carrier = path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("indexed field read has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::emission::primitive_store::lower_indexed_path(
        binding.structural_type,
        &carrier,
        &binding.declarations,
    )
    .map(|(path, scalar_type)| Some((binding.source, path, scalar_type)))
    .or(Ok(None))
}

/// Resolve a read that selects inside nested fixed arrays (`self.a[i][k]`):
/// `path` reaches the outer array and `inner_depth` further runtime selectors
/// descend through its array elements to one primitive leaf. Returns the
/// read's source, the static prefix and the leaf scalar type.
pub(crate) fn resolve_nested_indexed_array(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    inner_depth: usize,
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        ScalarType,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "nested indexed read has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("nested indexed read has ambiguous bindings");
    }
    let carrier = path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("nested indexed read has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, scalar_type) = crate::emission::primitive_store::lower_nested_indexed_path(
        binding.structural_type,
        &carrier,
        inner_depth,
        &binding.declarations,
    )?;
    Ok((binding.source, path, scalar_type))
}

/// Resolve a scalar field of a runtime-selected record element
/// (`cells[i].hp`): `path` reaches the fixed array and `element_path` walks
/// the selected element to one relevant integer or Boolean field. Returns the
/// read's source, the array prefix, the leaf route — a flat element path or
/// the hoist a bounded leaf needs — and the field's scalar type.
pub(crate) fn resolve_indexed_element_field(
    fields: &StructuralFieldBindings,
    root: checked_trees::CheckedStorageRoot,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        crate::emission::primitive_store::IndexedElementLeaf,
        ScalarType,
    ),
    LoweringError,
> {
    let (source, structural_type) = fields.observation_source(root)?;
    let static_segments = |path: &[checked_trees::CheckedStructuralPredicatePathSegment]| {
        path.iter()
            .map(|segment| match segment {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                    Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
                }
                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                    Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
                }
                _ => unsupported("indexed element field read has an unsupported case path"),
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let (path, leaf, scalar_type) =
        crate::emission::primitive_store::lower_indexed_element_field_path(
            structural_type,
            &static_segments(path)?,
            &static_segments(element_path)?,
            fields.declarations(),
        )?;
    Ok((source, path, leaf, scalar_type))
}

/// A member element read whose element path carries a runtime selector —
/// `cells[i].data[j]` spells `data`'s element through the `Field` step's own
/// array. The `path` resolves exactly like `resolve_indexed_element_field`;
/// the element path's checked segments keep their `RuntimeIndex` payloads
/// for the caller to lower beside the read's other selectors.
pub(crate) fn resolve_indexed_element_runtime_path(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        crate::emission::primitive_store::IndexedElementLeaf,
        ScalarType,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "indexed element runtime path read has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("indexed element runtime path read has ambiguous bindings");
    }
    let static_segments = |path: &[checked_trees::CheckedStructuralPredicatePathSegment]| {
        path.iter()
            .map(|segment| match segment {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                    Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
                }
                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                    Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
                }
                _ => unsupported("indexed element runtime read has an unsupported case path"),
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let element_path = element_path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(index) => Ok(
                CheckedUnitStructuralPathSegment::RuntimeIndex(index.clone()),
            ),
            _ => unsupported("indexed element runtime path has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, leaf, scalar_type) =
        crate::emission::primitive_store::lower_indexed_element_runtime_path(
            binding.structural_type,
            &static_segments(path)?,
            &element_path,
            &binding.declarations,
        )?;
    Ok((binding.source, path, leaf, scalar_type))
}

pub(crate) fn resolve_primitive(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<
    (
        crate::emission::operation_emission::expressions::PrimitiveReadSource,
        Vec<terminal_psi::StructuralPathSegment>,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "primitive observation has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive observation has ambiguous bindings");
    }
    if path.iter().any(|segment| {
        matches!(
            segment,
            checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
        )
    }) {
        return resolve_case_primitive(binding, path, scalar_type);
    }
    let path = path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("primitive observation has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, declared_type) = crate::emission::primitive_store::lower_path(
        binding.structural_type,
        &path,
        &binding.declarations,
    )?;
    if declared_type != scalar_type {
        return unsupported("primitive observation changes its declared scalar type");
    }
    Ok((binding.source.into(), path))
}

/// Resolve each declared projection independently; no source arrangement grants
/// case membership. The emitted case-leaf copy still owes a dominating fact for
/// every case crossed, checked again by Terminal verification.
fn resolve_case_primitive(
    binding: &StructuralScalarFieldBinding,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<
    (
        crate::emission::operation_emission::expressions::PrimitiveReadSource,
        Vec<terminal_psi::StructuralPathSegment>,
    ),
    LoweringError,
> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Segment;
    use semantic_vocabulary::CanonicalStructuralPathSegment as Canonical;
    let Some((Segment::FixedIndex(index), carrier)) = path.split_last() else {
        return unsupported("case primitive observation requires an indexed primitive carrier");
    };
    let mut structural_type = binding.structural_type;
    let mut canonical = Vec::with_capacity(path.len());
    let mut case_fields = None;
    for segment in carrier {
        let mut matching = binding
            .declarations
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let (Some(declaration), None) = (matching.next(), matching.next()) else {
            return unsupported("primitive observation requires a unique carrier declaration");
        };
        match segment {
            Segment::Case(identity) => {
                if case_fields.is_some() {
                    return unsupported("primitive observation requires a field after its case");
                }
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return unsupported("primitive observation selects a case of a non-sum");
                };
                let mut matching = cases.iter().filter(|case| case.identity == *identity);
                let (Some(case), None) = (matching.next(), matching.next()) else {
                    return unsupported("primitive observation requires a unique case");
                };
                canonical.push(Canonical::Case(case.id));
                case_fields = Some(case.fields.as_slice());
            }
            Segment::Field(identity) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => match &declaration.shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                        _ => {
                            return unsupported(
                                "primitive observation field has no declared carrier",
                            );
                        }
                    },
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let (Some(field), None) = (matching.next(), matching.next()) else {
                    return unsupported("primitive observation requires a unique field");
                };
                if field.relevance.is_erased() {
                    return unsupported("primitive observation cannot read an erased field");
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return unsupported("primitive observation requires a structural leaf carrier");
                };
                canonical.push(Canonical::Field(field.id));
                structural_type = child;
            }
            Segment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                    return unsupported("primitive observation indexes a non-array");
                };
                if case_fields.is_some() || index >= length {
                    return unsupported("primitive observation index lacks its array or bound");
                }
                canonical.push(Canonical::FixedIndex(*index));
                structural_type = *element;
            }
            // A runtime element selector carries no canonical segment: the
            // case-primitive observation is contract vocabulary.
            Segment::RuntimeIndex(_) => {
                return unsupported("primitive observation selects no runtime element");
            }
        }
    }
    // Canonical lookup also rejects duplicate declaration-local identities;
    // source spellings above select identities but cannot certify them.
    if terminal_semantics::canonical_case_leaf_type(
        binding.declarations.iter(),
        binding.structural_type,
        &canonical,
    ) != Some(structural_type)
    {
        return unsupported("primitive observation lost its canonical carrier");
    }
    let (path, declared) = crate::emission::primitive_store::lower_path(
        structural_type,
        &[CheckedUnitStructuralPathSegment::FixedIndex(*index)],
        &binding.declarations,
    )?;
    if declared != scalar_type {
        return unsupported("primitive observation changes its declared scalar type");
    }
    Ok((
        crate::emission::operation_emission::expressions::PrimitiveReadSource::CaseLeaf {
            source: binding.source,
            path: canonical,
            structural_type,
        },
        path,
    ))
}
