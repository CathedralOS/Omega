//! Select supplying carrier leaves through retained structural values.

use checked_trees::{
    CheckFacts, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::data::DataMember;

/// Select one stored carrier through ordinary value composition. Every
/// dispatch alternative supplies provenance; unrelated sibling leaves do not.
pub(super) fn select(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    value: CheckedStructuralValueHandle,
    selection: &[CheckedUnitStructuralPathSegment],
    active: &mut Vec<CheckedStructuralValueHandle>,
    output: &mut Vec<CheckedUnitStructuralArgumentPlan>,
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
            output.push(source);
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
        // These form rather than transport a descriptor. Their admitted
        // formation/call lineage remains the independent parent authority.
        _ => {}
    }
    active.pop();
    Some(())
}
