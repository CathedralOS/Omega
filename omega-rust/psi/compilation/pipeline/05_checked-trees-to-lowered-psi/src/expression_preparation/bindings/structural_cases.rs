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

/// Resolve the exact emitted path without following an endpoint descriptor.
/// Cleanup and custody selection use this strict geometry: owning a reference
/// carrier never means owning the sum behind it.
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
    let path = prepare_case_path(identities, declarations, structural_type, steps)?;
    resolve_emitted_path(identities, declarations, structural_type, path, identity)
}

/// A tag read observes the saved value's readable referent, not its descriptor.
/// Complete only this observation's path; the captured subject remains intact
/// for wildcard arms and descriptor transport. The physical endpoint scopes
/// the case namespace, independently of the source expression's presentation.
pub(crate) fn resolve_observation_at(
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
    let mut path = prepare_case_path(identities, declarations, structural_type, steps)?;
    super::structural_paths::complete_read_observation_path(
        structural_type,
        &mut path,
        declarations,
    )?;
    resolve_emitted_path(identities, declarations, structural_type, path, identity)
}

fn prepare_case_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
    let prepared = super::structural_paths::prepare_argument_path(
        identities,
        steps,
        structural_type,
        declarations,
    )?;
    super::structural_paths::emit_argument_path(&prepared, None, &mut std::iter::empty(), None)
}

fn resolve_emitted_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    identity: &checked_trees::CheckedMemberIdentity,
) -> Result<
    (
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralCaseId,
    ),
    LoweringError,
> {
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
    let (sum, path) = prepare_observation_path(
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
    let (array, prefix) =
        prepare_observation_path(identities, declarations, structural_type, prefix)?;
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
    let (sum, element_path) =
        prepare_observation_path(identities, declarations, element, element_path)?;
    let case = selected_case(identities, declarations, sum, identity)?;
    Ok((
        source,
        prefix,
        element_path,
        case,
        observes_element_view_length,
    ))
}

/// An executable tag observation resolves the same declared geometry as a
/// scalar read. Checked predicates retain each exact reference crossing; the
/// shared observation preparation consumes them without inferring or erasing
/// pointer loads. This path establishes no case authority.
fn prepare_observation_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<(StructuralTypeId, Vec<terminal_psi::StructuralPathSegment>), LoweringError> {
    let observation = crate::emission::structural_observations::explicit_path(
        identities,
        structural_type,
        steps,
        declarations,
    )?;
    let selected = observation.declaration.ok_or(LoweringError::Unsupported(
        "case observation requires a structural carrier endpoint",
    ))?;
    let prepared = super::structural_paths::prepare_argument_path(
        identities,
        &observation.segments,
        structural_type,
        declarations,
    )?;
    let path = super::structural_paths::emit_argument_path(
        &prepared,
        None,
        &mut std::iter::empty(),
        None,
    )?;
    Ok((selected.id, path))
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
