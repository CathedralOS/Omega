//! Select a stored descriptor's producer through the checked value graph.
//! Constructor prefixes and projections belong to this graph; callers retain
//! the receiving loan's formation separately. A copied place inherits custody,
//! while a call keeps its exact occurrence and residual result path.
//! A reference root can supply either its own newly formed descriptor or a
//! selected carrier already stored in its referent. Preserve that distinction:
//! an aggregate extraction transports the child's loans, not the root loan.

use checked_trees::{
    CheckFacts, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::data::DataMember;

/// The producer of a selected stored descriptor. Call identity is retained
/// independently of its eventual local, field or store consumer.
pub enum StoredCarrierSource {
    Place(CheckedUnitStructuralArgumentPlan),
    /// A present descriptor over immutable image storage. It has no supplying
    /// activation-local holder to retire, but is not an inactive case slot.
    Image {
        value: CheckedStructuralValueHandle,
    },
    /// A fresh loan formed in this receiving descriptor slot. Its source
    /// holder is an immediate disposal dependency, not a transported ancestor.
    Borrow {
        value: CheckedStructuralValueHandle,
        source: CheckedUnitStructuralArgumentPlan,
    },
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
        CheckedStructuralValueKind::Reference {
            source,
            source_value,
            ..
        } => {
            if selection.is_empty() {
                output.push(StoredCarrierSource::Borrow {
                    value,
                    source: source.clone(),
                });
            } else {
                // A selected child belongs to the referent's storage, not to
                // the descriptor this node forms. Borrowed formals and fresh
                // temporary backing expose the same existing carrier leaves;
                // selecting one neither borrows its containing slot again nor
                // replaces its origin with the outer reference's permission.
                let mut path = source.path.clone();
                path.extend_from_slice(selection);
                if source_value.is_valid() {
                    select(program, facts, *source_value, &path, active, output)?;
                } else {
                    let mut source = source.clone();
                    source.path = path;
                    output.push(StoredCarrierSource::Place(source));
                }
            }
        }
        CheckedStructuralValueKind::BorrowedSliceView { source, .. }
        | CheckedStructuralValueKind::MutableSliceView { source, .. } => {
            if !selection.is_empty() {
                return None;
            }
            output.push(StoredCarrierSource::Borrow {
                value,
                source: source.clone(),
            });
        }
        CheckedStructuralValueKind::ByteLiteralView { .. } => {
            if !selection.is_empty() {
                return None;
            }
            // The value producer already admitted the literal's carrier and
            // predicates. Retirement needs its storage duration, not another
            // loan of a local holder or an absent-source fallback.
            output.push(StoredCarrierSource::Image { value });
        }
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
        checked_trees::record_or_structural_case_pattern!(data_symbol, fields, ..) => {
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
                                    if checked_trees::CheckedMemberIdentity::of_member(
                                        variant.identity,
                                        variant.name.clone(),
                                    ) == *case =>
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
                        .find(|declaration| {
                            checked_trees::CheckedMemberIdentity::of_member(
                                declaration.identity,
                                declaration.name.clone(),
                            ) == *field
                        })?;
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
                program.data_payload_fields(variant).iter().find(|field| {
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ) == *field_identity
                })?
            } else {
                program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field)
                            if checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            ) == *field_identity =>
                        {
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
        CheckedStructuralValueKind::Case(case) => {
            // A scalar-only case can establish that another case's descriptor
            // is inactive. Validate the requested nominal slot first; an
            // unsupported value kind must not masquerade as empty storage.
            let [CheckedUnitStructuralPathSegment::Case(requested), ..] = selection else {
                return None;
            };
            let data = program.closed_data_definition(case.type_reference)?;
            let selected = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == case.case => Some(variant),
                    _ => None,
                })?;
            if checked_trees::CheckedMemberIdentity::of_member(
                selected.identity,
                selected.name.clone(),
            ) == *requested
                || !super::record_carrier_sources(
                    program,
                    case.type_reference,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: values.nodes.get(value).state,
                    },
                )?
                .iter()
                .any(|leaf| leaf.path == selection)
            {
                return None;
            }
        }
        _ => return None,
    }
    active.pop();
    Some(())
}
