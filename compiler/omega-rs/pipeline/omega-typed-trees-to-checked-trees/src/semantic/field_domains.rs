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
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let Some(domain_symbol) =
                field_domain_symbol(program, field.type_reference).filter(|symbol| symbol.is_valid())
            else {
                continue;
            };

            // SOUNDNESS GATE: surface the entry-invariant only when the field's
            // ZERO/ZII value provably satisfies the domain. The invariant is
            // ALWAYS-holding at machine entry, so a read-with-no-prior-write would
            // discharge against it -- sound only if the empty/default value is
            // in-domain. A domain whose classifier the empty value violates (e.g.
            // `non_empty`) is withheld; its reads must follow an enforced write.
            if !crate::field_domain::domain_admits_empty_byte_sequence(program, domain_symbol) {
                continue;
            }

            // Place `self.<field>`: root the machine receiver symbol (named
            // `self`) + a Field segment for the declared field, so the canonical
            // label matches a `self.<field>` read.
            let place = facts.append_symbol_place(self_symbol);
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
                    // The `domain` path span is display-only; all proving/matching
                    // keys off `domain_symbol`. An empty span is sufficient (we
                    // cannot append into the program's path arena from here).
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
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            refs,
        );
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
