use diagnostics::Diagnostic;
use language_semantics::{MachineSupplyMode, ReferenceAccess};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod local_formation;
mod receiver;

#[derive(Clone)]
struct WriteOnlyRoot {
    symbol: SymbolHandle,
    receiver_machine: SymbolHandle,
    name: String,
    referee: TypeReferenceHandle,
}

pub(crate) fn validate_checked_write_only_slice(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut roots = program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    let TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::WriteOnly,
                        ..
                    } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    Some(WriteOnlyRoot {
                        symbol: parameter.symbol,
                        receiver_machine: if parameter.is_self {
                            machine.symbol
                        } else {
                            SymbolHandle::invalid()
                        },
                        name: parameter.name.as_str().to_owned(),
                        referee: *referee,
                    })
                })
                .collect::<Vec<_>>();
            roots.extend(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| {
                        let StatementNode::LocalData(local) = statement else {
                            return None;
                        };
                        let TypeReferenceNode::Reference {
                            referee,
                            access: ReferenceAccess::WriteOnly,
                            ..
                        } = program
                            .type_reference_table
                            .type_reference(local.type_reference)
                        else {
                            return None;
                        };
                        Some(WriteOnlyRoot {
                            symbol: local.symbol,
                            receiver_machine: SymbolHandle::invalid(),
                            name: local.name.as_str().to_owned(),
                            referee: *referee,
                        })
                    }),
            );
            // Declared `&write` parameters and locals are the state's
            // write-only roots. `&mut` parameters, `&mut` locals, and `mut`
            // value bindings are not: ordinary reads through them stay legal,
            // but each one is still a formation source for `&write` borrows —
            // an attenuation onto write-only access is sound only when the
            // lent place keeps every declared constraint atom. The walk
            // therefore runs whether or not any write-only root exists, so
            // `&write` formations from mutable places take the same exact-atom
            // gate in every state.
            if !roots.is_empty() {
                if !matches!(machine.supply_mode, MachineSupplyMode::CheckedBody) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` declares `&write`, but the current milestone proves non-observation only for checked Omega bodies; boundary, accepted, requirement, and external-provider declarations require an admitted write-only boundary claim",
                        machine.name,
                        state.name,
                    )));
                }

                for root in &roots {
                    if !is_supported_checked_referee(program, root.referee)
                        && receiver::record(program, root).is_none()
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{}` parameter `{}` uses `&write` with `{}`; the current checked slice supports unrestricted primitive scalars, integer scalars qualified only by closed literal ranges or a lone arithmetic policy, carriers qualified only by plain declared domains, recursively literal fixed arrays whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums, forwarding-only byte slices, non-generic invariant-free checked records, and closed material `[copy]` sums as atomic whole values",
                            machine.name,
                            state.name,
                            root.name,
                            program.display_type_reference_with_constraints(root.referee),
                        )));
                    }
                }
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_statement(program, machine, state, statement, &roots, diagnostics);
            }
        }
    }
}

fn is_unrestricted_scalar(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    !name.as_str().starts_with("Atomic")
        && program.primitive_type_reference(type_reference).is_some()
}

/// One qualified leaf a common-field store may displace: an exact integer
/// primitive whose only constraints are closed literal `[lo..=hi]` ranges.
/// The bounded-assignment obligation downstream re-derives the same integer
/// interval from the declaration and enforces every store into the place, so
/// admission adds no new proof burden — this gate still owns only the
/// content-independent place, never the value's containment proof. Named and
/// domain constraints, symbolic or unclosed endpoints, and non-integer
/// carriers stay unsupported leaves here: the proof layer cannot turn them
/// into an enforced integer range. `write_only_assignment_leaf` separately
/// admits the arithmetic-policy-only integer leaf.
fn is_closed_ranged_integer_scalar(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    let mut current = type_reference;
    let mut saw_range = false;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(current)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return false;
            };
            if crate::closed_integer_range_bound(program, *minimum).is_none()
                || crate::closed_integer_range_maximum(program, *maximum, *end_inclusive).is_none()
            {
                return false;
            }
            saw_range = true;
        }
        current = *base_type;
    }
    saw_range
        && matches!(
            program.type_reference_table.type_reference(current),
            TypeReferenceNode::Named { symbol, .. }
                if program.symbols.builtin_type_atom(*symbol).is_some_and(|atom| {
                    use symbols::BuiltinTypeAtom;
                    matches!(
                        atom,
                        BuiltinTypeAtom::U8
                            | BuiltinTypeAtom::U16
                            | BuiltinTypeAtom::U32
                            | BuiltinTypeAtom::U64
                            | BuiltinTypeAtom::I8
                            | BuiltinTypeAtom::I16
                            | BuiltinTypeAtom::I32
                            | BuiltinTypeAtom::I64
                    )
                })
        )
}

fn fixed_unrestricted_write_only_array_shape(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, usize)> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    is_unrestricted_write_only_array_element(program, *element_type)
        .then_some((*element_type, *length))
}

fn fixed_unrestricted_write_only_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    fixed_unrestricted_write_only_array_shape(program, type_reference).map(|(_, length)| length)
}

fn literal_fixed_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::FixedArray {
        length: FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    Some(*length)
}

fn is_supported_checked_referee(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_unrestricted_write_only_field_leaf(program, type_reference)
        || is_qualified_write_only_leaf(program, type_reference)
        || is_byte_slice(program, type_reference)
        || write_only_record(program, type_reference).is_some()
        || is_unrestricted_write_only_sum(program, type_reference)
}

/// The constrained referees a `&write` declaration may carry: the same
/// qualified leaves a field store may displace. Each declared atom re-enters
/// the place's own store obligations — a closed range as the bounded-value
/// proof, a lone arithmetic policy as a carrier behaviour tag, a plain
/// declared domain as a membership check on every stored value — so the
/// reference admits exactly what the checked slice can still enforce through
/// the referent, and nothing weaker or different. Named constraints, mixed
/// constraint classes, symbolic or open ranges, and non-plain domains stay
/// unsupported referees here.
fn is_qualified_write_only_leaf(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_closed_ranged_integer_scalar(program, type_reference)
        || crate::is_arithmetic_policy_only_integer(program, type_reference)
        || is_plain_domain_qualified_leaf(program, type_reference)
}

fn is_byte_slice(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Slice { element_type }
            if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}

fn is_write_only_length_metadata(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
    roots: &[WriteOnlyRoot],
) -> bool {
    if member.case_variant.is_some() || member.member.as_str() != "len" {
        return false;
    }

    if direct_write_only_root(program, member.receiver, roots).is_some_and(|root| {
        is_byte_slice(program, root.referee)
            || fixed_unrestricted_write_only_array_length(program, root.referee).is_some()
    }) {
        return true;
    }

    write_only_record_field_type(program, member.receiver, roots)
        .and_then(|field_type| literal_fixed_array_length(program, field_type))
        .is_some()
}

/// Resolve one closed nominal data definition whose shape is known without
/// substitution and whose authored domain cannot couple replacement to prior
/// content. Record traversal and atomic sum replacement apply their own shape
/// judgments below.
fn closed_write_only_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    closed_write_only_data_by_symbol(program, *symbol)
}

fn closed_write_only_data_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataDefinition> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    (definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && definition.lifetime_parameters.is_empty()
        && program.data_type_parameters(definition).is_empty()
        && definition.quotient.is_none()
        && definition.where_facts.is_empty()
        && !definition.zero_gated)
        .then_some(definition)
}

/// The first aggregate traversal rung is deliberately nominal and closed. A
/// record may contain wider siblings without making them writable; final-leaf
/// eligibility is checked separately at the exact assignment target.
fn write_only_record(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let definition = closed_write_only_data(program, type_reference)?;
    (DataDefinition::shape_kind_from_members(program.data_members(definition))
        == DataShapeKind::Record)
        .then_some(definition)
}

fn is_unrestricted_write_only_record(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    write_only_record(program, type_reference).is_some_and(|definition| {
        definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
    })
}

/// A closed material `[copy]` sum may be displaced only as one whole value.
/// The incoming value supplies its complete tag and payload, so this judgment
/// neither observes nor projects the prior case. Erased payload occurrences
/// remain outside the runtime replacement carrier.
fn is_unrestricted_write_only_sum(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    closed_write_only_data(program, type_reference).is_some_and(|definition| {
        definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
            && DataDefinition::shape_kind_from_members(program.data_members(definition))
                == DataShapeKind::Enum
            && program.data_members(definition).iter().all(|member| {
                let DataMember::Variant(variant) = member else {
                    return false;
                };
                program
                    .data_payload_fields(variant)
                    .iter()
                    .all(|field| !field.relevance.is_erased())
            })
    })
}

/// Fixed-array aggregate elements stay a closed runtime shape: the existing
/// primitive scalars plus eligible unrestricted records or sums whose direct
/// runtime occurrences are all material. A recursively literal fixed array of
/// the same eligible elements is also one atomic element of its enclosing
/// array. Generic/qualified shells do not enter through this judgment, and
/// aggregate elements remain atomic.
fn is_unrestricted_write_only_array_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    is_unrestricted_scalar(program, type_reference)
        || fixed_unrestricted_write_only_array_shape(program, type_reference).is_some()
        || write_only_record(program, type_reference).is_some_and(|definition| {
            definition.properties.multiplicity
                == language_semantics::Multiplicity::Unrestricted
                && program.data_members(definition).iter().all(|member| {
                    matches!(member, DataMember::Field(field) if !field.relevance.is_erased())
                })
        })
        || is_unrestricted_write_only_sum(program, type_reference)
}

fn whole_root_replacement_is_supported(program: &TypedTrees, root: &WriteOnlyRoot) -> bool {
    // The whole-root store displaces the complete referee footprint, so the
    // same leaf judgment that governs one content-independent field store
    // applies: an unrestricted leaf, or a qualified one whose atoms the
    // ordinary store obligation re-derives on the incoming value.
    write_only_assignment_leaf(program, root.referee)
        || receiver::record(program, root).is_some_and(|definition| {
            definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
        })
}

/// Resolve `root.record_field...leaf`, where every receiver is an admitted
/// plain record and every selected field is relevant and unconstrained, through
/// the same non-observing projection walk that receiver dispatch, projected
/// subloans, and local formation share — restricted here to member hops. This
/// is a content-independent place judgment: expression traversal rejects
/// reading the same path, and sum payloads never enter this walk.
fn write_only_record_field_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    // A bare root name denotes the whole referent, not a field path.
    if direct_write_only_root(program, expression, roots).is_some() {
        return None;
    }
    receiver::projected(
        program,
        expression,
        roots,
        receiver::ProjectionAdmission::MembersOnly,
    )
    .map(|(_, leaf, _)| leaf)
}

/// The final displaced record-path leaf must be an unrestricted primitive, a
/// literal fixed array of eligible unrestricted elements, a whole eligible
/// unrestricted record, or a closed material `[copy]` sum treated atomically.
/// Indexed element stores reuse the same exact path resolver below and apply
/// their own narrower leaf/index gate.
fn is_unrestricted_write_only_field_leaf(
    program: &TypedTrees,
    field_type: TypeReferenceHandle,
) -> bool {
    is_unrestricted_scalar(program, field_type)
        || fixed_unrestricted_write_only_array_length(program, field_type).is_some()
        || is_unrestricted_write_only_record(program, field_type)
        || is_unrestricted_write_only_sum(program, field_type)
}

/// A stored leaf additionally admits two qualified integer primitives: a
/// closed literal-ranged one, whose declared range re-enters the assignment
/// as the ordinary bounded-value obligation, and an arithmetic-policy-only
/// one (`u32 in Wrapping`). Decision 17 makes `in <policy>` a behaviour tag
/// on the carrier's operations, not a value-range predicate — every carrier
/// value is already a member, and `range-constraints-require-exact-domain`
/// keeps a hidden range from riding beneath the policy, so the store owes no
/// containment proof at all. A third qualified leaf is the plain
/// domain-qualified carrier below. Cross-class, narrowing, and domain-atom
/// weakening checks on the value are unchanged. Nothing else about the path
/// or the value relaxes.
fn write_only_assignment_leaf(program: &TypedTrees, field_type: TypeReferenceHandle) -> bool {
    is_unrestricted_write_only_field_leaf(program, field_type)
        || is_qualified_write_only_leaf(program, field_type)
}

/// One qualified leaf a common-field store may displace: an already-admissible
/// leaf carrying only plain declared domain constraints (`[u8; 16] in Utf8`).
/// `in <domain>` is a membership predicate on the whole incoming value, not a
/// different place shape — the store displaces the carrier's complete
/// footprint, so the place stays content-independent. The ordinary write-side
/// domain enforcement re-derives every declared domain from the target's
/// declaration and discharges it against the stored value — a place declared
/// `in D` requires every write to be established in that domain, the same
/// obligation a `&mut` store already supplies — so admission adds no proof
/// burden here. The base recurses through `write_only_assignment_leaf`, so a
/// domain wrapper composes over the ranged and policy leaves exactly as the
/// type nests.
///
/// Named constraints, compiler-owned domain subjects (carry, value, layout),
/// classified or routed domains, element and range stores into a
/// domain-qualified carrier (a partial write cannot re-establish whole-value
/// membership), and `&write` subloans of a domain-qualified leaf (the
/// attenuated referee cannot carry the atom) stay outside the envelope.
fn is_plain_domain_qualified_leaf(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    let constraints = program.type_reference_table.constraints(*constraints);
    !constraints.is_empty()
        && constraints.iter().all(|constraint| {
            matches!(
                constraint,
                TypeConstraintNode::Domain(domain)
                    if crate::value_custody::storage_contents::is_plain_value_domain(
                        program, domain,
                    )
            )
        })
        && write_only_assignment_leaf(program, *base_type)
}

fn write_only_record_field_assignment(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    write_only_record_field_type(program, expression, roots)
        .is_some_and(|field_type| write_only_assignment_leaf(program, field_type))
}

/// Mutable-authority bindings that may source a `&write` formation alongside
/// the state's declared write-only roots: `&mut` state parameters, `mut`
/// value parameters, and the eligible locals declared before
/// `stop_before_local` (or every such local when `None`) — immutable `&mut`
/// carriers plus `let mut` value bindings. A `&mut` parameter may attenuate
/// at formation, an earlier immutable `&mut` carrier preserves mutable
/// authority until attenuation, and a `mut` value binding owns writable
/// storage outright — but none is a write-only root: ordinary reads through
/// them stay legal, so they join the walk only where a `&write` borrow is
/// being formed. A mutable binding of reference type is never a source: the
/// binding could be reseated beneath the loan. An erased binding owns no
/// runtime storage at all.
fn mutable_formation_sources(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    stop_before_local: Option<SymbolHandle>,
) -> Vec<WriteOnlyRoot> {
    let mut sources = Vec::new();
    sources.extend(
        program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                let (referee, receiver_machine) = match program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                {
                    // A `&mut` parameter lends its referent.
                    TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::Mutable,
                        ..
                    } => (
                        *referee,
                        if parameter.is_self {
                            machine.symbol
                        } else {
                            SymbolHandle::invalid()
                        },
                    ),
                    // A `mut` value parameter lends its own storage — `mut
                    // self` included, as a consuming receiver with declared
                    // mutable authority.
                    _ if parameter.is_mutable
                        && !parameter.is_const
                        && !parameter.relevance.is_erased() =>
                    {
                        (
                            parameter.type_reference,
                            if parameter.is_self {
                                machine.symbol
                            } else {
                                SymbolHandle::invalid()
                            },
                        )
                    }
                    _ => return None,
                };
                Some(WriteOnlyRoot {
                    symbol: parameter.symbol,
                    receiver_machine,
                    name: parameter.name.as_str().to_owned(),
                    referee,
                })
            }),
    );
    sources.extend(
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take_while(|statement| {
                !matches!(
                    statement,
                    StatementNode::LocalData(candidate)
                        if Some(candidate.symbol) == stop_before_local
                )
            })
            .filter_map(|statement| {
                let StatementNode::LocalData(source) = statement else {
                    return None;
                };
                if !source.symbol.is_valid() || source.relevance.is_erased() {
                    return None;
                }
                let referee = match program
                    .type_reference_table
                    .type_reference(source.type_reference)
                {
                    TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::Mutable,
                        ..
                    } if !source.is_mutable => *referee,
                    TypeReferenceNode::Reference { .. } => return None,
                    _ if source.is_mutable => source.type_reference,
                    _ => return None,
                };
                Some(WriteOnlyRoot {
                    symbol: source.symbol,
                    receiver_machine: SymbolHandle::invalid(),
                    name: source.name.as_str().to_owned(),
                    referee,
                })
            }),
    );
    sources
}

/// One `&write` borrow at a checked-call argument boundary. The lent place
/// keeps every constraint atom it declared: a write-only subloan is sound
/// only when the callee's declared `&write` referee is the place's type
/// atom-for-atom — a weaker referee would let the callee write values the
/// caller's place forbids, and a stronger or different one asserts atoms the
/// place never owned. Returns `true` once the argument is resolved — admitted
/// silently, or rejected with a directed diagnostic — and `false` when the
/// shape is no admitted subloan, leaving the ordinary expression walk to
/// report it.
fn write_only_call_subloan(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    declared_parameter: Option<TypeReferenceHandle>,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    // The argument's own `&write` referee, when the callee declares one. An
    // unresolvable target (a requirement signature or an unresolved symbol)
    // keeps the legacy callee-agnostic leaf judgment below; the call's own
    // access and arity checks own its diagnostics.
    let declared_referee = declared_parameter.and_then(|parameter| {
        match program.type_reference_table.type_reference(parameter) {
            TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::WriteOnly,
                ..
            } => Some(*referee),
            _ => None,
        }
    });

    // `&write` formation is also the attenuation boundary for every mutable
    // place: a borrow lent from a `&mut` reference or a `mut` value binding
    // faces the same exact-atom gate as a write-only reborrow, so the subloan
    // rungs resolve the lent place over the state's write-only roots plus its
    // mutable formation sources. The added bindings never become write-only
    // roots elsewhere — expression validation keeps reading through them.
    let mut sources = roots.to_vec();
    sources.extend(mutable_formation_sources(program, machine, state, None));
    let roots = &sources;

    // Whole-root forwarding lends the complete declared place.
    if let Some(root) = direct_write_only_root(program, target, roots) {
        return match declared_referee {
            Some(declared) => {
                subloan_referee_matches(program, root.referee, declared)
                    || reject_subloan_referee_mismatch(
                        program,
                        machine,
                        state,
                        target,
                        root.referee,
                        declared,
                        diagnostics,
                    )
            }
            None => true,
        };
    }

    if let Some(field_type) = write_only_record_field_type(program, target, roots) {
        return match declared_referee {
            Some(declared) => {
                write_only_assignment_leaf(program, field_type)
                    && (subloan_referee_matches(program, field_type, declared)
                        || reject_subloan_referee_mismatch(
                            program,
                            machine,
                            state,
                            target,
                            field_type,
                            declared,
                            diagnostics,
                        ))
            }
            // Without a resolvable `&write` referee the callee's own checks
            // own the argument; only the legacy unrestricted leaf kinds keep
            // their callee-agnostic admission.
            None => is_unrestricted_write_only_field_leaf(program, field_type),
        };
    }

    if let Some(leaf) = write_only_literal_indexed_subloan_leaf(program, target, roots) {
        if !is_unrestricted_scalar(program, leaf) {
            return false;
        }
        return match declared_referee {
            Some(declared) => {
                subloan_referee_matches(program, leaf, declared)
                    || reject_subloan_referee_mismatch(
                        program,
                        machine,
                        state,
                        target,
                        leaf,
                        declared,
                        diagnostics,
                    )
            }
            None => true,
        };
    }

    false
}

/// Whether the lent place's referee is the callee's declared `&write` referee
/// atom-for-atom. Normalized identity carries the comparison; the one extra
/// equation resolves an attached-machine `Self` to the data it names, since a
/// `self` root's declared type spells `Self` while the callee's parameter
/// spells the data name for the identical carrier.
fn subloan_referee_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    declared: TypeReferenceHandle,
) -> bool {
    if crate::value_custody::type_references::type_references_match(program, actual, declared) {
        return true;
    }
    let (
        TypeReferenceNode::Named {
            symbol: actual_symbol,
            ..
        },
        TypeReferenceNode::Named {
            symbol: declared_symbol,
            ..
        },
    ) = (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(declared),
    )
    else {
        return false;
    };
    program.machines().iter().any(|machine| {
        machine.symbol == *actual_symbol && machine.attached_data_symbol == *declared_symbol
    })
}

/// Report an `&write` subloan whose lent place type is not the callee's
/// declared referee atom-for-atom, then treat the argument as resolved: the
/// generic projection envelope must not report the same borrow again.
fn reject_subloan_referee_mismatch(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    actual: TypeReferenceHandle,
    declared: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` lends `{}` of type `{}` as `&write` to a parameter declared `&write {}`; a write-only subloan must preserve the callee-declared constraint atoms exactly, so a weaker, stronger, or different referee remains rejected",
        machine.name,
        state.name,
        program.expression_table.display_name(target),
        program.display_type_reference_with_constraints(actual),
        program.display_type_reference_with_constraints(declared),
    )));
    true
}

/// Admit one relevant primitive field beneath one literal fixed-array element
/// reached through an otherwise ordinary common-field record path or directly
/// beneath a write-only array root. The array element stays a closed
/// unrestricted record, and the literal index fixes the complete write
/// footprint without observing the referent.
fn write_only_literal_indexed_record_field_assignment(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    let ExpressionNode::Member(final_member) = program.expression_table.expression(expression)
    else {
        return false;
    };
    if final_member.case_variant.is_some() {
        return false;
    }
    let ExpressionNode::Indexed(indexed) =
        program.expression_table.expression(final_member.receiver)
    else {
        return false;
    };
    // A bare `&write` array root is itself the indexed collection; a member
    // path reaches the array through the shared non-observing record walk.
    let Some(collection_type) = direct_write_only_root(program, indexed.collection, roots)
        .map(|root| root.referee)
        .or_else(|| write_only_record_field_type(program, indexed.collection, roots))
    else {
        return false;
    };
    let Some((element_type, length)) =
        fixed_unrestricted_write_only_array_shape(program, collection_type)
    else {
        return false;
    };
    let Some(index) = program
        .expression_table
        .constant_integer_value(indexed.index)
        .and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if index >= length {
        return false;
    }
    let Some(definition) = write_only_record(program, element_type) else {
        return false;
    };
    program.data_members(definition).iter().any(|candidate| {
        let DataMember::Field(field) = candidate else {
            return false;
        };
        ((final_member.member_symbol.is_valid() && field.symbol == final_member.member_symbol)
            || (!final_member.member_symbol.is_valid()
                && field.name.as_str() == final_member.member.as_str()))
            && !field.relevance.is_erased()
            && write_only_assignment_leaf(program, field.type_reference)
    })
}

fn validate_statement(
    program: &TypedTrees,
    machine_definition: &Machine,
    state_definition: &State,
    statement: &StatementNode,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machine = machine_definition.name.as_str();
    let state = state_definition.name.as_str();
    match statement {
        StatementNode::RootBinding(_) => {}
        StatementNode::AssemblyFact(fact) => validate_expression(
            program,
            machine_definition,
            state_definition,
            fact.expression,
            roots,
            diagnostics,
        ),
        StatementNode::Assignment(assignment) => {
            if let Some(root) = direct_write_only_root(program, assignment.target, roots) {
                if !whole_root_replacement_is_supported(program, root) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` replaces whole write-only aggregate `{}`; whole-root replacement requires a freely discardable supported root, so replace one eligible leaf through an invariant-free record path or declare and prove an unrestricted material aggregate instead",
                        root.name,
                    )));
                }
                // An admitted whole-value replacement observes no prior
                // content. Aggregate roots additionally satisfy the explicit
                // closed-shape/material/discardability checks above.
            } else if write_only_record_field_assignment(program, assignment.target, roots) {
                // One content-independent common-field-path store. The exact
                // field place is retained by the ordinary checked mutation facts.
            } else if write_only_literal_indexed_record_field_assignment(
                program,
                assignment.target,
                roots,
            ) {
                // One literal fixed-array element and its exact relevant
                // primitive record field form a content-independent place.
            } else if validate_write_only_fixed_array_range_assignment(
                program,
                machine_definition,
                state_definition,
                assignment.target,
                assignment.value,
                roots,
                diagnostics,
            ) {
                // The range-specific gate owns non-observation and RHS shape.
                // Ordinary range validation independently owns order/bounds.
            } else if let Some(index) =
                write_only_element_assignment_index(program, assignment.target, roots)
            {
                // The normal range checker separately proves a dynamic index
                // is in bounds. This gate owns only non-observation: the index
                // expression must not recover information from the write-only
                // referent.
                validate_expression(
                    program,
                    machine_definition,
                    state_definition,
                    index,
                    roots,
                    diagnostics,
                );
            } else if expression_mentions_write_only_root(program, assignment.target, roots) {
                diagnose_unsupported_write_only_assignment_target(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                );
            } else {
                validate_expression(
                    program,
                    machine_definition,
                    state_definition,
                    assignment.target,
                    roots,
                    diagnostics,
                );
            }
            validate_expression(
                program,
                machine_definition,
                state_definition,
                assignment.value,
                roots,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            receiver::validate_statement_call(program, machine, state, call, roots, diagnostics);
            // A receiver that names a declaration rather than a runtime place
            // (`Record::replace(&write leaf)`) supplies `self` as its first
            // argument — the same correspondence the resolved-target rung of
            // the call validator applies. A bare `self` receiver is the
            // receiverless spelling: `self` binds through the implicit
            // receiver even though its root resolves to the machine symbol,
            // so it must not shift the authored arguments right by one.
            let self_is_argument = receiver_names_declaration(program, call.receiver_symbol)
                && !matches!(
                    program.statement_table.name_path_members(call.receiver),
                    [member] if member.as_str() == "self"
                );
            let declared_parameters =
                crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                    .map(|(_, callee_state)| {
                        declared_state_parameters(program, callee_state, self_is_argument)
                    })
                    .unwrap_or_default();
            for (index, argument) in program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                validate_call_argument(
                    program,
                    machine_definition,
                    state_definition,
                    *argument,
                    declared_parameters.get(index).copied(),
                    roots,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => validate_expression(
            program,
            machine_definition,
            state_definition,
            *expression,
            roots,
            diagnostics,
        ),
        StatementNode::LocalData(local)
            if local_formation::admitted(
                program,
                machine_definition,
                state_definition,
                local,
                roots,
            ) => {}
        StatementNode::LocalData(local) => validate_expression(
            program,
            machine_definition,
            state_definition,
            local.initial_value,
            roots,
            diagnostics,
        ),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                validate_expression(
                    program,
                    machine_definition,
                    state_definition,
                    guard,
                    roots,
                    diagnostics,
                );
            }
            validate_transition_target(
                program,
                machine_definition,
                state_definition,
                transition.target,
                roots,
                diagnostics,
            );
            validate_transition_target(
                program,
                machine_definition,
                state_definition,
                transition.continuation,
                roots,
                diagnostics,
            );
        }
    }
}

/// Validate an exact range replacement: a statically normalized half-open
/// window of a direct fixed array of eligible unrestricted elements, or an
/// eligible common-field path ending in one, replaced by an array literal of
/// exactly the same element width. Returns whether the target was such a range
/// even when another checker owns its eventual rejection.
fn validate_write_only_fixed_array_range_assignment(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    value: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let (root, collection_type) =
        if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
            (root, root.referee)
        } else {
            let Some(collection_type) =
                write_only_record_field_type(program, indexed.collection, roots)
            else {
                return false;
            };
            let Some(root) = mentioned_write_only_root(program, indexed.collection, roots) else {
                return false;
            };
            (root, collection_type)
        };
    let Some((element_type, collection_len)) =
        fixed_unrestricted_write_only_array_shape(program, collection_type)
    else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };

    if range.start.is_valid() {
        validate_expression(program, machine, state, range.start, roots, diagnostics);
    }
    if range.end.is_valid() {
        validate_expression(program, machine, state, range.end, roots, diagnostics);
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces a write-only fixed-array range with an omitted end; this exact-footprint rung requires a statically known end bound",
            machine.name, state.name,
        )));
        return true;
    }

    let start = if range.start.is_valid() {
        crate::normalize_immutable_integer_bound_to_usize(program, range.start)
    } else {
        Some(0)
    };
    let end =
        crate::normalize_immutable_integer_bound_to_usize(program, range.end).and_then(|end| {
            if range.end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        });
    let (Some(start), Some(end)) = (start, end) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces a write-only fixed-array range whose bounds are not statically known; exact range replacement currently requires literal bounds or finite immutable local-copy aliases",
            machine.name, state.name,
        )));
        return true;
    };

    // The ordinary range checker emits the directed order/bounds diagnostic.
    // Do not add a misleading write-only-shape error for the same invalid range.
    if start > end || end > collection_len {
        return true;
    }
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::ArrayLiteral(_)
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces write-only fixed-array range `{}[{}..{}]` from a non-literal value; the exact range-replacement rung requires an array literal of {} element(s)",
            machine.name,
            state.name,
            root.name,
            start,
            end,
            end - start,
        )));
        return true;
    }
    crate::value_custody::struct_literals::validate_array_literal_elements_for_shape(
        program,
        machine,
        state,
        value,
        element_type,
        Some(end - start),
        diagnostics,
    );
    true
}

fn write_only_element_assignment_index(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<ExpressionHandle> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let (collection_type, direct_byte_slice) =
        if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
            (root.referee, is_byte_slice(program, root.referee))
        } else {
            (
                write_only_record_field_type(program, indexed.collection, roots)?,
                false,
            )
        };
    if direct_byte_slice {
        return (!matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ))
        .then_some(indexed.index);
    }
    let length = fixed_unrestricted_write_only_array_length(program, collection_type)?;
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(_) => None,
        ExpressionNode::Integer(index) => {
            let index = usize::try_from(index.value_i64()?).ok()?;
            (index < length).then_some(indexed.index)
        }
        _ => Some(indexed.index),
    }
}

/// The exact leaf type of one literal-indexed subloan shape: an admitted
/// write-only place reached through member hops plus at least one indexed
/// hop. The shared non-observing projection walk — the same judgment local
/// formation applies through `captured_type` — owns root and bare-field
/// bases, relevant-field identity, builtin index meaning, and in-bounds
/// literal coordinates. The call gate above keeps only the rung's shape
/// rules: an unrestricted primitive leaf, and exact referee identity against
/// the declared parameter. Dynamic indices, ranges, and aggregate elements
/// remain excluded.
fn write_only_literal_indexed_subloan_leaf(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    let mut cursor = expression;
    let mut visited = Vec::new();
    let has_indexed_hop = loop {
        if visited.contains(&cursor) || !program.expression_table.expression_is_valid(cursor) {
            break false;
        }
        visited.push(cursor);
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => member.receiver,
            ExpressionNode::Indexed(_) => break true,
            _ => break false,
        };
    };
    if !has_indexed_hop {
        return None;
    }
    receiver::projected(
        program,
        expression,
        roots,
        receiver::ProjectionAdmission::LiteralIndexes,
    )
    .map(|(_, leaf, _)| leaf)
}

fn diagnose_unsupported_write_only_assignment_target(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        && let Some(root) = direct_write_only_root(program, indexed.collection, roots)
        && fixed_unrestricted_write_only_array_length(program, root.referee).is_some()
    {
        let detail = match program.expression_table.expression(indexed.index) {
            ExpressionNode::Range(_) => "range projection is not implemented",
            ExpressionNode::Integer(index) => match index.value_i64() {
                Some(value) if value < 0 => "the index must be non-negative",
                Some(_) => "the literal index is outside the fixed array",
                None => "the literal index is outside the supported index range",
            },
            _ => "the index expression is not an admissible element place",
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine}` state `{state}` writes through unsupported projection of write-only fixed array `{}`; {detail}; whole-array replacement, proven-in-bounds element replacement, and statically normalized closed-range replacement are accepted",
            root.name,
        )));
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{machine}` state `{state}` writes through an unsupported write-only projection; accepted partial stores are a content-independent common-field path through non-generic invariant-free records when every field is relevant and unconstrained and the displaced leaf is an unrestricted primitive, a closed literal-ranged integer primitive proven in range at the store, an integer primitive carrying only an arithmetic-policy constraint, or a carrier qualified only by plain declared domains whose membership is proven on the stored value, a whole eligible unrestricted record or closed material `[copy]` sum, or a recursively literal fixed array whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums, one relevant primitive field beneath a literal fixed-array record element, a proven-in-bounds element or statically normalized closed range of such a fixed array, or a proven-in-bounds element of a direct byte slice; nested array projection, sum case/payload projection, named or non-plain domain qualification, element and range stores into a domain-qualified carrier, invariant-dependent, symbolic or open range, take, swap, and read-modify-write operations remain rejected"
    )));
}

fn validate_transition_target(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: typed_trees::statement::TransitionTargetHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            receiver::validate_state_transfer(
                program,
                machine.name.as_str(),
                state.name.as_str(),
                path.symbol,
                roots,
                diagnostics,
            );
            // A named transfer never passes `self` as an authored argument,
            // exactly as the transition argument validator aligns it.
            let declared_parameters =
                crate::declarations::transitions::resolved_transition_target_state(
                    program,
                    path.symbol,
                )
                .map(|(_, callee_state)| declared_state_parameters(program, callee_state, false))
                .unwrap_or_default();
            for (index, argument) in program
                .statement_table
                .expression_handles(*arguments)
                .iter()
                .enumerate()
            {
                validate_call_argument(
                    program,
                    machine,
                    state,
                    *argument,
                    declared_parameters.get(index).copied(),
                    roots,
                    diagnostics,
                );
            }
        }
        TransitionTargetNode::Value(value) => {
            validate_expression(program, machine, state, *value, roots, diagnostics)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

/// The callee parameters an argument list corresponds to, in authored order.
/// A static carrier receiver (`Record::replace(&write leaf)`) supplies `self`
/// as its first argument exactly as the arity and access checks align it; a
/// place receiver binds `self` through the receiver operand, so `self` stays
/// out of the correspondence.
fn declared_state_parameters(
    program: &TypedTrees,
    state: &State,
    self_is_argument: bool,
) -> Vec<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| self_is_argument || !parameter.is_self)
        .map(|parameter| parameter.type_reference)
        .collect()
}

/// Whether a statement call's receiver names a declaration rather than a
/// runtime place, so `self` is an explicit argument — the same rule the
/// resolved-target rung of the call validator applies to the argument
/// correspondence.
fn receiver_names_declaration(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    matches!(
        program.symbols.get(symbol).kind,
        symbols::SymbolKind::BuiltinType
            | symbols::SymbolKind::Data
            | symbols::SymbolKind::Domain
            | symbols::SymbolKind::Machine
            | symbols::SymbolKind::Module
            | symbols::SymbolKind::Trait
            | symbols::SymbolKind::ConformanceParameter
    )
}

fn validate_call_argument(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    declared_parameter: Option<TypeReferenceHandle>,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
        && borrow.access == ReferenceAccess::WriteOnly
        && write_only_call_subloan(
            program,
            machine,
            state,
            borrow.target,
            declared_parameter,
            roots,
            diagnostics,
        )
    {
        // This milestone admits the exact projected or whole-root subloan
        // only at a direct checked-call argument boundary — statement calls,
        // expression calls, and named state-transition targets all deliver
        // arguments to callee parameters through the same borrow-call facts.
        // The lent place carries the declared `&write` referee atom-for-atom:
        // nothing weaker, stronger, or different crosses here. The gate does
        // not create a reusable local reference or widen general expression
        // formation.
        return;
    }
    validate_expression(program, machine, state, expression, roots, diagnostics);
}

fn validate_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machine_name = machine.name.as_str();
    let state_name = state.name.as_str();
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            validate_expression(
                program,
                machine,
                state,
                dispatch.subject,
                roots,
                diagnostics,
            );
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    validate_expression(program, machine, state, pattern, roots, diagnostics);
                }
                validate_expression(program, machine, state, arm.value, roots, diagnostics);
            }
        }
        ExpressionNode::Name(path) => {
            if let Some(root) = roots
                .iter()
                .find(|root| receiver::mentions_name(program, root, path))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine_name}` state `{state_name}` reads write-only parameter `{}`; `&write` permits replacement or exact `&write` forwarding, never observation",
                    root.name,
                )));
            }
        }
        ExpressionNode::Borrow(borrow) => match borrow.access {
            ReferenceAccess::WriteOnly => {
                if let Some(binding) =
                    binding_without_write_authority(program, state, borrow.target)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` forms `&write` on `{binding}`, a binding without mutable authority; `&write` formation requires a declared `&write` root, a `&mut` place, or a `mut` value binding — a plain `let` or immutable parameter cannot lend write access"
                    )));
                } else if !is_direct_name(program, borrow.target) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` forms `&write` from an unsupported projection or computed expression; the current checked slice supports explicit attenuation of a whole parameter, an exact direct-reborrow local, or one eligible content-independent common-field path optionally followed by a finite nonempty suffix of in-bounds literal fixed-array indexes only as a direct checked-call argument whose declared `&write` referee carries exactly the same type"
                    )));
                }
            }
            ReferenceAccess::Mutable => {
                if let Some(root) = mentioned_write_only_root(program, borrow.target, roots) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` widens write-only parameter `{}` to `&mut`; forward it explicitly as `&write {}` instead",
                        root.name, root.name,
                    )));
                } else {
                    validate_expression(program, machine, state, borrow.target, roots, diagnostics);
                }
            }
            ReferenceAccess::Shared => {
                validate_expression(program, machine, state, borrow.target, roots, diagnostics)
            }
        },
        ExpressionNode::Member(member) => {
            if is_write_only_length_metadata(program, member, roots) {
                // A direct slice length belongs to its descriptor. A literal
                // fixed-array length belongs to its static type, including when
                // the array is reached only through statically known common
                // fields of plain records. Neither reads referent content. Do
                // not recurse into the receiver: doing so would misclassify this
                // exact metadata read as observation.
            } else if let Some(root) = mentioned_write_only_root(program, member.receiver, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine_name}` state `{state_name}` reads field `{}` from write-only parameter `{}`; an eligible record-field path may be replaced as an assignment target, but write-only projection never grants observation",
                    member.member, root.name,
                )));
            } else {
                validate_expression(program, machine, state, member.receiver, roots, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(root) = mentioned_write_only_root(program, indexed.collection, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine_name}` state `{state_name}` reads through index projection of write-only parameter `{}`; `&write` permits admitted fixed-array element replacement but never observation",
                    root.name,
                )));
            } else {
                validate_expression(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    roots,
                    diagnostics,
                );
            }
            validate_expression(program, machine, state, indexed.index, roots, diagnostics);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(program, machine, state, *value, roots, diagnostics);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            validate_expression(program, machine, state, atomic.value, roots, diagnostics);
            validate_expression(program, machine, state, atomic.result, roots, diagnostics);
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(program, machine, state, binary.left, roots, diagnostics);
            validate_expression(program, machine, state, binary.right, roots, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            validate_expression(program, machine, state, cast.value, roots, diagnostics)
        }
        ExpressionNode::Call(call) => {
            let nonobserving_receiver =
                receiver::admits_expression_call(program, call.receiver, roots, call.target_symbol);
            if nonobserving_receiver {
                receiver::validate_operands(
                    program,
                    machine,
                    state,
                    call.receiver,
                    roots,
                    diagnostics,
                );
            } else if call.receiver.is_valid() {
                validate_expression(program, machine, state, call.receiver, roots, diagnostics);
            }
            // Every expression-call argument is the same direct checked-call
            // boundary a statement call presents; the shared borrow-fact
            // collection makes no distinction, so the non-observing subloan
            // gate applies uniformly. Only the receiver operand keeps its
            // observing/non-observing split above. A receiver that names a
            // declaration rather than a runtime place supplies `self` as the
            // first argument — the same correspondence the value-position
            // argument validator derives from `declared_place_type`.
            let self_is_argument = crate::value_custody::places::declared_place_type(
                program,
                machine,
                Some(state),
                call.receiver,
            )
            .is_none();
            let declared_parameters =
                crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                    .map(|(_, callee_state)| {
                        declared_state_parameters(program, callee_state, self_is_argument)
                    })
                    .unwrap_or_default();
            for (index, argument) in program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                validate_call_argument(
                    program,
                    machine,
                    state,
                    *argument,
                    declared_parameters.get(index).copied(),
                    roots,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            validate_expression(program, machine, state, range.start, roots, diagnostics);
            validate_expression(program, machine, state, range.end, roots, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_expression(program, machine, state, field.value, roots, diagnostics);
            }
        }
        ExpressionNode::Unary(unary) => {
            validate_expression(program, machine, state, unary.operand, roots, diagnostics)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn is_direct_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if program.expression_table.name_path_members(path.members).len() == 1
    )
}

/// A bare-name `&write` target naming a binding that holds no write
/// authority: an immutable `let` value local, a non-`mut` value parameter,
/// or a `const`. Reference-typed bindings stay outside — `&write` on a
/// reference is a reborrow of the referent, and the borrow lattice owns its
/// authority question — and receivers stay outside as well: `self` answers
/// to the owned-place rules, not to this binding check.
fn binding_without_write_authority(
    program: &TypedTrees,
    state: &State,
    target: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Name(path) = program.expression_table.expression(target) else {
        return None;
    };
    if program
        .expression_table
        .name_path_members(path.members)
        .len()
        != 1
        || !path.head_symbol.is_valid()
    {
        return None;
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == path.head_symbol)
    {
        return (!parameter.is_self
            && !parameter.is_mutable
            && !matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference { .. }
            ))
        .then(|| parameter.name.as_str().to_owned());
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            if local.symbol != path.head_symbol || local.is_mutable {
                return None;
            }
            (!matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                TypeReferenceNode::Reference { .. }
            ))
            .then(|| local.name.as_str().to_owned())
        })
}

fn direct_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            roots
                .iter()
                .find(|root| receiver::matches_name(program, root, path))
        })
        .flatten()
}

fn mentioned_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    roots
        .iter()
        .find(|root| expression_mentions_root(program, expression, root))
}

fn expression_mentions_write_only_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    mentioned_write_only_root(program, expression, roots).is_some()
}

fn expression_mentions_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    root: &WriteOnlyRoot,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            expression_mentions_root(program, dispatch.subject, root)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_mentions_root(program, pattern, root))
                            || expression_mentions_root(program, arm.value, root)
                    })
        }
        ExpressionNode::Name(path) => receiver::mentions_name(program, root, path),
        ExpressionNode::Borrow(value) => expression_mentions_root(program, value.target, root),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_mentions_root(program, *value, root)),
        ExpressionNode::Atomic(atomic) => {
            expression_mentions_root(program, atomic.value, root)
                || expression_mentions_root(program, atomic.result, root)
        }
        ExpressionNode::Binary(binary) => {
            expression_mentions_root(program, binary.left, root)
                || expression_mentions_root(program, binary.right, root)
        }
        ExpressionNode::Cast(cast) => expression_mentions_root(program, cast.value, root),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_mentions_root(program, call.receiver, root))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_mentions_root(program, *argument, root))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_root(program, indexed.collection, root)
                || expression_mentions_root(program, indexed.index, root)
        }
        ExpressionNode::Member(member) => expression_mentions_root(program, member.receiver, root),
        ExpressionNode::Range(range) => {
            expression_mentions_root(program, range.start, root)
                || expression_mentions_root(program, range.end, root)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_mentions_root(program, field.value, root)),
        ExpressionNode::Unary(unary) => expression_mentions_root(program, unary.operand, root),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
