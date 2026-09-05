//! Declared default-domain fields become entry facts at exact storage places.
//! Ordinary mutations invalidate these facts; a nominal annotation alone does
//! not restore them. Call and state-arrival checks require the same field
//! obligations over the current incoming values before reusing an entry fact.
//!
//! Machine storage additionally requires its ZII value to satisfy the domain.
//! Nominal input storage instead relies on the checked incoming argument.
//! Write-only input views supply no readable entry facts. Independent fields
//! use separate contexts so invalidation does not erase unrelated evidence.

use psi_facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::TypeReferenceHandle;

pub(super) fn append_machine_field_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        let Some(self_symbol) = machine_self_parameter_symbol(program, machine) else {
            continue;
        };
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == attached.as_str())
        else {
            continue;
        };

        let mut refs = psi_arena::HandleSpan::empty();
        append_data_field_domain_facts(
            program,
            facts,
            machine,
            self_symbol,
            data,
            &[],
            &[data.name.as_str()],
            &mut refs,
        );

        if refs.is_empty() {
            continue;
        }
        append_independent_place_contexts(
            facts,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            refs,
        );
    }
}

/// Seed the entry-invariant `self.a.b…c in Domain` fact for every domained field
/// reachable through the attached data -- ONE level or NESTED. `prefix` is the
/// `Field` segment chain from `self` to `data`; `visited` is the data-type names
/// on the current path (a cycle guard for self-referential data). Mirrors the
/// nested resolution in `field_domain::attached_data_field_type`: the read trust
/// seeded here is sound because every write to such a field (one-level or nested)
/// is domain-enforced through the SAME multi-level resolver, and each field is
/// gated on its ZII/empty value satisfying the domain.
#[allow(clippy::too_many_arguments)]
fn append_data_field_domain_facts(
    program: &TypedTrees,
    facts: &mut FactPlan,
    machine: &psi_typed_trees::machine::Machine,
    self_symbol: psi_symbols::SymbolHandle,
    data: &psi_typed_trees::data::DataDefinition,
    prefix: &[psi_symbols::SymbolHandle],
    visited: &[&str],
    refs: &mut psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };

        // SOUNDNESS GATE (per field, one-level or nested): surface the
        // entry-invariant only when the field's ZERO/ZII value provably satisfies
        // the domain -- a read-with-no-prior-write discharges against it, sound
        // only if the empty/default is in-domain. A domain the empty value
        // violates (e.g. `non_empty`) is withheld; its reads must follow a write.
        for domain_symbol in field_domain_symbols(program, field.type_reference)
            .into_iter()
            .filter(|symbol| {
                crate::field_domain::domain_admits_empty_byte_sequence(program, *symbol)
            })
        {
            // Place `self.<prefix…>.<field>`: root the machine receiver symbol
            // (`self`) + the Field-segment chain, so the canonical label matches
            // a `self.a.b` read exactly where the nested write established it.
            let place = facts.append_symbol_place(self_symbol);
            for segment in prefix {
                if let Some(variant) = psi_facts::payload_variant_for_field(program, *segment) {
                    facts.push_place_segment(place, PlaceSegment::Case { variant });
                }
                facts.push_place_segment(place, PlaceSegment::Field { symbol: *segment });
            }
            if let Some(variant) = psi_facts::payload_variant_for_field(program, field.symbol) {
                facts.push_place_segment(place, PlaceSegment::Case { variant });
            }
            facts.push_place_segment(
                place,
                PlaceSegment::Field {
                    symbol: field.symbol,
                },
            );

            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Machine {
                    machine_symbol: machine.symbol,
                },
                origin: FactOrigin::MachineFieldDomain {
                    machine_symbol: machine.symbol,
                },
                evidence: QualificationEvidence::from_origin(
                    psi_language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    machine.symbol,
                ),
                payload: FactPayload::DomainMembership {
                    value: psi_typed_trees::expression::ExpressionHandle::invalid(),
                    domain: psi_arena::HandleSpan::empty(),
                    domain_symbol,
                },
            });
            facts.append_ref(refs, fact);
        }

        // Descend into a struct-typed field so its own domained fields are seeded
        // too. The cycle guard keeps a self-referential data type from looping.
        if let Some(nested) =
            crate::field_domain::data_definition_for_field_type(program, field.type_reference)
            && !visited.contains(&nested.name.as_str())
        {
            let mut next_prefix = prefix.to_vec();
            next_prefix.push(field.symbol);
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.name.as_str());
            append_data_field_domain_facts(
                program,
                facts,
                machine,
                self_symbol,
                nested,
                &next_prefix,
                &next_visited,
                refs,
            );
        }
    }
}

/// #66/P1a param entry-assumption: an IMMUTABLE state parameter declared with a
/// domain qualification carries that domain throughout the state body. The
/// param's implicit `requires param in Domain` makes every caller establish
/// membership (by proof for a bodyful predicate, by retained evidence for a
/// bodyless qualification), and immutability means it can never be reassigned
/// out of the domain. Surfaced at `ProgramPoint::State` (the param belongs to the state),
/// which `build_state_flow_fact` folds into the state entry; the flow's context
/// threading then carries it to every call, including guarded-transition
/// fallthrough arms (now that a transition no longer leaks its branch-taken exit
/// context onto its sibling fallthrough -- see flow/statements.rs).
///
/// Nominal parameters additionally carry their declared field predicates on
/// entry, including readable mutable references: calls and transitions are
/// default-domain consumption points. These are live entry facts, not facts
/// restored after arbitrary writes. Ordinary storage invalidation retires them.
/// Write-only views cannot inspect the incoming value and receive no such facts.
pub(super) fn append_state_parameter_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = psi_arena::HandleSpan::empty();
            for parameter in program.state_parameters(state) {
                if parameter.is_self {
                    continue;
                }
                if let Some(data) = readable_nominal_definition(program, parameter.type_reference) {
                    append_state_parameter_data_field_domain_facts(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        data,
                        &[],
                        &[data.symbol],
                        &mut refs,
                    );
                }
                // Keep the existing root qualification/resource permission
                // rule separate from default-domain fields of nominal values.
                if parameter.is_mutable {
                    continue;
                }
                let mut has_resource_claim = false;
                for domain_symbol in crate::field_domain::domain_constraint_symbols(
                    program,
                    parameter.type_reference,
                ) {
                    has_resource_claim |= state_parameter_domain_is_resource_claim(
                        program,
                        parameter.type_reference,
                        domain_symbol,
                    );
                    append_state_parameter_domain_fact(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        &[],
                        domain_symbol,
                        &mut refs,
                    );
                }
                if has_resource_claim {
                    append_state_parameter_carry_origin(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        &mut refs,
                    );
                }
                for permission in carry_constraint_permissions(program, parameter.type_reference) {
                    append_state_parameter_carry_fact(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        permission,
                        &mut refs,
                    );
                }
            }

            if refs.is_empty() {
                continue;
            }
            append_independent_place_contexts(
                facts,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

fn append_independent_place_contexts(
    facts: &mut FactPlan,
    point: ProgramPoint,
    refs: psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    // Invalidating one storage coordinate must not discard independent
    // parameter or machine-field facts. Facts over the same place stay coupled.
    let mut groups: Vec<(FactPlace, Vec<psi_facts::FactRef>)> = Vec::new();
    for reference in facts.refs.span_or_empty(refs) {
        let place = facts.facts.get(reference.fact).place;
        if let Some((_, group)) =
            groups
                .iter_mut()
                .find(|(candidate, _)| match (*candidate, place) {
                    (FactPlace::Place(left), FactPlace::Place(right)) => {
                        facts.places_equal(left, right)
                    }
                    _ => *candidate == place,
                })
        {
            group.push(*reference);
        } else {
            groups.push((place, vec![*reference]));
        }
    }
    for (_, group) in groups {
        let mut refs = psi_arena::HandleSpan::empty();
        for reference in group {
            facts.refs.append_to_span(&mut refs, reference);
        }
        facts.append_context(point, refs);
    }
}

fn state_parameter_domain_is_resource_claim(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    crate::checks::type_multiplicity(program, type_reference)
        == psi_language_semantics::Multiplicity::Linear
        && program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == domain_symbol)
            .is_some_and(|domain| {
                domain.predicate_body == psi_language_semantics::DomainPredicateBody::Bodyless
            })
}

fn append_state_parameter_carry_origin(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    refs: &mut psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            psi_language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::CarryOrigin {
            value: psi_typed_trees::expression::ExpressionHandle::invalid(),
        },
    });
    facts.append_ref(refs, fact);
}

fn carry_constraint_permissions(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Vec<psi_language_semantics::CarryPermission> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            carry_constraint_permissions(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                psi_typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    psi_language_semantics::CarryPermission::from_name(domain.name.as_str())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn append_state_parameter_carry_fact(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    permission: psi_language_semantics::CarryPermission,
    refs: &mut psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            psi_language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::CarryPermission {
            value: psi_typed_trees::expression::ExpressionHandle::invalid(),
            permission,
        },
    });
    facts.append_ref(refs, fact);
}

#[allow(clippy::too_many_arguments)]
fn append_state_parameter_data_field_domain_facts(
    program: &TypedTrees,
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    data: &psi_typed_trees::data::DataDefinition,
    prefix: &[SymbolHandle],
    visited: &[SymbolHandle],
    refs: &mut psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if readable_type_reference(program, field.type_reference).is_none() {
            continue;
        }
        for domain_symbol in field_domain_symbols(program, field.type_reference) {
            let mut path = prefix.to_vec();
            path.push(field.symbol);
            append_state_parameter_domain_fact(
                program,
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                &path,
                domain_symbol,
                refs,
            );
        }
        if let Some(nested) = readable_nominal_definition(program, field.type_reference)
            && !visited.contains(&nested.symbol)
        {
            let mut next_prefix = prefix.to_vec();
            next_prefix.push(field.symbol);
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.symbol);
            append_state_parameter_data_field_domain_facts(
                program,
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                nested,
                &next_prefix,
                &next_visited,
                refs,
            );
        }
    }
}

/// Peel only readable reference/constraint shells. Array elements and generic
/// substitutions need their own structural evidence, not a nominal-name guess.
fn readable_type_reference(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    use psi_typed_trees::types::TypeReferenceNode;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => reference = *referee,
            TypeReferenceNode::Reference { .. } => return None,
            _ => return Some(reference),
        }
    }
    None
}

fn readable_nominal_definition(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    use psi_typed_trees::types::TypeReferenceNode;
    let reference = readable_type_reference(program, reference)?;
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if arguments.is_empty() => *base_symbol,
        _ => return None,
    };
    program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol && data.type_parameters.is_empty())
}

fn append_state_parameter_domain_fact(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    fields: &[SymbolHandle],
    domain_symbol: SymbolHandle,
    refs: &mut psi_arena::HandleSpan<psi_facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    for field in fields {
        if let Some(variant) = psi_facts::payload_variant_for_field(program, *field) {
            facts.push_place_segment(place, PlaceSegment::Case { variant });
        }
        facts.push_place_segment(place, PlaceSegment::Field { symbol: *field });
    }
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            psi_language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::DomainMembership {
            value: psi_typed_trees::expression::ExpressionHandle::invalid(),
            domain: psi_arena::HandleSpan::empty(),
            domain_symbol,
        },
    });
    facts.append_ref(refs, fact);
}

/// #66 case-payload forwarding: a local constructed as a sum CASE with a
/// domain-refined payload (`let cmd = Command::Say { text: "ok" }`, where
/// `Command::Say`'s `text` is `&[u8] in Utf8`) carries that payload domain on
/// `cmd.<payload>`. Construction enforcement (#60-1c, checks::contracts::writes)
/// already proved the payload value in-domain at construction, so a later read of
/// the payload -- in particular a destructured `Command::Say { text }` forwarded
/// as a `requires <arg> in D` call argument, which resolves to `cmd.<payload>` --
/// discharges with no re-proof. Surfaced at `ProgramPoint::State`, folded into the
/// state entry and threaded to the matched arm's calls; the flow's mutation
/// invalidation drops it if `cmd` is reassigned (so it is sound for mutable locals
/// too -- no immutability gate needed).
///
/// CONSERVATIVE: a sibling arm that matches a DIFFERENT case is dead when `cmd` was
/// constructed as one specific case; its `cmd.<other-payload>` obligation shares
/// the same canonical place label, so it discharges only when the domains agree
/// (a mixed-domain dead arm would be rejected -- sound, just incomplete).
pub(super) fn append_local_case_payload_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = psi_arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                if !local_data.initial_value.is_valid() {
                    continue;
                }
                let ExpressionNode::StructLiteral(literal) = program
                    .expression_table
                    .expression(local_data.initial_value)
                else {
                    continue;
                };
                let Some(case_name) = literal.case_name.as_ref() else {
                    continue;
                };
                let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.name.as_str() == literal.type_name.as_str())
                else {
                    continue;
                };
                let Some(variant) =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.name.as_str() == case_name.as_str() =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                else {
                    continue;
                };

                for payload_field in program.data_payload_fields(variant) {
                    for domain_symbol in field_domain_symbols(program, payload_field.type_reference)
                    {
                        // Place `cmd.<payload-field>`: root the local symbol + a Field
                        // segment for the variant's payload field, matching how a
                        // destructured payload arg resolves at the call site.
                        let place = facts.append_symbol_place(local_data.symbol);
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Case {
                                variant: variant.symbol,
                            },
                        );
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Field {
                                symbol: payload_field.symbol,
                            },
                        );
                        let fact = facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point: ProgramPoint::State {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            origin: FactOrigin::LocalCasePayloadDomain {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            evidence: QualificationEvidence::from_origin(
                                psi_language_semantics::QualificationEvidenceOrigin::Prover,
                                state.symbol,
                            ),
                            payload: FactPayload::DomainMembership {
                                value: psi_typed_trees::expression::ExpressionHandle::invalid(),
                                domain: psi_arena::HandleSpan::empty(),
                                domain_symbol,
                            },
                        });
                        facts.append_ref(&mut refs, fact);
                    }
                }
            }

            if refs.is_empty() {
                continue;
            }
            facts.append_context(
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

/// A symbol named `self` for the machine's receiver -- the `is_self` parameter
/// of any of its states (all spelled `self`, so the canonical label is identical
/// regardless of which state's receiver symbol is used).
fn machine_self_parameter_symbol(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<SymbolHandle> {
    program.machine_states(machine).iter().find_map(|state| {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol)
    })
}

/// Every normalized predicate-domain symbol on a field type, looking through a
/// leading reference (`&[u8] in Utf8 & NoNul`). Semantic-only constraints do
/// not become flow facts.
fn field_domain_symbols(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    crate::field_domain::predicate_domain_constraint_symbols(program, type_reference)
}
