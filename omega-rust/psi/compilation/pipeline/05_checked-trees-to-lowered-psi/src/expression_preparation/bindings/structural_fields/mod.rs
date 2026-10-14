//! Bind source field identities to the exact emitted receiver declaration.
//!
//! Selected structural payloads use the same field-path resolver as ordinary
//! records, carrying a case-leaf copy to the observation's emission point. Retain the
//! complete canonical carrier path: a leaf declaration may occur below several
//! siblings, and its identity alone cannot select the observed value. Case
//! membership and ownership remain obligations of the selecting edge and replay.
//! Static payload snapshots retain their checked carrier-copy operation.
//! Runtime element reads retain case selection on the original storage path;
//! observing a scalar must not duplicate the collection's affine siblings.
//! One declaration walk retains the scalar endpoint and both logical and physical
//! projection meaning. Inline fields keep canonical proof identity; references
//! and descriptors emit memory reads with explicit crossings and live bounds.
//! The endpoint being a field rather than a primitive element never selects a
//! different collection grammar or authorizes a copied backing value.

use super::super::{
    CheckedUnitStructuralPathSegment, StructuralFieldId, StructuralFieldType, StructuralTypeId,
    StructuralTypeShape,
};
use super::{
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::expressions::LoweredElementPathSegment;
use semantic_vocabulary::StructuralCaseId;

mod case_observation;
mod observation_paths;
pub(crate) use case_observation::prepare as prepare_case_observation;

#[derive(Clone)]
pub(crate) struct StructuralScalarFieldBinding {
    source_position: u32,
    source: PlaceId,
    access: StructuralAccess,
    structural_type: StructuralTypeId,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
    /// Payload values a dominating `StructuralCase` dispatch bound for this root.
    ///
    /// Case authority is decided before lowering: checking proved the selected
    /// case for every payload access it admitted, whether from a dominating
    /// membership test, an entry `requires root in Sum::Case`, or a callee's
    /// case guarantee. Lowering therefore chooses only where the value comes
    /// from. A read below a dispatch that bound the field uses that dispatch's
    /// parameter; every other admitted read observes the payload in place
    /// through its canonical `Case` carrier, as a sum below record fields and a
    /// structural payload member already do. Guard decision lowering rewrites
    /// in-place reads below the dispatch it plans into that dispatch's
    /// parameters. Lowering never re-decides whether the case is active:
    /// refusing an unbound read would duplicate the checker's judgement and
    /// reject admitted programs whose case knowledge comes from a contract.
    case_payloads: Vec<EstablishedCasePayload>,
}

/// Observation roots share one declaration catalog, including when the machine
/// has no structural parameters. A later local observation must not depend on
/// an unrelated formal supplying its type declarations.
#[derive(Clone, Default)]
pub(crate) struct StructuralFieldBindings {
    roots: Vec<StructuralScalarFieldBinding>,
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

    /// Adopt the full declaration catalog when an evaluation built without
    /// structural parameters establishes its first structural local.
    pub(crate) fn adopt_declarations(
        &mut self,
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
    }

    /// All observations retain the established home, including a projected
    /// alias prefix. The endpoint declaration describes that prefix, never the
    /// physical root. Moves and rebindings update this same local roster.
    pub(crate) fn observation_home(
        &self,
        root: checked_trees::CheckedStorageRoot,
        structural_locals: &[super::StructuralLocalBinding],
        view_locals: &[super::view_locals::ViewLocalBinding],
    ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError> {
        let candidates: Vec<_> = match root {
            checked_trees::CheckedStorageRoot::Parameter { index } => self
                .roots
                .iter()
                .filter(|binding| binding.source_position == index)
                .map(|binding| {
                    (
                        terminal_psi::StructuralArgument {
                            place: binding.source,
                            path: Vec::new(),
                            access: binding.access,
                        },
                        binding.structural_type,
                    )
                })
                .collect(),
            checked_trees::CheckedStorageRoot::ViewLocal { symbol }
            | checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                let owned = structural_locals
                    .iter()
                    .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol))
                    .map(|local| (local.argument.clone(), local.structural_type))
                    .collect::<Vec<_>>();
                if owned.is_empty()
                    && matches!(root, checked_trees::CheckedStorageRoot::ViewLocal { .. })
                {
                    view_locals
                        .iter()
                        .filter(|local| local.symbol == symbol)
                        .map(|local| {
                            (
                                terminal_psi::StructuralArgument {
                                    place: local.place,
                                    path: Vec::new(),
                                    access: local.access,
                                },
                                local.structural_type,
                            )
                        })
                        .collect()
                } else {
                    owned
                }
            }
        };
        match candidates.as_slice() {
            [home] => Ok(home.clone()),
            _ => unsupported("field observation lost its unique established root"),
        }
    }

    pub(crate) fn established_payload(
        &self,
        source: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        case: StructuralCaseId,
        field: StructuralFieldId,
    ) -> Result<Option<usize>, LoweringError> {
        let mut rows = self.established_payloads.iter().filter(|row| {
            row.source == source && row.path == path && row.case == case && row.field == field
        });
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

/// One payload field bound by a dominating `StructuralCase` successor, at its
/// dense position in the scalar namespace of the code it dominates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EstablishedCasePayload {
    pub(crate) source: PlaceId,
    // This is an observation identity, not an owned-place proposition. Retain
    // pointer crossings so two reference leaves cannot share a snapshot key.
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) case: StructuralCaseId,
    pub(crate) field: StructuralFieldId,
    pub(crate) position: usize,
}

/// A resolved case-qualified payload read.
pub(crate) enum CasePayloadRead {
    /// An in-place read whose active case checking already proved. Guard
    /// lowering may still bind a scalar read through the dispatch it plans;
    /// structural carriers are copied at the observation point.
    Observation { expression: LoweredDirectExpression },
    /// The dominating dispatch's payload parameter.
    Established { position: usize },
}

/// Bind reads of payloads a dominating dispatch selected on every incoming path.
pub(crate) fn establish_case_payloads(
    fields: &mut StructuralFieldBindings,
    established: &[EstablishedCasePayload],
) {
    // Locals and parameters share the selected edge's value namespace. A
    // temporary subject need not have a parameter-root binding to retain it.
    fields.established_payloads.retain(|prior| {
        !established
            .iter()
            .any(|row| row.source == prior.source && row.path == prior.path)
    });
    fields.established_payloads.extend_from_slice(established);
    for field in &mut fields.roots {
        let rows = fields
            .established_payloads
            .iter()
            .filter(|row| row.source == field.source)
            .cloned()
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            field.case_payloads = rows;
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
                StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
            ) {
                continue;
            }
            bindings.push(StructuralScalarFieldBinding {
                source_position: *position,
                source: parameter.place,
                access: parameter.access,
                structural_type: parameter.structural_type,
                declarations: std::sync::Arc::clone(&declarations),
                case_payloads: Vec::new(),
            });
        }
        StructuralFieldBindings {
            roots: bindings,
            established_payloads: Vec::new(),
            declarations,
        }
    }
}

pub(crate) fn resolve(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let observation = resolve_typed(identities, fields, position, path)?;
    if observation.scalar_type() != scalar_type {
        return unsupported("runtime field observation changes its declared scalar type");
    }
    Ok(observation)
}

/// Resolve the declared read operation and scalar type together. Inline fields
/// keep canonical field identity for proof terms; stored references keep their
/// pointer crossings in a primitive read's memory path. Runtime selectors use
/// this same observation rather than reconstructing a field-only operation.
pub(crate) fn resolve_typed(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<LoweredDirectExpression, LoweringError> {
    use super::structural_paths::PreparedStructuralPathSegment as Prepared;

    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "runtime field observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("runtime field observation has ambiguous bindings");
    }
    // The projection walk owns endpoint typing as well as pointer and collection
    // geometry. A second field-only walk rejected ordinary record leaves below
    // descriptors and disagreed with primitive-element reads about live bounds.
    let projection = observation_paths::explicit_path(
        identities,
        binding.structural_type,
        path,
        &binding.declarations,
    )?;
    let scalar_type = projection.scalar_type().ok_or(LoweringError::Unsupported(
        "runtime observation requires a declared scalar endpoint",
    ))?;
    if projection.scalar_field.is_none()
        && path.iter().any(|segment| {
            matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
            )
        })
    {
        let (source, path) = resolve_case_primitive(identities, binding, projection, scalar_type)?;
        return Ok(LoweredDirectExpression::PrimitiveRead {
            source,
            path,
            scalar_type,
        });
    }
    let prepared = super::structural_paths::prepare_argument_path(
        identities,
        &projection.segments,
        binding.structural_type,
        &binding.declarations,
    )?;
    if prepared
        .iter()
        .any(|segment| matches!(segment, Prepared::ViewExtent { .. }))
    {
        return descriptor_observation(binding, &prepared, scalar_type);
    }
    let physical_path = prepared
        .into_iter()
        .map(|step| match step {
            Prepared::Static(step) => Ok(step),
            _ => unsupported("static observation requires its captured selector"),
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    if let Some(field) = projection.scalar_field {
        // A dominating selected edge may already own this scalar snapshot.
        // Geometry alone never manufactures payload authority.
        if let Some((terminal_psi::StructuralPathSegment::Field(_), carrier)) =
            physical_path.split_last()
            && let Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) =
                carrier.split_last()
            && let Some(row) = binding
                .case_payloads
                .iter()
                .find(|row| row.path == prefix && row.case == *case && row.field == field.id)
        {
            return Ok(LoweredDirectExpression::Parameter {
                position: row.position,
                scalar_type,
            });
        }
        if let Some(mut canonical) = projection.canonical {
            canonical.pop();
            return Ok(canonical_field_read(
                binding.source.into(),
                canonical,
                field.id,
                scalar_type,
            ));
        }
    }
    Ok(LoweredDirectExpression::PrimitiveRead {
        source: binding.source.into(),
        path: physical_path,
        scalar_type,
    })
}

/// Descriptor indexing is a memory operation, even when its selector is a
/// literal and its endpoint is a field. Retain every descriptor's own count
/// before its selector; nested fields and fixed arrays then continue through
/// the same prepared path. No descriptor acquires a fabricated static extent.
fn descriptor_observation(
    binding: &StructuralScalarFieldBinding,
    prepared: &[super::structural_paths::PreparedStructuralPathSegment],
    scalar_type: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    use super::structural_paths::PreparedStructuralPathSegment as Prepared;
    use crate::emission::operation_emission::expressions::LoweredElementPathSegment as Segment;

    let count_type = crate::emission::scalar_types::terminal_scalar_type(
        typed_trees::types::PrimitiveType::U64,
    )?;
    let mut path = Vec::with_capacity(prepared.len());
    let mut extent = None;
    for segment in prepared {
        match segment {
            Prepared::ViewExtent { bytes } if extent.is_none() => {
                extent = Some(if *bytes {
                    LoweredDirectExpression::ByteSequenceLength {
                        source: binding.source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                        scalar_type: count_type,
                    }
                } else {
                    LoweredDirectExpression::ElementViewLength {
                        source: binding.source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                        scalar_type: count_type,
                    }
                });
            }
            Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(index))
                if extent.is_some() =>
            {
                path.push(Segment::RuntimeIndex {
                    selector: LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                        scalar_type: count_type,
                    },
                    extent: extent.take(),
                    bound_decided: false,
                });
            }
            Prepared::Static(segment) if extent.is_none() => {
                path.push(Segment::Static(segment.clone()));
            }
            _ => return unsupported("descriptor observation requires its retained selector"),
        }
    }
    if extent.is_some() {
        return unsupported("descriptor observation lost its element selector");
    }
    let selectors = path
        .iter()
        .filter(|segment| matches!(segment, Segment::RuntimeIndex { .. }))
        .count();
    if selectors > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
    {
        return unsupported("descriptor observation exceeds the runtime selector namespace");
    }
    let first = path
        .iter()
        .position(|segment| matches!(segment, Segment::RuntimeIndex { .. }))
        .ok_or(LoweringError::Unsupported(
            "descriptor observation has no element selector",
        ))?;
    let element_path = path.split_off(first + 1);
    let Some(Segment::RuntimeIndex {
        selector,
        extent,
        bound_decided,
    }) = path.pop()
    else {
        return unsupported("descriptor observation lost its prepared selector");
    };
    let prefix = path
        .into_iter()
        .map(|segment| match segment {
            Segment::Static(segment) => Ok(segment),
            _ => unsupported("descriptor observation prefix contains an unprepared selector"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LoweredDirectExpression::IndexedPrimitiveRead {
        source: binding.source,
        path: prefix,
        index: Box::new(selector),
        extent: extent.map(Box::new),
        element_path,
        scalar_type,
        bound_decided,
    })
}

fn canonical_field_read(
    source: crate::emission::operation_emission::expressions::PrimitiveReadSource,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    field: StructuralFieldId,
    scalar_type: ScalarType,
) -> LoweredDirectExpression {
    match scalar_type {
        ScalarType::Boolean => LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                source,
                path,
                field,
            }),
        },
        _ => LoweredDirectExpression::StructuralField {
            source,
            path,
            field,
            scalar_type,
        },
    }
}

/// Preserve the Boolean AST's field and primitive-read distinctions: its
/// field node emits BooleanStructuralField and carries canonical proof identity.
/// Any other Boolean read (a representation view, for example) is a scalar
/// observation of its own carrier.
pub(crate) fn boolean_observation(
    observation: LoweredDirectExpression,
) -> LoweredBooleanReturnExpression {
    match observation {
        LoweredDirectExpression::Boolean { expression } => *expression,
        LoweredDirectExpression::PrimitiveRead {
            source,
            path,
            scalar_type: ScalarType::Boolean,
        } => LoweredBooleanReturnExpression::PrimitiveRead { source, path },
        operand => LoweredBooleanReturnExpression::ScalarObservation {
            operand: Box::new(operand),
        },
    }
}

/// Resolve a scalar read below one sum root's selected payload. The selected case
/// must belong to the root's exact declared sum and the leaf must be a
/// relevant scalar leaf of the requested carrier. A scalar payload a dominating
/// dispatch bound reads that binding; every other admitted read observes the
/// payload in place, and structural payload members retain a checked copy
/// source. Checking proved the active case before this resolution runs.
pub(crate) fn resolve_case_payload(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
    let mut selected = cases
        .iter()
        .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
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
                .filter(|field| field.identity == identities.intern(&field_identity.spelling()));
            let field = payload.next().ok_or(LoweringError::Unsupported(
                "case payload observation lost its declared payload field",
            ))?;
            if payload.next().is_some() || field.relevance.is_erased() {
                return unsupported("case payload observation has an erased or ambiguous payload");
            }
            let declared = field
                .field_type
                .scalar_type()
                .ok_or(LoweringError::Unsupported(
                    "case payload observation requires a scalar payload",
                ))?;
            if declared != scalar_type {
                return unsupported("case payload observation changes its declared scalar type");
            }
            field.id
        }
        [
            checked_trees::CheckedStructuralPredicatePathSegment::Field(_),
            projection @ ..,
        ] if !projection.is_empty() => {
            let mut payload = plan_case_payload_leaf(identities, fields, position, path).ok_or(
                LoweringError::Unsupported(
                    "case payload observation has no exact scalar projection",
                ),
            )?;
            if payload.observation.scalar_type() != scalar_type {
                return unsupported("case payload observation changes its declared scalar type");
            }
            // The observation owns preparation, not the successor argument
            // scanner. Its carrier copy is emitted only where this expression
            // executes, and Terminal replay checks the selected case there.
            let source = prepare_case_observation(
                payload.source,
                binding.structural_type,
                &[
                    semantic_vocabulary::CanonicalStructuralPathSegment::Case(payload.case),
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(payload.member),
                ],
                &binding.declarations,
            )?
            .source;
            match &mut payload.observation {
                LoweredDirectExpression::PrimitiveRead { source: target, .. }
                | LoweredDirectExpression::StructuralField { source: target, .. } => {
                    *target = source
                }
                LoweredDirectExpression::Boolean { expression } => match expression.as_mut() {
                    LoweredBooleanReturnExpression::StructuralField { source: target, .. }
                    | LoweredBooleanReturnExpression::PrimitiveRead { source: target, .. } => {
                        *target = source
                    }
                    _ => {
                        return unsupported(
                            "case payload did not resolve to a Boolean observation",
                        );
                    }
                },
                _ => return unsupported("case payload did not resolve to a field observation"),
            }
            return Ok(CasePayloadRead::Observation {
                expression: payload.observation,
            });
        }
        _ => {
            return unsupported("case payload observation requires one case and one payload field");
        }
    };
    if let Some(row) = binding
        .case_payloads
        .iter()
        .find(|row| row.path.is_empty() && row.case == case.id && row.field == field)
    {
        return Ok(CasePayloadRead::Established {
            position: row.position,
        });
    }
    Ok(CasePayloadRead::Observation {
        expression: canonical_field_read(
            binding.source.into(),
            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                case.id,
            )],
            field,
            scalar_type,
        ),
    })
}

/// One planned leaf read below a case payload's record member: `member` is
/// copied where the selecting case edge dominates and the observation reads
/// its scalar leaf. The shared operation emitter allocates
/// both at the expression's actual execution point.
#[derive(Debug, Clone)]
pub(crate) struct PlannedCasePayloadLeaf {
    pub(crate) source: PlaceId,
    pub(crate) case: StructuralCaseId,
    pub(crate) member: StructuralFieldId,
    pub(crate) observation: LoweredDirectExpression,
}

/// Resolve the selected payload member, then use ordinary field-path resolution
/// below it. The copy is still established only on the selecting edge. Keeping
/// the full canonical carrier path makes arbitrary record nesting compositional
/// and distinguishes sibling occurrences of the same declared leaf field.
pub(crate) fn plan_case_payload_leaf(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
    let cases = declaration.shape.sum_cases()?;
    let mut selected = cases
        .iter()
        .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
    let case = selected.next()?;
    if selected.next().is_some() {
        return None;
    }
    let mut members = case
        .fields
        .iter()
        .filter(|field| field.identity == identities.intern(&member_identity.spelling()));
    let member = members.next()?;
    if members.next().is_some() || member.relevance.is_erased() {
        return None;
    }
    let member_type = member.field_type.as_structural()?;
    let projected = StructuralScalarFieldBinding {
        source_position: position,
        source: binding.source,
        access: binding.access,
        structural_type: member_type,
        declarations: std::sync::Arc::clone(&binding.declarations),
        case_payloads: Vec::new(),
    };
    let observation = resolve_typed(
        identities,
        std::slice::from_ref(&projected),
        position,
        projection,
    )
    .ok()?;
    Some(PlannedCasePayloadLeaf {
        source: binding.source,
        case: case.id,
        member: member.id,
        observation,
    })
}

/// Select the sequence at the completed observation path. The same declared
/// walk used for scalar reads supplies stored-reference crossings and per-prefix
/// view extents; a field-only resolver would lose nested descriptors.
/// A valid non-descriptor endpoint returns None: indexed byte reads also use
/// this query to distinguish inline arrays from live descriptors. Invalid paths
/// and unreadable crossings remain errors, not a different lowering route.
pub(crate) fn prepare_sequence_observation(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    Option<(
        Vec<super::structural_paths::PreparedStructuralPathSegment>,
        Option<StructuralFieldId>,
        super::view_locals::ViewCarrier,
    )>,
    LoweringError,
> {
    let projection =
        observation_paths::explicit_path(identities, structural_type, path, declarations)?;
    let (field, carrier) = if let Some(field) = projection.scalar_field {
        if !matches!(
            &field.field_type,
            StructuralFieldType::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            ) | StructuralFieldType::PredicateByteSequence {
                carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    | terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                ..
            }
        ) {
            return Ok(None);
        }
        (Some(field), super::view_locals::ViewCarrier::Bytes)
    } else {
        let declaration = projection.declaration.ok_or(LoweringError::Unsupported(
            "length observation lost its sequence declaration",
        ))?;
        let Some(carrier) = super::view_locals::ViewCarrier::of(declaration.id, declarations)
        else {
            return Ok(None);
        };
        (None, carrier)
    };
    let mut steps = super::structural_paths::prepare_argument_path(
        identities,
        &projection.segments,
        structural_type,
        declarations,
    )?;
    // The complete path is prepared first, so a case payload field validates
    // against its selected case. The field endpoint is then carried apart from
    // the container path, which may end at that case.
    if let Some(field) = field
        && steps.pop()
            != Some(
                super::structural_paths::PreparedStructuralPathSegment::Static(
                    terminal_psi::StructuralPathSegment::Field(field.identity),
                ),
            )
    {
        return unsupported("byte length observation lost its field endpoint");
    }
    Ok(Some((steps, field.map(|field| field.id), carrier)))
}

/// Resolve a readable sequence through the same declared reference crossings
/// used by field and indexed observations. Preserve the completed actual prefix.
pub(crate) fn sequence_home(
    mut home: terminal_psi::StructuralArgument,
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<
    (
        terminal_psi::StructuralArgument,
        super::view_locals::ViewCarrier,
    ),
    LoweringError,
> {
    if !matches!(
        home.access,
        StructuralAccess::Owned | terminal_psi::reading_borrow_pair_pattern!()
    ) {
        return unsupported("sequence observation requires readable storage");
    }
    let (declaration, crossings) =
        observation_paths::readable_carrier(structural_type, declarations)?;
    let carrier = super::view_locals::ViewCarrier::of(declaration.id, declarations).ok_or(
        LoweringError::Unsupported("sequence observation requires a descriptor"),
    )?;
    home.path.extend(std::iter::repeat_n(
        terminal_psi::StructuralPathSegment::Referent,
        crossings,
    ));
    home.access = StructuralAccess::SharedBorrow;
    Ok((home, carrier))
}

pub(crate) fn sequence_length(
    home: &terminal_psi::StructuralArgument,
    carrier: super::view_locals::ViewCarrier,
    scalar_type: ScalarType,
) -> LoweredDirectExpression {
    let path = home
        .path
        .iter()
        .cloned()
        .map(LoweredElementPathSegment::Static)
        .collect();
    match carrier {
        super::view_locals::ViewCarrier::Bytes => LoweredDirectExpression::ByteSequenceLength {
            source: home.place,
            path,
            access: StructuralAccess::SharedBorrow,
            scalar_type,
        },
        super::view_locals::ViewCarrier::Elements { .. } => {
            LoweredDirectExpression::ElementViewLength {
                source: home.place,
                path,
                access: StructuralAccess::SharedBorrow,
                scalar_type,
            }
        }
    }
}

/// Declared collection geometry and its independent live descriptor extent.
pub(crate) struct IndexedElementObservation {
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) element_path: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) scalar_type: ScalarType,
    pub(crate) extent: Option<Box<LoweredDirectExpression>>,
}

/// Resolve an ordered scalar read from its established storage home. Root
/// location does not choose a different projection grammar. Readable reference
/// crossings come from declarations, while each runtime marker retains the
/// ordinal of the evaluated selector supplied by the read plan.
pub(crate) fn resolve_indexed_element(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    fields: &StructuralFieldBindings,
    home: &terminal_psi::StructuralArgument,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<IndexedElementObservation, LoweringError> {
    let projection =
        observation_paths::explicit_path(identities, structural_type, path, fields.declarations())?;
    let declaration = projection.declaration.ok_or(LoweringError::Unsupported(
        "indexed observation lost its collection carrier",
    ))?;
    let (element_path, scalar_type, carrier) = match &declaration.shape {
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element, .. } => {
            let element_projection = observation_paths::explicit_path(
                identities,
                *element,
                element_path,
                fields.declarations(),
            )?;
            let scalar_type = crate::emission::primitive_store::indexed_element_scalar_type(
                identities,
                *element,
                &element_projection.segments,
                fields.declarations(),
            )?;
            (
                element_projection.segments,
                scalar_type,
                super::view_locals::ViewCarrier::of(declaration.id, fields.declarations()),
            )
        }
        StructuralTypeShape::ByteSequence(_) if element_path.is_empty() => (
            Vec::new(),
            crate::emission::scalar_types::terminal_scalar_type(
                typed_trees::types::PrimitiveType::U8,
            )?,
            Some(super::view_locals::ViewCarrier::Bytes),
        ),
        _ => return unsupported("indexed observation requires a scalar collection endpoint"),
    };
    let path = super::structural_paths::prepare_argument_path(
        identities,
        &projection.segments,
        structural_type,
        fields.declarations(),
    )?
    .into_iter()
    .map(|step| match step {
        super::structural_paths::PreparedStructuralPathSegment::Static(step) => Ok(step),
        _ => unsupported("indexed collection prefix requires static projections"),
    })
    .collect::<Result<Vec<_>, _>>()?;
    let path = home.path.iter().cloned().chain(path).collect::<Vec<_>>();
    let selectors = path
        .iter()
        .filter(|step| {
            matches!(
                step,
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
            )
        })
        .count()
        + element_path
            .iter()
            .filter(|step| matches!(step, CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
            .count();
    if selectors > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
    {
        return unsupported("indexed read exceeds its selector namespace");
    }
    // The bound and read name the same descriptor, including every alias prefix
    // and readable reference crossing. Capacity never substitutes for live count.
    let extent = carrier
        .map(|carrier| {
            Ok::<_, LoweringError>(Box::new(sequence_length(
                &terminal_psi::StructuralArgument {
                    place: home.place,
                    path: path.clone(),
                    access: home.access,
                },
                carrier,
                crate::emission::scalar_types::terminal_scalar_type(
                    typed_trees::types::PrimitiveType::U64,
                )?,
            )))
        })
        .transpose()?;
    Ok(IndexedElementObservation {
        source: home.place,
        path,
        element_path,
        scalar_type,
        extent,
    })
}

/// Resolve each declared projection independently; no source arrangement grants
/// case membership. The emitted case-leaf copy still owes a dominating fact for
/// every case crossed, checked again by Terminal verification.
fn resolve_case_primitive(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    binding: &StructuralScalarFieldBinding,
    projection: observation_paths::ObservationPath<'_>,
    scalar_type: ScalarType,
) -> Result<
    (
        crate::emission::operation_emission::expressions::PrimitiveReadSource,
        Vec<terminal_psi::StructuralPathSegment>,
    ),
    LoweringError,
> {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Canonical;

    // The observation walk already selected exact declaration-local identities.
    // A case-leaf copy needs that inline carrier, not a second source-name walk.
    // Canonical geometry still grants neither active-case facts nor copy custody.
    let Some(mut canonical) = projection.canonical else {
        return unsupported("case primitive observation requires an inline canonical carrier");
    };
    let Some(Canonical::FixedIndex(index)) = canonical.pop() else {
        return unsupported("case primitive observation requires an indexed primitive carrier");
    };
    let Some(structural_type) = terminal_semantics::canonical_case_leaf_type(
        binding.declarations.iter(),
        binding.structural_type,
        &canonical,
    ) else {
        return unsupported("primitive observation lost its canonical carrier");
    };
    let (path, declared) = crate::emission::primitive_store::lower_path(
        identities,
        structural_type,
        &[CheckedUnitStructuralPathSegment::FixedIndex(index)],
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
