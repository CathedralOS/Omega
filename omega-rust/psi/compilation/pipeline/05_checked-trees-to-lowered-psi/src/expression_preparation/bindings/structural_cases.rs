//! The emitted source type scopes case identity. Equal case spellings in two
//! owners never select each other's tags, and write-only parameters supply no
//! runtime observation even when a proposition already names their active case.

use super::super::{StructuralFieldType, StructuralTypeId, StructuralTypeShape};
use super::{
    LoweringError, PlaceId, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};

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

/// Resolve static case observation from any established readable place.
/// The source binding owns custody; this walk owns declared field, extent and
/// case identity. Locals do not need a separately reconstructed case catalog.
pub(crate) fn resolve_at(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedUnitStructuralPathSegment],
    identity: &str,
) -> Result<
    (
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralCaseId,
    ),
    LoweringError,
> {
    let steps = steps
        .iter()
        .map(|step| match step {
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => Ok(
                checked_trees::CheckedStructuralPredicatePathSegment::Field(identity.clone()),
            ),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                Ok(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(*index))
            }
            _ => unsupported("static case observation requires record fields and fixed indices"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (sum, path) = walk(declarations, structural_type, &steps)?;
    Ok((path, selected_case(declarations, sum, identity)?))
}

pub(crate) fn resolve(
    bindings: &[StructuralCaseBinding],
    subject: &checked_trees::CheckedStructuralParameterField,
    identity: &str,
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
        &binding.declarations,
        binding.structural_type,
        &subject.path,
    )?;
    Ok((
        binding.source,
        path,
        selected_case(&binding.declarations, sum, identity)?,
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
    bindings: &[StructuralCaseBinding],
    parameter_position: u32,
    prefix: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    identity: &str,
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
    let mut matching = bindings
        .iter()
        .filter(|binding| binding.source_position == parameter_position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "case observation has no exact readable source and case binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("case observation has ambiguous source bindings");
    }
    let (array, prefix) = walk(&binding.declarations, binding.structural_type, prefix)?;
    let (element, observes_element_view_length) = match &declaration(&binding.declarations, array)?
        .shape
    {
        StructuralTypeShape::FixedArray { element, .. } => (*element, false),
        StructuralTypeShape::ElementView { element } if prefix.is_empty() => (*element, true),
        _ => {
            return unsupported("runtime case observation requires an array or whole element view");
        }
    };
    let (sum, element_path) = walk(&binding.declarations, element, element_path)?;
    let case = selected_case(&binding.declarations, sum, identity)?;
    Ok((
        binding.source,
        prefix,
        element_path,
        case,
        observes_element_view_length,
    ))
}

fn declaration(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut declarations = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let selected = declarations.next().ok_or(LoweringError::Unsupported(
        "case observation lost its exact carrier type",
    ))?;
    if declarations.next().is_some() {
        return unsupported("case observation has ambiguous carrier declarations");
    }
    Ok(selected)
}

/// Walk relevant structural record fields and in-range literal indexes from
/// `structural_type`, lowering each step.
fn walk(
    declarations: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<(StructuralTypeId, Vec<terminal_psi::StructuralPathSegment>), LoweringError> {
    let mut path = Vec::with_capacity(steps.len());
    for segment in steps {
        match (segment, &declaration(declarations, structural_type)?.shape) {
            (
                checked_trees::CheckedStructuralPredicatePathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut selected = fields.iter().filter(|field| field.identity == *identity);
                let field = selected.next().ok_or(LoweringError::Unsupported(
                    "case observation lost its carrier field",
                ))?;
                if selected.next().is_some() || field.relevance.is_erased() {
                    return unsupported(
                        "case observation has an erased or ambiguous carrier field",
                    );
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return unsupported("case observation requires a structural carrier field");
                };
                structural_type = child;
                path.push(terminal_psi::StructuralPathSegment::Field(identity.clone()));
            }
            (
                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index),
                StructuralTypeShape::FixedArray { element, length },
            ) if element_index < length => {
                structural_type = *element;
                path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                    *element_index,
                ));
            }
            _ => {
                return unsupported(
                    "case observation requires relevant record fields and in-range fixed indices",
                );
            }
        }
    }
    Ok((structural_type, path))
}

fn selected_case(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    identity: &str,
) -> Result<semantic_vocabulary::StructuralCaseId, LoweringError> {
    let cases = match &declaration(declarations, structural_type)?.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return unsupported("case observation requires an exact selected sum"),
    };
    let mut selected = cases.iter().filter(|case| case.identity == identity);
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
    let declaration = binding
        .declarations
        .iter()
        .find(|declaration| declaration.id == binding.structural_type)?;
    match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            Some(cases.as_slice())
        }
        _ => None,
    }
}
