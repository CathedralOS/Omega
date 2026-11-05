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
use crate::proof_contracts::arithmetic_domains::BigIntInterval;
use crate::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
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
    receiver: &crate::CanonicalPlace,
    receiver_type: typed_trees::types::TypeReferenceHandle,
    preserves_scalar_value: bool,
    destination: &crate::CanonicalPlace,
    place: &TrackedPlace<'_>,
    written_field: SymbolHandle,
    written_value: ExpressionHandle,
    predicate: ExpressionHandle,
) -> bool {
    let Some(completed_prefix) = statement_index.checked_add(1) else {
        return false;
    };
    // A representation-changing store supplies source bits, not the RHS's
    // numerical value. The execution-prefix owner interprets those bits at
    // the captured source coordinate before proving its predicate.
    if !preserves_scalar_value {
        return crate::proof_contracts::contract_entailment::captured_data_predicate_holds(
            program,
            machine,
            state,
            crate::proof_contracts::contract_entailment::CapturedPredicatePoint::Prefix(
                completed_prefix,
            ),
            receiver,
            receiver_type,
            place.definition,
            predicate,
        );
    }
    // Exact scalar substitution can finish without reconstructing private
    // arrivals. Both queries are independently sufficient proofs; replay the
    // execution prefix only when the scalar query cannot establish the fact.
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

    // The store's value is already formed at its own occurrence. Transport
    // those observed bounds with its identity; an aggregate sibling's unknown
    // contents do not erase this scalar observation. No destination predicate
    // participates in establishing the operand or its bounds.
    // A field copy carries the source declaration's interval when this
    // statement has no tighter occurrence bound. `signature.parameter_count`
    // is `<= 32` for every `BoundarySignature`, so storing it into
    // `CallPlan.parameter_count` restores that same declared conjunct.
    let written_bounds =
        crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            written_value,
        );
    // A shared or immutable carrier still has its data declaration's field
    // bounds. Those bounds belong to the source value, so they may prove the
    // destination field. An exclusive binding can sit in a restoration window,
    // and its current contents are not the declaration interval.
    let declared_bounds = source_field_declaration_interval(program, machine, state, written_value);
    let written = ScopedArithmeticValue::Term(ScopedArithmeticExpression {
        owner: machine.symbol,
        expression: written_value,
        bindings: arrivals.clone(),
    });
    let interval = match written_bounds {
        Some((minimum, maximum)) => {
            let observed = BigIntInterval {
                low: Some(BigInt::from_i128(i128::from(minimum))),
                high: Some(BigInt::from_i128(i128::from(maximum))),
            };
            Some(match declared_bounds {
                Some(declared) => observed.intersect(&declared),
                None => observed,
            })
        }
        None => declared_bounds,
    };
    let written = match interval {
        Some(interval) => ScopedArithmeticValue::BoundedInteger {
            value: Box::new(ScopedArithmeticValue::Atom {
                identity: format!(
                    "\0stored-value:{:?}:{:?}:{statement_index}:{written_value:?}",
                    machine.symbol, state.symbol
                ),
                unsigned: crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    written_value,
                )
                .and_then(|reference| program.primitive_type_reference(reference))
                .is_some_and(|primitive| !primitive.is_signed_integer()),
            }),
            interval,
        },
        None => written,
    };
    let mut fields = Vec::new();
    let relative = destination
        .segments
        .strip_prefix(receiver.segments.as_slice());
    crate::queries::expression_traversal::visit_pre_order(
        program,
        vec![predicate],
        |expression, _| {
            if let Some(projection) = super::place_queries::predicate_projection(
                program,
                place.definition,
                receiver_type,
                expression,
            ) && relative == Some(projection.as_slice())
                && crate::project_type_reference_from_segments(program, receiver_type, &projection)
                    .is_some_and(|reference| {
                        program
                            .primitive_type_reference(reference)
                            .is_some_and(PrimitiveType::is_fixed_integer)
                    })
            {
                fields.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value: written.clone(),
                });
            }
            false
        },
    );
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
        let value = if field.symbol == written_field && relative.is_some_and(|path| path.len() == 1)
        {
            Some(written.clone())
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
    // A logical-subject projection and its resolved declaration field denote
    // the same substituted scalar. Bind exact occurrences, not a partial
    // constructor that could stand for an unobserved complete record.
    let mut projections = Vec::new();
    crate::queries::expression_traversal::visit_pre_order(
        program,
        vec![predicate],
        |expression, _| {
            if let Some(field) =
                super::place_queries::membership_field(program, place.definition, expression)
                && let Some(binding) = fields
                    .iter()
                    .find(|binding| binding.binder == ScopedArithmeticBinder::Symbol(field.symbol))
            {
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value: binding.value.clone(),
                });
            }
            false
        },
    );
    fields.extend(projections);
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
        || crate::proof_contracts::contract_entailment::captured_data_predicate_holds(
            program,
            machine,
            state,
            crate::proof_contracts::contract_entailment::CapturedPredicatePoint::Prefix(
                completed_prefix,
            ),
            receiver,
            receiver_type,
            place.definition,
            predicate,
        )
}

/// The declaration interval of a field read whose carrier cannot have dropped
/// that interval. `None` when the read is not one field of a shared or
/// immutable binding, or when the declaration does not bound that field.
fn source_field_declaration_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<BigIntInterval> {
    let table = crate::frozen_program::expression_table(program);
    if let Some(member) = table.member_node(expression) {
        if !receiver_retains_declared_domain(program, state, member.receiver) {
            return None;
        }
        let definition = super::place_queries::data_definition_for_expression(
            program,
            machine,
            Some(state),
            member.receiver,
        )?;
        return declared_field_interval(program, definition, member.member.as_str());
    }
    let path = table.name_path_node(expression)?;
    let members = table.name_path_members(path.members);
    let [field_name] = members else {
        return None;
    };
    if !binding_retains_declared_domain(program, state, path.head_symbol) {
        return None;
    }
    let binding_type = binding_type_reference(program, state, path.head_symbol)?;
    let definition = super::place_queries::data_definition_for_type(program, binding_type)?;
    declared_field_interval(program, definition, field_name.as_str())
}

fn declared_field_interval(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field_name: &str,
) -> Option<BigIntInterval> {
    let field = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name => Some(field.symbol),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })?;
    let (_, low, high) =
        super::where_fact_intervals::data_where_field_intervals(program, definition)?
            .into_iter()
            .find(|(symbol, _, _)| *symbol == field)?;
    if low.is_none() && high.is_none() {
        return None;
    }
    Some(BigIntInterval {
        low: low.map(|bound| BigInt::from_i128(i128::from(bound))),
        high: high.map(|bound| BigInt::from_i128(i128::from(bound))),
    })
}

fn receiver_retains_declared_domain(
    program: &dyn crate::CheckingInput,
    state: &State,
    mut expression: ExpressionHandle,
) -> bool {
    let table = crate::frozen_program::expression_table(program);
    for _ in 0..16 {
        if !expression.is_valid() {
            return false;
        }
        match table.expression(expression) {
            typed_trees::expression::ExpressionNode::Member(member) => {
                expression = member.receiver;
            }
            typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                if borrow.access.is_exclusive() {
                    return false;
                }
                expression = borrow.target;
            }
            typed_trees::expression::ExpressionNode::Name(path) => {
                return binding_retains_declared_domain(program, state, path.head_symbol);
            }
            _ => return false,
        }
    }
    false
}

fn binding_retains_declared_domain(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    let Some(type_reference) = binding_type_reference(program, state, symbol) else {
        return false;
    };
    if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
        if parameter.is_mutable {
            return false;
        }
    } else if let Some((_, local)) = program.state_local_declaration(state, symbol) {
        if local.is_mutable {
            return false;
        }
    } else {
        return false;
    }
    crate::reference_shell(program, type_reference).is_none_or(|shell| !shell.access.is_exclusive())
}

fn binding_type_reference(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
        return Some(parameter.type_reference);
    }
    program
        .state_local_declaration(state, symbol)
        .map(|(_, local)| local.type_reference)
}

fn scalar_binding(symbol: SymbolHandle, primitive: PrimitiveType) -> ScopedArithmeticBinding {
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
