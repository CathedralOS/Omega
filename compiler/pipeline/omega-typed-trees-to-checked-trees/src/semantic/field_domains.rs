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
