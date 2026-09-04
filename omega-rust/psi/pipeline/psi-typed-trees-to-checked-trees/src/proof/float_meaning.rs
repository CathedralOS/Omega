//! Erase validated source float-projection invocations into checked proof rows.

use psi_checked_trees::{
    CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
    CheckedDirectStructuralFloatLeaf, CheckedFloatMeaningEqualityProposition,
    CheckedFloatMeaningProjection, CheckedFloatMeaningProjectionOccurrence,
    CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionInput,
    CheckedFloatProjectionInputId, CheckedFloatProjectionSource, CheckedProofOnlyValueType,
    CheckedProofPropositionId, CheckedProofValueDeclaration, CheckedProofValueId,
    ContractProofFactKind, ProofFacts,
};
use psi_diagnostics::Diagnostic;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};
use psi_typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
use psi_typed_trees::types::PrimitiveType;
use psi_validation::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckedFloatProjectionSourceKey {
    DirectMachineParameter {
        owner_machine: psi_symbols::SymbolHandle,
        parameter: psi_symbols::SymbolHandle,
    },
    DirectMachineResult {
        owner_machine: psi_symbols::SymbolHandle,
    },
    DirectStructuralLeaf {
        owner_machine: psi_symbols::SymbolHandle,
        field: psi_checked_trees::CheckedStructuralParameterField,
    },
    ResolvedSymbol(psi_symbols::SymbolHandle),
    Binary32Literal(u32),
    Binary64Literal(u64),
    /// Transitional exact typed-expression custody for source forms whose
    /// artifact-reconstructible Terminal coordinate has not landed yet.
    TypedExpression(psi_typed_trees::expression::ExpressionHandle),
}

fn projection_source_key(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> CheckedFloatProjectionSourceKey {
    match program.expression_table.expression(fact.source) {
        ExpressionNode::Name(path) => direct_machine_parameter_source(program, proof, fact)
            .map(|(owner_machine, parameter)| {
                CheckedFloatProjectionSourceKey::DirectMachineParameter {
                    owner_machine,
                    parameter,
                }
            })
            .or_else(|| {
                direct_machine_result_source(program, proof, fact).map(|owner_machine| {
                    CheckedFloatProjectionSourceKey::DirectMachineResult { owner_machine }
                })
            })
            .or_else(|| {
                direct_structural_float_leaf_source(program, proof, fact).map(
                    |(owner_machine, field)| {
                        CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                            owner_machine,
                            field,
                        }
                    },
                )
            })
            .unwrap_or_else(|| {
                if path.symbol.is_valid() {
                    CheckedFloatProjectionSourceKey::ResolvedSymbol(path.symbol)
                } else {
                    CheckedFloatProjectionSourceKey::TypedExpression(fact.source)
                }
            }),
        ExpressionNode::Float(literal) => match fact.source_primitive {
            PrimitiveType::F32 => {
                CheckedFloatProjectionSourceKey::Binary32Literal(literal.f32_bits())
            }
            PrimitiveType::F64 => {
                CheckedFloatProjectionSourceKey::Binary64Literal(literal.landed_f64().to_bits())
            }
            _ => CheckedFloatProjectionSourceKey::TypedExpression(fact.source),
        },
        _ => direct_structural_float_leaf_source(program, proof, fact)
            .map(
                |(owner_machine, field)| CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                    owner_machine,
                    field,
                },
            )
            .unwrap_or(CheckedFloatProjectionSourceKey::TypedExpression(
                fact.source,
            )),
    }
}

fn direct_machine_contract_owner(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<psi_symbols::SymbolHandle> {
    let mut owners = proof.contract_facts.iter().filter_map(|(_, contract)| {
        let psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol } = contract.owner
        else {
            return None;
        };
        proof_fact_contains_expression(program, contract.fact, fact.invocation)
            .then_some(machine_symbol)
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

fn direct_structural_float_leaf_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<(
    psi_symbols::SymbolHandle,
    psi_checked_trees::CheckedStructuralParameterField,
)> {
    let owner_machine = direct_machine_contract_owner(program, proof, fact)?;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner_machine)?;
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    let place = crate::flow::canonical_place_from_expression(program, fact.source)?;
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let parameter_position = parameters
        .iter()
        .position(|parameter| {
            parameter.symbol == root
                && !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .and_then(|position| u32::try_from(position).ok())?;
    let path = place
        .segments
        .iter()
        .map(|segment| match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                structural_member_identity(program, *symbol)
                    .map(psi_checked_trees::CheckedStructuralPredicatePathSegment::Field)
            }
            psi_facts::PlaceSegment::Case { variant } => {
                structural_member_identity(program, *variant)
                    .map(psi_checked_trees::CheckedStructuralPredicatePathSegment::Case)
            }
            psi_facts::PlaceSegment::FixedIndex { .. }
            | psi_facts::PlaceSegment::FixedRange { .. }
            | psi_facts::PlaceSegment::Index { .. } => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if path.is_empty() {
        return None;
    }
    Some((
        owner_machine,
        psi_checked_trees::CheckedStructuralParameterField {
            parameter_position,
            path,
        },
    ))
}

fn structural_member_identity(
    program: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Option<String> {
    program.data_definitions().iter().find_map(|data| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => Some(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ),
                psi_typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(
                        variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned()),
                    )
                }
                psi_typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol)
                    .map(|field| {
                        field
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| field.name.as_str().to_owned())
                    }),
                psi_typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

fn direct_machine_result_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<psi_symbols::SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(fact.source) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if name.as_str() != "result" {
        return None;
    }
    let owner_machine = direct_machine_contract_owner(program, proof, fact)?;
    let owning_contract = proof.contract_facts.iter().any(|(_, contract)| {
        matches!(
            contract.owner,
            psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol }
                if machine_symbol == owner_machine
        ) && contract.kind == ContractProofFactKind::Ensures
            && proof_fact_contains_expression(program, contract.fact, fact.invocation)
    });
    if !owning_contract {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner_machine)?;
    let entry = program.machine_states(machine).first()?;
    if program
        .state_parameters(entry)
        .iter()
        .any(|parameter| !parameter.is_self && program.symbols.name(parameter.symbol) == "result")
    {
        return None;
    }
    let primitive = program.primitive_type_reference(entry.return_type)?;
    if primitive != fact.source_primitive
        || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
    {
        return None;
    }
    Some(owner_machine)
}

fn direct_machine_parameter_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<(psi_symbols::SymbolHandle, psi_symbols::SymbolHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(fact.source) else {
        return None;
    };
    if program
        .expression_table
        .name_path_members(path.members)
        .len()
        != 1
    {
        return None;
    }
    let owner_machine = direct_machine_contract_owner(program, proof, fact)?;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner_machine)?;
    let entry = program.machine_states(machine).first()?;
    let parameter = program
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)?;
    if parameter.is_const || parameter.is_self {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    if primitive != fact.source_primitive
        || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
    {
        return None;
    }
    Some((owner_machine, parameter.symbol))
}

fn proof_fact_contains_expression(
    program: &TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    target: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let roots: &[psi_typed_trees::expression::ExpressionHandle] =
        match program.proof_facts.get(fact) {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                std::slice::from_ref(expression)
            }
            psi_typed_trees::domain::ProofFact::Membership(membership) => {
                std::slice::from_ref(&membership.value)
            }
            psi_typed_trees::domain::ProofFact::Proposition(application) => program
                .expression_table
                .expression_handles(application.arguments),
        };
    roots
        .iter()
        .any(|root| expression_contains(program, *root, target, &mut Vec::new()))
}

fn expression_contains(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) -> bool {
    if expression == target {
        return true;
    }
    if !expression.is_valid() || visited.contains(&expression) {
        return false;
    }
    visited.push(expression);
    let mut children = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => children.extend(
            program
                .expression_table
                .expression_handles(*values)
                .iter()
                .copied(),
        ),
        ExpressionNode::Atomic(atomic) => children.extend([atomic.value, atomic.result]),
        ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
        ExpressionNode::Borrow(borrow) => children.push(borrow.target),
        ExpressionNode::Call(call) => {
            children.push(call.receiver);
            children.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        ExpressionNode::Cast(cast) => children.push(cast.value),
        ExpressionNode::Indexed(indexed) => children.extend([indexed.collection, indexed.index]),
        ExpressionNode::Member(member) => children.push(member.receiver),
        ExpressionNode::Range(range) => children.extend([range.start, range.end]),
        ExpressionNode::StructLiteral(literal) => children.extend(
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| field.value),
        ),
        ExpressionNode::Unary(unary) => children.push(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    children
        .into_iter()
        .any(|child| expression_contains(program, child, target, visited))
}

fn replay_invocation(
    program: &TypedTrees,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Result<psi_numerics::float_projection::FloatProjectionContractIdentity, Diagnostic> {
    let ExpressionNode::Call(call) = program.expression_table.expression(fact.invocation) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection invocation is no longer a call",
        ));
    };
    let operator = resolve_named_expression_call(program, call)
        .or_else(|| {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [namespace] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if namespace.as_str() != "Float" {
                return None;
            }
            let static_receiver = [namespace.as_str()];
            resolve_named_call(
                program,
                call.target_symbol,
                Some(&static_receiver),
                call.target.as_str(),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
                false,
            )
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "validated float-meaning projection operator no longer resolves exactly",
            )
        })?;
    if operator.symbol != fact.selected_operator_symbol {
        return Err(Diagnostic::error(
            "validated float-meaning projection operator identity drifted before checked binding",
        ));
    }
    let [namespace, name] = program.operator_path_members(operator.name) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection operator path is no longer exact",
        ));
    };
    let operation =
        FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
            .ok_or_else(|| {
                Diagnostic::error(
                    "validated float-meaning projection operator identity is not canonical",
                )
            })?;
    let Some((replayed_primitive, contract)) =
        psi_validation::exact_toolchain_float_projection_contract(program, operator, operation)
    else {
        return Err(Diagnostic::error(
            "validated float-meaning projection lost its sealed toolchain declaration before checked binding",
        ));
    };
    if operation != fact.operation {
        return Err(Diagnostic::error(
            "validated float-meaning projection operation drifted before checked binding",
        ));
    }
    if contract != fact.contract {
        return Err(Diagnostic::error(
            "validated float-meaning projection contract/catalog identity drifted before checked binding",
        ));
    }
    let [parameter] = program.operator_parameters(operator) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection signature no longer has one source parameter",
        ));
    };
    if !operator.symbol.is_valid()
        || operator.is_boundary
        || operator.spelling.is_some()
        || !operator.lifetime_parameters.is_empty()
        || !program.operator_type_parameters(operator).is_empty()
        || parameter.is_const
        || parameter.is_mutable
        || parameter.is_self
    {
        return Err(Diagnostic::error(
            "validated float-meaning projection declaration shape drifted before checked binding",
        ));
    }
    let [source] = program.expression_table.expression_handles(call.arguments) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection call no longer has one source operand",
        ));
    };
    if *source != fact.source || replayed_primitive != fact.source_primitive {
        return Err(Diagnostic::error(
            "validated float-meaning projection source/result shape drifted before checked binding",
        ));
    }
    Ok(contract)
}

pub(crate) fn bind_float_meaning_projection_facts(
    program: &TypedTrees,
    proof: &mut ProofFacts,
    facts: &[ValidatedFloatMeaningProjectionInvocation],
    equality_facts: &[ValidatedFloatMeaningEqualityProposition],
) -> Result<(), Vec<Diagnostic>> {
    let mut projections = Vec::with_capacity(facts.len());
    let mut transitional_source_keys = Vec::<CheckedFloatProjectionSourceKey>::new();
    let mut projection_keys = Vec::<(
        CheckedFloatProjectionSource,
        FloatProjectionOperation,
        psi_numerics::float_projection::FloatProjectionContractIdentity,
    )>::new();
    let mut invocation_values = Vec::with_capacity(facts.len());
    let mut occurrences = Vec::with_capacity(facts.len());
    for (index, fact) in facts.iter().copied().enumerate() {
        let contract = replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
        let source_key = projection_source_key(program, proof, fact);
        let source = match source_key {
            CheckedFloatProjectionSourceKey::Binary32Literal(bits) => {
                CheckedFloatProjectionSource::ExactBinary32Literal(bits)
            }
            CheckedFloatProjectionSourceKey::Binary64Literal(bits) => {
                CheckedFloatProjectionSource::ExactBinary64Literal(bits)
            }
            transitional => {
                let source_index = match transitional_source_keys
                    .iter()
                    .position(|key| *key == transitional)
                {
                    Some(index) => index,
                    None => {
                        transitional_source_keys.push(transitional.clone());
                        transitional_source_keys.len() - 1
                    }
                };
                let source_id =
                    CheckedFloatProjectionInputId(u32::try_from(source_index).map_err(|_| {
                        vec![Diagnostic::error(
                            "float-meaning projection sources exceed their dense identity space",
                        )]
                    })?);
                let fallback = CheckedFloatProjectionInput {
                    id: source_id,
                    primitive: fact.source_primitive,
                };
                match transitional {
                    CheckedFloatProjectionSourceKey::DirectMachineParameter {
                        owner_machine,
                        parameter,
                    } => CheckedFloatProjectionSource::DirectMachineParameter(
                        CheckedDirectMachineFloatParameter {
                            owner_machine,
                            parameter,
                            fallback,
                        },
                    ),
                    CheckedFloatProjectionSourceKey::DirectMachineResult { owner_machine } => {
                        CheckedFloatProjectionSource::DirectMachineResult(
                            CheckedDirectMachineFloatResult {
                                owner_machine,
                                fallback,
                            },
                        )
                    }
                    CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                        owner_machine,
                        field,
                    } => CheckedFloatProjectionSource::DirectStructuralLeaf(
                        CheckedDirectStructuralFloatLeaf {
                            owner_machine,
                            field,
                            fallback,
                        },
                    ),
                    CheckedFloatProjectionSourceKey::ResolvedSymbol(_)
                    | CheckedFloatProjectionSourceKey::TypedExpression(_) => {
                        CheckedFloatProjectionSource::TransitionalInput(fallback)
                    }
                    CheckedFloatProjectionSourceKey::Binary32Literal(_)
                    | CheckedFloatProjectionSourceKey::Binary64Literal(_) => {
                        unreachable!("exact literals were handled before transitional allocation")
                    }
                }
            }
        };
        let projection_key = (source.clone(), fact.operation, contract);
        let value_index = match projection_keys
            .iter()
            .position(|key| *key == projection_key)
        {
            Some(index) => index,
            None => {
                projection_keys.push(projection_key);
                let id = u32::try_from(projections.len()).map_err(|_| {
                    vec![Diagnostic::error(
                        "float-meaning projection plan exceeds its dense identity space",
                    )]
                })?;
                let projection = CheckedFloatMeaningProjection {
                    result: CheckedProofValueDeclaration {
                        id: CheckedProofValueId(id),
                        value_type: CheckedProofOnlyValueType::FloatMeaning,
                    },
                    source,
                    operation: fact.operation,
                    contract,
                };
                projection.validate().map_err(|_| {
                    vec![Diagnostic::error(
                        "checked float-meaning projection failed exact format replay",
                    )]
                })?;
                projections.push(projection);
                projections.len() - 1
            }
        };
        let value = CheckedProofValueId(u32::try_from(value_index).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning projection plan exceeds its dense identity space",
            )]
        })?);
        invocation_values.push((fact.invocation, value));
        occurrences.push(CheckedFloatMeaningProjectionOccurrence {
            id: CheckedFloatMeaningProjectionOccurrenceId(u32::try_from(index).map_err(|_| {
                vec![Diagnostic::error(
                    "float-meaning projection occurrences exceed their dense identity space",
                )]
            })?),
            value,
            source_span: program.expression_table.source_span(fact.invocation),
        });
    }
    if projection_keys.len() != projections.len() {
        return Err(vec![Diagnostic::error(
            "checked float-meaning projection canonicalization lost a semantic key",
        )]);
    }
    let mut equalities = Vec::with_capacity(equality_facts.len());
    for (index, fact) in equality_facts.iter().copied().enumerate() {
        let ExpressionNode::Binary(expression) =
            program.expression_table.expression(fact.expression)
        else {
            return Err(vec![Diagnostic::error(
                "validated float-meaning equality is no longer a binary proposition",
            )]);
        };
        if expression.operator != BinaryOperator::Equal
            || expression.left != fact.left
            || expression.right != fact.right
        {
            return Err(vec![Diagnostic::error(
                "validated float-meaning equality identity drifted before checked binding",
            )]);
        }
        let left_projection = facts
            .iter()
            .find(|projection| projection.invocation == fact.left)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its left projection contract",
                )]
            })?;
        let right_projection = facts
            .iter()
            .find(|projection| projection.invocation == fact.right)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its right projection contract",
                )]
            })?;
        if left_projection.source_primitive != right_projection.source_primitive
            || left_projection.operation != right_projection.operation
            || left_projection.contract != right_projection.contract
        {
            return Err(vec![Diagnostic::error(
                "validated FloatMeaningEqual operands do not share one exact format and projection contract",
            )]);
        }
        let left = invocation_values
            .iter()
            .find(|(invocation, _)| *invocation == fact.left)
            .map(|(_, value)| value.0)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its left projection",
                )]
            })?;
        let right = invocation_values
            .iter()
            .find(|(invocation, _)| *invocation == fact.right)
            .map(|(_, value)| value.0)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its right projection",
                )]
            })?;
        let id = u32::try_from(index).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning equality plan exceeds its dense identity space",
            )]
        })?;
        equalities.push(CheckedFloatMeaningEqualityProposition {
            id: CheckedProofPropositionId(id),
            left: CheckedProofValueId(left.min(right)),
            right: CheckedProofValueId(left.max(right)),
            source_expression: fact.expression,
        });
    }
    proof.float_meaning_projections = projections;
    proof.float_meaning_projection_occurrences = occurrences;
    proof.float_meaning_equalities = equalities;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::{CheckedFloatProjectionInputId, CheckedProofValueId};
    use psi_source::{SourceMap, SourceOrigin};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::{
        lower_syntax_trees, lower_syntax_trees_with_sources,
    };
    use psi_tokens_to_syntax_trees::{
        parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    const CORE_FLOAT_MEANING: &str = "data FloatMeaning { }";
    const CORE_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
    "#;

    fn lower_projection_fixture(source: &str) -> TypedTrees {
        lower_projection_fixture_with_meaning_origin(source, SourceOrigin::Toolchain)
    }

    fn lower_projection_fixture_with_meaning_origin(
        source: &str,
        meaning_origin: SourceOrigin,
    ) -> TypedTrees {
        lower_projection_fixture_with_metadata(
            source,
            meaning_origin,
            CORE_PROJECTIONS,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        )
    }

    fn lower_projection_fixture_with_metadata(
        source: &str,
        meaning_origin: SourceOrigin,
        projection_declarations: &str,
        projection_file: &str,
        projection_origin: SourceOrigin,
    ) -> TypedTrees {
        let mut sources = SourceMap::default();
        let meaning_source_id = sources
            .add_with_metadata(
                PathBuf::from("source/library/core/float_meaning.omg"),
                CORE_FLOAT_MEANING.to_owned(),
                PathBuf::from("source/library/core"),
                None,
                meaning_origin,
            )
            .source_id;
        let projection_source_id = sources
            .add_with_metadata(
                PathBuf::from("source/library/core").join(projection_file),
                projection_declarations.to_owned(),
                PathBuf::from("source/library/core"),
                None,
                projection_origin,
            )
            .source_id;
        let user_source_id = sources
            .add(
                PathBuf::from("tests/float_projection/main.omg"),
                source.to_owned(),
            )
            .source_id;
        let meaning_tokens = Lexer::new(CORE_FLOAT_MEANING)
            .tokenize()
            .expect("tokenize float meaning");
        let mut syntax = parse_syntax_trees_with_id(meaning_source_id, &meaning_tokens)
            .expect("parse float meaning");
        let projection_tokens = Lexer::new(projection_declarations)
            .tokenize()
            .expect("tokenize projections");
        parse_syntax_trees_into_with_id(&mut syntax, projection_source_id, &projection_tokens)
            .expect("parse core projections");
        let user_tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
        parse_syntax_trees_into_with_id(&mut syntax, user_source_id, &user_tokens)
            .expect("parse fixture");
        let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
            .expect("resolve source-aware projection fixture");
        lower_symbol_resolved_trees(&resolved).expect("type projection fixture")
    }

    fn lower_local_projection_lookalike(source: &str) -> TypedTrees {
        let combined = format!("{CORE_FLOAT_MEANING}\n{CORE_PROJECTIONS}\n{source}");
        let tokens = Lexer::new(&combined)
            .tokenize()
            .expect("tokenize lookalike");
        let syntax = parse_syntax_trees(&tokens).expect("parse lookalike");
        let resolved = lower_syntax_trees(&syntax).expect("resolve lookalike");
        lower_symbol_resolved_trees(&resolved).expect("type lookalike")
    }

    fn typed_projection_program() -> TypedTrees {
        lower_projection_fixture(projection_source())
    }

    fn local_projection_program() -> TypedTrees {
        lower_local_projection_lookalike(projection_source())
    }

    fn projection_source() -> &'static str {
        r#"
            machine prove(value32: f32, value64: f64)
            requires
                Float::meaning32(value32) == Float::meaning32(value32);
                Float::meaning64(value64) == Float::meaning64(value64);
            { }
        "#
    }

    fn typed_literal_projection_program() -> TypedTrees {
        let source = r#"
            machine prove()
            requires
                Float::meaning32(0.0f32) == Float::meaning32(0.00f32);
                Float::meaning32(-0.0f32) == Float::meaning32(-0.00f32);
                Float::meaning64(0.1f64) == Float::meaning64(0.10f64);
            { }
        "#;
        lower_projection_fixture(source)
    }

    fn transitional_source(input: CheckedFloatProjectionInput) -> CheckedFloatProjectionSource {
        CheckedFloatProjectionSource::TransitionalInput(input)
    }

    fn bind_projection_facts_without_exit_proof(program: &TypedTrees) -> ProofFacts {
        let validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(program)
                .expect("validate projection facts");
        let proof_plan = psi_proof::obligations::build_proof_plan(program);
        let borrow = crate::build_borrow_facts(program);
        let mut proof = crate::build_proof_facts(program, &proof_plan, &borrow);
        bind_float_meaning_projection_facts(
            program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect("bind projection facts");
        proof
    }

    #[test]
    fn actual_checked_projection_invocations_deduplicate_values_and_retain_occurrences() {
        let checked = crate::lower_typed_trees(typed_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 2);
        assert_eq!(projections[0].result.id, CheckedProofValueId(0));
        let CheckedFloatProjectionSource::DirectMachineParameter(narrow) = projections[0].source
        else {
            panic!("direct f32 parameter should retain checked provenance")
        };
        assert_eq!(checked.symbols.name(narrow.owner_machine), "prove");
        assert_eq!(checked.symbols.name(narrow.parameter), "value32");
        assert_eq!(
            narrow.fallback,
            CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F32,
            }
        );
        assert_eq!(
            projections[0].operation,
            FloatProjectionOperation::Meaning32
        );
        assert_eq!(projections[1].result.id, CheckedProofValueId(1));
        let CheckedFloatProjectionSource::DirectMachineParameter(wide) = projections[1].source
        else {
            panic!("direct f64 parameter should retain checked provenance")
        };
        assert_eq!(wide.owner_machine, narrow.owner_machine);
        assert_ne!(wide.parameter, narrow.parameter);
        assert_eq!(checked.symbols.name(wide.parameter), "value64");
        assert_eq!(
            wide.fallback,
            CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(1),
                primitive: PrimitiveType::F64,
            }
        );
        assert_eq!(
            projections[1].operation,
            FloatProjectionOperation::Meaning64
        );
        let owner = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == narrow.owner_machine)
            .expect("retained owner machine");
        let entry = checked
            .machine_states(owner)
            .first()
            .expect("machine entry state");
        let parameters = checked.state_parameters(entry);
        assert_eq!(parameters[0].symbol, narrow.parameter);
        assert_eq!(parameters[1].symbol, wide.parameter);
        assert_eq!(
            checked.primitive_type_reference(parameters[0].type_reference),
            Some(PrimitiveType::F32)
        );
        assert_eq!(
            checked.primitive_type_reference(parameters[1].type_reference),
            Some(PrimitiveType::F64)
        );
        let occurrences = &checked.facts.proof.float_meaning_projection_occurrences;
        assert_eq!(occurrences.len(), 4);
        assert_eq!(occurrences[0].value, CheckedProofValueId(0));
        assert_eq!(occurrences[1].value, CheckedProofValueId(0));
        assert_eq!(occurrences[2].value, CheckedProofValueId(1));
        assert_eq!(occurrences[3].value, CheckedProofValueId(1));
        assert_eq!(occurrences[0].id.0, 0);
        assert_eq!(occurrences[1].id.0, 1);
        assert_eq!(checked.facts.proof.float_meaning_equalities.len(), 2);
        let narrow_equality = checked.facts.proof.float_meaning_equalities[0];
        assert_eq!(narrow_equality.id, CheckedProofPropositionId(0));
        assert_eq!(narrow_equality.left, CheckedProofValueId(0));
        assert_eq!(narrow_equality.right, CheckedProofValueId(0));
        assert!(matches!(
            checked
                .expression_table
                .expression(narrow_equality.source_expression),
            ExpressionNode::Binary(expression) if expression.operator == BinaryOperator::Equal
        ));
    }

    #[test]
    fn direct_parameter_identity_includes_its_exact_machine_owner() {
        let checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine narrow(value: f32)
                requires Float::meaning32(value) == Float::meaning32(value);
                {}

                machine wide(value: f32)
                requires Float::meaning32(value) == Float::meaning32(value);
                {}
            "#,
        ))
        .expect("checked");
        let [narrow, wide] = checked.facts.proof.float_meaning_projections.as_slice() else {
            panic!("one projection for each exact machine parameter")
        };
        let CheckedFloatProjectionSource::DirectMachineParameter(narrow) = narrow.source else {
            panic!("narrow source should retain direct parameter provenance")
        };
        let CheckedFloatProjectionSource::DirectMachineParameter(wide) = wide.source else {
            panic!("wide source should retain direct parameter provenance")
        };
        assert_eq!(checked.symbols.name(narrow.owner_machine), "narrow");
        assert_eq!(checked.symbols.name(wide.owner_machine), "wide");
        assert_ne!(narrow.owner_machine, wide.owner_machine);
        assert_ne!(narrow.parameter, wide.parameter);
        assert_eq!(narrow.fallback.id, CheckedFloatProjectionInputId(0));
        assert_eq!(wide.fallback.id, CheckedFloatProjectionInputId(1));
    }

    #[test]
    fn top_level_scalar_result_retains_direct_checked_provenance() {
        let checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine result_source(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }
            "#,
        ))
        .expect("direct result reflexivity should pass ordinary exit checking");
        let result_proof = &checked.facts.proof;
        let CheckedFloatProjectionSource::DirectMachineResult(result) =
            result_proof.float_meaning_projections[0].source
        else {
            panic!("top-level scalar result should retain direct provenance")
        };
        assert_eq!(checked.symbols.name(result.owner_machine), "result_source");
        assert_eq!(
            result.fallback,
            CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F32,
            }
        );
    }

    #[test]
    fn direct_result_identity_includes_exact_owner_and_primitive_format() {
        let checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine narrow(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine wide(value: f64) -> f64
                ensures Float::meaning64(result) == Float::meaning64(result);
                { value }
            "#,
        ))
        .expect("direct result reflexivity should pass for both primitive formats");
        let proof = &checked.facts.proof;
        let [narrow, wide] = proof.float_meaning_projections.as_slice() else {
            panic!("one result projection per owning machine")
        };
        let CheckedFloatProjectionSource::DirectMachineResult(narrow) = narrow.source else {
            panic!("narrow result provenance")
        };
        let CheckedFloatProjectionSource::DirectMachineResult(wide) = wide.source else {
            panic!("wide result provenance")
        };
        assert_eq!(checked.symbols.name(narrow.owner_machine), "narrow");
        assert_eq!(checked.symbols.name(wide.owner_machine), "wide");
        assert_ne!(narrow.owner_machine, wide.owner_machine);
        assert_eq!(narrow.fallback.primitive, PrimitiveType::F32);
        assert_eq!(wide.fallback.primitive, PrimitiveType::F64);
    }

    #[test]
    fn direct_result_reflexivity_does_not_prove_a_distinct_parameter_projection() {
        let diagnostics = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine distinct(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(value);
                { value }
            "#,
        ))
        .expect_err("distinct checked projection terms require explicit evidence");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove ensures contract for exit from distinct")
        }));
    }

    #[test]
    fn raw_float_result_equality_does_not_borrow_float_meaning_reflexivity() {
        let diagnostics = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine raw(value: f32) -> f32
                ensures result == result;
                { value }
            "#,
        ))
        .expect_err("IEEE equality is not FloatMeaning structural equality");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove ensures contract for exit from raw")
        }));
    }

    #[test]
    fn real_result_named_parameter_shadows_the_contract_pseudo_result() {
        let program = lower_projection_fixture(
            r#"
                machine shadow(result: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { result }
            "#,
        );
        let proof = bind_projection_facts_without_exit_proof(&program);
        let CheckedFloatProjectionSource::DirectMachineParameter(parameter) =
            proof.float_meaning_projections[0].source
        else {
            panic!("real parameter must shadow the reserved pseudo-result")
        };
        assert_eq!(program.symbols.name(parameter.owner_machine), "shadow");
        assert_eq!(program.symbols.name(parameter.parameter), "result");
        let diagnostics = crate::lower_typed_trees(program)
            .expect_err("a real result parameter must not receive pseudo-result reflexivity");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove ensures contract for exit from shadow")
        }));
    }

    #[test]
    fn direct_structural_member_retains_checked_owner_and_path() {
        let member_checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                data Sample { value: f32; }
                machine member_source(sample: Sample)
                requires Float::meaning32(sample.value) == Float::meaning32(sample.value);
                {}
            "#,
        ))
        .expect("checked member source");
        let CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) =
            &member_checked.facts.proof.float_meaning_projections[0].source
        else {
            panic!("direct structural member should retain checked provenance")
        };
        assert_eq!(
            member_checked.symbols.name(leaf.owner_machine),
            "member_source"
        );
        assert_eq!(leaf.field.parameter_position, 0);
        assert_eq!(
            leaf.field.path,
            [psi_checked_trees::CheckedStructuralPredicatePathSegment::Field("value".to_owned())]
        );
        assert_eq!(
            leaf.fallback,
            CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F32,
            }
        );
    }

    #[test]
    fn cast_and_state_owned_sources_remain_transitional() {
        let cast_checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine cast_source(value: f32)
                requires
                    Float::meaning64(value as f64) == Float::meaning64(value as f64);
                {}
            "#,
        ))
        .expect("checked cast source");
        assert_eq!(
            cast_checked.facts.proof.float_meaning_projections[0].source,
            transitional_source(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F64,
            })
        );

        let state_checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine state_source() {
                    state inspect(value: f64)
                    requires Float::meaning64(value) == Float::meaning64(value);
                    {}
                }
            "#,
        ))
        .expect("checked state-owned source");
        assert_eq!(
            state_checked.facts.proof.float_meaning_projections[0].source,
            transitional_source(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F64,
            })
        );

        let const_checked = crate::lower_typed_trees(lower_projection_fixture(
            r#"
                machine const_source<const Value: f32>()
                requires Float::meaning32(Value) == Float::meaning32(Value);
                {}
            "#,
        ))
        .expect("checked const-parameter source");
        assert_eq!(
            const_checked.facts.proof.float_meaning_projections[0].source,
            transitional_source(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F32,
            })
        );
    }

    #[test]
    fn exact_literal_bits_are_the_checked_semantic_source_identity() {
        let checked =
            crate::lower_typed_trees(typed_literal_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 3);
        assert_eq!(
            projections[0].source,
            CheckedFloatProjectionSource::ExactBinary32Literal(0.0_f32.to_bits())
        );
        assert_eq!(
            projections[1].source,
            CheckedFloatProjectionSource::ExactBinary32Literal((-0.0_f32).to_bits())
        );
        assert_eq!(
            projections[2].source,
            CheckedFloatProjectionSource::ExactBinary64Literal(0.1_f64.to_bits())
        );
        assert_ne!(projections[0].result.id, projections[1].result.id);
        assert_eq!(
            checked
                .facts
                .proof
                .float_meaning_projection_occurrences
                .iter()
                .map(|occurrence| occurrence.value)
                .collect::<Vec<_>>(),
            vec![
                CheckedProofValueId(0),
                CheckedProofValueId(0),
                CheckedProofValueId(1),
                CheckedProofValueId(1),
                CheckedProofValueId(2),
                CheckedProofValueId(2),
            ]
        );
    }

    #[test]
    fn checked_binding_rejects_source_identity_substitution() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        assert_eq!(validation.float_meaning_projection_invocations.len(), 4);
        validation.float_meaning_projection_invocations[0].source =
            validation.float_meaning_projection_invocations[1].invocation;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("substituted source identity must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("source/result shape drifted")
        );
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_cross_format_operation_tamper() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        validation.float_meaning_projection_invocations[0].operation =
            FloatProjectionOperation::Meaning64;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("cross-format operation must reject");
        assert!(diagnostics[0].message.contains("operation drifted"));
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_catalog_contract_tamper() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        validation.float_meaning_projection_invocations[0]
            .contract
            .catalog_version += 1;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("catalog contract substitution must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("contract/catalog identity drifted")
        );
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_forged_cross_format_equality_fact() {
        let mut program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        let cross_format_right = validation.float_meaning_equality_propositions[1].right;
        let equality = &mut validation.float_meaning_equality_propositions[0];
        equality.right = cross_format_right;
        let ExpressionNode::Binary(expression) =
            program.expression_table.expression_mut(equality.expression)
        else {
            unreachable!("validated equality is binary")
        };
        expression.right = cross_format_right;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("cross-format forged equality fact must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("do not share one exact format")
        );
        assert!(proof.float_meaning_equalities.is_empty());
    }

    #[test]
    fn source_validation_rejects_cross_format_float_meaning_equal() {
        let program = lower_projection_fixture(
            r#"
                machine prove()
                requires Float::meaning32(0.0f32) == Float::meaning64(0.0f64);
                { }
            "#,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("FloatMeaningEqual is carrier-specific");
        assert!(
            diagnostics[0]
                .message
                .contains("same exact format and projection contract")
        );
    }

    #[test]
    fn proof_projection_call_rejects_a_local_operator_lookalike() {
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(
                &local_projection_program(),
            )
            .expect_err("a local projection spelling has no closed-catalog authority");
        assert!(
            diagnostics[0]
                .message
                .contains("did not resolve one exact canonical operator signature"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn canonical_projection_rejects_a_user_owned_float_meaning_result() {
        let program =
            lower_projection_fixture_with_meaning_origin(projection_source(), SourceOrigin::User);
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the canonical operator cannot return a user FloatMeaning lookalike");
        assert!(
            diagnostics[0]
                .message
                .contains("sealed toolchain `FloatMeaning`"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn projection_call_rejects_a_toolchain_declaration_from_the_wrong_file() {
        let program = lower_projection_fixture_with_metadata(
            projection_source(),
            SourceOrigin::Toolchain,
            CORE_PROJECTIONS,
            "float_projection_lookalike.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("a different toolchain file cannot own Float projection semantics");
        assert!(
            diagnostics[0]
                .message
                .contains("did not resolve one exact canonical operator signature"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn canonical_projection_declaration_rejects_source_format_drift() {
        let program = lower_projection_fixture_with_metadata(
            "machine main() { }",
            SourceOrigin::Toolchain,
            r#"
                operator Float::meaning32(value: f64) -> FloatMeaning;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the sealed meaning32 declaration cannot drift to binary64");
        assert!(
            diagnostics[0].message.contains("from `f32`"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn canonical_projection_declaration_rejects_public_visibility_drift() {
        let program = lower_projection_fixture_with_metadata(
            projection_source(),
            SourceOrigin::Toolchain,
            r#"
                pub operator Float::meaning32(value: f32) -> FloatMeaning;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the sealed projection declaration is private");
        assert!(
            diagnostics[0]
                .message
                .contains("ordinary tokenless operator")
        );
    }

    #[test]
    fn canonical_projection_declaration_rejects_contract_drift() {
        let program = lower_projection_fixture_with_metadata(
            projection_source(),
            SourceOrigin::Toolchain,
            r#"
                operator Float::meaning32(value: f32) -> FloatMeaning
                requires true == true;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the sealed projection declaration is contract-free");
        assert!(
            diagnostics[0]
                .message
                .contains("ordinary tokenless operator")
        );
    }

    #[test]
    fn checked_binding_rejects_validated_facts_replayed_on_a_local_lookalike() {
        let canonical = typed_projection_program();
        let validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(
                &canonical,
            )
            .expect("validate canonical toolchain projections");
        let local = local_projection_program();
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &local,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("validated facts cannot transfer to a local projection lookalike");
        assert!(
            diagnostics[0]
                .message
                .contains("sealed toolchain declaration"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_equality_operand_substitution_transactionally() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        validation.float_meaning_equality_propositions[0].left =
            validation.float_meaning_projection_invocations[2].invocation;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("substituted equality operand must reject");
        assert!(diagnostics[0].message.contains("identity drifted"));
        assert!(proof.float_meaning_projections.is_empty());
        assert!(proof.float_meaning_equalities.is_empty());
    }
}
