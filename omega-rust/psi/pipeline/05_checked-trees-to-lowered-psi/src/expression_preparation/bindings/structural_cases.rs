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
    let declaration = |structural_type| {
        let mut declarations = binding
            .declarations
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let selected = declarations.next()?;
        declarations.next().is_none().then_some(selected)
    };
    let mut structural_type = binding.structural_type;
    let mut path = Vec::with_capacity(subject.path.len());
    for segment in &subject.path {
        let shape = &declaration(structural_type)
            .ok_or(LoweringError::Unsupported(
                "case observation lost its exact carrier type",
            ))?
            .shape;
        match (segment, shape) {
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
    let shape = &declaration(structural_type)
        .ok_or(LoweringError::Unsupported(
            "case observation lost its selected sum",
        ))?
        .shape;
    let cases = match shape {
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
    Ok((binding.source, path, case.id))
}
