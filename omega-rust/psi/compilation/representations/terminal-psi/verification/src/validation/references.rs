//! Reference carriers own loan permission, never their primitive referent.
//!
//! Origins and immediate parents are reconstructed from operations and checked
//! call interfaces. No producer-supplied lifetime or origin table is authority.
//! Field observations through a reference use its live permission, not owned
//! root access. Geometry checks validate the selected field separately; this
//! frontier check keeps released carriers and suspended parents unreadable.

use super::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, PlaceId, StructuralAccess,
    StructuralArgument, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
    Terminator,
};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReferenceIdentity {
    place: PlaceId,
    /// Initial formation or formal-leaf coordinate, unaffected by owned moves.
    path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReferenceParent {
    Root(ReferenceOrigin),
    Reference(ReferenceIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ReferenceOrigin {
    Primitive(PlaceId),
    /// An internal owned ingress assumes one existing permission per leaf.
    /// Its callers must supply the actual lineage; this is not local backing.
    IngressLeaf(ReferenceIdentity),
    /// One runtime-selected referent, conservatively pinning every candidate
    /// beneath one ingress. These are alternatives, not simultaneous loans.
    Alternatives {
        place: PlaceId,
        paths: Vec<Vec<StructuralPathSegment>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveReference {
    /// Formation identity is stable across owned moves; parents name this ID,
    /// never a carrier's current record location.
    identity: ReferenceIdentity,
    pub(super) carrier: PlaceId,
    pub(super) carrier_path: Vec<StructuralPathSegment>,
    root: ReferenceOrigin,
    parent: ReferenceParent,
}

pub(crate) fn argument_owns_references(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    argument.access == StructuralAccess::Owned
        && super::structural::result_contracts::source_signature(machine, argument.place)
            .is_some_and(|source| contains_reference(module, source.structural_type))
}

// Incoming types have no explicit leaf roster to bound reconstruction. Keep
// this private verification-work capacity separate from resource authority;
// local/result rosters remain bounded by their existing captured rows.
const MAX_INCOMING_REFERENCE_LEAVES: usize = 4096;

pub(super) fn entry_references(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<LiveReference>, ModuleError> {
    let mut live = Vec::new();
    for parameter in &machine.structural_parameters {
        if !contains_reference(module, parameter.structural_type) {
            continue;
        }
        let paths = leaf_paths(
            module,
            parameter.structural_type,
            MAX_INCOMING_REFERENCE_LEAVES - live.len(),
        )
        .ok_or_else(|| {
            invalid(
                machine,
                "incoming reference leaf reconstruction capacity exceeded",
            )
        })?;
        for path in paths {
            let identity = ReferenceIdentity {
                place: parameter.place,
                path: path.clone(),
            };
            let root = ReferenceOrigin::IngressLeaf(identity.clone());
            live.push(LiveReference {
                identity,
                carrier: parameter.place,
                carrier_path: path,
                root: root.clone(),
                parent: ReferenceParent::Root(root),
            });
        }
    }
    live.sort_by(|left, right| left.identity.cmp(&right.identity));
    Ok(live)
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(super) fn contains_reference(module: &TerminalModule, root: StructuralTypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|item| item.id == current)
        else {
            continue;
        };
        let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::Record {
                fields: declarations,
            } => fields(declarations),
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::Mixed {
                fields: declarations,
                cases,
            } => {
                fields(declarations);
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element } => pending.push(*element),
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FusedServiceBinding => {}
        }
    }
    false
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// Foundation has already rejected cycles and unsupported containment kinds.
fn leaf_paths(
    module: &TerminalModule,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        // A type DAG can describe exponentially many occurrences. Existing
        // live descriptors bound the possible roster; do not expand an absent
        // roster or recurse through repeated reference-free payload subtrees.
        if !contains_reference(module, current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|item| item.id == current)
        else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            StructuralTypeShape::Record { fields } => {
                for field in fields.iter().rev() {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(module, child) {
                            continue;
                        }
                        // Every pending owned subtree owes at least one leaf.
                        // Bound the worklist before a wide DAG can amplify it.
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                        pending.push((child, child_path));
                    }
                }
            }
            _ => {}
        }
    }
    Some(output)
}

fn projected_carrier_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    let (StructuralPathSegment::Referent, fields) = source.path.split_last()? else {
        return None;
    };
    let current = super::structural::result_contracts::source_signature(machine, source.place)?
        .structural_type;
    let leaf = leaf_reference_type(module, current, fields)?;
    Some(referent(module, leaf).unwrap_or(leaf))
}

fn leaf_referent(
    module: &TerminalModule,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    referent(module, leaf_reference_type(module, current, fields)?)
}

/// The reference type declared at `fields` below `current`: the carrier a
/// `[fields.., Referent]` projection dereferences.
fn leaf_reference_type(
    module: &TerminalModule,
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    if let Some((_, declaration)) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        current,
        fields,
    ) {
        return Some(declaration.id);
    }
    for segment in fields {
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        match segment {
            StructuralPathSegment::Field(identity) => {
                // Common fields project identically through a record or a
                // mixed sum; case-only fields are not individually
                // borrowable storage.
                let fields = match &declaration.shape {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => return None,
                };
                let field = fields.iter().find(|field| &field.identity == identity)?;
                if field.relevance != terminal_psi::BindingRelevance::Relevant {
                    return None;
                }
                current = match &field.field_type {
                    StructuralFieldType::Structural(child) => *child,
                    // A scalar member names the module's matching primitive
                    // storage declaration; borrowing it carries that leaf
                    // type, not a reference formed at the member.
                    StructuralFieldType::Scalar(scalar) => module
                        .structural_types
                        .iter()
                        .find(|item| {
                            matches!(
                                item.shape,
                                StructuralTypeShape::PrimitiveScalar(shape) if shape == *scalar
                            )
                        })
                        .map(|item| item.id)?,
                    _ => return None,
                };
            }
            // A fixed index selects an element of an in-bound array: the
            // projected leaf is the element type when the index stays
            // inside the declared length.
            StructuralPathSegment::FixedIndex(index) => {
                current = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length } if *index < *length => {
                        *element
                    }
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    Some(current)
}

/// The access a reference type grants over its referent.
fn reference_access(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<StructuralAccess> {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        })
}

/// Loan strength: whether a place held with `authority` may lend
/// `requested`. Ownership lends any loan, a mutable loan may be reborrowed
/// shared or write-only, and a shared loan lends only a shared one. Every
/// reference this module mints, projects or returns must sit below its
/// source on this order; the `&'a` shared arms admitted beside the `&mut`
/// ones rely on it to keep a shared loan from becoming a mutable one.
fn access_covers(authority: StructuralAccess, requested: StructuralAccess) -> bool {
    authority == requested
        || matches!(
            (authority, requested),
            (StructuralAccess::Owned, _)
                | (
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                )
        )
}

/// The loan authority `source` holds over the storage it names. A whole
/// parameter holds its declared access and a machine-owned place holds
/// ownership. A `[fields.., Referent]` projection holds the weaker of its
/// carrier's authority and the dereferenced reference's own access, so a
/// `&mut` leaf read through a shared carrier lends only shared.
fn source_authority(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralAccess> {
    let carrier = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
        .or_else(|| super::block_views::parameter(machine, source.place))
        .map_or(StructuralAccess::Owned, |parameter| parameter.access);
    let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() else {
        return direct_storage_type(module, machine, source)
            .is_some()
            .then_some(carrier);
    };
    let signature = super::structural::result_contracts::source_signature(machine, source.place)?;
    // A reference leaf grants its own referent access; the storage a
    // non-reference leaf names lends exactly the carrier's authority, the
    // same as borrowing that member's whole place.
    let leaf = leaf_reference_type(module, signature.structural_type, fields)?;
    let leaf = reference_access(module, leaf).unwrap_or(carrier);
    Some(if access_covers(carrier, leaf) {
        leaf
    } else {
        carrier
    })
}

/// Whether the reference leaf at `path` below `root` grants no more than
/// `source` lends.
fn leaf_access_within(
    module: &TerminalModule,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    source: StructuralAccess,
) -> bool {
    leaf_reference_type(module, root, path)
        .and_then(|leaf| reference_access(module, leaf))
        .is_some_and(|granted| access_covers(source, granted))
}

pub(crate) fn is_reference_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> bool {
    (projected_carrier_type(module, machine, source).is_some()
        || projected_call_referent_type(module, machine, source).is_some())
        && source.access != StructuralAccess::Owned
}

/// A synchronous call can lend a declared subtree of a live mutable record
/// referent. Keep the complete suffix on the actual argument; this type walk
/// neither forms a child carrier nor supplies an escaping result origin.
fn projected_call_referent_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        StructuralAccess::MutableBorrow
            | StructuralAccess::SharedBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    let carrier_access = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == source.place)
        .map_or(StructuralAccess::Owned, |parameter| parameter.access);
    if !access_covers(carrier_access, source.access) {
        return None;
    }
    let (referent, suffix) =
        mutable_record_field_destination(module, machine, source.place, &source.path)?;
    if suffix.is_empty() {
        return None;
    }
    leaf_reference_type(module, referent, suffix)
}

pub(super) fn call_source_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
    allow_referent_subtree: bool,
) -> Option<StructuralTypeId> {
    source_type(module, machine, source).or_else(|| {
        allow_referent_subtree
            .then(|| projected_call_referent_type(module, machine, source))
            .flatten()
    })
}

pub(super) fn invalid(machine: &TerminalMachine, reason: &'static str) -> ModuleError {
    ModuleError::InvalidReferenceCustody {
        machine: machine.id,
        reason,
    }
}

pub(super) fn referent(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    module.structural_types.iter().find_map(|declaration| {
        if declaration.id != structural_type {
            return None;
        }
        match declaration.shape {
            StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::MutableBorrow,
            } if mutable_referent(module, referent) => Some(referent),
            // A `&'a V` shared view loans a named referent the same way a
            // `&mut` primitive loans its scalar: the referent is the leaf the
            // result's reference_sources project to.
            StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::SharedBorrow,
            } => Some(referent),
            _ => None,
        }
    })
}

fn mutable_referent(module: &TerminalModule, referent: StructuralTypeId) -> bool {
    super::primitive_storage::scalar_type(module, referent).is_some()
        || (module.structural_types.iter().any(|declaration| {
            declaration.id == referent
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        }) && super::structural::result_contracts::has_plain_owned_shape(module, referent))
}

/// Geometry is available only through an exact, unqualified mutable permission
/// carrier. The frontier separately requires that carrier and its lineage live.
pub(super) fn mutable_record_field_destination<'path>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: PlaceId,
    path: &'path [StructuralPathSegment],
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let (StructuralPathSegment::Referent, path) = path.split_first()? else {
        return None;
    };
    let signature = super::structural::result_contracts::source_signature(machine, destination)?;
    if signature.multiplicity != StructuralMultiplicity::Affine
        || !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
        || reference_access(module, signature.structural_type)
            != Some(StructuralAccess::MutableBorrow)
    {
        return None;
    }
    let referent = referent(module, signature.structural_type)?;
    module
        .structural_types
        .iter()
        .any(|declaration| {
            declaration.id == referent
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
        .then_some((referent, path))
}

/// Reconstruct the live permission behind a mutable record destination.
/// Geometry alone cannot authorize a referent write: the exact carrier must
/// still own its permission, and no child may keep that parent suspended.
/// Returning `None` means this is not a reference-crossing destination and
/// lets the caller apply its ordinary owned-root rules.
pub(super) fn live_mutable_record_field_destination<'path>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    destination: PlaceId,
    path: &'path [StructuralPathSegment],
) -> Result<Option<(StructuralTypeId, &'path [StructuralPathSegment])>, ModuleError> {
    let Some(destination_type) =
        mutable_record_field_destination(module, machine, destination, path)
    else {
        return Ok(None);
    };
    let reference = live
        .iter()
        .find(|reference| reference.carrier == destination && reference.carrier_path.is_empty())
        .ok_or_else(|| invalid(machine, "field destination reference is no longer live"))?;
    if live.iter().any(|child| {
        matches!(&child.parent,
            ReferenceParent::Reference(identity) if *identity == reference.identity)
    }) {
        return Err(invalid(
            machine,
            "field destination reference remains suspended by a live child",
        ));
    }
    Ok(Some(destination_type))
}

pub(super) fn carrier_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = super::structural::result_contracts::source_signature(machine, place)?;
    referent(module, signature.structural_type)
}

/// A reference declaration whose access is a shared borrow: `&` descriptors
/// copy freely, so a leaf copy of one relocates an existing loan's descriptor
/// rather than minting a custody edge.
fn shared_borrow_reference(module: &TerminalModule, structural_type: StructuralTypeId) -> bool {
    module.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                }
            )
    })
}

/// Result mappings describe the complete canonical-path-ordered leaf roster, never
/// assert live custody. Return replay checks the actual relocated carriers;
/// Borrowed ingress forms new child loans; owned ingress transfers existing
/// leaf identities. Partial transfers still require their own replay.
pub(super) fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    let is_reference = |structural_type| {
        module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(declaration.shape, StructuralTypeShape::Reference { .. })
        })
    };
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        if let Some(result) = operation.result.structural()
            && contains_reference(module, result.structural_type)
            && !is_reference(result.structural_type)
            && (!matches!(
                operation.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            ) || result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty())
        {
            return Err(invalid(
                machine,
                "stored references require an affine record establishment or checked call",
            ));
        }
        if operation
            .result
            .structural()
            .is_some_and(|result| is_reference(result.structural_type))
            && !matches!(
                operation.kind,
                OperationKind::EstablishReference { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
            // A shared-borrow leaf copy transfers an existing loan's
            // descriptor into fresh unrestricted storage; the loan stays
            // minted on its source root, so the copy establishes no custody
            // edge of its own.
            && !(matches!(
                operation.kind,
                OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
            ) && operation.result.structural().is_some_and(|result| {
                shared_borrow_reference(module, result.structural_type)
            }))
        {
            return Err(invalid(
                machine,
                "operation cannot establish reference custody",
            ));
        }
    }
    for parameter in machine
        .structural_parameters
        .iter()
        .filter(|parameter| contains_reference(module, parameter.structural_type))
    {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !super::record::constructible_type(module, parameter.structural_type)
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(invalid(
                machine,
                "incoming reference records require whole unqualified affine ownership",
            ));
        }
    }
    // A block parameter may hold a whole owned reference record: the same
    // eligibility as an incoming parameter. The frontier's live-reference
    // rows re-home onto the bound place at each edge, so edge custody, not
    // the parameter shape, carries the loan's exact evidence. Anything
    // weaker — borrowed access, a qualifier, a non-affine or non-constructible
    // carrier — still has no verified edge meaning.
    for parameter in machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| contains_reference(module, parameter.structural_type))
    {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !super::record::constructible_type(module, parameter.structural_type)
        {
            return Err(invalid(
                machine,
                "reference carrier block parameters require whole unqualified affine ownership",
            ));
        }
    }
    // Binding any subtree of a reference-bearing root would need partial-move
    // custody, even when the selected target parameter itself has no references.
    // A whole owned argument moves the entire record — and every stored loan
    // with it — across the edge; a borrow lane or a projected path cannot.
    for block in &machine.blocks {
        let arguments: &[StructuralArgument] = match &block.terminator {
            Terminator::Jump {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => &[],
        };
        let conditional_arguments = match &block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => Some(
                when_true
                    .structural_arguments
                    .iter()
                    .chain(&when_false.structural_arguments),
            ),
            _ => None,
        };
        if arguments
            .iter()
            .chain(conditional_arguments.into_iter().flatten())
            .any(|argument| {
                super::structural::result_contracts::source_signature(machine, argument.place)
                    .is_some_and(|source| contains_reference(module, source.structural_type))
                    && (argument.access != StructuralAccess::Owned || !argument.path.is_empty())
            })
        {
            return Err(invalid(
                machine,
                "reference-bearing continuation bindings are not yet supported",
            ));
        }
    }
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    if !contains_reference(module, result.structural_type) {
        if !result.reference_sources.is_empty() {
            return Err(invalid(
                machine,
                "non-reference result has reference source mappings",
            ));
        }
        return Ok(());
    }
    if referent(module, result.structural_type).is_none()
        && !super::record::constructible_type(module, result.structural_type)
    {
        return Err(invalid(
            machine,
            "reference result access or referent is unsupported",
        ));
    }
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid(
            machine,
            "reference result must own unqualified affine permission",
        ));
    }
    let mut paths = leaf_paths(
        module,
        result.structural_type,
        result.reference_sources.len(),
    )
    .ok_or_else(|| invalid(machine, "reference result is missing source mappings"))?;
    // Result sources are a canonical map, not a lifecycle schedule. Constructors
    // and cleanup keep declaration order; only this interface comparison sorts.
    paths.sort();
    if paths.len() != result.reference_sources.len() {
        return Err(invalid(
            machine,
            "reference result source roster has extra mappings",
        ));
    }
    if result
        .reference_sources
        .iter()
        .any(|mapping| mapping.sources.len() > 1)
    {
        validate_alternative_result_dependencies(module, machine)?;
    }
    let mut sources = BTreeSet::new();
    for (path, mapping) in paths.iter().zip(&result.reference_sources) {
        let expected_referent = leaf_referent(module, result.structural_type, path)
            .ok_or_else(|| invalid(machine, "reference result leaf is unsupported"))?;
        if mapping.path != *path
            || mapping.sources.is_empty()
            || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                machine,
                "reference result candidates are absent, duplicated, or unordered",
            ));
        }
        if mapping.sources.len() > 1 {
            let first = &mapping.sources[0];
            let record = module.structural_types.iter().any(|declaration| {
                declaration.id == expected_referent
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            });
            // Dynamic result addresses currently represent one whole mutable
            // record under one borrowed formal. General reference-bearing
            // records and inter-formal selections keep their explicit fence.
            if !path.is_empty()
                || !record
                || reference_access(module, result.structural_type)
                    != Some(StructuralAccess::MutableBorrow)
                || mapping.sources.iter().any(|source| {
                    source.place != first.place
                        || source.access != StructuralAccess::MutableBorrow
                        || source.path.is_empty()
                        || source.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        })
                })
            {
                return Err(invalid(
                    machine,
                    "reference result alternatives require one mutable record ingress",
                ));
            }
        }
        for source in &mapping.sources {
            if !matches!(
                source.access,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            ) || !sources.insert((source.place, source.path.clone()))
                || !machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == source.place)
                || source_type(module, machine, source) != Some(expected_referent)
                || !leaf_access_within(module, result.structural_type, path, source.access)
                || formal_origin(machine, source).is_none()
            {
                return Err(invalid(
                    machine,
                    "reference result does not name an exact mutable formal referent",
                ));
            }
        }
    }
    Ok(())
}

/// Exact candidate coverage must be founded in return bodies, not a recursive
/// interface naming itself. Until recursive origin summaries have their own
/// least-fixed-point replay, grouped-result dependency cycles remain explicit.
fn validate_alternative_result_dependencies(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    // A descendant with a singleton declaration can otherwise provide circular
    // evidence for one of this machine's candidates. Detect every reachable
    // reference-result cycle, while permitting shared acyclic dependencies.
    let mut pending = vec![(machine.id, false)];
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    while let Some((current, exiting)) = pending.pop() {
        if exiting {
            active.remove(&current);
            completed.insert(current);
            continue;
        }
        if completed.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return Err(invalid(
                machine,
                "recursive selected-reference results require founded origin summaries",
            ));
        }
        pending.push((current, true));
        let Some(current) = module
            .machines
            .iter()
            .find(|candidate| candidate.id == current)
        else {
            return Err(invalid(machine, "reference result dependency is absent"));
        };
        for operation in current.blocks.iter().flat_map(|block| &block.operations) {
            let callee = match &operation.kind {
                OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. }
                    if operation.result.structural().is_some_and(|result| {
                        contains_reference(module, result.structural_type)
                    }) =>
                {
                    *callee
                }
                _ => continue,
            };
            pending.push((callee, false));
        }
    }
    Ok(())
}

fn formal_origin(
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<ReferenceOrigin> {
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    if source.path.is_empty()
        && matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::Primitive(source.place))
    } else if !source.path.is_empty()
        && source.path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        })
        && matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: source.path.clone(),
        }))
    } else if let Some((StructuralPathSegment::Referent, path)) = source.path.split_last()
        && matches!(
            parameter.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: path.to_vec(),
        }))
    } else {
        None
    }
}

pub(super) fn validate_result(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let result = operation.result.structural().ok_or_else(|| {
        invalid(
            machine,
            "reference establishment requires a structural result",
        )
    })?;
    if !contains_reference(module, result.structural_type)
        || (referent(module, result.structural_type).is_none()
            && !super::record::constructible_type(module, result.structural_type))
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        })
    {
        return Err(invalid(
            machine,
            "invalid reference-bearing operation result",
        ));
    }
    Ok(())
}

/// Static source formation; path availability and suspension are checked in the
/// same operation transaction as the owning frontier.
pub(super) fn source_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) || !source_authority(module, machine, source)
        .is_some_and(|authority| access_covers(authority, source.access))
    {
        return None;
    }
    projected_carrier_type(module, machine, source)
        .or_else(|| direct_storage_type(module, machine, source))
}

/// Parameters, block parameters and operation results all name storage through
/// their published signatures. A direct borrow retains its complete field/index
/// coordinate; it neither copies storage nor dereferences a permission carrier.
/// Reference traversal stays on the carrier path, with its own access checks.
fn direct_storage_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    let signature = super::structural::result_contracts::source_signature(machine, source.place)?;
    if source
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Referent))
        || signature.multiplicity == StructuralMultiplicity::Linear
        || !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == source.place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source.place)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                operation.result.structural().is_some_and(|result| {
                    result.place == source.place
                        && (!result.claims.is_empty()
                            || !result.qualification_establishments.is_empty())
                })
            })
    {
        return None;
    }
    leaf_reference_type(module, signature.structural_type, &source.path)
}

pub(super) fn validate_establishment(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: &StructuralArgument,
) -> Result<(), ModuleError> {
    validate_result(module, machine, operation)?;
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference result is absent"))?;
    let expected = referent(module, result.structural_type).ok_or_else(|| {
        invalid(
            machine,
            "reference establishment requires a whole reference result",
        )
    })?;
    if !matches!(
        source.access,
        StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
    ) || source_type(module, machine, source) != Some(expected)
        || !leaf_access_within(module, result.structural_type, &[], source.access)
    {
        return Err(invalid(
            machine,
            "reference formation exceeds its source authority",
        ));
    }
    Ok(())
}

pub(super) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    if let OperationKind::StructuralScalarFieldStore {
        destination, path, ..
    }
    | OperationKind::StructuralByteSequenceFieldStore {
        destination, path, ..
    }
    | OperationKind::StructuralByteViewFieldStore {
        destination, path, ..
    } = &operation.kind
        && path.first() == Some(&StructuralPathSegment::Referent)
        && !machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *destination)
        && !available.contains(destination)
    {
        return Err(invalid(
            machine,
            "field destination reference has not been established on every arrival",
        ));
    }
    if let OperationKind::EstablishReference { source } = &operation.kind {
        let parameter = machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == source.place);
        if !parameter && !available.contains(&source.place) {
            return Err(invalid(
                machine,
                "reference source has not been established on every arrival",
            ));
        }
    }
    Ok(())
}

fn call_source(
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    formal: &StructuralArgument,
) -> Option<StructuralArgument> {
    let position = callee
        .structural_parameters
        .iter()
        .position(|parameter| parameter.place == formal.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(formal.path.iter().cloned());
    source.access = formal.access;
    Some(source)
}

pub(super) fn validate_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<bool, ModuleError> {
    let (
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
    ) = match &operation.kind {
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => (
            *callee,
            &[][..],
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            erased_arguments: _,
            erased_proof_arguments: _,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *callee,
            arguments.as_slice(),
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        _ => return Ok(false),
    };
    let Some(callee) = machines.get(&callee).copied() else {
        return Ok(false);
    };
    let Some(result) = callee
        .result
        .structural()
        .filter(|result| contains_reference(module, result.structural_type))
    else {
        return Ok(false);
    };
    validate_result(module, machine, operation)?;
    let actual = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference call result is absent"))?;
    if !super::structural::result_contracts::call_result_matches(actual, result)
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !callee.entry_claims.is_empty()
        || !callee.content_entry_claims.is_empty()
        || arguments.len() != callee.parameters.len()
        || requirement_obligations.len() != callee.contract.requires.len()
    {
        return Err(invalid(
            machine,
            "reference call does not match its complete callee interface",
        ));
    }
    super::structural::operations::validate_structural_arguments(module, machine, structural_arguments, &callee.structural_parameters, operation.id, true,
        super::structural::operations::StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)?;
    super::structural::operations::validate_unit_call_contract_places(callee, operation.id)?;
    super::structural::operations::validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    super::structural::operations::validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
    )?;
    super::structural::operations::validate_unit_call_crash_continuations(
        module,
        machine,
        callee,
        arguments,
        structural_arguments,
        crash_continuations,
        operation.id,
    )?;
    Ok(true)
}

fn normalized_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(ReferenceOrigin, ReferenceParent), ModuleError> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        if let Some(parent) = live.iter().find(|reference| {
            reference.carrier == source.place && reference.carrier_path == carrier_path
        }) {
            if live
                .iter()
                .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &parent.identity))
            {
                return Err(invalid(
                    machine,
                    "reference parent remains suspended by a live child",
                ));
            }
            return Ok((
                parent.root.clone(),
                ReferenceParent::Reference(parent.identity.clone()),
            ));
        }
        // A `[fields.., Referent]` leaf that is not itself a reference borrows
        // a member's own storage: the loan roots at that member under the
        // source place rather than continuing a live parent loan.
        let leaf = super::structural::result_contracts::source_signature(machine, source.place)
            .and_then(|signature| {
                leaf_reference_type(module, signature.structural_type, carrier_path)
            })
            .ok_or_else(|| invalid(machine, "projected reference custody is not yet supported"))?;
        if reference_access(module, leaf).is_some() {
            return Err(invalid(machine, "reference carrier is no longer live"));
        }
        if mutable_referent(module, leaf)
            && live
                .iter()
                .any(|reference| origin_root(&reference.root) == source.place)
        {
            return Err(invalid(
                machine,
                "projected ingress remains suspended by a live reference",
            ));
        }
        let root = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: carrier_path.to_vec(),
        });
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else if source.path.is_empty() {
        if live
            .iter()
            .any(|reference| origin_root(&reference.root) == source.place)
        {
            return Err(invalid(
                machine,
                "original referent remains suspended by a live reference",
            ));
        }
        let root = ReferenceOrigin::Primitive(source.place);
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else if direct_storage_type(module, machine, source).is_some() {
        if live
            .iter()
            .any(|reference| origin_root(&reference.root) == source.place)
        {
            return Err(invalid(
                machine,
                "projected ingress remains suspended by a live reference",
            ));
        }
        let root = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: source.path.clone(),
        });
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else {
        Err(invalid(
            machine,
            "projected reference custody is not yet supported",
        ))
    }
}

fn origin_root(origin: &ReferenceOrigin) -> PlaceId {
    match origin {
        ReferenceOrigin::Primitive(place) => *place,
        ReferenceOrigin::IngressLeaf(identity) => identity.place,
        ReferenceOrigin::Alternatives { place, .. } => *place,
    }
}

fn origins_overlap(left: &ReferenceOrigin, right: &ReferenceOrigin) -> bool {
    if origin_root(left) != origin_root(right) {
        return false;
    }
    let overlaps_path = |path: &[StructuralPathSegment]| match right {
        ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => {
            terminal_semantics::structural_paths_may_overlap(path, &identity.path)
        }
        ReferenceOrigin::Alternatives { paths, .. } => paths
            .iter()
            .any(|other| terminal_semantics::structural_paths_may_overlap(path, other)),
    };
    match left {
        ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => overlaps_path(&identity.path),
        ReferenceOrigin::Alternatives { paths, .. } => paths.iter().any(|path| overlaps_path(path)),
    }
}

fn contains_origin(origins: &ReferenceOrigin, candidate: &ReferenceOrigin) -> bool {
    match (origins, candidate) {
        (
            ReferenceOrigin::Alternatives { place, paths },
            ReferenceOrigin::IngressLeaf(identity),
        ) => *place == identity.place && paths.binary_search(&identity.path).is_ok(),
        _ => origins == candidate,
    }
}

/// An `EstablishReference` whose source ends in `Referent` beneath an owned
/// record carrier selects that carrier's leaf rather than borrowing through
/// it: the leaf re-homes under the result root and the carrier is consumed
/// whole. The move is only exact when the carrier's declared reference roster
/// is that one leaf — a partial leaf move would strand sibling custody under
/// a consumed place. A reference-typed carrier is the loan itself, so forming
/// `&mut *r` keeps the ordinary child-loan lane instead of consuming `r`.
/// The move lane exists to seat an owned ingress leaf beneath a bare result,
/// an origin `formal_origin` admits for owned machine parameters alone. A
/// local or block-parameter carrier keeps the ordinary child-loan lane:
/// `&mut *record.field` reborrows through the live leaf and leaves the
/// carrier affine-owned for its own edge cleanup.
pub(super) fn establishment_moves_leaf(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    operation: &terminal_psi::Operation,
) -> Option<(PlaceId, usize)> {
    let OperationKind::EstablishReference { source } = &operation.kind else {
        return None;
    };
    // Only an owned carrier can give its leaf away; a shared carrier lends
    // through it and keeps the leaf live for its caller.
    if !matches!(
        formal_origin(machine, source),
        Some(ReferenceOrigin::IngressLeaf(_))
    ) || !machine.structural_parameters.iter().any(|parameter| {
        parameter.place == source.place && parameter.access == StructuralAccess::Owned
    }) {
        return None;
    }
    let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() else {
        return None;
    };
    let (position, leaf) = live.iter().enumerate().find(|(_, reference)| {
        reference.carrier == source.place && reference.carrier_path == carrier_path
    })?;
    if leaf.parent != ReferenceParent::Root(leaf.root.clone()) {
        return None;
    }
    let signature = super::structural::result_contracts::source_signature(machine, source.place)?;
    if matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == signature.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Reference { .. })
    ) {
        return None;
    }
    let paths = leaf_paths(module, signature.structural_type, live.len())?;
    (paths.len() == 1
        && paths[0].as_slice() == carrier_path
        && live
            .iter()
            .filter(|reference| reference.carrier == source.place)
            .count()
            == 1
        && !live.iter().any(|child| {
            matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &leaf.identity)
        }))
    .then_some((source.place, position))
}

pub(super) fn release(
    machine: &TerminalMachine,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let position = live
        .iter()
        .position(|reference| reference.carrier == source && reference.carrier_path.is_empty())
        .ok_or_else(|| invalid(machine, "release requires one live reference carrier"))?;
    if live
        .iter()
        .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &live[position].identity))
    {
        return Err(invalid(machine, "a reference cannot end before its child"));
    }
    live.remove(position);
    Ok(())
}

/// Whole-owner disposal visits reference leaves in reverse declaration order.
/// Validate the entire schedule before ending any loan; a live external child
/// (or an incorrectly ordered contained child) keeps its parent suspended.
pub(super) fn discard_owned(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let Some(signature) = super::structural::result_contracts::source_signature(machine, source)
    else {
        return Ok(());
    };
    let paths = leaf_paths(module, signature.structural_type, live.len()).ok_or_else(|| {
        invalid(
            machine,
            "discard type requires more reference leaves than are live",
        )
    })?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let reference = live
            .iter()
            .find(|reference| reference.carrier == source && reference.carrier_path == *path)
            .ok_or_else(|| {
                invalid(
                    machine,
                    "discard requires every owned reference leaf to be live",
                )
            })?;
        if live
            .iter()
            .any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity) && !released.contains(&child.identity))
        {
            return Err(invalid(machine, "a reference cannot end before its child"));
        }
        released.insert(reference.identity.clone());
    }
    if live
        .iter()
        .any(|reference| reference.carrier == source && !released.contains(&reference.identity))
    {
        return Err(invalid(
            machine,
            "discard reference roster differs from its owned type",
        ));
    }
    live.retain(|reference| !released.contains(&reference.identity));
    Ok(())
}

/// Record construction moves carriers, not referents. Exact static bindings
/// and the owning frontier are checked in this same operation transaction;
/// relocation cannot turn an owned move into a second, child loan.
fn establish_record(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    live: &mut [LiveReference],
) -> Result<(), ModuleError> {
    let declarations = super::record::fields(module, machine, operation)?;
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        return Err(invalid(
            machine,
            "record transfer requires a record establishment",
        ));
    };
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "record result is absent"))?;
    if live.iter().any(|reference| {
        reference.carrier == result.place || reference.identity.place == result.place
    }) {
        return Err(invalid(
            machine,
            "record destination still owns live reference custody",
        ));
    }
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    for (declaration, field) in declarations.iter().zip(fields) {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let signature =
            super::structural::result_contracts::source_signature(machine, argument.place)
                .ok_or_else(|| invalid(machine, "record operand has no structural source"))?;
        let paths = leaf_paths(module, signature.structural_type, live.len()).ok_or_else(|| {
            invalid(
                machine,
                "record operand requires more reference leaves than are live",
            )
        })?;
        for path in paths {
            let position = live
                .iter()
                .position(|reference| {
                    reference.carrier == argument.place && reference.carrier_path == path
                })
                .ok_or_else(|| {
                    invalid(machine, "record operand does not own its reference leaf")
                })?;
            if !moved.insert(position) {
                return Err(invalid(
                    machine,
                    "record cannot duplicate reference custody",
                ));
            }
            let mut destination = vec![StructuralPathSegment::Field(declaration.identity.clone())];
            destination.extend(path);
            relocations.push((position, destination));
        }
        if live.iter().enumerate().any(|(position, reference)| {
            reference.carrier == argument.place && !moved.contains(&position)
        }) {
            return Err(invalid(
                machine,
                "record operand reference roster differs from its type",
            ));
        }
    }
    for (position, path) in relocations {
        live[position].carrier = result.place;
        live[position].carrier_path = path;
    }
    Ok(())
}

fn establish_structural_case(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    live: &mut [LiveReference],
) -> Result<(), ModuleError> {
    let declarations = super::structural::case::fields(module, machine, operation)?;
    let OperationKind::EstablishStructuralCase { fields, .. } = &operation.kind else {
        return Err(invalid(
            machine,
            "case transfer requires a structural-case establishment",
        ));
    };
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "case result is absent"))?;
    if live.iter().any(|reference| {
        reference.carrier == result.place || reference.identity.place == result.place
    }) {
        return Err(invalid(
            machine,
            "case destination still owns live reference custody",
        ));
    }
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    for (entry, field) in declarations.iter().zip(fields) {
        let declaration = entry.declaration;
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let signature =
            super::structural::result_contracts::source_signature(machine, argument.place)
                .ok_or_else(|| invalid(machine, "case operand has no structural source"))?;
        let paths = leaf_paths(module, signature.structural_type, live.len()).ok_or_else(|| {
            invalid(
                machine,
                "case operand requires more reference leaves than are live",
            )
        })?;
        for path in paths {
            let position = live
                .iter()
                .position(|reference| {
                    reference.carrier == argument.place && reference.carrier_path == path
                })
                .ok_or_else(|| invalid(machine, "case operand does not own its reference leaf"))?;
            if !moved.insert(position) {
                return Err(invalid(machine, "case cannot duplicate reference custody"));
            }
            let mut destination = vec![StructuralPathSegment::Field(declaration.identity.clone())];
            destination.extend(path);
            relocations.push((position, destination));
        }
        if live.iter().enumerate().any(|(position, reference)| {
            reference.carrier == argument.place && !moved.contains(&position)
        }) {
            return Err(invalid(
                machine,
                "case operand reference roster differs from its type",
            ));
        }
    }
    for (position, path) in relocations {
        live[position].carrier = result.place;
        live[position].carrier_path = path;
    }
    Ok(())
}

pub(super) fn check_root_access(
    machine: &TerminalMachine,
    live: &[LiveReference],
    place: PlaceId,
) -> Result<(), ModuleError> {
    if live
        .iter()
        .any(|reference| origin_root(&reference.root) == place || reference.carrier == place)
    {
        return Err(invalid(
            machine,
            "operation accesses suspended referent or treats reference carrier as owned data",
        ));
    }
    Ok(())
}

/// Ordinary storage access suspends the region actually borrowed, not every
/// sibling under its owner. Use the same canonical operation regions as
/// restoration windows, and compare all candidates of a selected origin.
/// An unresolved path conservatively covers the whole root. This query grants
/// no access through a reference carrier; those reads need live permission.
fn check_operation_root_access(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    operation: &terminal_psi::Operation,
    place: PlaceId,
) -> Result<(), ModuleError> {
    if live.is_empty() {
        return Ok(());
    }
    let regions = super::structural::access_regions::operation_regions(module, machine, operation);
    let unresolved = !regions.iter().any(|(root, _)| *root == place);
    let overlaps_path = |path: &[StructuralPathSegment]| {
        let borrowed = super::structural::operations::structural_argument_canonical_prefix(
            module,
            machine,
            &StructuralArgument {
                place,
                path: path.to_vec(),
                // Geometry only: this admits a final bounded-byte field in
                // the canonical prefix query, never a mutable permission.
                access: StructuralAccess::MutableBorrow,
            },
        )
        .unwrap_or_default();
        unresolved
            || regions.iter().any(|(root, observed)| {
                *root == place
                    && (observed.starts_with(&borrowed) || borrowed.starts_with(observed))
            })
    };
    if live.iter().any(|reference| {
        (reference.carrier == place && overlaps_path(&reference.carrier_path))
            || (origin_root(&reference.root) == place
                && match &reference.root {
                    ReferenceOrigin::Primitive(_) => true,
                    ReferenceOrigin::IngressLeaf(identity) => overlaps_path(&identity.path),
                    ReferenceOrigin::Alternatives { paths, .. } => {
                        paths.iter().any(|path| overlaps_path(path))
                    }
                })
    }) {
        return Err(invalid(
            machine,
            "operation accesses suspended referent or treats reference carrier as owned data",
        ));
    }
    Ok(())
}

/// Reading a referent is not reading the carrier as owned storage. The type
/// supplies access, but only the current frontier supplies live permission.
fn check_field_read(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    operation: &terminal_psi::Operation,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let access = super::structural::result_contracts::source_signature(machine, source)
        .and_then(|signature| reference_access(module, signature.structural_type));
    let Some(access) = access else {
        return check_operation_root_access(module, machine, live, operation, source);
    };
    if !matches!(
        access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        return Err(invalid(
            machine,
            "field source reference does not permit observation",
        ));
    }
    let reference = live
        .iter()
        .find(|reference| reference.carrier == source && reference.carrier_path.is_empty())
        .ok_or_else(|| invalid(machine, "field source reference is no longer live"))?;
    if live.iter().any(|child| {
        origin_root(&child.root) == source
            || matches!(&child.parent,
                ReferenceParent::Reference(identity) if *identity == reference.identity)
    }) {
        return Err(invalid(
            machine,
            "field source reference remains suspended by a live loan",
        ));
    }
    Ok(())
}

pub(super) fn apply_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
    live: &mut Vec<LiveReference>,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::ReleaseReference { source } => return release(machine, live, *source),
        OperationKind::EstablishRecord { .. } => {
            return establish_record(module, machine, operation, live);
        }
        OperationKind::EstablishStructuralCase { .. } => {
            return establish_structural_case(module, machine, operation, live);
        }
        OperationKind::PrimitiveScalarRead { source, .. }
        | OperationKind::StructuralCaseMembership { source, .. } => {
            check_operation_root_access(module, machine, live, operation, *source)?
        }
        OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. }
        | OperationKind::StructuralByteSequenceFieldLength { source, .. }
        | OperationKind::StructuralByteSequenceFieldRead { source, .. } => {
            check_field_read(module, machine, live, operation, *source)?
        }
        OperationKind::StructuralScalarFieldStore {
            destination, path, ..
        }
        | OperationKind::StructuralByteSequenceFieldStore {
            destination, path, ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination, path, ..
        } => {
            if live_mutable_record_field_destination(module, machine, live, *destination, path)?
                .is_none()
            {
                check_operation_root_access(module, machine, live, operation, *destination)?;
            }
        }
        OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::AtomicAccess {
            place: destination, ..
        } => check_operation_root_access(module, machine, live, operation, *destination)?,
        OperationKind::StructuralLeafCopy { source, .. }
        | OperationKind::StructuralCaseLeafCopy { source, .. } => {
            // A `&`-leaf copy relocates the source parameter's existing loan
            // descriptor: the result is a fresh live carrier rooted at that
            // same primitive loan — no new edge, no consumption of `source`.
            // The case-qualified variant relocates the same loan out of a
            // proven-active case payload.
            if let Some(result) = operation.result.structural()
                && shared_borrow_reference(module, result.structural_type)
            {
                let root = ReferenceOrigin::Primitive(*source);
                live.push(LiveReference {
                    identity: ReferenceIdentity {
                        place: result.place,
                        path: Vec::new(),
                    },
                    carrier: result.place,
                    carrier_path: Vec::new(),
                    root: root.clone(),
                    parent: ReferenceParent::Root(root),
                });
            }
        }
        _ => {}
    }
    let arguments = match &operation.kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    let mut normalized_arguments = Vec::new();
    let mut moved = BTreeSet::new();
    for argument in arguments {
        if argument_owns_references(module, machine, argument) {
            if matches!(operation.kind, OperationKind::BoundaryCall { .. }) {
                return Err(invalid(
                    machine,
                    "reference-bearing owned calls require a whole internal argument",
                ));
            }
            let signature =
                super::structural::result_contracts::source_signature(machine, argument.place)
                    .ok_or_else(|| invalid(machine, "owned reference argument has no source"))?;
            // A projected owned argument moves the complete leaf roster of
            // the declared record subtree it names: every edge must resolve
            // as a plain field, so an index or referent hop keeps the
            // whole-carrier requirement. Custody outside the moved subtree
            // stays with the carrier for the frontier's residual cleanup.
            let projected_type = if argument.path.is_empty() {
                Some(signature.structural_type)
            } else {
                argument
                    .path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    .then(|| {
                        super::resolve_structural_path(
                            module,
                            signature.structural_type,
                            &argument.path,
                        )
                    })
                    .flatten()
            }
            .ok_or_else(|| {
                invalid(
                    machine,
                    "projected reference argument path is not a declared record edge",
                )
            })?;
            let paths = leaf_paths(module, projected_type, live.len()).ok_or_else(|| {
                invalid(
                    machine,
                    "owned argument reference roster exceeds live custody",
                )
            })?;
            if paths.len()
                != live
                    .iter()
                    .filter(|reference| {
                        reference.carrier == argument.place
                            && reference.carrier_path.starts_with(argument.path.as_slice())
                    })
                    .count()
            {
                return Err(invalid(
                    machine,
                    "owned argument reference roster differs from its type",
                ));
            }
            for path in paths {
                let mut carrier_path = argument.path.clone();
                carrier_path.extend(path);
                let reference = live
                    .iter()
                    .find(|reference| {
                        reference.carrier == argument.place
                            && reference.carrier_path == carrier_path
                    })
                    .ok_or_else(|| invalid(machine, "owned argument reference leaf is not live"))?;
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
                    || !moved.insert(reference.identity.clone())
                {
                    return Err(invalid(machine, "owned reference argument duplicates or suspends a leaf"));
                }
                normalized_arguments.push((reference.root.clone(), argument.access, true));
            }
            continue;
        }
        if super::structural::result_contracts::source_signature(machine, argument.place)
            .is_some_and(|source| contains_reference(module, source.structural_type))
            || live
                .iter()
                .any(|reference| origin_root(&reference.root) == argument.place)
        {
            if !is_reference_projection(module, machine, argument) {
                return Err(invalid(
                    machine,
                    "call cannot access a suspended root or move its referent",
                ));
            }
            let root = if projected_call_referent_type(module, machine, argument).is_some() {
                let parent = live
                    .iter()
                    .find(|reference| {
                        reference.carrier == argument.place && reference.carrier_path.is_empty()
                    })
                    .ok_or_else(|| {
                        invalid(
                            machine,
                            "projected call reference carrier is no longer live",
                        )
                    })?;
                if live.iter().any(|reference|
                    matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &parent.identity))
                {
                    return Err(invalid(machine, "projected call reference remains suspended by a live child"));
                }
                // The call does not publish a child permission. Retaining the
                // complete parent's origin conservatively rejects overlapping
                // exclusive actuals even when two carriers spell them differently.
                // The argument itself still retains its exact projected address.
                parent.root.clone()
            } else {
                normalized_source(module, machine, live, argument)?.0
            };
            normalized_arguments.push((root, argument.access, true));
        } else {
            normalized_arguments.push((
                ReferenceOrigin::Primitive(argument.place),
                argument.access,
                false,
            ));
        }
    }
    for (position, (root, access, reference_access)) in normalized_arguments.iter().enumerate() {
        if normalized_arguments[position + 1..].iter().any(
            |(other, other_access, other_reference_access)| {
                (*reference_access || *other_reference_access)
                    && origins_overlap(root, other)
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
            },
        ) {
            return Err(invalid(
                machine,
                "call arguments overlap through reference origins",
            ));
        }
    }
    let sources = match &operation.kind {
        OperationKind::EstablishReference { source } => {
            let transfers_existing = establishment_moves_leaf(module, machine, live, operation);
            if let Some((_, position)) = transfers_existing {
                moved.insert(live[position].identity.clone());
            }
            vec![(
                Vec::new(),
                vec![(source.clone(), transfers_existing.is_some())],
            )]
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            ..
        } if operation
            .result
            .structural()
            .is_some_and(|result| contains_reference(module, result.structural_type)) =>
        {
            let callee = machines
                .get(callee)
                .ok_or_else(|| invalid(machine, "reference call target is absent"))?;
            let result = callee.result.structural().ok_or_else(|| {
                invalid(machine, "reference call target has no structural result")
            })?;
            result
                .reference_sources
                .iter()
                .map(|mapping| {
                    let sources = mapping
                        .sources
                        .iter()
                        .map(|formal| {
                            call_source(callee, structural_arguments, formal)
                        .map(|source| {
                            (
                                source,
                                matches!(
                                    formal_origin(callee, formal),
                                    Some(ReferenceOrigin::IngressLeaf(identity))
                                        if super::structural::result_contracts::source_signature(
                                            callee,
                                            identity.place,
                                        )
                                        .and_then(|signature| {
                                            leaf_reference_type(
                                                module,
                                                signature.structural_type,
                                                &identity.path,
                                            )
                                        })
                                        .is_some_and(|leaf| {
                                            reference_access(module, leaf).is_some()
                                        })
                                ),
                            )
                        })
                        .ok_or_else(|| invalid(machine, "reference call source mapping is absent"))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((mapping.path.clone(), sources))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => Vec::new(),
    };
    if !sources.is_empty() {
        let result = operation
            .result
            .structural()
            .ok_or_else(|| invalid(machine, "reference establishment result is absent"))?;
        if live.iter().any(|reference| {
            reference.carrier == result.place || reference.identity.place == result.place
        }) {
            return Err(invalid(machine, "reference carrier is already live"));
        }
        // Stage the complete result before publication. Independent leaves
        // cannot duplicate an exclusive origin, even through distinct actual
        // argument spellings. Borrowing a result does not move its parent loan.
        let mut established = Vec::with_capacity(sources.len());
        let mut roots = BTreeSet::new();
        for (path, candidates) in sources {
            let expected = leaf_referent(module, result.structural_type, &path)
                .ok_or_else(|| invalid(machine, "reference call result leaf is unsupported"))?;
            let mut alternatives = Vec::with_capacity(candidates.len());
            for (source, transfers_existing) in candidates {
                if source_type(module, machine, &source) != Some(expected) {
                    return Err(invalid(
                        machine,
                        "reference result changes its mapped source type",
                    ));
                }
                if !leaf_access_within(module, result.structural_type, &path, source.access) {
                    return Err(invalid(
                        machine,
                        "reference result exceeds its mapped source authority",
                    ));
                }
                let reference = if transfers_existing {
                    let Some((StructuralPathSegment::Referent, carrier_path)) =
                        source.path.split_last()
                    else {
                        return Err(invalid(
                            machine,
                            "owned result mapping has no incoming leaf",
                        ));
                    };
                    let mut reference = live
                        .iter()
                        .find(|reference| {
                            reference.carrier == source.place
                                && reference.carrier_path == carrier_path
                        })
                        .filter(|reference| moved.contains(&reference.identity))
                        .cloned()
                        .ok_or_else(|| {
                            invalid(
                                machine,
                                "returned reference is not among the transferred leaves",
                            )
                        })?;
                    reference.carrier = result.place;
                    reference.carrier_path = path.clone();
                    reference
                } else {
                    let (root, parent) = normalized_source(module, machine, live, &source)?;
                    LiveReference {
                        identity: ReferenceIdentity {
                            place: result.place,
                            path: path.clone(),
                        },
                        carrier: result.place,
                        carrier_path: path.clone(),
                        root,
                        parent,
                    }
                };
                alternatives.push(reference);
            }
            let reference = combine_alternative_references(machine, alternatives)?;
            if roots
                .iter()
                .any(|root| origins_overlap(root, &reference.root))
            {
                return Err(invalid(
                    machine,
                    "reference result aliases its mapped sources",
                ));
            }
            roots.insert(reference.root.clone());
            established.push(reference);
        }
        // Normal completion consumes the incoming owners. Returned carriers
        // preserve their identities and external parents; all other incoming
        // leaves were independently disposed by the callee's checked exits.
        live.retain(|reference| !moved.contains(&reference.identity));
        live.extend(established);
        live.sort_by(|left, right| left.identity.cmp(&right.identity));
    } else {
        live.retain(|reference| !moved.contains(&reference.identity));
    }
    Ok(())
}

fn combine_alternative_references(
    machine: &TerminalMachine,
    alternatives: Vec<LiveReference>,
) -> Result<LiveReference, ModuleError> {
    let mut alternatives = alternatives.into_iter();
    let mut reference = alternatives
        .next()
        .ok_or_else(|| invalid(machine, "reference result has no possible origin"))?;
    let mut paths = Vec::new();
    for alternative in alternatives {
        if reference.identity != alternative.identity
            || reference.carrier != alternative.carrier
            || reference.carrier_path != alternative.carrier_path
            || reference.parent != ReferenceParent::Root(reference.root.clone())
            || alternative.parent != ReferenceParent::Root(alternative.root.clone())
            || origin_root(&reference.root) != origin_root(&alternative.root)
        {
            return Err(invalid(
                machine,
                "reference result alternatives change carrier or ingress custody",
            ));
        }
        let ReferenceOrigin::IngressLeaf(identity) = alternative.root else {
            return Err(invalid(
                machine,
                "reference result alternatives require exact ingress paths",
            ));
        };
        paths.push(identity.path);
    }
    if !paths.is_empty() {
        let ReferenceOrigin::IngressLeaf(identity) = &reference.root else {
            return Err(invalid(
                machine,
                "reference result alternatives require exact ingress paths",
            ));
        };
        paths.push(identity.path.clone());
        paths.sort();
        if paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid(
                machine,
                "reference result alternatives duplicate one origin",
            ));
        }
        reference.root = ReferenceOrigin::Alternatives {
            place: identity.place,
            paths,
        };
        reference.parent = ReferenceParent::Root(reference.root.clone());
    }
    Ok(reference)
}

pub(super) fn transfer_return(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
    live: &mut Vec<LiveReference>,
    observed: &mut Vec<terminal_psi::StructuralReferenceResultSource>,
) -> Result<bool, ModuleError> {
    let Some(signature) = super::structural::result_contracts::source_signature(machine, source)
    else {
        return Ok(false);
    };
    if !contains_reference(module, signature.structural_type) {
        return Ok(false);
    }
    let result = machine
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference return has no declaration"))?;
    if signature.structural_type != result.structural_type
        || live
            .iter()
            .filter(|reference| reference.carrier == source)
            .count()
            != result.reference_sources.len()
    {
        return Err(invalid(
            machine,
            "returned reference leaf roster differs from its declaration",
        ));
    }
    // Interfaces cannot supply origins: compare every actual returned leaf.
    // Only direct ingress custody may escape; moving the carrier into a record
    // must not hide a local root or skip an intermediate live parent.
    for mapping in &result.reference_sources {
        let reference = live
            .iter()
            .find(|reference| reference.carrier == source && reference.carrier_path == mapping.path)
            .ok_or_else(|| invalid(machine, "returned reference carrier is not live"))?;
        let sources = mapping
            .sources
            .iter()
            .filter(|source| {
                formal_origin(machine, source)
                    .is_some_and(|origin| contains_origin(&reference.root, &origin))
            })
            .cloned()
            .collect::<Vec<_>>();
        let origin_count = match &reference.root {
            ReferenceOrigin::Alternatives { paths, .. } => paths.len(),
            _ => 1,
        };
        if sources.len() != origin_count
            || reference.parent != ReferenceParent::Root(reference.root.clone())
            || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
        {
            return Err(invalid(machine, "reference return escapes local custody or differs from its formal source"));
        }
        observed.push(terminal_psi::StructuralReferenceResultSource {
            path: mapping.path.clone(),
            sources,
        });
    }
    // This removes frame-local ownership rows, not the referents or the loans.
    // The caller independently instantiates the checked result source roster.
    live.retain(|reference| reference.carrier != source);
    Ok(true)
}

/// Membership rejects missing candidates at each exit; this union rejects
/// fabricated alternatives that no reachable normal return can produce.
pub(super) fn validate_return_coverage(
    machine: &TerminalMachine,
    observed: &[terminal_psi::StructuralReferenceResultSource],
) -> Result<(), ModuleError> {
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    for mapping in &result.reference_sources {
        if mapping.sources.len() <= 1 {
            continue;
        }
        let sources = observed
            .iter()
            .filter(|returned| returned.path == mapping.path)
            .flat_map(|returned| returned.sources.iter().cloned())
            .collect::<BTreeSet<_>>();
        if sources.into_iter().collect::<Vec<_>>() != mapping.sources {
            return Err(invalid(
                machine,
                "reference result alternatives differ from reachable return origins",
            ));
        }
    }
    Ok(())
}
