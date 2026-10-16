//! Entry provenance for canonical places uses the same binding and write window
//! as authored operands. Projection identities are retained until the final
//! crash-predicate presentation; no source expression is manufactured.
use super::mutable::PlaceSegment;
use super::{entry_operand_symbol_at, field_entry_name};
use checked_trees::{CheckedStructuralPredicatePathSegment, CrashPredicateExpression};
use symbols::SymbolHandle;

/// Entry identity for a captured place. The caller has authenticated the
/// evaluated operand prefix separately, so only preceding statements belong
/// to this provenance window, not the receiving invocation's future writes.
pub(crate) fn entry_place_before_statement(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
) -> Option<CrashPredicateExpression> {
    entry_place_at_write_boundary(program, machine, state, statement, statement, place)
}

fn entry_place_at_write_boundary(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    write_scan_end: usize,
    place: &validation::CanonicalPlace,
) -> Option<CrashPredicateExpression> {
    let symbol = place.root.as_symbol()?;
    // Even stable whole storage cannot authenticate an unrelated declaration's
    // field. Replay the complete path from this binding's declared type first.
    validation::canonical_place_type_reference(program, state_symbol, before_statement, place)?;
    let mut projection = Vec::with_capacity(place.segments.len());
    let mut members = Vec::with_capacity(place.segments.len());
    let mut selected_case = None;
    for segment in &place.segments {
        match *segment {
            facts::PlaceSegment::Case { variant } if selected_case.is_none() => {
                if !variant.is_valid() {
                    return None;
                }
                selected_case = Some(variant);
                projection.push(PlaceSegment::Case(variant));
            }
            facts::PlaceSegment::Field { symbol } => {
                if facts::payload_variant_for_field(program, symbol) != selected_case {
                    return None;
                }
                let field = program.any_field_owner(symbol).map(|(_, field)| field)?;
                projection.push(PlaceSegment::Field(symbol));
                members.push(Projection::Member(field_entry_name(
                    program,
                    symbol,
                    field.name.as_str(),
                    None,
                )));
                selected_case = None;
            }
            facts::PlaceSegment::FixedIndex { index } if selected_case.is_none() => {
                projection.push(PlaceSegment::FixedIndex(index));
                members.push(Projection::Index(index));
            }
            // A case is only a qualifier of its exact payload field. Dynamic
            // indices and range descriptors are not scalar place projections.
            _ => return None,
        }
    }
    if selected_case.is_some() {
        return None;
    }
    let root = entry_operand_symbol_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        write_scan_end,
        symbol,
        &projection,
        0,
    )?;
    Some(
        members
            .into_iter()
            .fold(root, |receiver, projection| match projection {
                Projection::Member(member) => CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member,
                },
                Projection::Index(index) => CrashPredicateExpression::Indexed {
                    collection: Box::new(receiver),
                    index: Box::new(CrashPredicateExpression::Integer(index.to_string())),
                },
            }),
    )
}

/// Resolve a current observation to its unchanged invocation parameter and
/// canonical structural path. The presentation comes only from the provenance
/// replay above; every member is then authenticated against the exact entry
/// type, including the case and field identities used by checked predicates.
pub(crate) fn entry_parameter_place(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    place: &validation::CanonicalPlace,
) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
    entry_parameter_place_at_write_boundary(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        before_statement.saturating_add(1),
        place,
    )
}

/// Invocation provenance at statement entry. The caller must authenticate the
/// observed operand's evaluated write prefix before using this namespace.
pub(crate) fn entry_parameter_place_before_statement(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    place: &validation::CanonicalPlace,
) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
    entry_parameter_place_at_write_boundary(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        before_statement,
        place,
    )
}

fn entry_parameter_place_at_write_boundary(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    write_scan_end: usize,
    place: &validation::CanonicalPlace,
) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
    let expression = entry_place_at_write_boundary(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        write_scan_end,
        place,
    )?;
    let mut projections = Vec::new();
    let mut root = &expression;
    let position = loop {
        match root {
            CrashPredicateExpression::Parameter(position) => break *position,
            CrashPredicateExpression::Member { receiver, member } => {
                projections.push(Projection::Member(member.clone()));
                root = receiver;
            }
            CrashPredicateExpression::Indexed { collection, index } => {
                let CrashPredicateExpression::Integer(index) = index.as_ref() else {
                    return None;
                };
                projections.push(Projection::Index(index.parse().ok()?));
                root = collection;
            }
            _ => return None,
        }
    };
    let machine = program.machine(machine_symbol)?;
    let entry = validation::activation_target_state_index(program, machine, machine_symbol)?;
    let parameters = program.state_parameters(program.machine_states(machine).get(entry)?);
    let mut reference = parameters
        .get(usize::try_from(position).ok()?)?
        .type_reference;
    let mut path = Vec::new();
    let mut identities = Vec::new();
    for projection in projections.into_iter().rev() {
        match projection {
            Projection::Index(index) => {
                path.push(CheckedStructuralPredicatePathSegment::FixedIndex(
                    u64::try_from(index).ok()?,
                ));
                identities.push(facts::PlaceSegment::FixedIndex { index });
            }
            Projection::Member(member) => {
                // structural_data also supports unresolved-name discovery for
                // other callers. This provenance query requires a resolved type.
                let owner = program
                    .type_reference_table
                    .peeled_type_reference(reference);
                match program.type_reference_table.type_reference(owner) {
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                    | typed_trees::types::TypeReferenceNode::Generic {
                        base_symbol: symbol,
                        ..
                    } if symbol.is_valid() => {}
                    _ => return None,
                }
                let data = crate::values::structural_data(program, reference)?;
                let mut selected = None;
                for declaration in program.data_members(data) {
                    let (variant, fields) = match declaration {
                        typed_trees::data::DataMember::Field(field) => {
                            (None, std::slice::from_ref(field))
                        }
                        typed_trees::data::DataMember::Variant(variant) => {
                            (Some(variant), program.data_payload_fields(variant))
                        }
                    };
                    for field in fields {
                        let presentation = field_entry_name(
                            program,
                            field.symbol,
                            field.name.as_str(),
                            variant.map(|variant| variant.name.as_str()),
                        );
                        if presentation == member {
                            if selected.is_some() || field.relevance.is_erased() {
                                return None;
                            }
                            selected = Some((variant, field));
                        }
                    }
                }
                let (variant, field) = selected?;
                if let Some(variant) = variant {
                    path.push(CheckedStructuralPredicatePathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        ),
                    ));
                    identities.push(facts::PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                }
                path.push(CheckedStructuralPredicatePathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                identities.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
            }
        }
        let (_, resolved, endpoint, _) =
            crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
        if resolved != identities {
            return None;
        }
        reference = endpoint;
    }
    // Authenticate even a whole-parameter observation with an empty path.
    crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
    Some((position, path))
}

/// Observation may read stable storage inside an owner whose whole value is
/// not an observation (for example, an owner with a nominal cleanup hook).
/// Every crossed descriptor validates its referee too: borrowed-storage
/// classification alone deliberately treats shared descriptors as opaque.
pub(super) fn projection_requires_write_scan(
    program: &dyn ::validation::CheckingInput,
    root_type: typed_trees::types::TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<bool> {
    let segments = path
        .iter()
        .map(|segment| match *segment {
            PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
            PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
            PlaceSegment::FixedIndex(index) => Some(facts::PlaceSegment::FixedIndex { index }),
            PlaceSegment::FixedRange { .. } | PlaceSegment::Opaque => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let mut mutable = false;
    for boundary in 0..segments.len() {
        // Case qualifies the following payload field; it is not another
        // storage location. Check the container before that combined hop.
        if boundary > 0 && matches!(segments[boundary - 1], facts::PlaceSegment::Case { .. }) {
            continue;
        }
        let current = validation::project_type_reference_from_segments(
            program,
            root_type,
            &segments[..boundary],
        )?;
        mutable |= traversed_storage_requires_write_scan(program, current)?;
    }
    let endpoint = validation::project_type_reference_from_segments(program, root_type, &segments)?;
    validation::has_contents(
        program,
        endpoint,
        &[],
        validation::ContentsRequirement::StableObservation,
    )
    .then_some(mutable)
}

fn traversed_storage_requires_write_scan(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<bool> {
    match program.type_reference_table.type_reference(reference) {
        typed_trees::types::TypeReferenceNode::Reference {
            access, referee, ..
        } => {
            let nested = traversed_storage_requires_write_scan(program, *referee)?;
            Some(nested || *access == language_core::ReferenceAccess::Mutable)
        }
        _ => (validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::BorrowedStorage,
        ) || validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::StableObservation,
        ))
        .then_some(false),
    }
}

enum Projection {
    Member(String),
    Index(usize),
}
