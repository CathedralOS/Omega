//! Sum inspection retains declared order, layout, and edge-produced parameters.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlTerminator,
    TargetScalarBlockValue, TargetStructuralCaseSource, TargetStructuralHomeLayout,
    TargetStructuralParameter,
};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(super) fn observe(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = operation
    else {
        return Err(invalid());
    };
    let identity = if let Some(home) = live.structural_homes.get(source) {
        home.structural_type()
    } else {
        parameter_root(prepared, *source)
            .ok_or_else(invalid)?
            .structural_type
    };
    let (tag_byte_offset, case_tag) = case_projection(identity, path, *case, types)?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(invalid());
    }
    super::primitive_storage::retain_result(*psi_operation, *result, live)?;
    operations.push(TargetUnitOperation::StructuralCaseMembership {
        psi_operation: *psi_operation,
        result: *result,
        source: *source,
        path: path.clone(),
        tag_byte_offset,
        case: *case,
        case_tag,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// A tag observation may read the function's own incoming parameter under any
/// readable access; a write-only loan carries no readable tag.
fn parameter_root(
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    source: PlaceId,
) -> Option<&TargetStructuralParameter> {
    prepared
        .parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)
}

/// Resolve the dispatched sum. A live home dispatches as before. Otherwise the
/// root is the function's own parameter, resolved exactly as a tag observation
/// resolves it, and then held to the owned-arrival contract a block parameter
/// home already meets: payload bindings read the activation's value copy, so a
/// borrowed referent, a linear value, or a qualified root has no such copy.
fn case_source(
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    source: PlaceId,
) -> Result<TargetStructuralCaseSource, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if let Some(home) = live.structural_homes.get(&source) {
        return Ok(TargetStructuralCaseSource::Home(home.clone()));
    }
    let parameter = parameter_root(prepared, source).ok_or_else(invalid)?;
    let declaration = function
        .structural_parameters
        .iter()
        .find(|declaration| declaration.place == source)
        .ok_or_else(invalid)?;
    if declaration.access != StructuralAccess::Owned
        || declaration.is_self
        || declaration.multiplicity == StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || !function.entry_claims.is_empty()
        || parameter.access != declaration.access
        || parameter.multiplicity != declaration.multiplicity
        || parameter.structural_type != declaration.structural_type
        || !parameter.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let layout = crate::lowering::structural_layout::structural_sum_layout(
        parameter.structural_type,
        structural_types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    if layout.shape != parameter.shape {
        return Err(invalid());
    }
    Ok(TargetStructuralCaseSource::Parameter {
        parameter: parameter.clone(),
        layout: TargetStructuralHomeLayout::Sum(layout),
    })
}

fn case_projection(
    identity: StructuralTypeId,
    path: &[StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    types: &StructuralTypeLookup<'_>,
) -> Result<(u32, u32), LoweringError> {
    let invalid = || LoweringError::UnknownStructuralType(identity);
    let (identity, tag_byte_offset) = if path.is_empty() {
        (identity, 0)
    } else {
        let (endpoint, _, offset) =
            crate::lowering::structural_layout::resolve_structural_projection_path(
                identity,
                path,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
        (endpoint, offset)
    };
    let declaration = types.get(&identity).ok_or_else(invalid)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid());
    };
    let case_tag = cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or_else(invalid)?;
    Ok((tag_byte_offset, case_tag))
}

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetControlTerminator, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::StructuralCase { source, cases } = operation else {
        return Err(invalid());
    };
    let home = case_source(function, prepared, live, structural_types, *source)?;
    let declaration = structural_types
        .get(&home.structural_type())
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Sum {
        cases: declared_cases,
    } = &declaration.shape
    else {
        return Err(invalid());
    };
    let layout = home.layout().sum().ok_or_else(invalid)?;
    if cases.len() != declared_cases.len() || cases.len() != layout.cases.len() {
        return Err(invalid());
    }
    let mut lowered = Vec::with_capacity(cases.len());
    for (case_ordinal, (case, declared)) in cases.iter().zip(declared_cases).enumerate() {
        let target = function
            .block_entries
            .iter()
            .find(|block| block.block == case.target)
            .ok_or_else(invalid)?;
        if case.case != declared.id
            || !target.structural_parameters.is_empty()
            || case.payloads.len() != target.parameters.len()
        {
            return Err(invalid());
        }
        // This checks the supported cleanup carrier, not ownership liveness.
        // The validated abstract graph remains the custody authority.
        let mut discards = BTreeSet::new();
        for place in &case.trivial_affine_discards {
            let discarded = live.structural_homes.get(place).ok_or_else(invalid)?;
            if !discards.insert(*place)
                || discarded.multiplicity() != terminal_psi::StructuralMultiplicity::Affine
                || discarded.has_claims()
                || !discarded.qualifications().is_empty()
                || !discarded.projected_qualifications().is_empty()
            {
                return Err(invalid());
            }
        }
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if fields.len() != layout.cases[case_ordinal].fields.len() {
            return Err(invalid());
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&target.parameters) {
            let field_ordinal = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or_else(invalid)?;
            let field = fields[field_ordinal];
            let ScalarType::Integer(integer_type) = payload.scalar_type else {
                return Err(invalid());
            };
            let shape = crate::lowering::scalar_abi::fixed_native_integer_shape(integer_type)
                .ok_or_else(invalid)?;
            let field_layout = layout.cases[case_ordinal].fields[field_ordinal];
            if field.field_type.scalar_type() != Some(payload.scalar_type)
                || field_layout.shape != shape
                || parameter.value != payload.parameter
                || parameter.scalar_type != payload.scalar_type
            {
                return Err(invalid());
            }
            payloads.push(TargetControlCasePayload {
                field: payload.field,
                field_byte_offset: u32::from(field_layout.byte_offset),
                parameter: TargetScalarBlockValue {
                    block: target.block,
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            });
        }
        lowered.push(TargetControlCaseSuccessor {
            psi_edge: case.psi_edge,
            case: case.case,
            case_tag: i32::try_from(case_ordinal).map_err(|_| invalid())?,
            target: case.target,
            payloads,
            trivial_affine_discards: case.trivial_affine_discards.clone(),
        });
    }
    provenance
        .edges
        .extend(cases.iter().map(|case| case.psi_edge));
    Ok(TargetControlTerminator::StructuralCase {
        source: home,
        cases: lowered,
    })
}

#[cfg(test)]
mod tests {
    use super::case_projection;
    use crate::lowering::structural_type_lookup::StructuralTypeLookup;
    use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[test]
    fn projected_case_layout_retains_nested_offset_and_nominal_ordinal() {
        let sum = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let record = StructuralTypeId::new(3).unwrap();
        let red = semantic_vocabulary::StructuralCaseId::new(90).unwrap();
        let blue = semantic_vocabulary::StructuralCaseId::new(12).unwrap();
        let field = |number, identity: &str, field_type| terminal_psi::StructuralFieldDeclaration {
            id: StructuralFieldId::new(number).unwrap(),
            identity: identity.into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type,
        };
        let catalog: abstract_operations::StructuralTypeCatalog = vec![
            StructuralTypeDeclaration {
                id: sum,
                identity: "Color".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![
                        terminal_psi::StructuralCaseDeclaration {
                            id: red,
                            identity: "Red".into(),
                            fields: Vec::new(),
                        },
                        terminal_psi::StructuralCaseDeclaration {
                            id: blue,
                            identity: "Blue".into(),
                            fields: Vec::new(),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "Colors".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: sum,
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: record,
                identity: "Record".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        field(
                            1,
                            "prefix",
                            StructuralFieldType::Scalar(ScalarType::Boolean),
                        ),
                        field(2, "colors", StructuralFieldType::Structural(array)),
                    ],
                },
            },
        ]
        .into();
        let types = StructuralTypeLookup::new(&catalog);
        let path = vec![
            StructuralPathSegment::Field("colors".into()),
            StructuralPathSegment::FixedIndex(1),
        ];
        // A one-byte prefix is followed by four-byte aligned tags: second tag is at 8.
        assert_eq!(
            case_projection(record, &path, blue, &types).unwrap(),
            (8, 1)
        );
        assert_eq!(case_projection(sum, &[], red, &types).unwrap(), (0, 0));
        assert_eq!(
            case_projection(array, &[StructuralPathSegment::FixedIndex(0)], blue, &types).unwrap(),
            (0, 1)
        );
        for invalid in [
            vec![
                StructuralPathSegment::Field("colors".into()),
                StructuralPathSegment::FixedIndex(2),
            ],
            vec![StructuralPathSegment::Field("missing".into())],
            vec![StructuralPathSegment::Field("prefix".into())],
            vec![StructuralPathSegment::Field("colors".into())],
            vec![StructuralPathSegment::Referent],
        ] {
            assert!(case_projection(record, &invalid, blue, &types).is_err());
        }
        assert!(
            case_projection(
                record,
                &path,
                semantic_vocabulary::StructuralCaseId::new(99).unwrap(),
                &types
            )
            .is_err()
        );
    }
}
