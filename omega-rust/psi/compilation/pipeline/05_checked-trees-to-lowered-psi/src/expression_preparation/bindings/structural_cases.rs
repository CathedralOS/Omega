//! The emitted source type scopes case identity. Equal case spellings in two
//! owners never select each other's tags, and write-only parameters supply no
//! runtime observation even when a proposition already names their active case.

use super::super::{StructuralTypeId, StructuralTypeShape};
use super::{
    LoweringError, PlaceId, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
use crate::structural_types::unique_declaration;

#[derive(Clone)]
pub(crate) struct StructuralCaseBinding {
    source_position: u32,
    source: PlaceId,
    structural_type: StructuralTypeId,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
}

impl StructuralCaseBinding {
    pub(crate) fn collect(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> Vec<Self> {
        let declarations: std::sync::Arc<[StructuralTypeDeclaration]> = types.into();
        let mut bindings = Vec::new();
        for (position, parameter) in parameters {
            if parameter.access == StructuralAccess::WriteOnlyBorrow {
                continue;
            }
            bindings.push(Self {
                source_position: *position,
                source: parameter.place,
                structural_type: parameter.structural_type,
                declarations: std::sync::Arc::clone(&declarations),
            });
        }
        bindings
    }
}

/// Resolve an executable observation at an established readable place. Its
/// captured address path may cross reference descriptors; that is not a
/// predicate's canonical owned-place path. Selectors still require their real
/// emitting operand frame, and source bindings retain live custody.
pub(crate) fn resolve_at(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedUnitStructuralPathSegment],
    identity: &checked_trees::CheckedMemberIdentity,
) -> Result<
    (
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralCaseId,
    ),
    LoweringError,
> {
    let prepared = super::structural_paths::prepare_argument_path(
        identities,
        steps,
        structural_type,
        declarations,
    )?;
    let path = super::structural_paths::emit_argument_path(
        &prepared,
        None,
        &mut std::iter::empty(),
        None,
    )?;
    for (position, segment) in path.iter().enumerate() {
        if *segment != terminal_psi::StructuralPathSegment::Referent {
            continue;
        }
        let reference = terminal_semantics::runtime_structural_path_tip(
            declarations.iter(),
            structural_type,
            &path[..position],
        )
        .ok_or(LoweringError::Unsupported(
            "case observation lost its exact reference carrier",
        ))?;
        if !matches!(
            reference.shape,
            StructuralTypeShape::Reference {
                access: terminal_psi::reading_borrow_pair_pattern!(),
                ..
            }
        ) {
            return unsupported("case observation requires a readable reference crossing");
        }
    }
    let sum = terminal_semantics::runtime_structural_path_tip(
        declarations.iter(),
        structural_type,
        &path,
    )
    .ok_or(LoweringError::Unsupported(
        "case observation has no exact declared carrier",
    ))?;
    Ok((
        path,
        selected_case(identities, declarations, sum.id, identity)?,
    ))
}

pub(crate) fn resolve(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    bindings: &[StructuralCaseBinding],
    subject: &checked_trees::CheckedStructuralParameterField,
    identity: &checked_trees::CheckedMemberIdentity,
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralCaseId,
    ),
    LoweringError,
> {
    let mut matching = bindings
        .iter()
        .filter(|binding| binding.source_position == subject.parameter_position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "case observation has no exact readable source and case binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("case observation has ambiguous source bindings");
    }
    let (sum, path) = walk(
        identities,
        &binding.declarations,
        binding.structural_type,
        &subject.path,
    )?;
    Ok((
        binding.source,
        path,
        selected_case(identities, &binding.declarations, sum, identity)?,
    ))
}

/// Resolve a case observation through one runtime-selected element
/// (`context.items[position] in Color::Red`): `prefix` walks from the bound
/// parameter to a fixed array or whole element view, and `element_path` walks
/// the selected element to the sum. Returns the source, the two lowered halves
/// surrounding `RuntimeIndex`, the selected case, and whether the exact carrier
/// needs a live element-view length observation. The element's
/// bound is not checked here: the observation owns that obligation and the
/// verifier reconstructs it against the array's declared extent or the exact
/// view's dominating live-length observation. A nested view needs a separately
/// established descriptor root before its length can be observed.
pub(crate) fn resolve_runtime_element(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    source: PlaceId,
    prefix: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    identity: &checked_trees::CheckedMemberIdentity,
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralCaseId,
        bool,
    ),
    LoweringError,
> {
    let (array, prefix) = walk(identities, declarations, structural_type, prefix)?;
    let (element, observes_element_view_length) = match &unique_declaration(
        declarations,
        array,
        "case observation lost its exact carrier type",
        "case observation has ambiguous carrier declarations",
    )?
    .shape
    {
        StructuralTypeShape::FixedArray { element, .. } => (*element, false),
        StructuralTypeShape::ElementView { element, .. } if prefix.is_empty() => (*element, true),
        _ => {
            return unsupported("runtime case observation requires an array or whole element view");
        }
    };
    let (sum, element_path) = walk(identities, declarations, element, element_path)?;
    let case = selected_case(identities, declarations, sum, identity)?;
    Ok((
        source,
        prefix,
        element_path,
        case,
        observes_element_view_length,
    ))
}

/// Walk a proof predicate's owned-place coordinates. Reference crossings and
/// evaluated selectors belong to executable address preparation, not this walk.
fn walk(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<(StructuralTypeId, Vec<terminal_psi::StructuralPathSegment>), LoweringError> {
    let mut path = Vec::with_capacity(steps.len());
    // A `Case` step selects the namespace of the payload field that follows;
    // otherwise a field is a record's or a mixed carrier's common field.
    let mut case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    for segment in steps {
        let shape = &unique_declaration(
            declarations,
            structural_type,
            "case observation lost its exact carrier type",
            "case observation has ambiguous carrier declarations",
        )?
        .shape;
        match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                let fields = case_fields.take().or_else(|| shape.direct_fields()).ok_or(
                    LoweringError::Unsupported("case observation field left its record namespace"),
                )?;
                let mut selected = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let field = selected.next().ok_or(LoweringError::Unsupported(
                    "case observation lost its carrier field",
                ))?;
                if selected.next().is_some() || field.relevance.is_erased() {
                    return unsupported(
                        "case observation has an erased or ambiguous carrier field",
                    );
                }
                let Some(child) = field.field_type.as_structural() else {
                    return unsupported("case observation requires a structural carrier field");
                };
                structural_type = child;
                path.push(terminal_psi::StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                ));
            }
            checked_trees::CheckedStructuralPredicatePathSegment::Case(identity)
                if case_fields.is_none() =>
            {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    shape
                else {
                    return unsupported("case observation case step left its sum namespace");
                };
                let mut selected = cases
                    .iter()
                    .filter(|case| case.identity == identities.intern(&identity.spelling()));
                let (Some(case), None) = (selected.next(), selected.next()) else {
                    return unsupported("case observation case step names no single case");
                };
                case_fields = Some(case.fields.as_slice());
                path.push(terminal_psi::StructuralPathSegment::Case(case.id));
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index)
                if case_fields.is_none() =>
            {
                let StructuralTypeShape::FixedArray { element, length } = shape else {
                    return unsupported("case observation index left its array namespace");
                };
                if element_index >= length {
                    return unsupported("case observation index is outside its array");
                }
                structural_type = *element;
                path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                    *element_index,
                ));
            }
            _ => {
                return unsupported(
                    "case observation requires fields, case payload fields and fixed indices",
                );
            }
        }
    }
    if case_fields.is_some() {
        return unsupported("case observation ends on a case step without its payload field");
    }
    Ok((structural_type, path))
}

fn selected_case(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    identity: &checked_trees::CheckedMemberIdentity,
) -> Result<semantic_vocabulary::StructuralCaseId, LoweringError> {
    let cases = match &unique_declaration(
        declarations,
        structural_type,
        "case observation lost its exact carrier type",
        "case observation has ambiguous carrier declarations",
    )?
    .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return unsupported("case observation requires an exact selected sum"),
    };
    let mut selected = cases
        .iter()
        .filter(|case| case.identity == identities.intern(&identity.spelling()));
    let case = selected.next().ok_or(LoweringError::Unsupported(
        "case observation lost its exact selected case",
    ))?;
    if selected.next().is_some() {
        return unsupported("case observation has ambiguous selected cases");
    }
    Ok(case.id)
}

/// The declared cases of the whole-root sum `source` binds, in declaration
/// order. The binding's declarations are retained on the binding itself, so a
/// dispatch built from them never reaches for the module's type catalog.
pub(crate) fn sum_cases(
    bindings: &[StructuralCaseBinding],
    source: PlaceId,
) -> Option<&[terminal_psi::StructuralCaseDeclaration]> {
    let binding = bindings.iter().find(|binding| binding.source == source)?;
    let declaration =
        terminal_psi::find_structural_type(&binding.declarations, binding.structural_type)?;
    match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            Some(cases.as_slice())
        }
        _ => None,
    }
}
