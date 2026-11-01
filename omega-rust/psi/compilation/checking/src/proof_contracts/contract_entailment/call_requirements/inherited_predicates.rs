//! Bind a migrated clause without replacing the invocation's physical formals.
//!
//! The call-owned reference, exact satisfaction edge and authored Requires row
//! jointly identify the proposition. Applied signatures provide its logical
//! payload; retained specialization bindings provide its complete static tuple.
//! These are substitutions, not assumptions or a second proof strategy. The
//! ordinary prefix owner still authenticates every completed physical operand.

use super::super::specification_calls::SelectedCallContract;
use super::{ExpressionHandle, Machine, State, StructuralTerm, SymbolHandle};
use checked_trees::{ContractProofFact, ContractProofFactKind, ContractProofFactOwner, ProofFacts};
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone)]
pub(super) struct InheritedPredicate {
    pub owner: SymbolHandle,
    pub expression: ExpressionHandle,
    pub parameters: Vec<StateParameter>,
    pub values: Vec<(StateParameter, StructuralTerm)>,
    pub selections: Vec<(SymbolHandle, TypeReferenceHandle, StaticMachineArgument)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inherited_predicate(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    caller_state: &State,
    statement_index: usize,
    site: crate::CallSite<'_>,
    owner: &SelectedCallContract<'_>,
    physical_parameters: &[StateParameter],
    expression: ExpressionHandle,
    proof: &ProofFacts,
    contract_handle: arena::Handle<ContractProofFact>,
) -> Result<InheritedPredicate, &'static str> {
    let SelectedCallContract::Concrete(machine, state) = owner else {
        return Err("the inherited clause has no concrete selected invocation");
    };
    if !contract_handle.is_valid() {
        return Err("the inherited clause has no exact contract reference");
    }
    let contract = proof.contract_facts.get(contract_handle);
    if contract.kind != ContractProofFactKind::Requires
        || contract.owner
            != (ContractProofFactOwner::MachineState {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            })
        || program
            .proof_facts
            .get(contract.fact)
            .as_expression()
            .copied()
            != Some(expression)
    {
        return Err("the inherited clause does not match the selected Requires owner and fact");
    }
    // Membership belongs to this exact occurrence, not just a nearby target
    // or an expression shared by two differently applied satisfaction edges.
    let mut calls = proof.contract_calls.iter().filter(|(_, call)| {
        call.caller_machine_symbol == caller.symbol
            && call.caller_state_symbol == caller_state.symbol
            && call.statement_index == statement_index
            && call.target_machine_symbol == machine.symbol
            && call.target_state_symbol == state.symbol
            && proof
                .contract_fact_refs
                .span_or_empty(call.requires)
                .iter()
                .any(|reference| reference.fact == contract_handle)
            && crate::find_call_site(
                program,
                caller.symbol,
                caller_state.symbol,
                statement_index,
                call.call_ordinal,
            )
            .is_some_and(|actual| match (actual, site) {
                (
                    crate::CallSite::Expression {
                        expression: left, ..
                    },
                    crate::CallSite::Expression {
                        expression: right, ..
                    },
                ) => left == right,
                (crate::CallSite::Statement(left), crate::CallSite::Statement(right)) => {
                    std::ptr::eq(left, right)
                }
                _ => false,
            })
    });
    calls
        .next()
        .ok_or("the exact invocation does not own this inherited Requires reference")?;
    if calls.next().is_some() {
        return Err("the inherited reference has multiple matching invocation occurrences");
    }
    let scope = proof.inherited_contract_scopes.get(
        contract
            .inherited_scope
            .ok_or("the contract has no inherited declaration scope")?,
    );
    if scope.conformance_machine != machine.symbol {
        return Err("the inherited scope belongs to another realization");
    }
    let declaration = program
        .trait_definition_by_symbol(scope.declaring_trait)
        .ok_or("the inherited trait declaration is unavailable")?;
    let declared = program
        .trait_machine_signature_by_symbol(declaration, scope.requirement)
        .ok_or("the exact inherited requirement is unavailable")?;
    if !program
        .state_signature_contracts(declared)
        .iter()
        .any(|clause| {
            clause.kind == SignatureContractKind::Requires
                && (0..clause.facts.len()).any(|offset| {
                    u32::try_from(offset)
                        .ok()
                        .is_some_and(|offset| clause.facts.handle_at(offset) == contract.fact)
                })
        })
    {
        return Err("the authored fact is not in this requirement's Requires roster");
    }
    let mut applications = program
        .machine_trait_conformances(machine)
        .iter()
        .filter(|edge| edge.requirement_symbol == declared.symbol)
        .filter_map(|edge| {
            AppliedTraitScope::from_machine_conformance(program.typed_program(), machine, edge)
        })
        .filter_map(|application| {
            application.declaring_scope(program.typed_program(), declaration.symbol)
        });
    let application = applications
        .next()
        .ok_or("the exact satisfaction application is unavailable")?;
    if applications.any(|other| !application.same_application(program.typed_program(), &other)) {
        return Err("the inherited requirement has conflicting satisfaction applications");
    }
    let trait_parameters = program.trait_type_parameters(declaration);
    if trait_parameters.len() != scope.trait_arguments.len() {
        return Err("the inherited trait application has an incomplete telescope");
    }
    for (parameter, supplied) in trait_parameters.iter().zip(&scope.trait_arguments) {
        let selected = application
            .substitutions
            .iter()
            .find(|(symbol, _)| *symbol == parameter.symbol)
            .ok_or("the inherited trait parameter has no exact supplied operand")?
            .1;
        if program.normalized_type_identity(selected) != program.normalized_type_identity(*supplied)
        {
            return Err(
                "the inherited scope and satisfaction edge select different trait arguments",
            );
        }
    }
    let applied = program
        .applied_callable_signature(&application, declared.symbol)
        .ok_or("the inherited requirement has no prepared callable signature")?;
    let applied_expression = program
        .applied_signature_contract_fact(declared, applied, contract.fact)
        .ok_or("the inherited fact has no corresponding applied clause")?
        .as_expression()
        .copied()
        .ok_or("the applied inherited clause is not a Boolean expression")?;
    let required = program.state_signature_parameters(applied);
    let actual = crate::inherited_satisfier_parameters(program, declaration, applied, state)
        .ok_or("the inherited requirement and realization have different runtime arity")?;
    let offset = physical_parameters
        .len()
        .checked_sub(actual.len())
        .ok_or("the physical invocation cannot contain its realization parameters")?;
    if required.len() != actual.len() || physical_parameters[offset..] != *actual {
        return Err("the physical invocation does not retain its exact realization formals");
    }
    let mut parameters = physical_parameters.to_vec();
    for (slot, public) in parameters[offset..].iter_mut().zip(required) {
        slot.symbol = public.symbol;
        slot.name = format!(
            "\0inherited_{}_{}",
            public.symbol.arena_index(),
            public.symbol.generation()
        )
        .as_str()
        .into();
    }

    let mut specializations = program
        .machine_specializations
        .iter()
        .filter(|item| item.instance == machine.symbol);
    let specialization = specializations.next();
    if specializations.next().is_some() {
        return Err("the selected realization has multiple specialization binding ledgers");
    }
    let family = specialization
        .and_then(|item| program.machine_by_symbol(item.template))
        .unwrap_or(machine);
    let template_parameters = specialization.map_or_else(
        || program.machine_type_parameters(family),
        |item| {
            program
                .data_type_parameters
                .span_or_empty(item.template_parameters)
        },
    );
    let private = crate::machine_calls::static_arguments::invocation_parameters(
        program,
        machine,
        family,
        template_parameters,
        declaration.symbol,
        declared.symbol,
    );
    let public = program.state_signature_type_parameters(applied);
    if public.len() != private.len() {
        return Err("the public and private member invocation telescopes have different arity");
    }
    let mut values = Vec::new();
    let mut selections = Vec::new();
    // Member-local binders correspond positionally. Captures instead keep
    // their exact source symbol: the applied payload already maps the trait
    // binder onto that source, but only the retained ledger closes its value.
    let captures = template_parameters.iter().filter(|parameter| {
        !private
            .iter()
            .any(|invocation| invocation.symbol == parameter.symbol)
    });
    for (public, private) in public
        .iter()
        .zip(&private)
        .chain(captures.map(|parameter| (parameter, parameter)))
    {
        if matches!(
            (&public.kind, &private.kind),
            (TypeParameterKind::Type, TypeParameterKind::Type)
        ) {
            let selected = if let Some(specialization) = specialization {
                let Some(StaticMachineBindingValue::Type(reference)) =
                    program.specialization_binding(specialization, private.symbol)
                else {
                    return Err("the source type binder has no unique type specialization operand");
                };
                program.static_operand_from_type_reference(*reference)?
            } else {
                StaticMachineArgument {
                    symbol: private.symbol,
                    path: Box::from([private.name.clone()]),
                    ..Default::default()
                }
            };
            selections.push((public.symbol, TypeReferenceHandle::invalid(), selected));
            continue;
        }
        let (required_type, actual_type) = match (&public.kind, &private.kind) {
            (
                TypeParameterKind::Const {
                    type_reference: required,
                },
                TypeParameterKind::Const {
                    type_reference: actual,
                },
            )
            | (
                TypeParameterKind::Value {
                    type_reference: required,
                },
                TypeParameterKind::Value {
                    type_reference: actual,
                },
            ) => (*required, *actual),
            // A non-scalar telescope needs its complete callable/type
            // substitution, not a scalar approximation or a dropped operand.
            _ => {
                return Err("the inherited static binder needs a complete non-scalar substitution");
            }
        };
        if program.normalized_type_identity(required_type)
            != program.normalized_type_identity(actual_type)
        {
            return Err("the public and private static binders have different carriers");
        }
        let selected = if let Some(specialization) = specialization {
            let StaticMachineBindingValue::Const(reference) = program
                .specialization_binding(specialization, private.symbol)
                .ok_or("the source scalar binder has no unique specialization operand")?
            else {
                return Err("the source scalar binder did not specialize to a const operand");
            };
            program.static_operand_from_type_reference(*reference)?
        } else {
            StaticMachineArgument {
                symbol: private.symbol,
                path: Box::from([private.name.clone()]),
                ..Default::default()
            }
        };
        let value = match super::super::static_applications::constant_value(program, &selected) {
            Some(super::super::static_applications::ConstantValue::Integer(value))
                if program
                    .primitive_type_reference(required_type)
                    .is_some_and(|primitive| primitive.accepts_integer_literal()) =>
            {
                StructuralTerm::Integer(value)
            }
            Some(super::super::static_applications::ConstantValue::Boolean(value))
                if program.primitive_type_reference(required_type)
                    == Some(typed_trees::types::PrimitiveType::Bool) =>
            {
                StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: value.to_string(),
                    fields: Vec::new(),
                }
            }
            None if {
                let forwarded =
                    crate::machine_calls::static_arguments::forwarded_type(program, &selected);
                forwarded.is_valid()
                    && program.normalized_type_identity(forwarded)
                        == program.normalized_type_identity(required_type)
            } =>
            {
                StructuralTerm::BoundValue(selected.symbol)
            }
            _ => {
                return Err(
                    "the inherited scalar operand has no exact value at its receiving carrier",
                );
            }
        };
        values.push((
            StateParameter {
                symbol: public.symbol,
                name: format!(
                    "\0inherited_static_{}_{}",
                    public.symbol.arena_index(),
                    public.symbol.generation()
                )
                .as_str()
                .into(),
                type_reference: required_type,
                is_const: true,
                ..Default::default()
            },
            value,
        ));
        selections.push((public.symbol, required_type, selected));
    }
    Ok(InheritedPredicate {
        owner: applied.symbol,
        expression: applied_expression,
        parameters,
        values,
        selections,
    })
}
