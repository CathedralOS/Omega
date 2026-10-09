//! Select a stored descriptor's producer through the checked value graph.
//! Constructor prefixes and projections belong to this graph; callers retain
//! the receiving loan's formation separately. A copied place inherits custody,
//! while a call keeps its exact occurrence and residual result path.

use checked_trees::{
    CheckFacts, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::data::DataMember;

/// The producer of a selected stored descriptor. Call identity is retained
/// independently of its eventual local, field or store consumer.
pub enum StoredCarrierSource {
    Place(CheckedUnitStructuralArgumentPlan),
    Call {
        value: CheckedStructuralValueHandle,
        path: Vec<CheckedUnitStructuralPathSegment>,
    },
}

/// Resolve all possible producers of this exact receiving descriptor path.
/// This query creates no loan authority and never drops dispatch alternatives.
pub fn stored_carrier_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: CheckedStructuralValueHandle,
    selection: &[CheckedUnitStructuralPathSegment],
) -> Option<Vec<StoredCarrierSource>> {
    let mut output = Vec::new();
    select(
        program,
        facts,
        value,
        selection,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

/// Select one stored carrier through ordinary value composition. Every
/// dispatch alternative supplies provenance; unrelated sibling leaves do not.
fn select(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: CheckedStructuralValueHandle,
    selection: &[CheckedUnitStructuralPathSegment],
    active: &mut Vec<CheckedStructuralValueHandle>,
    output: &mut Vec<StoredCarrierSource>,
) -> Option<()> {
    let values = &facts.values.structural_values;
    if !values.nodes.is_valid(value) || active.contains(&value) {
        return None;
    }
    active.push(value);
    match &values.nodes.get(value).kind {
        CheckedStructuralValueKind::Place(source)
        | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
            let mut source = source.clone();
            source.path.extend_from_slice(selection);
            output.push(StoredCarrierSource::Place(source));
        }
        CheckedStructuralValueKind::Projection { source, path, .. } => {
            let mut path = path.clone();
            path.extend_from_slice(selection);
            select(program, facts, *source, &path, active, output)?;
        }
        CheckedStructuralValueKind::Dispatch { arms, .. } => {
            let arms = values.dispatch_arms.span(*arms)?;
            if arms.is_empty() {
                return None;
            }
            for arm in arms {
                select(program, facts, arm.value, selection, active, output)?;
            }
        }
        CheckedStructuralValueKind::FixedArray { elements } => {
            let (CheckedUnitStructuralPathSegment::FixedIndex(element_index), suffix) =
                selection.split_first()?
            else {
                return None;
            };
            let child = elements.get(usize::try_from(*element_index).ok()?)?;
            select(program, facts, *child, suffix, active, output)?;
        }
        CheckedStructuralValueKind::Record {
            data_symbol,
            fields,
        }
        | CheckedStructuralValueKind::StructuralCase {
            data_symbol,
            fields,
            ..
        } => {
            let data = program.data_definition_by_symbol(*data_symbol)?;
            let (field_identity, suffix, payload) = match selection {
                [
                    CheckedUnitStructuralPathSegment::Case(case),
                    CheckedUnitStructuralPathSegment::Field(field),
                    suffix @ ..,
                ] => {
                    let CheckedStructuralValueKind::StructuralCase { case: selected, .. } =
                        &values.nodes.get(value).kind
                    else {
                        return None;
                    };
                    let variant =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant)
                                    if variant.path_identity() == *case =>
                                {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    // The requested leaf must belong to this nominal case,
                    // even when another admitted arm establishes no such
                    // payload. Inactive storage contributes no supplier.
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|declaration| declaration.path_identity() == *field)?;
                    let actual =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.symbol == *selected => {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    if actual.symbol != variant.symbol {
                        active.pop();
                        return Some(());
                    }
                    (field, suffix, Some(variant))
                }
                [CheckedUnitStructuralPathSegment::Field(field), suffix @ ..] => {
                    (field, suffix, None)
                }
                _ => return None,
            };
            let declaration = if let Some(variant) = payload {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.path_identity() == *field_identity)?
            } else {
                program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.path_identity() == *field_identity => {
                            Some(field)
                        }
                        _ => None,
                    })?
            };
            let field = values
                .record_fields
                .span(*fields)?
                .iter()
                .find(|field| field.field == declaration.symbol)?;
            let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) = field.value
            else {
                return None;
            };
            select(program, facts, child, suffix, active, output)?;
        }
        CheckedStructuralValueKind::Call { .. } => {
            output.push(StoredCarrierSource::Call {
                value,
                path: selection.to_vec(),
            });
        }
        // These form rather than transport a descriptor. Their admitted
        // formation/call lineage remains the independent parent authority.
        _ => {}
    }
    active.pop();
    Some(())
}
