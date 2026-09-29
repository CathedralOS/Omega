//! The emitted source type scopes case identity. Equal case spellings in two
//! owners never select each other's tags, and write-only parameters supply no
//! runtime observation even when a proposition already names their active case.

use super::super::{StructuralFieldType, StructuralTypeId, StructuralTypeShape};
use super::{
    CheckedTrees, LoweringError, PlaceId, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
/// A local's observed cases belong to its declared type, not to whichever
/// constructor happened to supply a selected arm.
#[derive(Clone)]
pub(crate) struct LocalCaseBinding {
    pub(super) symbol: symbols::SymbolHandle,
    pub(super) source: PlaceId,
    pub(super) type_identity: String,
    pub(super) cases: Vec<(symbols::SymbolHandle, semantic_vocabulary::StructuralCaseId)>,
}

impl LocalCaseBinding {
    pub(crate) fn new(
        checked: &CheckedTrees,
        symbol: symbols::SymbolHandle,
        reference: checked_trees::types::TypeReferenceHandle,
        source: PlaceId,
        types: &[StructuralTypeDeclaration],
    ) -> Result<Self, LoweringError> {
        let type_identity = checked.normalized_type_identity(reference).into_string();
        let checked_trees::types::TypeReferenceNode::Named { symbol: owner, .. } =
            checked.type_reference_table.type_reference(reference)
        else {
            return unsupported("observed local requires an exact nominal sum");
        };
        let data = checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *owner)
            .ok_or(LoweringError::Unsupported(
                "observed local lost its nominal owner",
            ))?;
        let declaration = types
            .iter()
            .find(|declaration| declaration.identity == type_identity)
            .ok_or(LoweringError::Unsupported(
                "observed local lost its structural type",
            ))?;
        let StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return unsupported("observed local requires a plain sum");
        };
        let mut bindings = Vec::new();
        for member in checked.data_members(data) {
            let checked_trees::data::DataMember::Variant(variant) = member else {
                continue;
            };
            let identity = variant.path_identity();
            let case = cases.iter().find(|case| case.identity == identity).ok_or(
                LoweringError::Unsupported("observed local has a missing declared case"),
            )?;
            bindings.push((variant.symbol, case.id));
        }
        Ok(Self {
            symbol,
            source,
            type_identity,
            cases: bindings,
        })
    }
}

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
    let (sum, path) = walk(binding, binding.structural_type, &subject.path)?;
    Ok((binding.source, path, selected_case(binding, sum, identity)?))
}

/// Resolve a case observation through one runtime-selected element
/// (`self.items[self.w] is Color::Red`): `prefix` walks from the bound
/// parameter to a fixed array and `element_path` walks the selected element to
/// the sum. Returns the source, the two lowered halves the caller places the
/// `RuntimeIndex` segment between, and the selected case. The element's
/// bound is not checked here: the observation owns that obligation and the
/// verifier reconstructs it against the array's declared extent.
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
    let (array, prefix) = walk(binding, binding.structural_type, prefix)?;
    let StructuralTypeShape::FixedArray { element, .. } = &declaration(binding, array)?.shape
    else {
        return unsupported("runtime case observation does not select in a fixed array");
    };
    let (sum, element_path) = walk(binding, *element, element_path)?;
    let case = selected_case(binding, sum, identity)?;
    Ok((binding.source, prefix, element_path, case))
}

fn declaration(
    binding: &StructuralCaseBinding,
    structural_type: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut declarations = binding
        .declarations
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
    binding: &StructuralCaseBinding,
    mut structural_type: StructuralTypeId,
    steps: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<(StructuralTypeId, Vec<terminal_psi::StructuralPathSegment>), LoweringError> {
    let mut path = Vec::with_capacity(steps.len());
    for segment in steps {
        match (segment, &declaration(binding, structural_type)?.shape) {
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
    binding: &StructuralCaseBinding,
    structural_type: StructuralTypeId,
    identity: &str,
) -> Result<semantic_vocabulary::StructuralCaseId, LoweringError> {
    let cases = match &declaration(binding, structural_type)?.shape {
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
