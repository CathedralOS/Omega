//! Shared structural-type retention for structural result and control lanes.

use super::{
    BTreeSet, ByteSequenceCarrier, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, IeeeFloatFormat, LoweringError, PrimitiveType,
    StructuralAccess, StructuralCaseDeclaration, StructuralCaseId, StructuralFieldDeclaration,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TerminalModule, allocate_dense, dense_identity, lookup_type_id, structural_field_id,
    structural_type_id, terminal_scalar_type, unsupported,
};
use crate::expression_preparation::bindings::structural_paths::terminal_access;

fn reference_access(
    access: checked_trees::CheckedStructuralAccess,
) -> Result<StructuralAccess, LoweringError> {
    match access {
        checked_trees::CheckedStructuralAccess::MutableBorrow => {
            Ok(StructuralAccess::MutableBorrow)
        }
        _ => unsupported("stored reference access is outside retained mutable custody"),
    }
}

pub(crate) fn terminal_structural_field_type(
    primitive: PrimitiveType,
) -> Result<StructuralFieldType, LoweringError> {
    Ok(match primitive {
        PrimitiveType::F32 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
        PrimitiveType::F64 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
        primitive => StructuralFieldType::Scalar(terminal_scalar_type(primitive)?),
    })
}

pub(crate) fn terminal_byte_sequence_carrier(
    carrier: checked_trees::CheckedByteSequenceCarrier,
) -> ByteSequenceCarrier {
    match carrier {
        // The access crosses with the carrier. Collapsing it here would hand
        // Terminal a shared view for every authored `&mut [u8]`, which is what
        // left the verifier unable to check a byte store's authority.
        checked_trees::CheckedByteSequenceCarrier::BorrowedView { access } => {
            ByteSequenceCarrier::BorrowedView {
                access: access.map(terminal_access),
            }
        }
        checked_trees::CheckedByteSequenceCarrier::BoundedView { access, capacity } => {
            ByteSequenceCarrier::BoundedView {
                access: access.map(terminal_access),
                capacity,
            }
        }
        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } => {
            ByteSequenceCarrier::BoundedOwned { capacity }
        }
    }
}

/// Preserve one field contract across records, case payloads and retained types.
/// Atomic access authority belongs to the declaration, never to a caller shape.
pub(crate) fn lower_field_type(
    field_type: &CheckedUnitStructuralFieldType,
    type_ids: &[(String, StructuralTypeId)],
) -> Result<StructuralFieldType, LoweringError> {
    Ok(match field_type {
        CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
            StructuralFieldType::BoundedInteger(*integer)
        }
        CheckedUnitStructuralFieldType::Atomic(primitive) => {
            StructuralFieldType::Atomic(terminal_scalar_type(*primitive)?)
        }
        CheckedUnitStructuralFieldType::Scalar(primitive) => {
            terminal_structural_field_type(*primitive)?
        }
        CheckedUnitStructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => StructuralFieldType::PredicateByteSequence {
            carrier: terminal_byte_sequence_carrier(*carrier),
            predicates: *predicates,
        },
        CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
            StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(*carrier))
        }
        CheckedUnitStructuralFieldType::Structural { type_identity } => {
            StructuralFieldType::Structural(lookup_type_id(type_ids, type_identity)?)
        }
        CheckedUnitStructuralFieldType::ProviderBacked { .. }
        | CheckedUnitStructuralFieldType::FusedServiceBacked { .. } => {
            return unsupported("provider-backed fields require attached provider erasure");
        }
        CheckedUnitStructuralFieldType::Erased { type_identity } => StructuralFieldType::Erased {
            type_identity: type_identity.clone(),
        },
    })
}

pub(crate) fn lower_mixed_fields(
    fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
    type_ids: &[(String, StructuralTypeId)],
    next_field: &mut u64,
) -> Result<Vec<StructuralFieldDeclaration>, LoweringError> {
    let mut identities = BTreeSet::new();
    fields
        .iter()
        .map(|field| {
            if field.identity.is_empty() || !identities.insert(field.identity.as_str()) {
                return unsupported("mixed structural type has duplicate field identities");
            }
            let field_type = lower_field_type(&field.field_type, type_ids)?;
            Ok(StructuralFieldDeclaration {
                id: structural_field_id(allocate_dense(next_field)?),
                identity: field.identity.clone(),
                relevance: field.relevance,
                field_type,
            })
        })
        .collect()
}

pub(crate) fn lower_mixed_cases(
    cases: &[checked_trees::CheckedUnitStructuralCasePlan],
    type_ids: &[(String, StructuralTypeId)],
    next_field: &mut u64,
    next_case: &mut u64,
) -> Result<Vec<StructuralCaseDeclaration>, LoweringError> {
    let mut identities = BTreeSet::new();
    cases
        .iter()
        .map(|case| {
            if case.identity.is_empty() || !identities.insert(case.identity.as_str()) {
                return unsupported("mixed structural type has duplicate case identities");
            }
            Ok(StructuralCaseDeclaration {
                id: StructuralCaseId::new(allocate_dense(next_case)?)
                    .expect("allocated structural case identity is nonzero"),
                identity: case.identity.clone(),
                fields: lower_mixed_fields(&case.fields, type_ids, next_field)?,
            })
        })
        .collect()
}

pub(crate) fn retain_additional_structural_types(
    module: &mut TerminalModule,
    plans: &[CheckedUnitStructuralTypePlan],
    root_identities: impl IntoIterator<Item = String>,
) -> Result<(), LoweringError> {
    fn collect(
        plans: &[CheckedUnitStructuralTypePlan],
        identity: &str,
        active: &mut Vec<String>,
        selected: &mut Vec<String>,
    ) -> Result<(), LoweringError> {
        if active.iter().any(|candidate| candidate == identity) {
            return unsupported("recursive structural type is outside scalar cleanup lowering");
        }
        if selected.iter().any(|candidate| candidate == identity) {
            return Ok(());
        }
        let mut matches = plans.iter().filter(|plan| plan.identity == identity);
        let plan = matches.next().ok_or(LoweringError::Unsupported(
            "scalar cleanup references a missing structural type",
        ))?;
        if matches.next().is_some() || identity.is_empty() {
            return unsupported("scalar cleanup structural type identity is invalid");
        }
        active.push(identity.to_owned());
        match &plan.shape {
            CheckedUnitStructuralTypeShape::FusedServiceBinding => {}
            CheckedUnitStructuralTypeShape::Reference {
                referent_identity, ..
            } => {
                collect(plans, referent_identity, active, selected)?;
            }
            CheckedUnitStructuralTypeShape::PrimitiveScalar(_) => {}
            CheckedUnitStructuralTypeShape::ByteSequence(_) => {}
            // A `&[T]` view borrows its element's contiguous storage, so the
            // retained ElementView descriptor keeps the element identity live.
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            }
            | CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } => collect(plans, element_type_identity, active, selected)?,
            CheckedUnitStructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
        }
        active.pop();
        selected.push(identity.to_owned());
        Ok(())
    }

    let mut selected = Vec::new();
    let mut active = Vec::new();
    for identity in root_identities {
        collect(plans, &identity, &mut active, &mut selected)?;
    }
    selected.retain(|identity| {
        !module
            .structural_types
            .iter()
            .any(|declaration| declaration.identity == *identity)
    });
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        return Ok(());
    }

    let mut next_type = module
        .structural_types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup structural type identity space is exhausted",
        ))?;
    let mut type_ids = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    for identity in &selected {
        type_ids.push((
            identity.clone(),
            structural_type_id(allocate_dense(&mut next_type)?),
        ));
    }
    let mut next_field = module
        .structural_types
        .iter()
        .flat_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().collect::<Vec<_>>(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| case.fields.iter()))
                .collect(),
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::FixedArray { .. } => Vec::new(),
        })
        .map(|field| field.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup structural field identity space is exhausted",
        ))?;
    let mut next_case = module
        .structural_types
        .iter()
        .flat_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => cases.as_slice(),
            StructuralTypeShape::Mixed { cases, .. } => cases.as_slice(),
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::Record { .. }
            | StructuralTypeShape::FixedArray { .. } => &[],
        })
        .map(|case| case.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup structural case identity space is exhausted",
        ))?;
    for identity in selected {
        let plan = plans
            .iter()
            .find(|plan| plan.identity == identity)
            .expect("selected scalar structural type was validated");
        let shape = match &plan.shape {
            // Catalog retention preserves service identity, never an empty
            // record representation that ordinary construction could forge.
            CheckedUnitStructuralTypeShape::FusedServiceBinding => {
                StructuralTypeShape::FusedServiceBinding
            }
            CheckedUnitStructuralTypeShape::Reference {
                referent_identity,
                access,
            } => StructuralTypeShape::Reference {
                referent: lookup_type_id(&type_ids, referent_identity)?,
                access: reference_access(*access)?,
            },
            CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) => {
                StructuralTypeShape::PrimitiveScalar(terminal_scalar_type(*primitive)?)
            }
            CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            // A `&[T]` view carries its extent as a stored runtime length, so
            // its descriptor names only the borrowed element type.
            CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            } => StructuralTypeShape::ElementView {
                element: lookup_type_id(&type_ids, element_type_identity)?,
            },
            CheckedUnitStructuralTypeShape::Record { fields } => {
                let mut identities = BTreeSet::new();
                let fields = fields
                    .iter()
                    .map(|field| {
                        if field.identity.is_empty() || !identities.insert(&field.identity) {
                            return Err(LoweringError::Unsupported(
                                "scalar cleanup structural fields are invalid",
                            ));
                        }
                        let field_type = lower_field_type(&field.field_type, &type_ids)?;
                        Ok(StructuralFieldDeclaration {
                            id: structural_field_id(allocate_dense(&mut next_field)?),
                            identity: field.identity.clone(),
                            relevance: field.relevance,
                            field_type,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Record { fields }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => StructuralTypeShape::FixedArray {
                element: lookup_type_id(&type_ids, element_type_identity)?,
                length: *length,
            },
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                let mut identities = BTreeSet::new();
                let cases = cases
                    .iter()
                    .map(|case| {
                        if case.identity.is_empty() || !identities.insert(&case.identity) {
                            return Err(LoweringError::Unsupported(
                                "scalar cleanup structural cases are invalid",
                            ));
                        }
                        Ok(StructuralCaseDeclaration {
                            id: StructuralCaseId::new(allocate_dense(&mut next_case)?)
                                .expect("allocated structural case identity is nonzero"),
                            identity: case.identity.clone(),
                            fields: {
                                let mut field_identities = BTreeSet::new();
                                case.fields
                                    .iter()
                                    .map(|field| {
                                        if field.identity.is_empty()
                                            || !field_identities.insert(field.identity.as_str())
                                        {
                                            return Err(LoweringError::Unsupported(
                                                "scalar cleanup structural sum payload fields are invalid",
                                            ));
                                        }
                                        let field_type = lower_field_type(&field.field_type, &type_ids)?;
                                        Ok(StructuralFieldDeclaration {
                                            id: structural_field_id(allocate_dense(
                                                &mut next_field,
                                            )?),
                                            identity: field.identity.clone(),
                                            relevance: field.relevance,
                                            field_type,
                                        })
                                    })
                                    .collect::<Result<Vec<_>, LoweringError>>()?
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Sum { cases }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => StructuralTypeShape::Mixed {
                fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                cases: lower_mixed_cases(cases, &type_ids, &mut next_field, &mut next_case)?,
            },
        };
        module.structural_types.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok(())
}

pub(crate) fn lower_structural_type_plans(
    plans: &[checked_trees::CheckedUnitStructuralTypePlan],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    if plans.iter().any(|plan| plan.identity.is_empty()) {
        return unsupported("structural Unit control type has an empty identity");
    }
    let mut ordered = plans.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
    if ordered
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("structural Unit control types contain duplicate identities");
    }
    let type_ids = ordered
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            Ok((
                plan.identity.clone(),
                structural_type_id(dense_identity(index)?),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let mut next_case = 1_u64;
    let declarations = ordered
        .into_iter()
        .map(|plan| {
            let shape = match &plan.shape {
                CheckedUnitStructuralTypeShape::FusedServiceBinding => {
                    StructuralTypeShape::FusedServiceBinding
                }
                CheckedUnitStructuralTypeShape::Reference { referent_identity, access } => {
                    StructuralTypeShape::Reference {
                        referent: lookup_type_id(&type_ids, referent_identity)?,
                        access: reference_access(*access)?,
                    }
                }
                CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) => {
                    StructuralTypeShape::PrimitiveScalar(terminal_scalar_type(*primitive)?)
                }
                CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                    StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
                }
                // A `&[T]` view carries its extent as a stored runtime length,
                // so its descriptor names only the borrowed element type.
                CheckedUnitStructuralTypeShape::BorrowedSliceView {
                    element_type_identity,
                } => StructuralTypeShape::ElementView {
                    element: lookup_type_id(&type_ids, element_type_identity)?,
                },
                CheckedUnitStructuralTypeShape::Record { fields } => {
                    let mut identities = BTreeSet::new();
                    let fields = fields
                        .iter()
                        .map(|field| {
                            if field.identity.is_empty()
                                || !identities.insert(field.identity.as_str())
                            {
                                return Err(LoweringError::Unsupported(
                                    "structural Unit control type has duplicate field identities",
                                ));
                            }
                            let field_type = lower_field_type(&field.field_type, &type_ids)?;
                            Ok(StructuralFieldDeclaration {
                                id: structural_field_id(allocate_dense(&mut next_field)?),
                                identity: field.identity.clone(),
                                relevance: field.relevance,
                                field_type,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    StructuralTypeShape::Record { fields }
                }
                CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length,
                } => StructuralTypeShape::FixedArray {
                    element: lookup_type_id(&type_ids, element_type_identity)?,
                    length: *length,
                },
                CheckedUnitStructuralTypeShape::Sum { cases } => {
                    let mut identities = BTreeSet::new();
                    let cases = cases
                        .iter()
                        .map(|case| {
                            if case.identity.is_empty() || !identities.insert(&case.identity) {
                                return Err(LoweringError::Unsupported(
                                    "structural Unit control type has duplicate case identities",
                                ));
                            }
                            Ok(StructuralCaseDeclaration {
                                id: StructuralCaseId::new(allocate_dense(&mut next_case)?)
                                    .expect("allocated structural case identity is nonzero"),
                                identity: case.identity.clone(),
                                fields: {
                                    let mut field_identities = BTreeSet::new();
                                    case.fields
                                        .iter()
                                        .map(|field| {
                                            if field.identity.is_empty()
                                                || !field_identities
                                                    .insert(field.identity.as_str())
                                            {
                                                return Err(LoweringError::Unsupported(
                                                    "structural sum case has duplicate payload field identities",
                                                ));
                                            }
                                            let field_type = lower_field_type(&field.field_type, &type_ids)?;
                                            Ok(StructuralFieldDeclaration {
                                                id: structural_field_id(allocate_dense(
                                                    &mut next_field,
                                                )?),
                                                identity: field.identity.clone(),
                                                relevance: field.relevance,
                                                field_type,
                                            })
                                        })
                                        .collect::<Result<Vec<_>, LoweringError>>()?
                                },
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    StructuralTypeShape::Sum { cases }
                }
                CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                    StructuralTypeShape::Mixed {
                        fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                        cases: lower_mixed_cases(
                            cases,
                            &type_ids,
                            &mut next_field,
                            &mut next_case,
                        )?,
                    }
                }
            };
            Ok(StructuralTypeDeclaration {
                id: lookup_type_id(&type_ids, &plan.identity)?,
                identity: plan.identity.clone(),
                shape,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, type_ids))
}
