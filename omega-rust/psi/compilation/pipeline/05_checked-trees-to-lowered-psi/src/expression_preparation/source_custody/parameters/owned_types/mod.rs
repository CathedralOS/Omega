//! Reconstruct graph catalog shapes from their exact typed declarations.
//!
//! Plain ownership does not imply an unrestricted payload. Numeric constraints
//! carry no cleanup, but their admitted values remain part of field identity.
//! Reconstruct those restrictions from the original type and substitutions before
//! comparing catalogs; agreement between retained catalogs cannot replace this.
//! Borrowed referents use the same reconstruction as owned parameters:
//! borrowing changes access, not the referent's fields or numeric restrictions.
//! A borrowed provider field retains its exact Binding carrier and selected
//! Fused receipt. That loan never grants the owned path permission to copy,
//! erase, or establish a service value.

use checked_trees::TypeIdentityRequest;
use checked_trees::data::{DataDefinition, DataField, DataMember};
use checked_trees::state::State;
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralCasePlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape,
};
use symbols::SymbolHandle;

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

mod scalar_fields;
#[cfg(test)]
mod tests;

pub(super) fn validate_referent_type(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
    retained_identity: &str,
) -> Result<(), LoweringError> {
    let mut source = SourceTypes {
        checked,
        borrowed_referent: true,
        active: Vec::new(),
        complete: Vec::new(),
    };
    if source.validate_type(reference, &[])? != retained_identity {
        return unsupported("scalar graph referent type identity differs from its source");
    }
    Ok(())
}

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
        borrowed_referent: false,
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
    borrowed_referent: bool,
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
        // Element-view fields retain their slice carrier, not an owned
        // reference shell. Reconstruct that carrier from the authored type
        // before looking it up; the element declaration is replayed below too.
        // The current stored descriptor has shared custody. A mutable shell
        // cannot silently become the same access-free slice shape.
        let element_view = validation::slice_view_shell(&checked.typed, reference, substitutions)
            .filter(|view| {
                checked.primitive_type_reference(view.element)
                    != Some(checked_trees::types::PrimitiveType::U8)
            });
        if element_view
            .is_some_and(|view| view.access != language_semantics::ReferenceAccess::Shared)
        {
            return unsupported("stored element-view type requires shared descriptor custody");
        }
        let identity = checked
            .type_identity(TypeIdentityRequest {
                substitutions,
                ..TypeIdentityRequest::ordinary(element_view.map_or(reference, |view| view.slice))
            })
            .into_string();
        if self.complete.contains(&identity) {
            return Ok(identity);
        }
        if self.active.contains(&identity) {
            return unsupported("owned type custody contains a recursive source shape");
        }
        // A borrowed referent's declaration is the scalar graph's business:
        // the graph retains the shapes it reads. An owned local may instead
        // enroll its declaration in the unit roster — a memberwise
        // establishment describes storage no scalar value names — so owned
        // custody joins both catalogs. Each lane still admits one row per
        // identity; a row that repeats across lanes must agree on the plan.
        let catalogs: [&[_]; 2] = [
            &checked.facts.flow.terminal_scalar_graphs.structural_types,
            &checked.facts.flow.terminal_unit_effects.structural_types,
        ];
        let mut retained = None;
        for catalog in catalogs[..usize::from(!self.borrowed_referent) + 1].iter() {
            let mut candidates = catalog.iter().filter(|plan| plan.identity == identity);
            let lane = candidates.next();
            if candidates.next().is_some() {
                return unsupported("owned type custody has duplicate structural declarations");
            }
            match (retained, lane) {
                (Some(existing), Some(candidate)) if existing != candidate => {
                    return unsupported("owned type custody has duplicate structural declarations");
                }
                (None, next) => retained = next,
                _ => {}
            }
        }
        let retained = retained.ok_or(LoweringError::Unsupported(
            "owned type custody lost a source structural declaration",
        ))?;
        self.active.push(identity.clone());
        let expected = if let Some(view) = element_view {
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity: self.validate_type(view.element, substitutions)?,
            }
        } else if let Some(primitive) = checked.primitive_type_reference(reference) {
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
                TypeReferenceNode::Reference { access, .. } => {
                    // A stored-loan leaf keeps its reference shell as a
                    // `Reference` shape over the peeled referee -- the same
                    // plan `add_view_type` registers for it. An exclusive
                    // shell lends whatever it borrows; a shared shell lends
                    // only a named referent, and slice views register
                    // through their own carriers, so neither arrives here as
                    // a record leaf. The referee replays under no
                    // substitutions, exactly as the collector registers it.
                    let (referee, _) = match access {
                        language_semantics::ReferenceAccess::Mutable => {
                            validation::reference_result_custody::mutable_borrowed_parts(
                                &checked.typed,
                                reference,
                            )
                        }
                        language_semantics::ReferenceAccess::Shared => {
                            validation::reference_result_custody::shared_borrowed_parts(
                                &checked.typed,
                                reference,
                            )
                        }
                        language_semantics::ReferenceAccess::WriteOnly => None,
                    }
                    .ok_or(LoweringError::Unsupported(
                        "owned source type has no admitted structural shape",
                    ))?;
                    CheckedUnitStructuralTypeShape::Reference {
                        referent_identity: self.validate_type(referee, &[])?,
                        access: (*access).into(),
                    }
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
                        identity: variant.path_identity(),
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
                    .type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(field.type_reference)
                    })
                    .into_string(),
            }
        } else if let Some(provider) = self.provider_field(field.type_reference, substitutions)? {
            provider
        } else if let Some(carrier) =
            validation::byte_sequence_carrier(&self.checked.typed, reference, substitutions)
        {
            // A byte member carries its own view or bounded storage shape; the
            // retained catalog records it under the same ByteSequence family
            // the type collector writes.
            let declared = typed_trees::byte_predicates::type_reference_domain_predicates(
                &self.checked.typed,
                reference,
            );
            let mut predicates =
                language_semantics::byte_predicates::ByteSequencePredicateSet::EMPTY;
            for (_, predicate) in declared {
                let Some(predicate) = predicate else {
                    return unsupported("owned byte field has an unrecognized domain predicate");
                };
                predicates.insert(predicate);
            }
            if predicates.is_empty() {
                CheckedUnitStructuralFieldType::ByteSequence(carrier)
            } else {
                CheckedUnitStructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates,
                }
            }
        } else if let Some(primitive) = self.checked.primitive_type_reference(reference) {
            scalar_fields::reconstruct(self.checked, field.type_reference, substitutions, primitive)
                .ok_or(LoweringError::Unsupported(
                    "owned scalar field has unsupported source restrictions",
                ))?
        } else {
            CheckedUnitStructuralFieldType::Structural {
                type_identity: self.validate_type(field.type_reference, substitutions)?,
            }
        };
        Ok(CheckedUnitStructuralFieldPlan {
            identity: field.path_identity(),
            relevance: field.relevance,
            field_type,
        })
    }

    fn provider_field(
        &self,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<Option<CheckedUnitStructuralFieldType>, LoweringError> {
        if !self.borrowed_referent {
            return Ok(None);
        }
        // Substitute a bare formal without unwrapping constraints: the exact
        // Binding carrier rejects authored qualifications on its source shell.
        let mut active = Vec::new();
        loop {
            if !self
                .checked
                .type_reference_table
                .contains_type_reference(reference)
                || active.contains(&reference)
            {
                return unsupported("borrowed provider field has a stale or cyclic type reference");
            }
            active.push(reference);
            let TypeReferenceNode::Named { symbol, .. } =
                self.checked.type_reference_table.type_reference(reference)
            else {
                break;
            };
            let Some((_, replacement)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            else {
                break;
            };
            reference = *replacement;
        }
        let carrier = typed_trees::service::classify_exact_bound_service_carrier(
            &self.checked.typed,
            reference,
        )
        .map_err(|_| {
            LoweringError::Unsupported("borrowed provider field has an invalid Binding carrier")
        })?;
        let Some(carrier) = carrier else {
            return Ok(None);
        };
        if self
            .checked
            .fused_service_erasure(carrier.requirement)
            .is_none()
        {
            return unsupported("borrowed provider field lacks its exact Fused authorization");
        }
        // This reconstructs the borrowed field's existing erased shape. It
        // grants no provisioning or transfer authority to the ordinary call.
        Ok(Some(CheckedUnitStructuralFieldType::FusedServiceBacked {
            provider_type_identity: self
                .checked
                .type_identity(TypeIdentityRequest {
                    substitutions,
                    ..TypeIdentityRequest::ordinary(reference)
                })
                .into_string(),
            erasure: checked_trees::CheckedFusedServiceErasureReceipt {
                requirement: carrier.requirement,
            },
        }))
    }
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
