//! Replay exact stored type evidence through a place's field/index segments.
//! Generic substitutions follow the reaching application; closed fields never rebind to siblings.

use super::canonical_projection::CanonicalPlace;
use super::canonical_projection::canonical_place_from_expression_in_state;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// The declared type an expression's value carries. The canonical place walk
/// roots the expression at its storage — a symbol for names and their
/// projections, the expression itself for value-producing leaves — then the
/// place type replays the root's stored evidence through the retained
/// segments. Because an expression root defers to `expression_type_position`,
/// every leaf the position walk proves answers here too: a call resumes at
/// its declared return type, a cast or zero value at its stored reference,
/// an array or match leaf at its joined position projected through the
/// segments. Leaves whose values own no declaration identity — scalar
/// literals, unary and binary results, range operands — keep no reference
/// rather than borrowing a same-shaped row.
pub fn expression_type_reference_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    let place = canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    canonical_place_type_reference(program, state_symbol, statement_index, &place)
}

/// An admitted observation must retain a finite path in its own source
/// namespace. This describes malformed geometry, never missing read authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageObservationTypePathError {
    pub segment_ordinal: usize,
    pub segment: Option<checked_trees::CheckedUnitStructuralPathSegment>,
    pub source_type: typed_trees::types::TypeReferenceHandle,
    pub current_type: typed_trees::types::TypeReferenceHandle,
    pub current_type_identity: String,
    pub reason: &'static str,
}

/// One finite observation walk shares its append-only substitution context.
/// Each crossed type retains the exact prefix visible at that occurrence;
/// later generic applications cannot reinterpret an earlier carrier.
#[derive(Debug, Clone)]
pub struct StorageObservationTypePath {
    entries: Vec<(typed_trees::types::TypeReferenceHandle, usize)>,
    substitutions: Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
}

impl StorageObservationTypePath {
    pub fn entries(
        &self,
    ) -> impl Iterator<
        Item = (
            typed_trees::types::TypeReferenceHandle,
            &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        ),
    > {
        self.entries
            .iter()
            .map(|(reference, end)| (*reference, &self.substitutions[..*end]))
    }
}

/// Concrete declarations crossed by a checked field observation. The retained
/// projection is relative to `source_type`; its authored expression may still
/// identify an earlier enclosing place for source correspondence. Replaying
/// that expression here would interpret its prefix in the wrong namespace.
/// Reference crossings remain explicit and finite. This grants neither access
/// nor a case assertion, selector bound, lifetime, or establishment permission.
pub fn observed_storage_type_references(
    program: &dyn crate::CheckingInput,
    source_type: typed_trees::types::TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
    field: SymbolHandle,
    field_identity: &checked_trees::CheckedMemberIdentity,
    atomic: bool,
) -> Result<StorageObservationTypePath, StorageObservationTypePathError> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    use typed_trees::data::DataMember;
    use typed_trees::types::TypeReferenceNode;

    let final_field = field
        .is_valid()
        .then(|| Segment::Field(field_identity.clone()));
    let mut current = source_type;
    let mut references = vec![(current, 0)];
    let mut substitutions = Vec::new();
    let mut selected_case = None;
    for (ordinal, segment) in path.iter().chain(final_field.iter()).enumerate() {
        current = substituted_type_reference(program, current, &substitutions);
        let carrier = crate::peel_declared_constraints(program, current);
        let current_type_identity =
            program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                substitutions: &substitutions,
                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(current)
            });
        let failure = |reason| StorageObservationTypePathError {
            segment_ordinal: ordinal,
            segment: Some(segment.clone()),
            source_type,
            current_type: current,
            current_type_identity: program
                .type_identity_text(current_type_identity)
                .unwrap_or_default(),
            reason,
        };
        current = match segment {
            Segment::Referent => {
                if selected_case.is_some() {
                    return Err(failure("reference crossing interrupts a selected case"));
                }
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return Err(failure("reference crossing has no reference carrier"));
                };
                if !access.is_readable() {
                    return Err(failure("field observation crosses a write-only reference"));
                }
                *referee
            }
            Segment::Case(identity) => {
                if selected_case.is_some() {
                    return Err(failure("case selection interrupts another selected case"));
                }
                let data = observation_data_declaration(program, carrier, &substitutions)
                    .ok_or_else(|| failure("case carrier has no exact data declaration"))?;
                let mut matching = program.data_members(data).iter().filter_map(|member| {
                    let DataMember::Variant(case) = member else {
                        return None;
                    };
                    (checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ) == *identity)
                        .then_some(case)
                });
                selected_case = Some(
                    matching
                        .next()
                        .ok_or_else(|| failure("case identity is absent from its carrier"))?,
                );
                if matching.next().is_some() {
                    return Err(failure("case identity is ambiguous in its carrier"));
                }
                continue;
            }
            Segment::Field(identity) => {
                let data = observation_data_declaration(program, carrier, &substitutions)
                    .ok_or_else(|| failure("field carrier has no exact data declaration"))?;
                let case = selected_case.take();
                let payloads = case.map_or(&[][..], |case| program.data_payload_fields(case));
                let fields = program
                    .data_members(data)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) if case.is_none() => Some(field),
                        _ => None,
                    });
                let mut matching = payloads.iter().chain(fields).filter(|candidate| {
                    checked_trees::CheckedMemberIdentity::of_member(
                        candidate.identity,
                        candidate.name.clone(),
                    ) == *identity
                });
                let selected = matching
                    .next()
                    .ok_or_else(|| failure("field identity is absent from its carrier"))?;
                if matching.next().is_some() {
                    return Err(failure("field identity is ambiguous in its carrier"));
                }
                field_type_reference(program, current, selected.symbol, &mut substitutions)
                    .ok_or_else(|| failure("field type lost its supplying application"))?
            }
            Segment::FixedIndex(index) => {
                if selected_case.is_some() {
                    return Err(failure("index interrupts a selected case payload"));
                }
                if let Some(length) = crate::fixed_array_type_length(program, carrier)
                    && usize::try_from(*index).map_or(true, |index| index >= length)
                {
                    return Err(failure("fixed index exceeds its declared collection"));
                }
                if matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::Reference { .. }
                ) {
                    return Err(failure("index requires its explicit reference crossing"));
                }
                indexed_element_type_reference(program, current, &substitutions)
                    .ok_or_else(|| failure("index carrier has no collection element"))?
            }
            Segment::RuntimeIndex(_) | Segment::ComputedIndex(_) => {
                if selected_case.is_some() {
                    return Err(failure("index interrupts a selected case payload"));
                }
                if matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::Reference { .. }
                ) {
                    return Err(failure("index requires its explicit reference crossing"));
                }
                indexed_element_type_reference(program, current, &substitutions)
                    .ok_or_else(|| failure("index carrier has no collection element"))?
            }
            _ => {
                return Err(failure(
                    "field observation has a non-field projection segment",
                ));
            }
        };
        current = substituted_type_reference(program, current, &substitutions);
        references.push((current, substitutions.len()));
    }
    let failure = |reason| StorageObservationTypePathError {
        segment_ordinal: path.len() + usize::from(field.is_valid()),
        segment: None,
        source_type,
        current_type: current,
        current_type_identity: observation_type_identity_text(program, current, &substitutions),
        reason,
    };
    if selected_case.is_some() {
        return Err(failure("field observation ends in an unprojected case"));
    }
    if atomic {
        let leaf = crate::peel_declared_constraints(program, current);
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(leaf)
        else {
            return Err(failure("atomic observation has no atomic leaf declaration"));
        };
        if !program
            .symbols
            .builtin_type_atom(*symbol)
            .is_some_and(|atom| atom.is_atomic())
        {
            return Err(failure(
                "atomic observation ends at an ordinary scalar leaf",
            ));
        }
        // Atomic fields have Atomic(ScalarType) representation inside their
        // owner; no standalone structural wrapper is manufactured for them.
        references.pop();
    }
    Ok(StorageObservationTypePath {
        entries: references,
        substitutions,
    })
}

fn observation_type_identity_text(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> String {
    program
        .type_identity_text(program.type_identity(
            typed_trees::type_identity::TypeIdentityRequest {
                substitutions,
                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
            },
        ))
        .unwrap_or_default()
}

fn observation_data_declaration<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Option<&'program typed_trees::data::DataDefinition> {
    use typed_trees::types::TypeReferenceNode;
    let exact_application = substitutions.is_empty()
        || program.normalized_type_identity(reference)
            == program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                substitutions,
                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
            });
    program
        .data_application_definition(reference)
        .filter(|_| exact_application)
        .or_else(|| {
            program
                .closed_data_definition(reference)
                .filter(|_| exact_application)
        })
        .or_else(|| {
            let symbol = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => return None,
            };
            unique_data_definition_by_symbol(program, symbol)
        })
}

pub fn canonical_place_type_reference(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    canonical_place_type_reference_observing(
        program,
        state_symbol,
        statement_index,
        place,
        &mut |_| Some(()),
    )
}

/// Resolve one canonical path while proving that its root and every retained
/// prefix, including the leaf, remain in owned storage. This is the same
/// canonical projection as ordinary type lookup; an unknown prefix or a
/// reference descriptor cannot supply ownership of its referent.
pub fn canonical_owned_place_type_reference(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    canonical_place_type_reference_observing(
        program,
        state_symbol,
        statement_index,
        place,
        &mut |reference| {
            let reference = reference?;
            crate::reference_shell(program, reference)
                .is_none()
                .then_some(())
        },
    )
}

fn canonical_place_type_reference_observing(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let facts::PlaceRoot::InvocationOutput(identity) = place.root {
        if !identity.is_valid() {
            return None;
        }
        return project_type_reference_from_segment_iter_observing(
            program,
            crate::invocation_output_parameter(program, identity)?.type_reference,
            &place.segments,
            observe_prefix,
        );
    }
    if let facts::PlaceRoot::InvocationReferent { type_reference, .. } = place.root {
        return project_type_reference_from_segment_iter_observing(
            program,
            type_reference,
            &place.segments,
            observe_prefix,
        );
    }
    // An expression root resumes wherever the position walk's leaf evidence
    // places it — a call at its declared return type, a stored-type leaf at
    // its own reference, a window at its element — then replays the retained
    // segments through the same hop rules member resolution commits to.
    if let Some(expression) = place.root.as_expression() {
        return super::member_resolution::expression_place_type_reference_observing(
            program,
            expression,
            &place.segments,
            observe_prefix,
        );
    }
    // A type-reference root names the place's own stored type: its declared
    // type is that reference replayed through the retained segments.
    if let Some(reference) = place.root.as_type_reference() {
        return project_type_reference_from_segment_iter_observing(
            program,
            reference,
            &place.segments,
            observe_prefix,
        );
    }
    let root_symbol = place.root.as_symbol()?;

    // `self` places may retain the authored state-parameter symbol or be
    // normalized to the durable machine symbol. In either form, project their
    // fields through the exact attached data declaration rather than treating
    // the machine's nominal self type as an ordinary data reference.
    if let Some((machine, _)) = crate::find_state(program, state_symbol)
        && machine.symbol == root_symbol
        && let Some((facts::PlaceSegment::Field { symbol }, remaining)) =
            place.segments.split_first()
    {
        // The logical empty prefix is still the machine-symbol query. Its
        // absence must not prevent ordinary field lookup, but cannot prove
        // that the whole canonical path is owned.
        observe_prefix(canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        ))?;
        if machine.attached_data_application.is_valid() {
            let mut projection = ProjectionTypeCursor::new(machine.attached_data_application);
            projection.project(program, &place.segments[0])?;
            return projection.walk(program, remaining, observe_prefix);
        }
        let current = attached_data_field_type_reference(program, machine, *symbol)?;
        return project_type_reference_from_segment_iter_observing(
            program,
            current,
            remaining,
            observe_prefix,
        );
    }

    if let Some((machine, _)) = crate::find_state(program, state_symbol)
        && machine.attached_data_application.is_valid()
        && let Some(field) = crate::exact_attached_field(
            program,
            machine,
            root_symbol,
            program.symbols.name(root_symbol),
        )
    {
        let root_field = facts::PlaceSegment::Field {
            symbol: field.symbol,
        };
        // The enclosing application is not a prefix of this field-rooted
        // place. Start observing only after its implicit field projection.
        let mut projection = ProjectionTypeCursor::new(machine.attached_data_application);
        projection.project(program, &root_field)?;
        return projection.walk(program, &place.segments, observe_prefix);
    }

    let current =
        symbol_type_reference_in_state(program, state_symbol, statement_index, root_symbol)?;
    project_type_reference_from_segment_iter_observing(
        program,
        current,
        &place.segments,
        observe_prefix,
    )
}

/// A semantic field occurrence keeps original declaration operands together
/// with their supplying applications. Storage projection above instead reads
/// prepared fields. Its substituted operands must not be combined with the
/// original telescopes when resolving exact regions or phantom arguments.
/// This query grants no loan, establishment or invocation permission.
pub fn canonical_place_source_type_application(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<typed_trees::types::TypeReferenceHandle>,
)> {
    let reference = canonical_place_type_reference(
        program,
        state_symbol,
        statement_index,
        &CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    )?;
    source_type_application_from_segments(program, reference, &place.segments)
}

/// Read a source declaration's supplying applications from an already resolved
/// root type. State and abstract-parameter scopes share this projection owner;
/// it grants no storage, lifetime or read authority.
pub fn source_type_application_from_segments(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)> {
    let mut applications = Vec::new();
    for segment in segments {
        let (projected, frames) =
            program.project_enclosing_data_type_reference(reference, &applications)?;
        reference = projected;
        applications.truncate(frames);
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                let parent = crate::unwrapped_type_reference(program, reference)?;
                let (parent, frames) =
                    program.project_enclosing_data_type_reference(parent, &applications)?;
                applications.truncate(frames);
                let owner = program
                    .type_reference_table
                    .type_reference(parent)
                    .type_symbol(&program.type_reference_table);
                let declaration = program.unique_data_definition_by_symbol(owner)?;
                applications.push(parent);
                reference = data_field_type_reference(program, declaration, *symbol)?;
            }
            facts::PlaceSegment::Case { .. } => {}
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                reference = collection_element_type_reference(program, reference)?;
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                return None;
            }
        }
    }
    let (reference, frames) =
        program.project_enclosing_data_type_reference(reference, &applications)?;
    applications.truncate(frames);
    Some((reference, applications))
}

pub fn project_type_reference_from_segments(
    program: &dyn crate::CheckingInput,
    current: typed_trees::types::TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    project_type_reference_from_segment_iter(program, current, segments)
}

/// Project only through the value's owned storage. A stored reference is a
/// copied descriptor boundary; its referee belongs to a separate live epoch.
/// Keep the same scoped generic environment across the complete projection.
pub fn owned_projection_type_reference(
    program: &dyn crate::CheckingInput,
    current: typed_trees::types::TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut projection = ProjectionTypeCursor::new(current);
    for segment in segments {
        if crate::reference_shell(program, projection.normalize(program)).is_some() {
            return None;
        }
        projection.advance(program, segment)?;
    }
    Some(projection.normalize(program))
}

/// Whether the slot carrying a loan belongs to this activation's binding or
/// owned aggregate. A reference in the final slot is the carried value; a
/// reference crossed before that slot instead reaches storage owned elsewhere.
/// Retained owner paths already rebase stores through tracked local aliases.
/// Unknown projection types cannot establish owned storage.
pub fn loan_carrier_has_owned_storage(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    owner_symbol: SymbolHandle,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
) -> bool {
    let Some(mut current) =
        symbol_type_reference_in_state(program, state_symbol, statement_index, owner_symbol)
    else {
        return false;
    };
    let mut substitutions = Vec::new();
    for segment in owner_path {
        current = substituted_type_reference(program, current, &substitutions);
        if crate::reference_shell(program, current).is_some() {
            return false;
        }
        let projected = match segment {
            checked_trees::BorrowLoanOwnerSegment::Case(_) => Some(current),
            checked_trees::BorrowLoanOwnerSegment::Field(symbol) => {
                field_type_reference(program, current, *symbol, &mut substitutions)
            }
            checked_trees::BorrowLoanOwnerSegment::FixedIndex(_)
            | checked_trees::BorrowLoanOwnerSegment::DynamicIndex => {
                indexed_element_type_reference(program, current, &substitutions)
            }
        };
        let Some(projected) = projected else {
            return false;
        };
        current = projected;
    }
    current.is_valid()
}

/// The element type a collection reference projects at an index hop — the
/// same element a half-open window shares with its collection, so
/// `&[Box<Context>]` resumes at `Box<Context>`. A reference with no element
/// (a record, a scalar, an unbound parameter) has none to resume at.
/// Numeric and nested collection elements inherit a non-Exact collection
/// policy only when they have no non-Exact policy of their own. Structural
/// elements retain their declaration. Typing keeps the complete qualification;
/// this query never substitutes a bare carrier or manufactures a new type.
pub fn collection_element_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    indexed_element_type_reference(program, type_reference, &[])
}

fn project_type_reference_from_segment_iter<'segment>(
    program: &dyn crate::CheckingInput,
    current: typed_trees::types::TypeReferenceHandle,
    segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    project_type_reference_from_segment_iter_observing(
        program,
        current,
        segments,
        &mut |_| Some(()),
    )
}

fn project_type_reference_from_segment_iter_observing<'segment>(
    program: &dyn crate::CheckingInput,
    current: typed_trees::types::TypeReferenceHandle,
    segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
    observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    ProjectionTypeCursor::new(current).walk(program, segments, observe_prefix)
}

/// Invocation-local projection scratch. Substitution visibility advances with
/// the path; a later field's bindings cannot reinterpret an earlier prefix.
struct ProjectionTypeCursor {
    current: typed_trees::types::TypeReferenceHandle,
    substitutions: Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
}

impl ProjectionTypeCursor {
    fn new(current: typed_trees::types::TypeReferenceHandle) -> Self {
        Self {
            current,
            substitutions: Vec::new(),
        }
    }

    fn normalize(
        &mut self,
        program: &dyn crate::CheckingInput,
    ) -> typed_trees::types::TypeReferenceHandle {
        self.current = substituted_type_reference(program, self.current, &self.substitutions);
        self.current
    }

    fn project(
        &mut self,
        program: &dyn crate::CheckingInput,
        segment: &facts::PlaceSegment,
    ) -> Option<()> {
        self.normalize(program);
        self.advance(program, segment)
    }

    fn advance(
        &mut self,
        program: &dyn crate::CheckingInput,
        segment: &facts::PlaceSegment,
    ) -> Option<()> {
        self.current = match segment {
            facts::PlaceSegment::Case { .. } => self.current,
            facts::PlaceSegment::Field { symbol } => {
                field_type_reference(program, self.current, *symbol, &mut self.substitutions)?
            }
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                indexed_element_type_reference(program, self.current, &self.substitutions)?
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                return None;
            }
        };
        Some(())
    }

    fn walk<'segment>(
        mut self,
        program: &dyn crate::CheckingInput,
        segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
        observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let mut reference = self.normalize(program);
        observe_prefix(Some(reference))?;
        for segment in segments {
            self.advance(program, segment)?;
            reference = self.normalize(program);
            observe_prefix(Some(reference))?;
        }
        Some(reference)
    }
}

fn symbol_type_reference_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let state = program.state_by_symbol(state_symbol)?;

    // State parameters and statement locals key their symbol parent to the
    // declaring state — the same storage invariant
    // `container_member_type_position` relies on — so a valid symbol whose
    // parent names another container (a machine member, a field, another
    // state's local) can never appear in this parameter list or statement
    // prefix. Skipping the scan on that definite miss keeps member-heavy
    // machines off the O(statements) prefix walk.
    let local_candidate = !symbol.is_valid()
        || program.symbols.get(symbol).parent == state_symbol
        || !program.symbols.get(symbol).parent.is_valid();

    if local_candidate {
        program
            .state_parameter_by_symbol(state, symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, _)| ordinal < statement_index)
                    .map(|(_, local_data)| local_data.type_reference)
            })
            .or_else(|| machine_member_type_reference(program, state_symbol, symbol))
    } else {
        machine_member_type_reference(program, state_symbol, symbol)
    }
}

fn machine_member_type_reference(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    crate::find_state(program, state_symbol).and_then(|(machine, _)| {
        program
            .machine_owned_data_by_symbol(machine, symbol)
            .map(|owned| owned.type_reference)
            .or_else(|| attached_data_field_type_reference(program, machine, symbol))
    })
}

fn attached_data_field_type_reference(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if machine.attached_data_application.is_valid() {
        return project_type_reference_from_segments(
            program,
            machine.attached_data_application,
            &[facts::PlaceSegment::Field { symbol }],
        );
    }
    let data = unique_data_definition_by_symbol(program, machine.attached_data_symbol)?;
    data_field_type_reference(program, data, symbol)
}

fn field_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    field_symbol: SymbolHandle,
    substitutions: &mut Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let type_reference = substituted_type_reference(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol, substitutions),
        typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            // A private scoped walk can carry outer bindings through a
            // referent not materialized by source preparation. Its raw type
            // row is not the applied occurrence when those bindings change
            // its identity, so that walk must retain its own substitutions.
            let exact_application = substitutions.is_empty()
                || program.normalized_type_identity(type_reference)
                    == program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                        substitutions,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(type_reference)
                    });
            if let Some(instance) = program
                .data_application_definition(type_reference)
                .filter(|_| exact_application)
                .or_else(|| program.closed_data_definition(type_reference))
            {
                // Canonical places use the selected instance's own fields.
                // Do not reinterpret a template or sibling-instance field as
                // though it belonged to this applied receiver. Symbolic views
                // retain caller binders just as closed views retain literals.
                return data_field_type_reference(program, instance, field_symbol);
            }
            let data = crate::field_domains::data_definition_by_symbol_or_name(
                program,
                *base_symbol,
                base_name,
            )?;
            let bindings: Vec<_> = program
                .data_type_parameters(data)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                )
                .filter(|&(parameter, _argument)| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                .map(|(parameter, argument)| {
                    (
                        parameter.symbol,
                        substituted_type_reference(program, *argument, substitutions),
                    )
                })
                .collect();
            substitutions.extend(bindings);
            data_field_type_reference(program, data, field_symbol)
        }
        typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => {
            if let Some(machine) = program.machine(*base_symbol) {
                attached_data_field_type_reference(program, machine, field_symbol)
            } else {
                crate::field_domains::data_definition_by_symbol_or_name(
                    program,
                    *base_symbol,
                    base_name,
                )
                .and_then(|data| data_field_type_reference(program, data, field_symbol))
            }
        }
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

fn indexed_element_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let type_reference = substituted_type_reference(program, type_reference, substitutions);
    let element = match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee, substitutions),
        typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
        | typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::Generic { .. }
        | typed_trees::types::TypeReferenceNode::Named { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Unit => None,
    }?;
    let collection_policy = program.arithmetic_domain_for_type_reference(type_reference);
    if program
        .type_reference_table
        .element_inherits_collection_arithmetic_policy(element, &program.symbols)
        && collection_policy != numerics::arithmetic::ArithmeticDomain::Exact
        && program.arithmetic_domain_for_type_reference(element)
            == numerics::arithmetic::ArithmeticDomain::Exact
    {
        // Typing retains the effective element shell. Reads, stores, literal
        // construction and views consume this same inherited policy rather
        // than reconstructing it after stripping the collection constraints.
        program
            .type_reference_table
            .find_policy_qualified_type_reference(element, collection_policy)
    } else {
        Some(element)
    }
}

fn substituted_type_reference(
    program: &dyn crate::CheckingInput,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> typed_trees::types::TypeReferenceHandle {
    let mut remaining = substitutions.len().saturating_add(1);
    while remaining > 0 {
        remaining -= 1;
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            break;
        };
        let Some(replacement) = substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| (*parameter == *symbol).then_some(*replacement))
        else {
            break;
        };
        if replacement == type_reference {
            break;
        }
        type_reference = replacement;
    }
    type_reference
}

/// Data-declaration identity is an exact premise: a lookup commits to one
/// declaration row, so a second row bearing the same symbol makes the premise
/// ambiguous. Refuse it rather than letting the first same-shaped row mint
/// evidence for the wrong subject. The representation's declaration index
/// keeps every position per symbol, so this is its unique query and not
/// another roster scan that must stay order-compatible with it.
pub fn unique_data_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.unique_data_definition_by_symbol(symbol)
}

fn data_field_type_reference(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if !field_symbol.is_valid() {
        return None;
    }

    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => {
                (field.symbol == field_symbol).then_some(field.type_reference)
            }
            typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| (field.symbol == field_symbol).then_some(field.type_reference)),
        })
}
