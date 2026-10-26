//! Interpret a declaration predicate over the store's actual value roster.
//!
//! The shared execution-prefix observer supplies completed values and call
//! guarantees at the post-store coordinate. Disjoint storage retains its
//! observations only when the write-frame owner proves their backing stable.
//! The scalar arithmetic judgment below also handles retained static binders
//! and immutable arrivals without assuming mutable storage still has its
//! entry contents. Neither route imports the current domain as a premise.
//!
//! Literal valuation handles zero and IEEE facts independently. A runtime
//! scalar instead owes an implication: the entered requirements must entail
//! the predicate after its field reads the written value and its static
//! binders read the retained application. Scoped arithmetic already owns that
//! simultaneous substitution and the selected operations' mathematical meaning.
//! No authored expression or typed application is rewritten here.
//!
//! Only immutable scalar arrivals and static binders can remain requirement
//! atoms throughout this state. Mutable arrivals, borrowed storage and locals
//! have no such premise: the ordinary flow owner must establish their current
//! facts. In particular, a same-spelled mutable value cannot borrow an entry
//! requirement after a write or a call. The written term retains its original
//! owner and arithmetic policy; a wrapping operation is not mathematical add.
//! No standing default-domain hypothesis participates in proving its own store.

use super::TrackedPlace;
use super::symbolic_values::{SymbolicValue, applied_const_argument, applied_const_integer};
use crate::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};
use numerics::bignum::BigInt;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

pub(super) fn post_store_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    receiver: ExpressionHandle,
    place: &TrackedPlace<'_>,
    written_field: symbols::SymbolHandle,
    written_value: ExpressionHandle,
    predicate: ExpressionHandle,
) -> bool {
    if crate::proof_contracts::contract_entailment::stored_data_predicate_holds(
        program,
        machine,
        state,
        statement_index,
        receiver,
        place.definition,
        predicate,
    ) {
        return true;
    }
    let mut arrivals = Vec::new();
    for parameter in program.state_parameters(state) {
        if !parameter.is_mutable
            && let Some(primitive) = program.primitive_type_reference(parameter.type_reference)
            && primitive.is_fixed_integer()
        {
            arrivals.push(scalar_binding(parameter.symbol, primitive));
        }
    }
    for parameter in program.machine_type_parameters(machine) {
        let (TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference }) = parameter.kind
        else {
            continue;
        };
        if let Some(primitive) = program.primitive_type_reference(type_reference)
            && primitive.is_fixed_integer()
        {
            arrivals.push(scalar_binding(parameter.symbol, primitive));
        }
    }

    let mut fields = Vec::new();
    for member in program.data_members(place.definition) {
        let Some(field) = member.as_field() else {
            continue;
        };
        if !program
            .primitive_type_reference(field.type_reference)
            .is_some_and(PrimitiveType::is_fixed_integer)
        {
            continue;
        }
        let value = if field.symbol == written_field {
            Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                owner: machine.symbol,
                expression: written_value,
                bindings: arrivals.clone(),
            }))
        } else if let Some((_, Some(value))) = place
            .fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
        {
            value
                .integer()
                .map(|value| ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
        } else {
            place
                .symbols
                .iter()
                .find(|(name, _)| name == field.name.as_str())
                .and_then(|(_, value)| match value {
                    SymbolicValue::Atom(symbol) => arrivals
                        .iter()
                        .find(|binding| binding.binder == ScopedArithmeticBinder::Symbol(*symbol))
                        .map(|binding| binding.value.clone()),
                    _ => None,
                })
        };
        if let Some(value) = value {
            fields.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(field.symbol),
                value,
            });
        }
    }
    if let Some(origin) = place.definition.generic_instance
        && let TypeReferenceNode::Generic { base_symbol, .. } =
            program.type_reference_table.type_reference(origin)
        && let Some(template) = program.unique_data_definition_by_symbol(*base_symbol)
    {
        for parameter in program.data_type_parameters(template) {
            let value = if let Some((_, value)) =
                applied_const_integer(program, place.definition, parameter.symbol)
            {
                Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
            } else {
                applied_const_argument(program, place.definition, parameter.symbol).and_then(
                    |(_, actual)| {
                        let TypeReferenceNode::Named { symbol, .. } =
                            program.type_reference_table.type_reference(actual)
                        else {
                            return None;
                        };
                        if !symbol.is_valid()
                            || program.symbols.get(*symbol).kind
                                != symbols::SymbolKind::TypeParameter
                        {
                            return None;
                        }
                        arrivals
                            .iter()
                            .find(|binding| {
                                binding.binder == ScopedArithmeticBinder::Symbol(*symbol)
                            })
                            .map(|binding| binding.value.clone())
                    },
                )
            };
            if let Some(value) = value {
                fields.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                    value,
                });
            }
        }
    }
    let contracts = if program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
    {
        program.machine_contracts(machine)
    } else {
        program.state_contracts(state)
    };
    let hypotheses = contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| {
            Some(ScopedArithmeticHypothesis {
                proposition: ScopedArithmeticExpression {
                    owner: machine.symbol,
                    expression: *fact.as_expression()?,
                    bindings: arrivals.clone(),
                },
                holds: true,
            })
        })
        .collect::<Vec<_>>();
    scoped_arithmetic_implication(
        program,
        machine,
        &hypotheses,
        &ScopedArithmeticExpression {
            owner: place.definition.symbol,
            expression: predicate,
            bindings: fields,
        },
    ) == StrictArithmeticImplicationJudgment::Proven
}

fn scalar_binding(
    symbol: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> ScopedArithmeticBinding {
    ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::Symbol(symbol),
        value: ScopedArithmeticValue::Atom {
            identity: format!(
                "default-domain:{}:{}",
                symbol.arena_index(),
                symbol.generation()
            ),
            unsigned: matches!(
                primitive,
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
            ),
        },
    }
}
