//! Source eligibility cannot be recovered from native layout: identical record
//! shapes may have different zero gates and nominal cleanup. Derive this private
//! correspondence before source erasure and bind it to the exact Terminal
//! context parameter and retained or erased projection.
//! It is not a standalone proof from source-free Terminal bytes or root authority.
//! Byte carriers erase their domain predicates, so empty-buffer eligibility must
//! rejoin the actual source field and evaluate every understood value constraint.

use checked_trees::data::{DataDefinition, DataMember, DataVariant};
use checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{CheckedTerminalMachineSelection, CheckedTrees};
use language_semantics::ReferenceAccess;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    CheckedProgramEntryDeterminedEvidenceField, CheckedProgramEntryFusedServiceField,
    CheckedProgramEntryReceiverCleanup, CheckedProgramEntryReceiverEligibility,
    CheckedProgramEntryReceiverProjection, StructuralAccess, StructuralFieldType,
    StructuralTypeShape, TerminalModule,
};

pub(super) fn derive(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
    module: &TerminalModule,
) -> Option<CheckedProgramEntryReceiverEligibility> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.machine)?;
    let state = checked.machine_states(machine).first()?;
    let source_parameters = checked.state_parameters(state);
    let mut receivers = source_parameters.iter().enumerate().filter(|(_, parameter)| {
        // An ordinary mutable record parameter can be provisioned by the
        // selected entry schema. This is eligibility, not root authority;
        // selection must still name and replay this exact source occurrence.
        // Do not strip refinements to manufacture zero-valid context storage.
        !parameter.relevance.is_erased()
            && matches!(
                checked.type_reference_table.type_reference(parameter.type_reference),
                TypeReferenceNode::Reference {
                    access: ReferenceAccess::Mutable,
                    referee,
                    ..
                } if matches!(checked.type_reference_table.type_reference(*referee), TypeReferenceNode::Named { .. })
            )
    });
    let (source_position, receiver) = receivers.next()?;
    if receivers.next().is_some() {
        return None;
    }
    let source_position = u32::try_from(source_position).ok()?;
    let mut owned = receiver.type_reference;
    loop {
        match checked.type_reference_table.type_reference(owned) {
            TypeReferenceNode::Reference { referee, .. } => owned = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => owned = *base_type,
            _ => break,
        }
    }
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(owned)
    else {
        return None;
    };
    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *symbol)?;
    // Source parameters name their nominal data directly. Rejoin that identity;
    // never infer eligibility from a matching layout or display name.
    if !checked.data_type_parameters(definition).is_empty()
        || validation::data_requires_establishment(&checked.typed, definition)
    {
        return None;
    }
    // Nominal cleanup does not reject outright: a retained receiver's hosted
    // extent can carry the obligation as ledger occupancy. Whether it must
    // is classified against the actual Terminal projection below.
    let requires_cleanup = validation::data_requires_nominal_drop(&checked.typed, definition);
    let terminal_position = ordinary_context_terminal_position(
        checked,
        selection,
        state.symbol,
        source_position,
        owned,
    )?;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)?;
    if entry.attachment.is_some() {
        return None;
    }
    let mut parameters = entry
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.position == terminal_position);
    let (terminal_receiver_type, projection) = match parameters.next() {
        Some(parameter) => {
            if parameters.next().is_some()
                || parameter.access != StructuralAccess::MutableBorrow
                || parameter.position != terminal_position
            {
                return None;
            }
            (
                parameter.structural_type,
                CheckedProgramEntryReceiverProjection::Retained {
                    terminal_self: parameter.place,
                    source_position,
                },
            )
        }
        None => {
            // Ordinary parameter erasure needs its own exact projection
            // evidence; absence alone cannot authorize implicit storage.
            return None;
        }
    };
    let cleanup = if requires_cleanup {
        if matches!(
            projection,
            CheckedProgramEntryReceiverProjection::Erased { .. }
        ) {
            // An erased receiver occupies no hosted extent, so nominal
            // cleanup has no ledger residence to track and still rejects.
            return None;
        }
        CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
    } else {
        CheckedProgramEntryReceiverCleanup::None
    };
    let mut structural_types = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == terminal_receiver_type);
    let structural_type = structural_types.next()?;
    if structural_types.next().is_some() {
        return None;
    }
    let owned_receiver_type_identity =
        checked.normalized_nominal_type_identity(definition.symbol)?;
    let StructuralTypeShape::Record { fields } = &structural_type.shape else {
        return None;
    };
    // Plain records and fixed primitive arrays need no initialization program.
    // Array eligibility follows its complete semantic element chain, including
    // empty dimensions; a zero byte count never excuses an invalid element.
    // Erased qualification establishment remains an independent installed
    // occurrence obligation; this correspondence supplies no Bound authority.
    if structural_type.identity != owned_receiver_type_identity.as_str()
        || !zero_valid_record_storage(
            checked,
            &module.structural_types,
            terminal_receiver_type,
            Some(definition),
            &mut Vec::new(),
            selection.machine,
        )
    {
        return None;
    }
    let mut fused_service_fields = Vec::new();
    collect_fused_service_fields(
        checked,
        &module.structural_types,
        definition,
        fields,
        &mut Vec::new(),
        &mut fused_service_fields,
    )?;
    let mut determined_evidence_fields = Vec::new();
    collect_determined_evidence_fields(
        checked,
        &module.structural_types,
        fields,
        selection.machine,
        &mut Vec::new(),
        &mut vec![terminal_receiver_type],
        &mut determined_evidence_fields,
    )?;
    if determined_evidence_fields.iter().any(|row| {
        fused_service_fields
            .iter()
            .any(|service| service.field_path() == row.field_path())
    }) {
        return None;
    }
    Some(
        CheckedProgramEntryReceiverEligibility::new(
            checked
                .normalized_type_identity(receiver.type_reference)
                .into_string(),
            owned_receiver_type_identity.into_string(),
            projection,
            terminal_receiver_type,
            fused_service_fields,
            cleanup,
        )
        .with_determined_evidence_fields(determined_evidence_fields),
    )
}

/// Source ordinals include scalar arguments; Terminal structural parameters
/// have their own dense order. Rejoin the checked signature instead of assuming
/// that a context is the first source argument or reusing a scalar ordinal.
fn ordinary_context_terminal_position(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
    state: symbols::SymbolHandle,
    source_position: u32,
    referent: TypeReferenceHandle,
) -> Option<u32> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut signatures = plans
        .machines
        .iter()
        .filter(|plan| plan.machine == selection.machine && plan.state == state)
        .map(|plan| plan.structural_parameters.as_slice())
        .chain(
            plans
                .composed_machines
                .iter()
                .filter(|plan| plan.machine == selection.machine)
                .flat_map(|plan| plan.states.iter())
                .filter(|plan| plan.state == state)
                .map(|plan| plan.structural_parameters.as_slice()),
        );
    let parameters = signatures.next()?;
    if signatures.next().is_some() {
        return None;
    }
    let mut matching = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.position == source_position);
    let (position, parameter) = matching.next()?;
    if matching.next().is_some()
        || parameter.access != checked_trees::CheckedStructuralAccess::MutableBorrow
        || parameter.type_identity != checked.normalized_type_identity(referent).as_str()
    {
        return None;
    }
    u32::try_from(position).ok()
}

/// Walk the receiver's Terminal field tree along the routes a roster row can
/// name -- record fields and mixed-shape common fields, never array elements
/// or case payloads -- and record one row per erased field whose evidence a
/// unique nullary constructor determines. The walk reads Terminal
/// declarations rather than source records so a closed generic owner such as
/// `Mixed<i32>` contributes its rows too; `nullary_determined_evidence`
/// rejoins each row to its exact source declaration by type identity. A
/// determined evidence type is never a `Binding` carrier, so these rows and
/// the fused-service rows name disjoint fields.
fn collect_determined_evidence_fields(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    entry: symbols::SymbolHandle,
    field_path: &mut Vec<String>,
    visiting: &mut Vec<StructuralTypeId>,
    rows: &mut Vec<CheckedProgramEntryDeterminedEvidenceField>,
) -> Option<()> {
    for field in fields {
        match &field.field_type {
            StructuralFieldType::Erased { type_identity }
                if field.relevance.is_erased()
                    && nullary_determined_evidence(checked, type_identity, entry) =>
            {
                field_path.push(field.identity.clone());
                if rows
                    .iter()
                    .any(|row| row.field_path() == field_path.as_slice())
                {
                    return None;
                }
                rows.push(CheckedProgramEntryDeterminedEvidenceField::new(
                    type_identity.clone(),
                    field_path.clone(),
                ));
                field_path.pop();
            }
            StructuralFieldType::Structural(child) if !visiting.contains(child) => {
                let mut nested = declarations
                    .iter()
                    .filter(|declaration| declaration.id == *child);
                let declaration = nested.next()?;
                if nested.next().is_some() {
                    return None;
                }
                let (StructuralTypeShape::Record {
                    fields: nested_fields,
                }
                | StructuralTypeShape::Mixed {
                    fields: nested_fields,
                    ..
                }) = &declaration.shape
                else {
                    continue;
                };
                field_path.push(field.identity.clone());
                visiting.push(*child);
                collect_determined_evidence_fields(
                    checked,
                    declarations,
                    nested_fields,
                    entry,
                    field_path,
                    visiting,
                    rows,
                )?;
                visiting.pop();
                field_path.pop();
            }
            _ => {}
        }
    }
    Some(())
}

/// The fused-service roster keeps the receiver's authored declaration order —
/// the receipt is read beside the source declaration, and entry settlement
/// rejoins each row by its exact identity pair rather than by position. Only
/// uniqueness is enforced: a repeated field route is refused, so two same-named
/// leaves under different record fields each keep their own row.
fn admit_fused_service_field(
    roster: &mut Vec<CheckedProgramEntryFusedServiceField>,
    field: CheckedProgramEntryFusedServiceField,
) -> bool {
    if roster
        .iter()
        .any(|existing| existing.field_path() == field.field_path())
    {
        return false;
    }
    roster.push(field);
    true
}

/// Walk the receiver's record-field tree: every authored bound-service field —
/// wherever nested record fields place it — must rejoin the same Terminal
/// erased or retained field shape and contributes one roster row keyed by its complete
/// field route. Plain nested-record fields only extend the route; every other
/// field kind contributes nothing and is skipped.
fn collect_fused_service_fields(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    source: &DataDefinition,
    fields: &[terminal_psi::StructuralFieldDeclaration],
    field_path: &mut Vec<String>,
    roster: &mut Vec<CheckedProgramEntryFusedServiceField>,
) -> Option<()> {
    for member in checked.data_members(source) {
        let DataMember::Field(field) = member else {
            continue;
        };
        let field_identity = field.path_identity();
        match checked
            .bound_service_parameter_carrier(field.type_reference)
            .ok()?
        {
            Some(carrier) => {
                let mut terminal_fields = fields
                    .iter()
                    .filter(|candidate| candidate.identity == field_identity);
                let terminal_field = terminal_fields.next()?;
                if terminal_fields.next().is_some()
                    || !terminal_semantics::fused_service_field_matches(
                        declarations,
                        terminal_field,
                        &carrier.carrier_type_identity,
                    )
                {
                    return None;
                }
                field_path.push(field_identity.clone());
                if !admit_fused_service_field(
                    roster,
                    CheckedProgramEntryFusedServiceField::new(
                        field_identity,
                        carrier.carrier_type_identity,
                        field_path.clone(),
                    ),
                ) {
                    return None;
                }
                field_path.pop();
            }
            None => {
                let Some(nested) = source_record(checked, field.type_reference) else {
                    continue;
                };
                let mut terminal_fields = fields
                    .iter()
                    .filter(|candidate| candidate.identity == field_identity);
                let structural_child = match (terminal_fields.next(), terminal_fields.next()) {
                    (Some(terminal_field), None) => match &terminal_field.field_type {
                        StructuralFieldType::Structural(child) => Some(*child),
                        _ => None,
                    },
                    _ => None,
                };
                let Some(child) = structural_child else {
                    // The subtree contributes no Terminal-visible route: a
                    // service leaf inside it could not name a roster row, so
                    // eligibility must refuse; a service-free subtree is
                    // skipped like every other non-carrier field kind.
                    if source_tree_has_service_field(checked, nested, &mut Vec::new()) {
                        return None;
                    }
                    continue;
                };
                let mut nested_declarations = declarations
                    .iter()
                    .filter(|declaration| declaration.id == child);
                let nested_declaration = nested_declarations.next()?;
                if nested_declarations.next().is_some() {
                    return None;
                }
                let nested_fields = match &nested_declaration.shape {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => continue,
                };
                field_path.push(field_identity);
                collect_fused_service_fields(
                    checked,
                    declarations,
                    nested,
                    nested_fields,
                    field_path,
                    roster,
                )?;
                field_path.pop();
            }
        }
    }
    Some(())
}

/// Whether the source record's field tree carries any bound-service field.
/// When a Terminal field cannot carry the nested route (no unique Structural
/// child), a service leaf inside it would still need a roster row the walk
/// cannot name, so eligibility must refuse rather than silently lose custody;
/// a service-free subtree is safe to skip. `visiting` bounds recursive
/// record cycles the same way `zero_valid` bound them.
fn source_tree_has_service_field(
    checked: &CheckedTrees,
    source: &DataDefinition,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    if visiting.contains(&source.symbol) {
        return false;
    }
    visiting.push(source.symbol);
    let has = checked.data_members(source).iter().any(|member| {
        let DataMember::Field(field) = member else {
            return false;
        };
        if matches!(
            checked.bound_service_parameter_carrier(field.type_reference),
            Ok(Some(_))
        ) {
            return true;
        }
        source_record(checked, field.type_reference)
            .map(|nested| source_tree_has_service_field(checked, nested, visiting))
            .unwrap_or(false)
    });
    visiting.pop();
    has
}

/// Whether zero-filled storage is an established value of the receiver's
/// structural declaration. Zero storage composes through record fields, fixed
/// arrays, and the payload of the zero-tag — first declared — sum case; the
/// same establishment rule `validation::data_requires_establishment` applies
/// to source is re-derived here against the exact Terminal declarations so a
/// module cannot smuggle an unestablished shape past the source-side gate.
fn zero_valid_record_storage(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    source: Option<&DataDefinition>,
    visiting: &mut Vec<StructuralTypeId>,
    entry: symbols::SymbolHandle,
) -> bool {
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let is_receiver = visiting.is_empty();
    visiting.push(structural_type);
    let valid = zero_valid_shape(
        checked,
        declarations,
        declaration,
        source,
        None,
        visiting,
        is_receiver,
        true,
        entry,
    );
    visiting.pop();
    valid
}

/// One nested declaration reached beneath the receiver. `source_reference` is
/// the exact source type reference for this node; a nominal reference rejoins
/// its data definition only when the declaration identity matches the
/// normalized source identity. Refined references stay unresolved rather than
/// stripping a constraint to reach nested storage. `admit_erased_services`
/// stays set only while the node sits on a record-field route from the
/// receiver — the route the fused-service roster can name — and clears at
/// array elements and case payloads.
fn zero_valid_node(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    source_reference: Option<TypeReferenceHandle>,
    visiting: &mut Vec<StructuralTypeId>,
    admit_erased_services: bool,
    entry: symbols::SymbolHandle,
) -> bool {
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let source = source_reference.and_then(|reference| {
        source_record(checked, reference).filter(|_| {
            declaration.identity == checked.normalized_type_identity(reference).into_string()
        })
    });
    visiting.push(structural_type);
    let valid = zero_valid_shape(
        checked,
        declarations,
        declaration,
        source,
        source_reference,
        visiting,
        false,
        admit_erased_services,
        entry,
    );
    visiting.pop();
    valid
}

/// The storage judgment for one declaration shape. `source` is the rejoined
/// source data definition (member lookup for record, common, and payload
/// fields); `source_reference` is the source type reference for this node
/// itself (element and byte-carrier checks).
fn zero_valid_shape(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    declaration: &terminal_psi::StructuralTypeDeclaration,
    source: Option<&DataDefinition>,
    source_reference: Option<TypeReferenceHandle>,
    visiting: &mut Vec<StructuralTypeId>,
    is_receiver: bool,
    admit_erased_services: bool,
    entry: symbols::SymbolHandle,
) -> bool {
    match &declaration.shape {
        // A binding leaf is admitted only by its exact source/roster join in
        // zero_valid_fields, never by generic recursion or zero-filled bytes.
        StructuralTypeShape::FusedServiceBinding => false,
        StructuralTypeShape::Record { fields } => zero_valid_fields(
            checked,
            declarations,
            fields,
            |field| source_field_type(checked, source, field),
            visiting,
            is_receiver,
            admit_erased_services,
            entry,
        ),
        StructuralTypeShape::FixedArray { element, .. } => {
            // Array eligibility follows the complete element chain at any
            // length; a zero element count never excuses an invalid element.
            // Element storage is not a record-field route the fused-service
            // roster can name, so nested erased service fields stay rejected.
            let element_reference = source_reference.and_then(|reference| {
                match checked.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray { element_type, .. } => Some(*element_type),
                    _ => None,
                }
            });
            zero_valid_node(
                checked,
                declarations,
                *element,
                element_reference,
                visiting,
                false,
                entry,
            )
        }
        StructuralTypeShape::Sum { cases } => {
            zero_valid_first_case(checked, declarations, source, cases, visiting, entry)
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            zero_valid_fields(
                checked,
                declarations,
                fields,
                |field| source_field_type(checked, source, field),
                visiting,
                is_receiver,
                admit_erased_services,
                entry,
            ) && zero_valid_first_case(checked, declarations, source, cases, visiting, entry)
        }
        StructuralTypeShape::PrimitiveScalar(scalar) => matches!(
            scalar,
            semantic_vocabulary::ScalarType::Boolean
                | semantic_vocabulary::ScalarType::Integer(_)
                | semantic_vocabulary::ScalarType::IeeeFloat(_)
        ),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity,
        }) => zero_valid_byte_field(checked, source_reference.unwrap_or_default(), *capacity),
        _ => false,
    }
}

/// Every field inhabiting zero storage must itself be zero-valid. A service
/// field instead rejoins its exact source carrier and independent establishment
/// roster. Only record-field routes can name that occurrence; neither its
/// legacy erasure nor its retained zero-payload marker establishes a value.
fn zero_valid_fields(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    field_reference: impl Fn(&terminal_psi::StructuralFieldDeclaration) -> TypeReferenceHandle,
    visiting: &mut Vec<StructuralTypeId>,
    is_receiver: bool,
    admit_erased_services: bool,
    entry: symbols::SymbolHandle,
) -> bool {
    fields.iter().all(|field| {
        // A source Binding leaf is established by the separate complete
        // service roster, never by recursively zero-filling its empty shape.
        if let Ok(Some(carrier)) = checked.bound_service_parameter_carrier(field_reference(field)) {
            return admit_erased_services
                && terminal_semantics::fused_service_field_matches(
                    declarations,
                    field,
                    &carrier.carrier_type_identity,
                );
        }
        // Determined evidence joins its own roster row on the same record-field
        // routes a service row can name (`collect_determined_evidence_fields`).
        let determined_erased = admit_erased_services
            && !is_receiver
            && field.relevance.is_erased()
            && matches!(&field.field_type, StructuralFieldType::Erased { type_identity }
                if nullary_determined_evidence(checked, type_identity, entry));
        (is_receiver || !field.relevance.is_erased() || determined_erased)
            && match field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0))
                }
                StructuralFieldType::RefinedScalar(_) => {
                    terminal_semantics::scalar_field_accepts_zero(&field.field_type)
                }
                StructuralFieldType::Structural(child) => {
                    let reference = field_reference(field);
                    zero_valid_node(
                        checked,
                        declarations,
                        child,
                        reference.is_valid().then_some(reference),
                        visiting,
                        admit_erased_services,
                        entry,
                    )
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ) => zero_valid_byte_field(checked, field_reference(field), capacity),
                StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    predicates,
                } => {
                    predicates.holds_for(&[])
                        && zero_valid_byte_field(checked, field_reference(field), capacity)
                }
                // Zero descriptor storage is the empty shared view: it
                // borrows no backing, so it owes no lifetime, and a declared
                // predicate must hold for the empty sequence
                // (structural_access.md, zero-initialized attachment storage).
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    },
                ) => zero_valid_view_field(checked, field_reference(field)),
                StructuralFieldType::PredicateByteSequence {
                    carrier:
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        },
                    predicates,
                } => {
                    predicates.holds_for(&[])
                        && zero_valid_view_field(checked, field_reference(field))
                }
                StructuralFieldType::Erased { .. } => is_receiver || determined_erased,
                StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::PredicateByteSequence { .. } => false,
            }
    })
}

/// A zero tag selects the first declared case: provisioning fills the four-byte
/// tag and the payload overlay with zeroes, so only `cases[0]` inhabits zero
/// storage. Later cases are unreachable here and are not judged.
fn zero_valid_first_case(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    source: Option<&DataDefinition>,
    cases: &[terminal_psi::StructuralCaseDeclaration],
    visiting: &mut Vec<StructuralTypeId>,
    entry: symbols::SymbolHandle,
) -> bool {
    let Some(case) = cases.first() else {
        return false;
    };
    let variant = source.and_then(|source| {
        checked.data_members(source).iter().find_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            (case.identity == variant.path_identity()).then_some(variant)
        })
    });
    // A case carrying its own `where` facts must construct through its
    // constructor; zero bytes alone cannot prove them.
    if variant.is_some_and(|variant| {
        !checked
            .proof_facts
            .span_or_empty(variant.where_facts)
            .is_empty()
    }) {
        return false;
    }
    zero_valid_fields(
        checked,
        declarations,
        &case.fields,
        |field| source_payload_field_type(checked, variant, field),
        visiting,
        false,
        false,
        entry,
    )
}

/// Whether the erased term of type `type_identity` is determined
/// structurally, so zero provisioning supplies it without discharging any
/// obligation. [Erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings)
/// let construction omit a term exactly when a visible accessible nullary
/// constructor determines it; there is no general inhabitance search.
/// Provisioning the receiver is that construction, so it applies the rule
/// stage 03's `nullary_erased_initializers` elaborates for literals: a
/// non-generic checked-shape declaration with no common fields and exactly
/// one payload-free case, which zero storage selects without an establishment
/// gate. The declaration must be public or share the entry's package, so
/// provisioning never mints evidence the entry could not construct itself.
fn nullary_determined_evidence(
    checked: &CheckedTrees,
    type_identity: &str,
    entry: symbols::SymbolHandle,
) -> bool {
    let mut declarations = checked.data_definitions().iter().filter(|definition| {
        checked
            .normalized_nominal_type_identity(definition.symbol)
            .is_some_and(|identity| identity.into_string() == type_identity)
    });
    let Some(evidence) = declarations.next() else {
        return false;
    };
    if declarations.next().is_some()
        || evidence.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || !checked.data_type_parameters(evidence).is_empty()
        || evidence.generic_instance.is_some()
        || validation::data_requires_establishment(&checked.typed, evidence)
        || !(evidence.is_public
            || checked.symbols.symbol_package_identity(evidence.symbol)
                == checked.symbols.symbol_package_identity(entry))
    {
        return false;
    }
    let members = checked.data_members(evidence);
    let mut nullary = members.iter().filter(|member| match member {
        DataMember::Variant(variant) => checked.data_payload_fields(variant).is_empty(),
        DataMember::Field(_) => false,
    });
    !members
        .iter()
        .any(|member| matches!(member, DataMember::Field(_)))
        && nullary.next().is_some()
        && nullary.next().is_none()
}

fn source_payload_field_type(
    checked: &CheckedTrees,
    variant: Option<&DataVariant>,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> TypeReferenceHandle {
    let Some(variant) = variant else {
        return TypeReferenceHandle::invalid();
    };
    checked
        .data_payload_fields(variant)
        .iter()
        .find_map(|candidate| {
            (field.identity == candidate.path_identity()).then_some(candidate.type_reference)
        })
        .unwrap_or_default()
}

fn source_field_type(
    checked: &CheckedTrees,
    source: Option<&DataDefinition>,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> TypeReferenceHandle {
    let Some(source) = source else {
        return TypeReferenceHandle::invalid();
    };
    checked
        .data_members(source)
        .iter()
        .find_map(|member| {
            let DataMember::Field(candidate) = member else {
                return None;
            };
            (field.identity == candidate.path_identity()).then_some(candidate.type_reference)
        })
        .unwrap_or_default()
}

fn source_record(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(reference)
    else {
        // Refined nominal paths need their own zero-membership evidence. Do
        // not strip those constraints merely to reach a nested byte field.
        return None;
    };
    checked.data_definitions().iter().find(|definition| {
        definition.symbol == *symbol
            && checked.data_type_parameters(definition).is_empty()
            // Checked zero membership, not absence of authored predicates,
            // decides whether this nominal source can describe initial storage.
            && !validation::data_requires_establishment(&checked.typed, definition)
    })
}

fn zero_valid_byte_field(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
    capacity: u64,
) -> bool {
    // Stable contents classify every constraint and exclude authority-bearing,
    // routed and unknown refinements. Stability alone is not membership: the
    // same resolved predicate denotation used by source checking must accept
    // the empty value for every domain, not merely one named Utf8.
    if !validation::has_contents(
        &checked.typed,
        reference,
        validation::ContentsRequirement::StableObservation,
    ) {
        return false;
    }
    let predicates =
        checked_trees::byte_predicates::type_reference_domain_predicates(&checked.typed, reference);
    if predicates.is_empty()
        || !predicates
            .iter()
            .all(|(_, predicate)| predicate.is_some_and(|predicate| predicate.holds_for(&[])))
    {
        return false;
    }
    let mut carrier = reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    matches!(
        checked.type_reference_table.type_reference(carrier),
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if u64::try_from(*length).ok() == Some(capacity)
            && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}

/// Zero descriptor storage is the empty view. Rejoin the source field: it
/// must declare a shared `&[u8]` view, and every declared domain must be a
/// byte predicate the empty sequence satisfies.
fn zero_valid_view_field(checked: &CheckedTrees, reference: TypeReferenceHandle) -> bool {
    if !reference.is_valid()
        || !checked_trees::byte_predicates::type_reference_domain_predicates(
            &checked.typed,
            reference,
        )
        .iter()
        .all(|(_, predicate)| predicate.is_some_and(|predicate| predicate.holds_for(&[])))
    {
        return false;
    }
    // `&[u8] in D` constrains the referee, `(&[u8]) in D` the reference;
    // either spelling denotes the same shared byte view.
    let unconstrained = |mut carrier: TypeReferenceHandle| {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            checked.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        carrier
    };
    let TypeReferenceNode::Reference {
        access: ReferenceAccess::Shared,
        referee,
        ..
    } = checked
        .type_reference_table
        .type_reference(unconstrained(reference))
    else {
        return false;
    };
    matches!(
        checked.type_reference_table.type_reference(unconstrained(*referee)),
        TypeReferenceNode::Slice { element_type }
            if checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}
