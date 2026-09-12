//! The emitted source type scopes case identity. Equal case spellings in two
//! owners never select each other's tags, and write-only parameters supply no
//! runtime observation even when a proposition already names their active case.

use super::*;

#[derive(Clone)]
pub(crate) struct StructuralCaseBinding {
    source_position: u32,
    identity: String,
    source: PlaceId,
    case: semantic_vocabulary::StructuralCaseId,
}

impl StructuralCaseBinding {
    pub(crate) fn collect(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> Vec<Self> {
        let mut bindings = Vec::new();
        for (position, parameter) in parameters {
            if parameter.access == StructuralAccess::WriteOnlyBorrow {
                continue;
            }
            let Some(declaration) = types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
            else {
                continue;
            };
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => continue,
            };
            bindings.extend(cases.iter().map(|case| Self {
                source_position: *position,
                identity: case.identity.clone(),
                source: parameter.place,
                case: case.id,
            }));
        }
        bindings
    }
}

pub(crate) fn resolve(
    bindings: &[StructuralCaseBinding],
    subject: &checked_trees::CheckedStructuralParameterField,
    identity: &str,
) -> Result<(PlaceId, semantic_vocabulary::StructuralCaseId), LoweringError> {
    if !subject.path.is_empty() {
        return unsupported("case observation requires an established whole sum referent");
    }
    let mut matching = bindings.iter().filter(|binding| {
        binding.source_position == subject.parameter_position && binding.identity == identity
    });
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "case observation has no exact readable source and case binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("case observation has ambiguous source bindings");
    }
    Ok((binding.source, binding.case))
}
