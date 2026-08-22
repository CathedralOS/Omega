//! Shared structural-type retention for structural result and control lanes.

use super::*;

pub(super) fn terminal_structural_field_type(
    primitive: PrimitiveType,
) -> Result<StructuralFieldType, LoweringError> {
    Ok(match primitive {
        PrimitiveType::F32 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
        PrimitiveType::F64 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
        primitive => StructuralFieldType::Scalar(terminal_scalar_type(primitive)?),
    })
}

pub(super) fn terminal_byte_sequence_carrier(
    carrier: psi_checked_trees::CheckedByteSequenceCarrier,
) -> ByteSequenceCarrier {
    match carrier {
        psi_checked_trees::CheckedByteSequenceCarrier::BorrowedView => {
            ByteSequenceCarrier::BorrowedView
        }
        psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } => {
            ByteSequenceCarrier::BoundedOwned { capacity }
        }
    }
}

pub(super) fn retain_additional_structural_types(
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
            CheckedUnitStructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } => collect(plans, element_type_identity, active, selected)?,
            CheckedUnitStructuralTypeShape::Sum { .. } => {}
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
            StructuralTypeShape::Record { fields } => fields.as_slice(),
            StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::Sum { .. } => &[],
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
            StructuralTypeShape::ByteSequence(_)
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
                        let field_type = match &field.field_type {
                            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                terminal_structural_field_type(*primitive)?
                            }
                            CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                                StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(
                                    *carrier,
                                ))
                            }
                            CheckedUnitStructuralFieldType::Structural { type_identity } => {
                                StructuralFieldType::Structural(lookup_type_id(
                                    &type_ids,
                                    type_identity,
                                )?)
                            }
                            CheckedUnitStructuralFieldType::Erased { type_identity } => {
                                StructuralFieldType::Erased {
                                    type_identity: type_identity.clone(),
                                }
                            }
                        };
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
                                        let field_type = match &field.field_type {
                                            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                                terminal_structural_field_type(*primitive)?
                                            }
                                            CheckedUnitStructuralFieldType::ByteSequence(
                                                carrier,
                                            ) => StructuralFieldType::ByteSequence(
                                                terminal_byte_sequence_carrier(*carrier),
                                            ),
                                            CheckedUnitStructuralFieldType::Structural {
                                                type_identity,
                                            } => StructuralFieldType::Structural(lookup_type_id(
                                                &type_ids,
                                                type_identity,
                                            )?),
                                            CheckedUnitStructuralFieldType::Erased {
                                                type_identity,
                                            } => StructuralFieldType::Erased {
                                                type_identity: type_identity.clone(),
                                            },
                                        };
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
        };
        module.structural_types.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok(())
}

pub(super) fn lower_structural_type_plans(
    plans: &[psi_checked_trees::CheckedUnitStructuralTypePlan],
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
                            let field_type = match &field.field_type {
                                CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                    terminal_structural_field_type(*primitive)?
                                }
                                CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                                    StructuralFieldType::ByteSequence(
                                        terminal_byte_sequence_carrier(*carrier),
                                    )
                                }
                                CheckedUnitStructuralFieldType::Structural { type_identity } => {
                                    StructuralFieldType::Structural(lookup_type_id(
                                        &type_ids,
                                        type_identity,
                                    )?)
                                }
                                CheckedUnitStructuralFieldType::Erased { type_identity } => {
                                    StructuralFieldType::Erased {
                                        type_identity: type_identity.clone(),
                                    }
                                }
                            };
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
                                            let field_type = match &field.field_type {
                                                CheckedUnitStructuralFieldType::Scalar(
                                                    primitive,
                                                ) => terminal_structural_field_type(*primitive)?,
                                                CheckedUnitStructuralFieldType::ByteSequence(
                                                    carrier,
                                                ) => StructuralFieldType::ByteSequence(
                                                    terminal_byte_sequence_carrier(*carrier),
                                                ),
                                                CheckedUnitStructuralFieldType::Structural {
                                                    type_identity,
                                                } => StructuralFieldType::Structural(
                                                    lookup_type_id(&type_ids, type_identity)?,
                                                ),
                                                CheckedUnitStructuralFieldType::Erased {
                                                    type_identity,
                                                } => StructuralFieldType::Erased {
                                                    type_identity: type_identity.clone(),
                                                },
                                            };
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
