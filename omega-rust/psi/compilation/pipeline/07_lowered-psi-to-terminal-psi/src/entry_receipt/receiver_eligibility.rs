//! Source eligibility cannot be recovered from native layout: identical record
//! shapes may have different zero gates and nominal cleanup. Derive this private
//! correspondence before source erasure and bind it to the exact Terminal
//! attachment and retained or erased receiver projection.
//! It is not a standalone proof from source-free Terminal bytes or root authority.
//! Byte carriers erase their domain predicates, so empty-buffer eligibility must
//! rejoin the actual source field and evaluate every understood value constraint.

use checked_trees::data::{DataDefinition, DataMember, DataVariant};
use checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{CheckedTerminalMachineSelection, CheckedTrees};
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
    let mut receivers = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self);
    let receiver = receivers.next()?;
    if receivers.next().is_some() || !receiver.is_mutable {
        return None;
    }
    let source_position = u32::try_from(
        checked
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.is_self)?,
    )
    .ok()?;
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
        .find(|definition| definition.symbol == machine.attached_data_symbol)?;
    // Resolved Self names its enclosing machine, not the data declaration.
    // Explicit nominal receivers name the data directly. Both must rejoin the
    // exact attachment; a matching source display name is insufficient.
    if (*symbol != machine.symbol && *symbol != definition.symbol)
        || machine.attached_data.as_ref() != Some(&definition.name)
        || !checked.data_type_parameters(definition).is_empty()
        || validation::data_requires_establishment(&checked.typed, definition)
    {
        return None;
    }
    // Nominal cleanup does not reject outright: a retained receiver's hosted
    // extent can carry the obligation as ledger occupancy. Whether it must
    // is classified against the actual Terminal projection below.
    let requires_cleanup = validation::data_requires_nominal_drop(&checked.typed, definition);
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)?;
    let terminal_receiver_type = entry.attachment?;
    let mut parameters = entry
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self);
    let projection = match parameters.next() {
        Some(parameter) => {
            if parameters.next().is_some()
                || parameter.access != StructuralAccess::MutableBorrow
                || parameter.position != source_position
                || parameter.structural_type != terminal_receiver_type
            {
                return None;
            }
            CheckedProgramEntryReceiverProjection::Retained {
                terminal_self: parameter.place,
                source_position,
            }
        }
        None => {
            // Clearing the marker on a surviving parameter is not erasure.
            // Structural positions retain their authored source positions,
            // independently of the compact scalar parameter partition.
            if entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.position == source_position)
            {
                return None;
            }
            CheckedProgramEntryReceiverProjection::Erased { source_position }
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
    let owner_path = checked.symbols.display_path(definition.symbol, "::");
    let mut owned_receiver_type_identity = String::from("named(name(");
    for character in owner_path.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            owned_receiver_type_identity.push('\\');
        }
        owned_receiver_type_identity.push(character);
    }
    owned_receiver_type_identity.push_str("))");
    let StructuralTypeShape::Record { fields } = &structural_type.shape else {
        return None;
    };
    // Plain records and fixed primitive arrays need no initialization program.
    // Array eligibility follows its complete semantic element chain, including
    // empty dimensions; a zero byte count never excuses an invalid element.
    // Erased qualification establishment remains an independent installed
    // occurrence obligation; this correspondence supplies no Bound authority.
    if structural_type.identity != owned_receiver_type_identity
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
            owned_receiver_type_identity,
            projection,
            terminal_receiver_type,
            fused_service_fields,
            cleanup,
        )
        .with_determined_evidence_fields(determined_evidence_fields),
    )
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
/// `Erased` field shape and contributes one roster row keyed by its complete
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
                    || !matches!(
                        &terminal_field.field_type,
                        StructuralFieldType::Erased { type_identity }
                            if type_identity == &carrier.carrier_type_identity
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

/// Every field inhabiting zero storage must itself be zero-valid. Erased
/// fields carry obligations this storage cannot discharge: the receiver's own
/// erased fields join fused-service establishment rows directly, and a nested
/// erased field joins the same rows only on a record-field route the roster
/// can name — detected by the erased carrier type rejoining a bound-service
/// source carrier, since the carrier erasure keeps the authored relevance.
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
        let nested_erased_service = admit_erased_services
            && !is_receiver
            && matches!(field.field_type, StructuralFieldType::Erased { .. })
            && checked
                .bound_service_parameter_carrier(field_reference(field))
                .is_ok_and(|carrier| carrier.is_some());
        // Determined evidence joins its own roster row on the same record-field
        // routes a service row can name (`collect_determined_evidence_fields`).
        let determined_erased = admit_erased_services
            && !is_receiver
            && field.relevance.is_erased()
            && matches!(&field.field_type, StructuralFieldType::Erased { type_identity }
                if nullary_determined_evidence(checked, type_identity, entry));
        (is_receiver || !field.relevance.is_erased() || nested_erased_service || determined_erased)
            && match field.field_type {
                StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0))
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
                StructuralFieldType::Erased { .. } => {
                    is_receiver || nested_erased_service || determined_erased
                }
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
    if !validation::has_stable_observable_contents(&checked.typed, reference) {
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

#[cfg(test)]
mod tests {
    use super::{
        CheckedProgramEntryFusedServiceField, CheckedProgramEntryReceiverProjection, CheckedTrees,
        StructuralFieldType, StructuralTypeShape, admit_fused_service_field, derive,
    };
    use checked_trees_to_lowered_psi::TerminalMachineSelection;
    use terminal_production::TerminalProductionCustody;
    use terminal_production::TerminalProductionRequest;
    use terminal_production::TerminalProductionTimings;

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap()
    }

    const SOURCE: &str =
        "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }";

    /// The determined-evidence rows of `source`'s `Main::run` receiver, or
    /// `None` when the receiver is not zero-eligible.
    fn determined_evidence_rows(source: &str) -> Option<Vec<Vec<String>>> {
        let checked = check_source(source);
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        produced
            .receipt()
            .expect("entry receipt")
            .receiver_eligibility()
            .map(|eligibility| {
                eligibility
                    .determined_evidence_fields()
                    .iter()
                    .map(|row| row.field_path().to_vec())
                    .collect()
            })
    }

    /// Erased evidence a unique nullary constructor determines is supplied by
    /// provisioning, as construction supplies an omitted erased term. Each
    /// such field on a nameable route -- a nested record field or a mixed
    /// shape's common field -- joins its own row. Ambiguous evidence
    /// determines nothing, and a case payload is not a nameable route, so
    /// both leave the receiver ineligible.
    #[test]
    fn determined_erased_evidence_joins_its_own_receipt_rows() {
        let rows = determined_evidence_rows(
            "data Evidence { case Only; }
             data Signal { proof [erased]: Evidence; case Red; case Green; }
             data Holder { value: i32; proof [erased]: Evidence; }
             data Main { value: i32; signal: Signal; holder: Holder; }
             machine Main::run(&mut self) { self.value = 7; }",
        )
        .expect("determined evidence leaves the receiver zero-eligible");
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows.iter().all(|path| path.len() == 2), "{rows:?}");
        assert_ne!(rows[0][0], rows[1][0], "one row per enclosing route");

        for ineligible in [
            "data Evidence { case First; case Second; }
             data Holder { value: i32; proof [erased]: Evidence; }
             data Main { value: i32; holder: Holder; }
             machine Main::run(&mut self) { self.value = 7; }",
            "data Evidence { case Only; }
             data Signal { case Red(proof [erased]: Evidence); case Green; }
             data Main { value: i32; signal: Signal; }
             machine Main::run(&mut self) { self.value = 7; }",
        ] {
            assert_eq!(determined_evidence_rows(ineligible), None, "{ineligible}");
        }
    }

    #[test]
    fn byte_receiver_zero_eligibility_rejoins_direct_and_named_nested_fields() {
        for (capacity, predicate) in [(3, "valid_utf8"), (9, "ascii_only"), (0, "no_nul")] {
            let checked = check_source(&format!(
                "domain [u8; {capacity}]::SafeBytes requires {predicate}(self);
                 data Child {{ #7 bytes: [u8; {capacity}] in SafeBytes; }}
                 data Main {{ value: i32; direct: [u8; {capacity}] in SafeBytes; child: Child; }}
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            assert!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_some(),
                "{predicate}"
            );
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            for corruption in 0..4 {
                let mut changed = module.clone();
                let declaration = changed
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.identity == "named(name(Child))")
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
                    panic!("child record");
                };
                match corruption {
                    0 => fields[0].identity = "bytes".into(),
                    1 => {
                        fields[0].field_type = StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                capacity: capacity + 1,
                            },
                        )
                    }
                    2 => {
                        fields[0].field_type = StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                            },
                        )
                    }
                    _ => declaration.identity = "named(name(OtherChild))".into(),
                }
                assert!(
                    derive(&checked, selection, &changed).is_none(),
                    "{predicate}: {corruption}"
                );
            }
        }
    }

    #[test]
    fn nested_byte_receiver_rejoins_zero_valid_where_constraints() {
        let source = |minimum| {
            format!(
                "domain [u8; 5]::Text requires valid_utf8(self);
             data Entry where {minimum} <= score, score <= 100 {{
                 name: [u8; 5] in Text; score: i32;
             }}
             data Entries {{ first: Entry; second: Entry; }}
             data Main {{ value: i32; entries: Entries; }}
             machine Main::run(&mut self) {{ self.value = 7; }}"
            )
        };
        let checked = check_source(&source(0));
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(produced.receipt().unwrap().receiver_eligibility().is_some());
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        // The same nominal layout cannot manufacture membership when zero no
        // longer satisfies the source's nested default-domain constraint.
        let gated = check_source(&source(1));
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &gated,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        assert!(derive(&gated, selection, &module).is_none());
    }

    #[test]
    fn byte_receiver_cannot_gain_domain_membership_from_a_relabelled_raw_array() {
        let checked = check_source(
            "data Main { value: i32; bytes: [u8; 3]; }
             machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(
            produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .is_some()
        );
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let declaration = module
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.identity == "named(name(Main))")
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
            panic!("receiver record");
        };
        fields
            .iter_mut()
            .find(|field| field.identity == "bytes")
            .unwrap()
            .field_type =
            StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 3,
            });
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &checked,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        assert!(derive(&checked, selection, &module).is_none());
    }

    #[test]
    fn byte_receiver_zero_eligibility_requires_every_resolved_domain_predicate() {
        for (domains, constraint, eligible) in [
            (
                "domain [u8; 3]::First requires valid_utf8(self); domain [u8; 3]::Second requires no_nul(self);",
                "First & Second",
                true,
            ),
            (
                "domain [u8; 3]::Utf8 requires non_empty(self);",
                "Utf8",
                false,
            ),
            (
                "domain [u8; 3]::First requires valid_utf8(self); domain [u8; 3]::Second requires non_empty(self);",
                "First & Second",
                false,
            ),
            (
                "domain [u8; 3]::Extra requires valid_utf8(self); non_empty(self);",
                "Extra",
                false,
            ),
        ] {
            let checked = check_source(&format!(
                "{domains}
                 data Child {{ bytes: [u8; 3] in {constraint}; }}
                 data Main {{ value: i32; child: Child; }}
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            assert_eq!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_some(),
                eligible,
                "{domains}"
            );
        }
    }

    #[test]
    fn nested_receiver_storage_requires_complete_zero_valid_records() {
        let checked = check_source(
            "data Counter { value: i32; bytes: [u8; 4]; } data Pair { first: Counter; second: Counter; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(
            produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .is_some()
        );
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &checked,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        let counter = module
            .structural_types
            .iter()
            .position(|declaration| declaration.identity == "named(name(Counter))")
            .unwrap();
        for corruption in 0..4 {
            let mut changed = module.clone();
            let counter_id = changed.structural_types[counter].id;
            if corruption == 0 {
                changed.structural_types.remove(counter);
            } else {
                let StructuralTypeShape::Record { fields } =
                    &mut changed.structural_types[counter].shape
                else {
                    panic!("record");
                };
                fields[0].field_type = match corruption {
                    1 => StructuralFieldType::Structural(counter_id),
                    2 => StructuralFieldType::Erased {
                        type_identity: "nested-service".into(),
                    },
                    _ => StructuralFieldType::BoundedInteger(
                        semantic_vocabulary::BoundedIntegerType::new(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Signed,
                                32,
                            )
                            .unwrap(),
                            semantic_vocabulary::IntegerValue::Signed(1),
                            semantic_vocabulary::IntegerValue::Signed(9),
                        )
                        .unwrap(),
                    ),
                };
            }
            assert!(
                derive(&checked, selection, &changed).is_none(),
                "corruption {corruption}"
            );
        }
    }

    #[test]
    fn receiver_array_eligibility_checks_the_complete_element_chain() {
        for array in ["[u8; 256]", "[[u16; 3]; 2]", "[u8; 0]"] {
            let checked = check_source(&format!(
                "data Main {{ value: i32; values: {array}; }} \
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            assert!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_some(),
                "{array}"
            );
            let mut module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let array = module
                .structural_types
                .iter_mut()
                .find(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                })
                .unwrap();
            let StructuralTypeShape::FixedArray { element, .. } = &mut array.shape else {
                unreachable!();
            };
            *element = array.id;
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            assert!(
                derive(&checked, selection, &module).is_none(),
                "cyclic elements reject even beneath an empty dimension"
            );
        }
    }

    #[test]
    fn receiver_sum_and_mixed_storage_follow_the_zero_tag_first_case() {
        // Zero-filled sum storage selects the first declared case, so the
        // payload of that case — and any record/array composed beneath it —
        // must itself be an established zero value.
        for source in [
            "data Event { case Quiet; case Loud(gain: i32); } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            "data Event { case Loud(gain: i32); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { level: i32; } data Event { case Hit(child: Child); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            "data Room { occupied: bool; case Empty; case Full(load: i32); } data Main { value: i32; room: Room; } machine Main::run(&mut self) { self.value = 7; }",
            "domain [u8; 3]::SafeBytes requires valid_utf8(self); data Event { case Say(text: [u8; 3] in SafeBytes, count: i32); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            "data Event { case Quiet; case Loud(gain: i32); } data Wrap { event: Event; } data Main { value: i32; wrap: Wrap; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            assert!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_some(),
                "{source}"
            );
        }
    }

    #[test]
    fn receiver_record_array_storage_checks_the_complete_element_chain() {
        // A record element array is admissible when the element record's
        // transitive storage is zero-valid; the element judgment follows the
        // complete chain through every dimension.
        for source in [
            "data Pair { first: i32; second: i32; } data Main { value: i32; grid: [Pair; 2]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Pair { first: i32; second: i32; } data Main { value: i32; grid: [[Pair; 2]; 3]; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
            assert!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_some(),
                "{source}"
            );
        }
        // A byte domain on the element record that refuses empty contents
        // rejects through the same element chain.
        let checked = check_source(
            "domain [u8; 3]::NonEmpty requires non_empty(self); data Pair { bytes: [u8; 3] in NonEmpty; } data Main { value: i32; grid: [Pair; 2]; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(
            produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .is_none()
        );
    }

    #[test]
    fn receiver_sum_storage_rejects_payloads_zero_cannot_establish() {
        for source in [
            // A byte domain that refuses the empty carrier beneath the zero tag.
            "domain [u8; 3]::NonEmpty requires non_empty(self); data Event { case Say(text: [u8; 3] in NonEmpty); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            // A borrowed view payload has no owned bytes to provision.
            "data Event { case Say(text: &[u8]); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
            // A range-gated payload beneath the zero tag is not an established
            // value; the same source gate also rejects this receiver earlier.
            "data Event { case Loud(gain: i32 [1..=9]); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([7; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            assert!(
                produced
                    .receipt()
                    .expect("entry receipt")
                    .receiver_eligibility()
                    .is_none(),
                "{source}"
            );
        }
    }

    #[test]
    fn receiver_sum_eligibility_rejoins_exact_case_and_payload_storage() {
        let checked = check_source(
            "domain [u8; 3]::SafeBytes requires valid_utf8(self); data Event { case Say(text: [u8; 3] in SafeBytes, count: i32); case Quiet; } data Main { value: i32; event: Event; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        let eligible = produced
            .receipt()
            .expect("entry receipt")
            .receiver_eligibility()
            .unwrap();
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &checked,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        for corruption in 0..4 {
            let mut changed = module.clone();
            let declaration = changed
                .structural_types
                .iter_mut()
                .find(|declaration| declaration.identity == "named(name(Event))")
                .unwrap();
            let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                panic!("sum");
            };
            match corruption {
                // The zero-tag case must rejoin the source variant that owns
                // its payload fields; relabelling erases the byte domain.
                0 => cases[0].identity = "Quiet".into(),
                1 => cases[0].identity = "Unknown".into(),
                // A payload field must rejoin its exact source field; renaming
                // to a sibling field swaps the byte carrier's domain away.
                2 => cases[0].fields[0].identity = "count".into(),
                // The inhabiting case's payload storage must stay zero-valid.
                _ => {
                    cases[0].fields[1].field_type = StructuralFieldType::BoundedInteger(
                        semantic_vocabulary::BoundedIntegerType::new(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Signed,
                                32,
                            )
                            .unwrap(),
                            semantic_vocabulary::IntegerValue::Signed(1),
                            semantic_vocabulary::IntegerValue::Signed(9),
                        )
                        .unwrap(),
                    )
                }
            }
            assert!(
                derive(&checked, selection, &changed).is_none(),
                "corruption {corruption}"
            );
        }
        // A case beyond the zero tag never inhabits provisioned storage:
        // corrupting its payload cannot revoke eligibility.
        let mut changed = module.clone();
        let declaration = changed
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.identity == "named(name(Event))")
            .unwrap();
        let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
            panic!("sum");
        };
        cases[1]
            .fields
            .push(terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(90).unwrap(),
                identity: "smuggled".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Erased {
                    type_identity: "unestablished".into(),
                },
            });
        assert_eq!(
            derive(&checked, selection, &changed).as_ref(),
            Some(eligible)
        );
    }

    #[test]
    fn source_receiver_eligibility_rejoins_exact_terminal_self() {
        let checked = check_source(SOURCE);
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        let receipt = produced.receipt().expect("entry receipt");
        let eligible = receipt
            .receiver_eligibility()
            .expect("plain source record needs no executable cleanup");
        assert!(matches!(
            eligible.projection(),
            CheckedProgramEntryReceiverProjection::Retained {
                source_position: 0,
                ..
            }
        ));
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &checked,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        assert_eq!(
            derive(&checked, selection, &module).as_ref(),
            Some(eligible)
        );
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        entry.structural_parameters[0].position += 1;
        assert!(derive(&checked, selection, &module).is_none());
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        entry.structural_parameters[0].is_self = false;
        assert!(
            derive(&checked, selection, &module).is_none(),
            "a retained receiver cannot become erased by losing its self marker"
        );
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        module
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == eligible.terminal_receiver_type())
            .unwrap()
            .identity = "different-owner".into();
        assert!(derive(&checked, selection, &module).is_none());
    }

    #[test]
    fn erased_receiver_eligibility_preserves_exact_attachment_without_a_place() {
        for source in [
            "data Main {} machine Main::run(&mut self) {}",
            "data Main { value: i32; bytes: [u8; 4]; } machine Main::run(&mut self) {}",
        ] {
            let checked = check_source(source);
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([9; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            let eligible = produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .unwrap();
            assert_eq!(
                eligible.projection(),
                CheckedProgramEntryReceiverProjection::Erased { source_position: 0 }
            );
            assert_eq!(eligible.owned_receiver_type_identity(), "named(name(Main))");
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let entry = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            assert_eq!(entry.attachment, Some(eligible.terminal_receiver_type()));
            assert!(
                entry
                    .structural_parameters
                    .iter()
                    .all(|parameter| !parameter.is_self)
            );
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            for corruption in 0..3 {
                let mut changed = module.clone();
                if corruption == 0 {
                    changed
                        .machines
                        .iter_mut()
                        .find(|machine| machine.id == changed.entry)
                        .unwrap()
                        .attachment = None;
                } else {
                    let declaration = changed
                        .structural_types
                        .iter_mut()
                        .find(|declaration| declaration.id == eligible.terminal_receiver_type())
                        .unwrap();
                    if corruption == 1 {
                        declaration.identity = "different-owner".into();
                    } else {
                        let duplicate = declaration.clone();
                        changed.structural_types.push(duplicate);
                    }
                }
                assert!(
                    derive(&checked, selection, &changed).is_none(),
                    "erased receiver attachment corruption {corruption}"
                );
            }
        }
    }

    #[test]
    fn ordinary_erased_fields_require_no_fused_service_establishment() {
        for body in ["", "self.value = 7;"] {
            let checked = check_source(&format!(
                "data Evidence {{}} data Main {{ value: i32; proof [erased]: Evidence; }} \
                 machine Main::run(&mut self) {{ {body} }}"
            ));
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([9; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            let eligible = produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .unwrap();
            assert!(eligible.fused_service_fields().is_empty());
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == eligible.terminal_receiver_type())
                .unwrap();
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                panic!("receiver remains a record");
            };
            assert!(
                fields.iter().any(|field| {
                    matches!(field.field_type, StructuralFieldType::Erased { .. })
                })
            );
        }
    }

    #[test]
    fn erased_receivers_cannot_hide_nominal_cleanup_or_nonzero_gates() {
        let checked = check_source("data Main { value: i32; } machine Main::run(&mut self) {}");
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([9; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        // The erased Terminal attachment cannot establish source obligations;
        // derive must inspect the actual nominal owner even without a self place.
        for source in [
            "data Helper {} machine Helper::finish() {} data Main { value: i32; } machine Main::drop(&mut self) { Helper::finish(); } machine Main::run(&mut self) {}",
            "data Child {} machine Child::drop(&mut self) {} data Main { value: i32; child: Child; } machine Main::run(&mut self) {}",
            "data Main { value: i32 [1..=9]; } machine Main::run(&mut self) {}",
            "data Child { value: i32 [1..=9]; } data Main { value: i32; child: Child; } machine Main::run(&mut self) {}",
        ] {
            let checked = check_source(source);
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            assert!(derive(&checked, selection, &module).is_none(), "{source}");
        }
    }

    #[test]
    fn retained_receiver_nominal_cleanup_classifies_hosted_extent_occupancy() {
        let base = check_source(SOURCE);
        let artifact =
            TerminalProductionRequest::new(&base, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        let module = terminal_codec::decode_module(artifact.artifact().semantic_bytes()).unwrap();
        // A retained receiver's hosted extent can carry the cleanup
        // obligation as ledger occupancy: nominal cleanup classifies into
        // `OccupiesHostedExtent` rather than rejecting eligibility outright.
        for source in [
            "data Main { value: i32; } machine Main::drop(&mut self) {} machine Main::run(&mut self) { self.value = 7; }",
            "data Child {} machine Child::drop(&mut self) {} data Main { value: i32; child: Child; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32; } machine Child::drop(&mut self) {} data Main { value: i32; values: [Child; 2]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32; } machine Child::drop(&mut self) {} data Pair { child: Child; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            let eligibility = derive(&checked, selection, &module)
                .unwrap_or_else(|| panic!("{source} must classify cleanup occupancy"));
            assert_eq!(
                eligibility.cleanup(),
                terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent,
                "{source}"
            );
            assert!(matches!(
                eligibility.projection(),
                terminal_psi::CheckedProgramEntryReceiverProjection::Retained { .. }
            ));
        }
        let checked = check_source(SOURCE);
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &checked,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        assert_eq!(
            derive(&checked, selection, &module).unwrap().cleanup(),
            terminal_psi::CheckedProgramEntryReceiverCleanup::None
        );
    }

    #[test]
    fn source_nonzero_gates_cannot_acquire_receiver_eligibility() {
        let base = check_source(SOURCE);
        let artifact =
            TerminalProductionRequest::new(&base, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        let module = terminal_codec::decode_module(artifact.artifact().semantic_bytes()).unwrap();
        // A same-named Terminal record cannot prove source establishment
        // absence: nonzero gates still reject on the actual source type graph.
        for source in [
            "data Main { value: i32 [1..=9]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Main { value: i32; values: [i32 [1..=9]; 2]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32 [1..=9]; } data Pair { child: Child; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let selection = checked_trees_to_lowered_psi::select_terminal_machine(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .unwrap();
            assert!(derive(&checked, selection, &module).is_none());
        }
        let mut missing = check_source(SOURCE);
        missing.typed.roots.data_definitions = Default::default();
        let selection = checked_trees_to_lowered_psi::select_terminal_machine(
            &missing,
            TerminalMachineSelection::Name("Main::run"),
        )
        .unwrap();
        assert!(derive(&missing, selection, &module).is_none());
    }

    #[test]
    fn receiver_eligibility_follows_attachment_not_unrelated_cleanup() {
        for source in [
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 8; }",
            "data Other {} machine Other::drop(&mut self) {} data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::run")
                .unwrap();
            let produced = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Symbol(machine.symbol),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([9; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            })
            .unwrap();
            let eligible = produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .unwrap();
            assert_eq!(eligible.owned_receiver_type_identity(), "named(name(Main))");
            assert_ne!(
                eligible.source_receiver_type_identity(),
                eligible.owned_receiver_type_identity()
            );
        }
        let checked = check_source("data Main {} machine Main::run() {}");
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([9; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(
            produced
                .receipt()
                .expect("entry receipt")
                .receiver_eligibility()
                .is_none(),
            "a free entry does not acquire an implicit receiver"
        );
    }

    #[test]
    fn non_exclusive_receiver_accesses_cannot_acquire_receiver_eligibility() {
        // The bridge provisions exclusive private storage and hands it to the
        // continuation as a mutable borrow, so only `&mut self` can match.
        // Other accesses are refused before or during production — where the
        // verifier flags a unit return discarding the receiver — and never
        // acquire eligibility.
        for source in [
            "data Main { value: i32; } machine Main::run(&self) {}",
            "data Main { value: i32; } machine Main::run(self) {}",
        ] {
            let checked = check_source(source);
            if let Ok(produced) = TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("Main::run"),
            )
            .produce(TerminalProductionCustody {
                retain_unoptimized: false,
                entry_identity: Some([9; 32]),
                callback_custody: (),
                timings: &mut TerminalProductionTimings::default(),
            }) {
                assert!(
                    produced
                        .receipt()
                        .expect("entry receipt")
                        .receiver_eligibility()
                        .is_none(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn fused_service_roster_keeps_authored_order_and_refuses_duplicates() {
        // Explicit numeric identities are strings like "#10" and "#2"; a
        // lexicographic ordering lists "#10" before "#2" and all "#N" before
        // any named field, diverging from the authored declaration order.
        let mut roster = Vec::new();
        for (identity, accepted) in [
            ("#2", true),
            ("#10", true),
            ("console", true),
            ("#2", false),
            ("console", false),
        ] {
            assert_eq!(
                admit_fused_service_field(
                    &mut roster,
                    CheckedProgramEntryFusedServiceField::new(
                        identity.to_owned(),
                        "carrier".to_owned(),
                        vec![identity.to_owned()]
                    )
                ),
                accepted
            );
        }
        let order: Vec<&str> = roster.iter().map(|field| field.field_identity()).collect();
        assert_eq!(order, ["#2", "#10", "console"]);
    }
}
