use crate::context::*;
use crate::flow::effective_member_symbol;
use crate::semantic_calls::call_target_parameters;
use crate::semantic_places::{append_place_segment, resolve_place_member_symbol};
use psi_facts::PlaceHandle;

pub(crate) fn contract_fact_place(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        psi_typed_trees::domain::ProofFact::Expression(expression) => FactPlace::Place(
            contract_expression_place(program, facts, contract, *expression)
                .unwrap_or_else(|| facts.append_place_from_expression(program, *expression)),
        ),
        psi_typed_trees::domain::ProofFact::Membership(membership) => FactPlace::Place(
            contract_expression_place(program, facts, contract, membership.value)
                .unwrap_or_else(|| facts.append_place_from_expression(program, membership.value)),
        ),
        psi_typed_trees::domain::ProofFact::Proposition(application) => {
            FactPlace::Place(facts.append_symbol_place(application.proposition))
        }
    }
}

/// Returns every place read by a boolean contract expression. Requires facts
/// are stored in one context with a copy rooted at each dependency, so
/// mutating any operand invalidates the whole assumption rather than leaving
/// a comparison alive under an opaque expression root.
pub(crate) fn contract_fact_dependency_places(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> Vec<PlaceHandle> {
    let psi_typed_trees::domain::ProofFact::Expression(expression) =
        program.proof_facts.get(contract.fact)
    else {
        return Vec::new();
    };
    let mut places = Vec::new();
    append_contract_expression_dependency_places(
        program,
        facts,
        contract,
        *expression,
        &mut places,
    );
    places
}

fn append_contract_expression_dependency_places(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    expression: ExpressionHandle,
    places: &mut Vec<PlaceHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            if let Some(place) = contract_expression_place(program, facts, contract, expression) {
                places.push(place);
            }
        }
        ExpressionNode::Mutable(inner) => {
            append_contract_expression_dependency_places(program, facts, contract, *inner, places)
        }
        ExpressionNode::Atomic(atomic) => {
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                atomic.value,
                places,
            );
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                atomic.result,
                places,
            );
        }
        ExpressionNode::Binary(binary) => {
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                binary.left,
                places,
            );
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                binary.right,
                places,
            );
        }
        ExpressionNode::Cast(cast) => append_contract_expression_dependency_places(
            program, facts, contract, cast.value, places,
        ),
        ExpressionNode::Call(call) => {
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                call.receiver,
                places,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_contract_expression_dependency_places(
                    program, facts, contract, *argument, places,
                );
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_contract_expression_dependency_places(
                    program, facts, contract, *value, places,
                );
            }
        }
        ExpressionNode::Range(range) => {
            append_contract_expression_dependency_places(
                program,
                facts,
                contract,
                range.start,
                places,
            );
            append_contract_expression_dependency_places(
                program, facts, contract, range.end, places,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_contract_expression_dependency_places(
                    program,
                    facts,
                    contract,
                    field.value,
                    places,
                );
            }
        }
        ExpressionNode::Unary(unary) => append_contract_expression_dependency_places(
            program,
            facts,
            contract,
            unary.operand,
            places,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn contract_expression_place(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            contract_expression_place(program, facts, contract, *inner)
        }
        ExpressionNode::Name(path) => contract_name_path_place(program, facts, contract, path),
        ExpressionNode::Member(member) => {
            let receiver = contract_expression_place(program, facts, contract, member.receiver)
                .unwrap_or_else(|| facts.append_place_from_expression(program, member.receiver));
            let symbol = {
                let symbol = effective_member_symbol(program, member.receiver, member);
                if symbol.is_valid() {
                    symbol
                } else {
                    resolve_place_member_symbol(program, facts, receiver, member.member.as_str())
                        .unwrap_or_else(SymbolHandle::invalid)
                }
            };
            let receiver =
                if let Some(variant) = psi_facts::payload_variant_for_field(program, symbol) {
                    append_place_segment(facts, receiver, psi_facts::PlaceSegment::Case { variant })
                } else {
                    receiver
                };
            Some(append_place_segment(
                facts,
                receiver,
                psi_facts::PlaceSegment::Field { symbol },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection =
                contract_expression_place(program, facts, contract, indexed.collection)
                    .unwrap_or_else(|| {
                        facts.append_place_from_expression(program, indexed.collection)
                    });
            Some(append_place_segment(
                facts,
                collection,
                crate::flow::index_place_segment(program, indexed.index),
            ))
        }
        _ => None,
    }
}

fn contract_name_path_place(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let member_symbols = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);

    let (mut place, start_index) = if path.head_symbol.is_valid() {
        (facts.append_symbol_place(path.head_symbol), 1usize)
    } else if let Some(parameter_symbol) =
        contract_owner_parameter_symbol(program, contract.owner, path, members.first())
    {
        // A head name that resolves to a parameter of the contract's owner
        // (for example `item` in `requires item in Item::Ready`) roots the
        // place at that parameter, matching how call-site obligations
        // instantiate the same contract against caller arguments.
        (facts.append_symbol_place(parameter_symbol), 1usize)
    } else {
        let self_symbol = contract_owner_self_symbol(program, contract.owner)?;
        let start_index = usize::from(
            members
                .first()
                .is_some_and(|member| member.as_str() == "self"),
        );
        (facts.append_symbol_place(self_symbol), start_index)
    };

    for (offset, member_name) in members.iter().skip(start_index).enumerate() {
        let member_symbol = member_symbols
            .get(offset + start_index)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| resolve_place_member_symbol(program, facts, place, member_name.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        if let Some(variant) = psi_facts::payload_variant_for_field(program, member_symbol) {
            place = append_place_segment(facts, place, psi_facts::PlaceSegment::Case { variant });
        }
        place = append_place_segment(
            facts,
            place,
            psi_facts::PlaceSegment::Field {
                symbol: member_symbol,
            },
        );
    }

    Some(place)
}

fn contract_owner_self_symbol(
    program: &psi_typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
) -> Option<SymbolHandle> {
    contract_owner_parameter(program, owner, |parameter| parameter.is_self)
        .map(|parameter| parameter.symbol)
}

/// Resolves a contract name-path head that names one of the owner's
/// parameters (for example `item` in `requires item in Item::Ready`). Returns
/// the parameter symbol so the fact place is rooted at the parameter, which
/// is how call-site obligations instantiate the same contract.
fn contract_owner_parameter_symbol(
    program: &psi_typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
    path: &psi_typed_trees::expression::TableNamePath,
    head_name: Option<&psi_typed_trees::name::Identifier>,
) -> Option<SymbolHandle> {
    let head_name = head_name?.as_str();
    if head_name == "self" {
        return None;
    }

    contract_owner_parameter(program, owner, |parameter| {
        !parameter.is_self
            && (parameter.name.as_str() == head_name
                || (path.symbol.is_valid() && parameter.symbol == path.symbol))
    })
    .map(|parameter| parameter.symbol)
}

fn contract_owner_parameter<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
    mut matches: impl FnMut(&psi_typed_trees::signature::StateParameter) -> bool,
) -> Option<&'program psi_typed_trees::signature::StateParameter> {
    let state_symbol = match owner {
        ContractProofFactOwner::MachineState { state_symbol, .. }
        | ContractProofFactOwner::StateSignature { state_symbol, .. } => state_symbol,
        ContractProofFactOwner::Machine { machine_symbol } => {
            return program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
                .and_then(|machine| {
                    program.machine_states(machine).iter().find_map(|state| {
                        program
                            .state_parameters(state)
                            .iter()
                            .find(|parameter| matches(parameter))
                    })
                });
        }
        ContractProofFactOwner::Unknown | ContractProofFactOwner::OperatorUse { .. } => {
            return None;
        }
    };

    // Machine states resolve through their machine; a StateSignature owner can
    // also be a trait machine signature (boundary trait contracts), whose
    // parameters live on the signature itself.
    call_target_parameters(program, state_symbol)?
        .iter()
        .find(|parameter| matches(parameter))
}
