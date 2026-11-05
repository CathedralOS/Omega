//! Describe an ordinary comparison body without granting it an order meaning.
//!
//! The proof producer needs every real return case and exact constructor/call
//! identity before it can prove a comparison implies structural descent. These
//! rows are source associations only; constructing them proves no inequality.

use super::structural_judgment::StructuralTerm;
use super::structural_terms::{identity_equality_terms, selected_application_machine};
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
mod source_graph;
pub use source_graph::{
    SourceApplication, SourceCall, SourceCase, SourceOutcome, SourceReturn, SourceValueOrigin,
};

/// An exact unary finite-inductive carrier. The base and recursive constructor
/// are selected by their declaration graph, never by their spelling.
#[derive(Clone, Debug, Default)]
pub struct OrdinalCarrier {
    pub reference: TypeReferenceHandle,
    pub identity: typed_trees::type_identity::TypeIdentityId,
    pub declaration: SymbolHandle,
    pub base: SymbolHandle,
    pub successor: SymbolHandle,
    pub child: SymbolHandle,
}

/// A source value description. Positions identify logical binders, not a
/// bounded integer approximation to the value's mathematical rank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceOrderValue {
    Variable(usize),
    Base,
    Successor(Box<SourceOrderValue>),
}

#[derive(Clone, Debug)]
pub struct SourceOrderBody {
    pub machine: SymbolHandle,
    pub entry: SymbolHandle,
    pub states: arena::HandleSpan<typed_trees::state::State>,
    pub parameter_types: Vec<TypeReferenceHandle>,
    pub result_type: TypeReferenceHandle,
    pub cases: Vec<SourceCase>,
}

pub fn ordinal_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<OrdinalCarrier, &'static str> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return Err("the structural rank has no exact nominal carrier application");
    };
    let definition = program
        .data_definition_by_symbol(*symbol)
        .ok_or("the structural rank carrier declaration is unavailable")?;
    let identity = program.package_qualified_type_identity(reference);
    if !identity.is_valid() {
        return Err("the structural rank has no complete carrier application identity");
    }
    let mut base = None;
    let mut successor = None;
    let mut child = None;
    for member in program.data_members(definition) {
        let DataMember::Variant(variant) = member else {
            return Err("the unary structural rank has an additional common field");
        };
        match program.data_payload_fields(variant) {
            [] if base.is_none() => base = Some(variant.symbol),
            [field]
                if successor.is_none()
                    && program.package_qualified_type_identity(field.type_reference)
                        == identity =>
            {
                successor = Some(variant.symbol);
                child = Some(field.symbol);
            }
            _ => return Err("the structural rank has no closed unary induction descriptor"),
        }
    }
    Ok(OrdinalCarrier {
        reference,
        identity,
        declaration: definition.symbol,
        base: base.ok_or("the structural rank has no base constructor")?,
        successor: successor.ok_or("the structural rank has no recursive constructor")?,
        child: child.ok_or("the structural rank has no exact recursive field")?,
    })
}

/// Source association owns any effect preparation needed for omitted guard
/// results. Plans are requested at most once for this demand cohort and never
/// depend on the computed-order theorem being constructed.
pub struct SourceOrderPreparation<'program> {
    program: &'program dyn crate::CheckingInput,
    guard_effects: Option<crate::PreparedEffectPlans>,
}

impl<'program> SourceOrderPreparation<'program> {
    pub fn new(program: &'program dyn crate::CheckingInput) -> Self {
        Self {
            program,
            guard_effects: None,
        }
    }

    pub fn body(
        &mut self,
        carrier: &OrdinalCarrier,
        target: SymbolHandle,
    ) -> Result<SourceOrderBody, &'static str> {
        let program = self.program;
        let (machine, entry) =
            crate::machine_calls::calls::machine_state_by_symbol(program, target)
                .ok_or("the comparison graph has no exact selected state body")?;
        if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
            program, machine, entry,
        ) {
            return Err(
                "the source comparison graph needs a checked recurrent state interpretation",
            );
        }
        let parameter_types = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>();
        if parameter_types.iter().any(|reference| {
            program.package_qualified_type_identity(*reference) != carrier.identity
        }) || (program.package_qualified_type_identity(entry.return_type) != carrier.identity
            && !matches!(
                program
                    .type_reference_table
                    .type_reference(entry.return_type),
                TypeReferenceNode::Unit
            ))
        {
            return Err("the comparison graph loses an exact value carrier application");
        }
        let cases = source_graph::body(program, carrier, machine, entry, &mut self.guard_effects)?;
        Ok(SourceOrderBody {
            machine: machine.symbol,
            entry: entry.symbol,
            states: machine.states,
            parameter_types,
            result_type: entry.return_type,
            cases,
        })
    }
}

/// Authored comparisons only request a proof. A matching signature or a
/// fieldless result does not establish an order.
#[derive(Clone, Debug)]
pub struct OrderDemand {
    pub target: SymbolHandle,
    pub carrier: OrdinalCarrier,
}

pub fn order_demands(program: &dyn crate::CheckingInput) -> Vec<OrderDemand> {
    let mut demands = Vec::new();
    let sites = crate::computed_order_demand_sites(program);
    let trace = std::env::var_os("OMEGA_STRUCT_TRACE").is_some();
    if trace {
        eprintln!(
            "STRUCT computed-order demand discovery sites={}",
            sites.len()
        );
    }
    for site in sites {
        let Some(machine) = program.machine_by_symbol(site.machine) else {
            continue;
        };
        let Some((_, state)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, site.state)
        else {
            continue;
        };
        let carrier = match ordinal_carrier(program, site.carrier) {
            Ok(carrier) => carrier,
            Err(reason) => {
                if trace {
                    eprintln!(
                        "STRUCT computed-order demand refused machine={} state={:?} carrier={:?}: {reason}",
                        machine.name.as_str(),
                        site.state,
                        site.carrier
                    );
                }
                continue;
            }
        };
        let Some(requests) = authored_order_requests(program, machine, state, &carrier) else {
            if trace {
                eprintln!(
                    "STRUCT computed-order demand refused machine={} state={:?}: authored comparison requests unavailable",
                    machine.name.as_str(),
                    site.state
                );
            }
            continue;
        };
        if trace {
            eprintln!(
                "STRUCT computed-order demand requests machine={} state={:?} carrier={:?} requests={}",
                machine.name.as_str(),
                site.state,
                carrier.identity,
                requests.len()
            );
        }
        for request in requests {
            if !demands.iter().any(|previous: &OrderDemand| {
                previous.target == request.target
                    && previous.carrier.identity == request.carrier.identity
            }) {
                demands.push(request);
            }
        }
    }
    demands
}

/// The producer and edge consumer read the same authored comparison requests.
/// Neither phase may select a different relation by looking at its spelling.
pub(super) fn authored_order_requests(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    carrier: &OrdinalCarrier,
) -> Option<Vec<OrderDemand>> {
    use typed_trees::statement::StatementNode;
    let mut owners = vec![machine];
    for statement in program.statement_table.statements(state.statement_nodes) {
        let target = match statement {
            StatementNode::Call(call) if call.receiver.is_empty() => {
                if call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                {
                    return None;
                }
                call.target_symbol
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                let Some(call) = program.expression_table.call_node(local.initial_value) else {
                    continue;
                };
                if call.receiver.is_valid() {
                    continue;
                }
                if call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                {
                    return None;
                }
                call.target_symbol
            }
            _ => continue,
        };
        if let Some(callee) = selected_application_machine(program, target)
            && !owners.iter().any(|owner| owner.symbol == callee.symbol)
        {
            owners.push(callee);
        }
    }
    let mut requests = Vec::new();
    let mut conjuncts = Vec::new();
    for owner in owners {
        let entry = program.machine_states(owner).first()?;
        let parameters = program.state_parameters(entry);
        for contract in program.machine_contracts(owner) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                conjuncts.clear();
                super::law_conformance::collect_equality_conjuncts(
                    program,
                    *expression,
                    &mut conjuncts,
                );
                for &expression in &conjuncts {
                    // Source calls and constructors can retain qualifiers or
                    // elaborated operands. Read their selected meaning through
                    // the same parameter scope used by ordinary equation checking.
                    let Some((left, right)) =
                        identity_equality_terms(program, owner.symbol, parameters, expression)
                    else {
                        continue;
                    };
                    for (operand, result) in [(&left, &right), (&right, &left)] {
                        let Some((target, result_type)) =
                            comparison_application(operand, result, carrier)
                        else {
                            continue;
                        };
                        let Some(relation) = selected_application_machine(program, target) else {
                            continue;
                        };
                        let entry = program.machine_states(relation).first()?;
                        let parameters = program.state_parameters(entry);
                        if program.package_qualified_type_identity(result_type) != carrier.identity
                            || parameters.len() != 2
                            || parameters.iter().any(|parameter| {
                                program.package_qualified_type_identity(parameter.type_reference)
                                    != carrier.identity
                            })
                            || program.package_qualified_type_identity(entry.return_type)
                                != carrier.identity
                        {
                            continue;
                        }
                        if !requests
                            .iter()
                            .any(|request: &OrderDemand| request.target == target)
                        {
                            requests.push(OrderDemand {
                                target,
                                carrier: carrier.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    Some(requests)
}

fn comparison_application(
    operand: &StructuralTerm,
    result: &StructuralTerm,
    carrier: &OrdinalCarrier,
) -> Option<(SymbolHandle, TypeReferenceHandle)> {
    let (
        StructuralTerm::Application {
            target,
            result_type,
            selections,
            arguments,
            ..
        },
        StructuralTerm::Constructor {
            declaration: Some(declaration),
            fields,
            ..
        },
    ) = (operand, result)
    else {
        return None;
    };
    (target.is_valid()
        && result_type.is_valid()
        && selections.is_empty()
        && arguments.len() == 2
        && declaration.data == carrier.declaration
        && declaration.case == Some(carrier.base)
        && fields.is_empty())
    .then_some((*target, *result_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof_contracts::contract_entailment::structural_judgment::ConstructorDeclaration;

    fn comparison() -> (OrdinalCarrier, StructuralTerm, StructuralTerm) {
        let carrier = OrdinalCarrier {
            declaration: SymbolHandle::from_arena_index(1),
            base: SymbolHandle::from_arena_index(2),
            ..Default::default()
        };
        let application = StructuralTerm::Application {
            result_type: TypeReferenceHandle::from_arena_index(3),
            target: SymbolHandle::from_arena_index(4),
            selections: Vec::new(),
            machine: "saturating_sub".into(),
            arguments: vec![
                StructuralTerm::Variable("left".into()),
                StructuralTerm::Variable("right".into()),
            ],
        };
        let zero = StructuralTerm::Constructor {
            declaration: Some(ConstructorDeclaration {
                data: carrier.declaration,
                case: Some(carrier.base),
            }),
            data: "Nat".into(),
            case: "Zero".into(),
            fields: Vec::new(),
        };
        (carrier, application, zero)
    }

    #[test]
    fn computed_order_request_keeps_the_selected_target() {
        let (carrier, mut application, zero) = comparison();
        assert_eq!(
            comparison_application(&application, &zero, &carrier),
            Some((
                SymbolHandle::from_arena_index(4),
                TypeReferenceHandle::from_arena_index(3)
            ))
        );
        let StructuralTerm::Application { target, .. } = &mut application else {
            unreachable!()
        };
        *target = SymbolHandle::from_arena_index(7);
        assert_eq!(
            comparison_application(&application, &zero, &carrier),
            Some((
                SymbolHandle::from_arena_index(7),
                TypeReferenceHandle::from_arena_index(3)
            ))
        );
        let StructuralTerm::Application { target, .. } = &mut application else {
            unreachable!()
        };
        *target = SymbolHandle::invalid();
        // The familiar display name cannot request evidence for an absent
        // selected declaration. A valid request still owes its body theorem.
        assert!(comparison_application(&application, &zero, &carrier).is_none());
    }

    #[test]
    fn computed_order_request_rejects_an_unowned_constructor() {
        let (carrier, application, zero) = comparison();
        for declaration in [
            None,
            Some(ConstructorDeclaration {
                data: SymbolHandle::from_arena_index(5),
                case: Some(carrier.base),
            }),
            Some(ConstructorDeclaration {
                data: carrier.declaration,
                case: Some(SymbolHandle::from_arena_index(6)),
            }),
        ] {
            let mut foreign = zero.clone();
            let StructuralTerm::Constructor {
                declaration: selected,
                ..
            } = &mut foreign
            else {
                unreachable!()
            };
            *selected = declaration;
            // All variants retain identical printed Nat/Zero names.
            assert!(comparison_application(&application, &foreign, &carrier).is_none());
        }
    }
}
