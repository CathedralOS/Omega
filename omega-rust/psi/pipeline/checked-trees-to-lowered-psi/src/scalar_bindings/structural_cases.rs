//! The emitted source type scopes case identity. Equal case spellings in two
//! owners never select each other's tags, and write-only parameters supply no
//! runtime observation even when a proposition already names their active case.

use super::*;

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
            let identity = variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned());
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
