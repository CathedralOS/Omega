//! Reconstruct owned graph catalog shapes from their exact typed declarations.

use checked_trees::data::{DataDefinition, DataField, DataMember};
use checked_trees::state::State;
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralCasePlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape,
};
use symbols::SymbolHandle;

use crate::{LoweringError, unsupported};

/// Signature admission owns plain-content and no-code eligibility. Catalog
/// correspondence must still be reconstructed: agreement among retained
/// catalogs cannot authorize an altered field, case, or array declaration.
pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    let mut source = SourceTypes {
        checked,
        active: Vec::new(),
        complete: Vec::new(),
    };
    for parameter in parameters
        .iter()
        .filter(|parameter| parameter.access == CheckedStructuralAccess::Owned)
    {
        let declaration = checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "owned type custody has no authored parameter",
            ))?;
        if source.validate_type(declaration.type_reference, &[])? != parameter.type_identity {
            return unsupported("owned type custody differs from its authored parameter type");
        }
    }
    Ok(())
}

struct SourceTypes<'checked> {
    checked: &'checked CheckedTrees,
    active: Vec<String>,
    complete: Vec<String>,
}

impl SourceTypes<'_> {
    fn validate_type(
        &mut self,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<String, LoweringError> {
        let checked = self.checked;
        let reference = resolve(checked, reference, substitutions)?;
        let identity = checked
            .normalized_type_identity_with_binders_and_substitutions(reference, &[], substitutions)
            .into_string();
        if self.complete.contains(&identity) {
            return Ok(identity);
        }
        if self.active.contains(&identity) {
            return unsupported("owned type custody contains a recursive source shape");
        }
        let mut candidates = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity);
        let retained = candidates.next().ok_or(LoweringError::Unsupported(
            "owned type custody lost a source structural declaration",
        ))?;
        if candidates.next().is_some() {
            return unsupported("owned type custody has duplicate structural declarations");
        }
        self.active.push(identity.clone());
        let expected = if let Some(primitive) = checked.primitive_type_reference(reference) {
            CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
        } else {
            match checked.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } => CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity: self.validate_type(*element_type, substitutions)?,
                    length: u64::try_from(*length).map_err(|_| {
                        LoweringError::Unsupported("owned source array length exceeds u64")
                    })?,
                },
                TypeReferenceNode::Named { symbol, .. } => {
                    let data = definition(checked, *symbol)?;
                    if !checked.data_type_parameters(data).is_empty() {
                        return unsupported("owned source type has unbound generic parameters");
                    }
                    self.data_shape(data, substitutions)?
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    let data = definition(checked, *base_symbol)?;
                    let parameters = checked.data_type_parameters(data);
                    let argument_count = arguments.count() as usize;
                    let arguments = checked
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    if parameters.len() != arguments.len() || arguments.len() != argument_count {
                        return unsupported(
                            "owned source generic arguments differ from their declaration",
                        );
                    }
                    let mut local_substitutions = substitutions.to_vec();
                    local_substitutions.extend(
                        parameters
                            .iter()
                            .zip(arguments)
                            .map(|(parameter, argument)| (parameter.symbol, *argument)),
                    );
                    self.data_shape(data, &local_substitutions)?
                }
                _ => return unsupported("owned source type has no admitted structural shape"),
            }
        };
        if retained.shape != expected {
            return unsupported("owned structural catalog differs from its typed declaration");
        }
        self.active.pop();
        self.complete.push(identity.clone());
        Ok(identity)
    }

    fn data_shape(
        &mut self,
        data: &DataDefinition,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<CheckedUnitStructuralTypeShape, LoweringError> {
        let checked = self.checked;
        let mut fields = Vec::new();
        let mut cases = Vec::new();
        for member in checked.data_members(data) {
            match member {
                DataMember::Field(field) => fields.push(self.field(field, substitutions)?),
                DataMember::Variant(variant) => {
                    let mut payload = Vec::new();
                    let source_fields = checked.data_payload_fields(variant);
                    if source_fields.len() != variant.payload.count() as usize {
                        return unsupported("owned type custody has a stale source case payload");
                    }
                    for field in source_fields {
                        payload.push(self.field(field, substitutions)?);
                    }
                    cases.push(CheckedUnitStructuralCasePlan {
                        identity: declaration_identity(variant.identity, variant.name.as_str()),
                        fields: payload,
                    });
                }
            }
        }
        Ok(if cases.is_empty() {
            CheckedUnitStructuralTypeShape::Record { fields }
        } else if fields.is_empty() {
            CheckedUnitStructuralTypeShape::Sum { cases }
        } else {
            CheckedUnitStructuralTypeShape::Mixed { fields, cases }
        })
    }

    fn field(
        &mut self,
        field: &DataField,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<CheckedUnitStructuralFieldPlan, LoweringError> {
        let reference = resolve(self.checked, field.type_reference, substitutions)?;
        let field_type = if field.relevance.is_erased() {
            CheckedUnitStructuralFieldType::Erased {
                type_identity: self
                    .checked
                    .normalized_type_identity_with_binders_and_substitutions(
                        field.type_reference,
                        &[],
                        substitutions,
                    )
                    .into_string(),
            }
        } else if let Some(primitive) = self.checked.primitive_type_reference(reference) {
            CheckedUnitStructuralFieldType::Scalar(primitive)
        } else {
            CheckedUnitStructuralFieldType::Structural {
                type_identity: self.validate_type(field.type_reference, substitutions)?,
            }
        };
        Ok(CheckedUnitStructuralFieldPlan {
            identity: declaration_identity(field.identity, field.name.as_str()),
            relevance: field.relevance,
            field_type,
        })
    }
}

fn declaration_identity(identity: Option<u64>, name: &str) -> String {
    identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| name.to_owned())
}

fn definition(
    checked: &CheckedTrees,
    symbol: SymbolHandle,
) -> Result<&DataDefinition, LoweringError> {
    let mut definitions = checked
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == symbol && symbol.is_valid());
    let data = definitions.next().ok_or(LoweringError::Unsupported(
        "owned type custody has no exact source data declaration",
    ))?;
    if definitions.next().is_some() {
        return unsupported("owned type custody has duplicate source data declarations");
    }
    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || checked.data_members(data).len() != data.members.count() as usize
        || checked.data_type_parameters(data).len() != data.type_parameters.count() as usize
    {
        return unsupported("owned type custody requires an exact checked source shape");
    }
    Ok(data)
}

fn resolve(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Result<TypeReferenceHandle, LoweringError> {
    let mut active = Vec::new();
    loop {
        if !checked
            .type_reference_table
            .contains_type_reference(reference)
            || active.contains(&reference)
        {
            return unsupported("owned type custody has a stale or cyclic type reference");
        }
        active.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    return Ok(reference);
                };
                reference = *replacement;
            }
            _ => return Ok(reference),
        }
    }
}
