//! #66 read-narrowing: a machine-attached-data field declared with an encoding
//! domain (`out: &[u8] in Utf8`) carries that domain as an ALWAYS-HOLDING
//! invariant -- every write is enforced in-domain (see `checks::contracts::writes`),
//! so every READ may trust it. This producer surfaces that invariant as a
//! `DomainMembership` fact over the place `self.<field>` at `ProgramPoint::Machine`,
//! which `build_state_flow_fact` folds into each state's entry context. Reads of
//! `self.<field>` then carry the `in Domain` fact (and the statement-transfer
//! machinery propagates it to copies), so a `requires <arg> in Domain` call /
//! return discharges with no re-proof -- the field analog of the declared-param
//! domain fact-flow (#66 Phase 1) and of the self.field range narrowing (#63).
//!
//! This is deliberately NOT a machine `requires` contract: a contract fact would
//! also become a CALLER obligation (`build_contract_call_facts` matches
//! `ContractProofFactOwner::Machine`), which is wrong for an always-true field
//! invariant. The fact is established directly, imposing no caller obligation.
//!
//! SOUNDNESS: the entry-invariant rests on the field's ZERO/ZII value being
//! in-domain. The fact is surfaced as ALWAYS-holding at machine entry, so a READ
//! with no prior write discharges against it -- which is sound only if the
//! field's default (for a slice carrier, the EMPTY byte sequence) satisfies the
//! domain. We therefore GATE the surfacing on
//! `crate::field_domain::domain_admits_empty_byte_sequence`: Utf8/NoNul/AsciiOnly
//! admit the empty sequence (surfaced as before), while an empty-violating domain
//! (e.g. `non_empty`, `len > 0`) is NOT surfaced -- a read-with-no-prior-write of
//! such a field cannot falsely discharge.

use omega_core::symbols::SymbolHandle;
use omega_facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataMember;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

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

        let mut refs = omega_core::arena::HandleSpan::empty();
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
        facts.append_context(
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
    machine: &omega_typed_trees::machine::Machine,
    self_symbol: omega_core::symbols::SymbolHandle,
    data: &omega_typed_trees::data::DataDefinition,
    prefix: &[omega_core::symbols::SymbolHandle],
    visited: &[&str],
    refs: &mut omega_core::arena::HandleSpan<omega_facts::FactRef>,
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
        if let Some(domain_symbol) =
            field_domain_symbol(program, field.type_reference).filter(|symbol| symbol.is_valid())
            && crate::field_domain::domain_admits_empty_byte_sequence(program, domain_symbol)
        {
            // Place `self.<prefix…>.<field>`: root the machine receiver symbol
            // (`self`) + the Field-segment chain, so the canonical label matches
            // a `self.a.b` read exactly where the nested write established it.
            let place = facts.append_symbol_place(self_symbol);
            for segment in prefix {
                facts.push_place_segment(place, PlaceSegment::Field { symbol: *segment });
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
                payload: FactPayload::DomainMembership {
                    value: omega_typed_trees::expression::ExpressionHandle::invalid(),
                    domain: omega_core::arena::HandleSpan::empty(),
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

/// #66 param entry-assumption: an IMMUTABLE state parameter declared with an
/// encoding domain (`check_text(text: &[u8] in Utf8)`) carries that domain
/// throughout the state body. The param's implicit `requires param in Domain`
/// (Phase 1) makes every caller prove membership, and immutability means it can
/// never be reassigned out of the domain -- so it holds at every program point in
/// the state. Surfaced at `ProgramPoint::State` (the param belongs to the state),
/// which `build_state_flow_fact` folds into the state entry; the flow's context
/// threading then carries it to every call, including guarded-transition
/// fallthrough arms (now that a transition no longer leaks its branch-taken exit
/// context onto its sibling fallthrough -- see flow/statements.rs).
///
/// Needs no empty/ZII soundness gate (a param is an argument a caller proved
/// in-domain, never a default). MUTABLE params are excluded for now: a
/// reassignment would have to invalidate the fact, which the flow does handle, but
/// the conservative immutable-only surface is enough for the current corpus.
pub(super) fn append_state_parameter_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = omega_core::arena::HandleSpan::empty();
            for parameter in program.state_parameters(state) {
                if parameter.is_self || parameter.is_mutable {
                    continue;
                }
                let Some(domain_symbol) = field_domain_symbol(program, parameter.type_reference)
                    .filter(|symbol| symbol.is_valid())
                else {
                    continue;
                };

                let place = facts.append_symbol_place(parameter.symbol);
                let fact = facts.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: ProgramPoint::State {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                    origin: FactOrigin::StateParameterDomain {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                    payload: FactPayload::DomainMembership {
                        value: omega_typed_trees::expression::ExpressionHandle::invalid(),
                        domain: omega_core::arena::HandleSpan::empty(),
                        domain_symbol,
                    },
                });
                facts.append_ref(&mut refs, fact);
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
            let mut refs = omega_core::arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                if !local_data.initial_value.is_valid() {
                    continue;
                }
                let ExpressionNode::StructLiteral(literal) =
                    program.expression_table.expression(local_data.initial_value)
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
                let Some(variant) = program.data_members(data).iter().find_map(|member| match member
                {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                }) else {
                    continue;
                };

                for payload_field in program.data_payload_fields(variant) {
                    let Some(domain_symbol) =
                        field_domain_symbol(program, payload_field.type_reference)
                            .filter(|symbol| symbol.is_valid())
                    else {
                        continue;
                    };

                    // Place `cmd.<payload-field>`: root the local symbol + a Field
                    // segment for the variant's payload field, matching how a
                    // destructured payload arg resolves at the call site.
                    let place = facts.append_symbol_place(local_data.symbol);
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
                        payload: FactPayload::DomainMembership {
                            value: omega_typed_trees::expression::ExpressionHandle::invalid(),
                            domain: omega_core::arena::HandleSpan::empty(),
                            domain_symbol,
                        },
                    });
                    facts.append_ref(&mut refs, fact);
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
    machine: &omega_typed_trees::machine::Machine,
) -> Option<SymbolHandle> {
    program.machine_states(machine).iter().find_map(|state| {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol)
    })
}

/// The declared encoding-domain symbol on a field type, looking through a leading
/// reference (`&[u8] in Utf8`). Resolves the short domain name (`Utf8`) to its
/// domain definition by the trailing path segment (`[u8]::Utf8`).
fn field_domain_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let domain_name = domain_constraint_name(program, type_reference)?;
    program.domain_definitions().iter().find_map(|domain| {
        let full = domain.name.as_str();
        (full.rsplit("::").next().unwrap_or(full) == domain_name).then_some(domain.symbol)
    })
}

fn domain_constraint_name<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program str> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => domain_constraint_name(program, *referee),
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Domain(name) => Some(name.as_str()),
                _ => None,
            }),
        _ => None,
    }
}
