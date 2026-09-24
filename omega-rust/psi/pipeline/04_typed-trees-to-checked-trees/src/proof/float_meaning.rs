//! Erase validated source float-projection invocations into checked proof
//! rows, and bind the sealed `FloatSemantics` applications those rows
//! reference so a contract obligation cites its exact checked kernel binding.
//!
//! Semantic applications ride the same projection table: each application
//! owns one canonical proof value whose projection source stays transitional
//! until lowering rejoins it to the Terminal `SemanticApplication` carrier.
//! Validation and checked binding share exact sealed contract and format
//! recognition. Binding records identity; it does not itself prove equality.

use checked_trees::{
    CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
    CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
    CheckedDirectOperationFloatResult, CheckedDirectStructuralFloatLeaf,
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionOccurrence, CheckedFloatMeaningProjectionOccurrenceId,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite,
    CheckedProofOnlyValueType, CheckedProofPropositionId, CheckedProofValueDeclaration,
    CheckedProofValueId, ContractProofFactKind, ProofFacts,
};
use diagnostics::Diagnostic;
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics_catalog::{FloatSemanticContractIdentity, FloatSemanticValueKind};
use semantic_vocabulary::IeeeFloatFormat;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
use typed_trees::types::PrimitiveType;
use validation::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
    exact_toolchain_float_format_const, exact_toolchain_float_semantic_contract,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckedFloatProjectionSourceKey {
    DirectMachineParameter {
        owner_machine: symbols::SymbolHandle,
        parameter: symbols::SymbolHandle,
    },
    DirectMachineResult {
        owner_machine: symbols::SymbolHandle,
    },
    DirectBlockParameter {
        owner_machine: symbols::SymbolHandle,
        owner_state: symbols::SymbolHandle,
        parameter: symbols::SymbolHandle,
    },
    DirectStructuralLeaf {
        owner_machine: symbols::SymbolHandle,
        field: checked_trees::CheckedStructuralParameterField,
    },
    /// The scalar result produced at one exact checked call use. A
    /// transported `ensures` clause names the importing use's produced
    /// result, never a shared declaration-level value.
    DirectCallResult {
        use_site: CheckedFloatUseSite,
    },
    /// The scalar result produced at one exact checked non-call operation
    /// use. `use_expression` keeps two uses sharing a statement distinct
    /// before the lowered occurrence join runs.
    DirectOperationResult {
        use_site: CheckedFloatUseSite,
        use_expression: typed_trees::expression::ExpressionHandle,
    },
    ResolvedSymbol(symbols::SymbolHandle),
    Binary32Literal(u32),
    Binary64Literal(u64),
    /// Transitional exact typed-expression custody for source forms whose
    /// artifact-reconstructible Terminal coordinate has not landed yet.
    TypedExpression(typed_trees::expression::ExpressionHandle),
    /// One application of a sealed `FloatSemantics` catalog row. The key
    /// carries the exact contract, declared result format, and bound operand
    /// values, so the result dedupes only when the whole semantic
    /// application repeats; distinct transport sites and operand bindings
    /// keep distinct transitional identities.
    SemanticApplication {
        contract: FloatSemanticContractIdentity,
        format: IeeeFloatFormat,
        operands: Vec<CheckedFloatSemanticApplicationOperand>,
    },
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
                direct_block_parameter_source(program, proof, fact).map(
                    |(owner_machine, owner_state, parameter)| {
                        CheckedFloatProjectionSourceKey::DirectBlockParameter {
                            owner_machine,
                            owner_state,
                            parameter,
                        }
                    },
                )
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

/// Unique machine contract owner carrying the validated invocation, retaining
/// the owning nested state when the fact belongs to a state-owned arrival
/// contract. Nested states admit `requires` only; their direct scalar
/// parameters are Terminal block parameters, a distinct source class from
/// machine parameters.
fn direct_machine_contract_owner(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<(symbols::SymbolHandle, Option<symbols::SymbolHandle>)> {
    let mut owners = proof.contract_facts.iter().filter_map(|(_, contract)| {
        let owner = match contract.owner {
            checked_trees::ContractProofFactOwner::Machine { machine_symbol } => {
                (machine_symbol, None)
            }
            checked_trees::ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } => (machine_symbol, Some(state_symbol)),
            _ => return None,
        };
        proof_fact_contains_expression(program, contract.fact, fact.invocation).then_some(owner)
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

fn direct_structural_float_leaf_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<(
    symbols::SymbolHandle,
    checked_trees::CheckedStructuralParameterField,
)> {
    let (owner_machine, _) = direct_machine_contract_owner(program, proof, fact)?;
    let machine = crate::lookup::machine_by_symbol(program, owner_machine)?;
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    let place = crate::flow::canonical_place_from_expression(program, fact.source)?;
    let facts::PlaceRoot::Symbol(root) = place.root else {
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
            facts::PlaceSegment::Field { symbol } => structural_member_identity(program, *symbol)
                .map(checked_trees::CheckedStructuralPredicatePathSegment::Field),
            facts::PlaceSegment::Case { variant } => structural_member_identity(program, *variant)
                .map(checked_trees::CheckedStructuralPredicatePathSegment::Case),
            facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::Index { .. } => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if path.is_empty() {
        return None;
    }
    Some((
        owner_machine,
        checked_trees::CheckedStructuralParameterField {
            parameter_position,
            path,
        },
    ))
}

fn structural_member_identity(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<String> {
    program.data_definitions().iter().find_map(|data| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => Some(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ),
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(
                        variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned()),
                    )
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol)
                    .map(|field| {
                        field
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| field.name.as_str().to_owned())
                    }),
                typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

fn direct_machine_result_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<symbols::SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(fact.source) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if name.as_str() != "result" {
        return None;
    }
    let (owner_machine, _) = direct_machine_contract_owner(program, proof, fact)?;
    let owning_contract = proof.contract_facts.iter().any(|(_, contract)| {
        matches!(
            contract.owner,
            checked_trees::ContractProofFactOwner::Machine { machine_symbol }
                if machine_symbol == owner_machine
        ) && contract.kind == ContractProofFactKind::Ensures
            && proof_fact_contains_expression(program, contract.fact, fact.invocation)
    });
    if !owning_contract {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, owner_machine)?;
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
) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
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
    let (owner_machine, _) = direct_machine_contract_owner(program, proof, fact)?;
    let machine = crate::lookup::machine_by_symbol(program, owner_machine)?;
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

/// A scalar parameter of the owning nested state is a Terminal block
/// parameter. The entry state is excluded: its parameters already carry the
/// machine-parameter class. States admit arrival `requires` only, so no
/// result carrier exists here.
fn direct_block_parameter_source(
    program: &TypedTrees,
    proof: &ProofFacts,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Option<(
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
)> {
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
    let (owner_machine, owner_state) = direct_machine_contract_owner(program, proof, fact)?;
    let owner_state = owner_state?;
    let machine = crate::lookup::machine_by_symbol(program, owner_machine)?;
    let entry = program.machine_states(machine).first()?;
    if entry.symbol == owner_state {
        return None;
    }
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == owner_state)?;
    let parameter = program
        .state_parameters(state)
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
    Some((owner_machine, owner_state, parameter.symbol))
}

fn proof_fact_contains_expression(
    program: &TypedTrees,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    target: typed_trees::expression::ExpressionHandle,
) -> bool {
    let roots: &[typed_trees::expression::ExpressionHandle] = match program.proof_facts.get(fact) {
        typed_trees::domain::ProofFact::Expression(expression) => std::slice::from_ref(expression),
        typed_trees::domain::ProofFact::Membership(membership) => {
            std::slice::from_ref(&membership.value)
        }
        typed_trees::domain::ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments),
    };
    roots
        .iter()
        .any(|root| expression_contains(program, *root, target, &mut Vec::new()))
}

fn expression_children(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut children = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            children.push(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    children.push(pattern);
                }
                children.push(arm.value);
            }
        }
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
}

fn expression_contains(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<typed_trees::expression::ExpressionHandle>,
) -> bool {
    if expression == target {
        return true;
    }
    if !expression.is_valid() || visited.contains(&expression) {
        return false;
    }
    visited.push(expression);
    expression_children(program, expression)
        .into_iter()
        .any(|child| expression_contains(program, child, target, visited))
}

fn replay_invocation(
    program: &TypedTrees,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Result<numerics::float_projection::FloatProjectionContractIdentity, Diagnostic> {
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
        validation::exact_toolchain_float_projection_contract(program, operator, operation)
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

type FloatMeaningProjectionKey = (
    CheckedFloatProjectionSource,
    FloatProjectionOperation,
    numerics::float_projection::FloatProjectionContractIdentity,
);

/// Canonicalize one checked float-meaning projection row: allocate the
/// transitional input identity once per exact source key, then dedupe on
/// (source, operation, contract) so a repeated invocation names the same
/// canonical proof value.
fn push_float_meaning_projection(
    projections: &mut Vec<CheckedFloatMeaningProjection>,
    projection_keys: &mut Vec<FloatMeaningProjectionKey>,
    transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
    source_key: CheckedFloatProjectionSourceKey,
    operation: FloatProjectionOperation,
    contract: numerics::float_projection::FloatProjectionContractIdentity,
    source_primitive: PrimitiveType,
) -> Result<CheckedProofValueId, Vec<Diagnostic>> {
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
                primitive: source_primitive,
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
                CheckedFloatProjectionSourceKey::DirectBlockParameter {
                    owner_machine,
                    owner_state,
                    parameter,
                } => CheckedFloatProjectionSource::DirectBlockParameter(
                    CheckedDirectBlockFloatParameter {
                        owner_machine,
                        owner_state,
                        parameter,
                        fallback,
                    },
                ),
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
                CheckedFloatProjectionSourceKey::DirectCallResult { use_site } => {
                    CheckedFloatProjectionSource::DirectCallResult(CheckedDirectCallFloatResult {
                        use_site,
                        fallback,
                    })
                }
                CheckedFloatProjectionSourceKey::DirectOperationResult {
                    use_site,
                    use_expression,
                } => CheckedFloatProjectionSource::DirectOperationResult(
                    CheckedDirectOperationFloatResult {
                        use_site,
                        use_expression,
                        fallback,
                    },
                ),
                CheckedFloatProjectionSourceKey::ResolvedSymbol(_)
                | CheckedFloatProjectionSourceKey::TypedExpression(_)
                | CheckedFloatProjectionSourceKey::SemanticApplication { .. } => {
                    CheckedFloatProjectionSource::TransitionalInput(fallback)
                }
                CheckedFloatProjectionSourceKey::Binary32Literal(_)
                | CheckedFloatProjectionSourceKey::Binary64Literal(_) => {
                    unreachable!("exact literals were handled before transitional allocation")
                }
            }
        }
    };
    let projection_key = (source.clone(), operation, contract);
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
                operation,
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
    Ok(CheckedProofValueId(u32::try_from(value_index).map_err(
        |_| {
            vec![Diagnostic::error(
                "float-meaning projection plan exceeds its dense identity space",
            )]
        },
    )?))
}

/// The source key one imported `ensures` operand takes at a call use site:
/// each callee parameter names the authored argument expression supplied at
/// the site, and the `result` operand names the scalar the call itself
/// produces there.
fn imported_call_operand_key(
    program: &TypedTrees,
    invocation: &ValidatedFloatMeaningProjectionInvocation,
    use_site: CheckedFloatUseSite,
    target_parameters: Option<&[typed_trees::signature::StateParameter]>,
    argument_expressions: Option<&[typed_trees::expression::ExpressionHandle]>,
) -> CheckedFloatProjectionSourceKey {
    let ExpressionNode::Name(path) = program.expression_table.expression(invocation.source) else {
        return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
    };
    if path.symbol.is_valid()
        && let Some(parameters) = target_parameters
        && let Some(position) = parameters
            .iter()
            .position(|parameter| parameter.symbol == path.symbol)
        && let Some(arguments) = argument_expressions
        && let Some(argument) = arguments.get(position)
    {
        return CheckedFloatProjectionSourceKey::TypedExpression(*argument);
    }
    let parameter_named_result = target_parameters.is_some_and(|parameters| {
        parameters.iter().any(|parameter| {
            !parameter.is_self && program.symbols.name(parameter.symbol) == "result"
        })
    });
    if name.as_str() == "result" && !parameter_named_result {
        return CheckedFloatProjectionSourceKey::DirectCallResult { use_site };
    }
    CheckedFloatProjectionSourceKey::TypedExpression(invocation.source)
}

/// The source key one imported `ensures` operand takes at a boundary
/// operation use: the `result` operand names the scalar the selected
/// operation produces there.
fn imported_operation_operand_key(
    program: &TypedTrees,
    invocation: &ValidatedFloatMeaningProjectionInvocation,
    use_site: CheckedFloatUseSite,
    use_expression: typed_trees::expression::ExpressionHandle,
) -> CheckedFloatProjectionSourceKey {
    let ExpressionNode::Name(path) = program.expression_table.expression(invocation.source) else {
        return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
    };
    if name.as_str() == "result" {
        return CheckedFloatProjectionSourceKey::DirectOperationResult {
            use_site,
            use_expression,
        };
    }
    CheckedFloatProjectionSourceKey::TypedExpression(invocation.source)
}

/// Instantiate one transported `ensures` expression at one use site: for
/// every validated float-meaning equality the expression authored, emit the
/// per-site equality whose operands re-bind through `operand_key`. Imported
/// rows keep the declaration's `source_expression`; the use-site coordinate
/// keeps the reflexivity rejoin's (owner, expression, use site) key disjoint
/// from declaration rows.
fn instantiate_equalities_at_use(
    facts: &[ValidatedFloatMeaningProjectionInvocation],
    equality_facts: &[ValidatedFloatMeaningEqualityProposition],
    expression: typed_trees::expression::ExpressionHandle,
    use_site: CheckedFloatUseSite,
    operand_key: &dyn Fn(
        &ValidatedFloatMeaningProjectionInvocation,
    ) -> CheckedFloatProjectionSourceKey,
    projections: &mut Vec<CheckedFloatMeaningProjection>,
    projection_keys: &mut Vec<FloatMeaningProjectionKey>,
    transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
    invocation_contracts: &[(
        typed_trees::expression::ExpressionHandle,
        numerics::float_projection::FloatProjectionContractIdentity,
    )],
    equalities: &mut Vec<CheckedFloatMeaningEqualityProposition>,
) -> Result<(), Vec<Diagnostic>> {
    for equality_fact in equality_facts
        .iter()
        .copied()
        .filter(|fact| fact.expression == expression)
    {
        let mut operands = Vec::with_capacity(2);
        for invocation_handle in [equality_fact.left, equality_fact.right] {
            let invocation = facts
                .iter()
                .find(|invocation| invocation.invocation == invocation_handle)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "validated float-meaning equality lost its projection contract",
                    )]
                })?;
            let contract = invocation_contracts
                .iter()
                .find(|(invocation, _)| *invocation == invocation_handle)
                .map(|(_, contract)| *contract)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "validated float-meaning equality lost its projection contract",
                    )]
                })?;
            let key = operand_key(invocation);
            operands.push(push_float_meaning_projection(
                projections,
                projection_keys,
                transitional_source_keys,
                key,
                invocation.operation,
                contract,
                invocation.source_primitive,
            )?);
        }
        let [left, right] = operands.as_slice() else {
            unreachable!("float-meaning equality has exactly two operands")
        };
        let id = u32::try_from(equalities.len()).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning equality plan exceeds its dense identity space",
            )]
        })?;
        equalities.push(CheckedFloatMeaningEqualityProposition {
            id: CheckedProofPropositionId(id),
            left: CheckedProofValueId(left.0.min(right.0)),
            right: CheckedProofValueId(left.0.max(right.0)),
            source_expression: equality_fact.expression,
            use_site: Some(use_site),
        });
    }
    Ok(())
}

/// A transported `ensures` clause instantiates per use site: every imported
/// equality re-binds `result` to the scalar produced by that exact call or
/// boundary operation use and each callee parameter to the authored argument
/// expression supplied there. Imported instances share the declaration's
/// checked contract expression; the use-site coordinate joins each
/// instantiation to the exact producer the verifier rejoins.
fn instantiate_transported_ensures(
    program: &TypedTrees,
    proof: &ProofFacts,
    facts: &[ValidatedFloatMeaningProjectionInvocation],
    equality_facts: &[ValidatedFloatMeaningEqualityProposition],
    projections: &mut Vec<CheckedFloatMeaningProjection>,
    projection_keys: &mut Vec<FloatMeaningProjectionKey>,
    transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
    invocation_contracts: &[(
        typed_trees::expression::ExpressionHandle,
        numerics::float_projection::FloatProjectionContractIdentity,
    )],
    equalities: &mut Vec<CheckedFloatMeaningEqualityProposition>,
    applications: &mut Vec<CheckedFloatSemanticApplication>,
) -> Result<(), Vec<Diagnostic>> {
    for (_, call) in proof.contract_calls.iter() {
        let use_site = CheckedFloatUseSite {
            owner_machine: call.caller_machine_symbol,
            owner_state: call.caller_state_symbol,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
        };
        let argument_expressions = crate::semantic::calls::find_call_site(
            program,
            call.caller_machine_symbol,
            call.caller_state_symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .map(|site| crate::semantic::calls::call_site_argument_expressions(program, &site));
        let target_parameters =
            crate::semantic::calls::call_target_parameters(program, call.target_state_symbol);
        let operand_key = |invocation: &ValidatedFloatMeaningProjectionInvocation| {
            imported_call_operand_key(
                program,
                invocation,
                use_site,
                target_parameters,
                argument_expressions,
            )
        };
        for fact_ref in proof.contract_fact_refs.span_or_empty(call.ensures) {
            let contract_fact = proof.contract_facts.get(fact_ref.fact);
            if contract_fact.kind != ContractProofFactKind::Ensures {
                continue;
            }
            let typed_trees::domain::ProofFact::Expression(expression) =
                program.proof_facts.get(contract_fact.fact)
            else {
                continue;
            };
            let expression = *expression;
            instantiate_equalities_at_use(
                facts,
                equality_facts,
                expression,
                use_site,
                &operand_key,
                projections,
                projection_keys,
                transitional_source_keys,
                invocation_contracts,
                equalities,
            )?;
            let mut application_values = Vec::new();
            let mut application_tables = SemanticApplicationTables {
                projections,
                projection_keys,
                transitional_source_keys,
                applications,
                application_values: &mut application_values,
                equalities,
            };
            bind_semantic_applications_in_expression(
                program,
                expression,
                &SemanticOperandSite::UseSite {
                    facts,
                    invocation_contracts,
                    operand_key: &operand_key,
                },
                Some(use_site),
                &mut application_tables,
                &mut Vec::new(),
            )?;
        }
    }
    for (_, operator_use) in proof.contract_operator_uses.iter() {
        let checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: owner_machine,
            state_symbol: owner_state,
            statement_index,
            ..
        } = operator_use.origin
        else {
            continue;
        };
        let use_site = CheckedFloatUseSite {
            owner_machine,
            owner_state,
            statement_index,
            // The selected IEEE fused multiply-add — the one non-call float
            // producer carrying contract uses — occupies call ordinal zero of
            // its containing statement; the lowered occurrence join rejoins
            // that exact coordinate.
            call_ordinal: 0,
        };
        let use_expression = operator_use.expression;
        let operand_key = |invocation: &ValidatedFloatMeaningProjectionInvocation| {
            imported_operation_operand_key(program, invocation, use_site, use_expression)
        };
        for fact_ref in proof.contract_fact_refs.span_or_empty(operator_use.ensures) {
            let contract_fact = proof.contract_facts.get(fact_ref.fact);
            if contract_fact.kind != ContractProofFactKind::Ensures {
                continue;
            }
            let typed_trees::domain::ProofFact::Expression(expression) =
                program.proof_facts.get(contract_fact.fact)
            else {
                continue;
            };
            let expression = *expression;
            instantiate_equalities_at_use(
                facts,
                equality_facts,
                expression,
                use_site,
                &operand_key,
                projections,
                projection_keys,
                transitional_source_keys,
                invocation_contracts,
                equalities,
            )?;
            let mut application_values = Vec::new();
            let mut application_tables = SemanticApplicationTables {
                projections,
                projection_keys,
                transitional_source_keys,
                applications,
                application_values: &mut application_values,
                equalities,
            };
            bind_semantic_applications_in_expression(
                program,
                expression,
                &SemanticOperandSite::UseSite {
                    facts,
                    invocation_contracts,
                    operand_key: &operand_key,
                },
                Some(use_site),
                &mut application_tables,
                &mut Vec::new(),
            )?;
        }
    }
    Ok(())
}

fn ieee_format_primitive(format: IeeeFloatFormat) -> PrimitiveType {
    match format {
        IeeeFloatFormat::Binary32 => PrimitiveType::F32,
        IeeeFloatFormat::Binary64 => PrimitiveType::F64,
    }
}

fn ieee_format_operation(format: IeeeFloatFormat) -> FloatProjectionOperation {
    match format {
        IeeeFloatFormat::Binary32 => FloatProjectionOperation::Meaning32,
        IeeeFloatFormat::Binary64 => FloatProjectionOperation::Meaning64,
    }
}

/// How one `Meaning`-typed application argument reaches its checked proof
/// value. A declaration position names the authored invocation's canonical
/// value; a transported-ensures use site re-binds each operand invocation
/// through the site's operand key so the per-site application cites the
/// exact argument the use produced.
enum SemanticOperandSite<'a> {
    Declaration(
        &'a [(
            typed_trees::expression::ExpressionHandle,
            CheckedProofValueId,
        )],
    ),
    UseSite {
        facts: &'a [ValidatedFloatMeaningProjectionInvocation],
        invocation_contracts: &'a [(
            typed_trees::expression::ExpressionHandle,
            numerics::float_projection::FloatProjectionContractIdentity,
        )],
        operand_key: &'a dyn Fn(
            &ValidatedFloatMeaningProjectionInvocation,
        ) -> CheckedFloatProjectionSourceKey,
    },
}

/// The tables semantic-application binding writes through, shared by the
/// declaration pass and every transported-ensures instantiation so each
/// application result joins the one canonical projection space.
/// `application_values` maps an authored application expression to its
/// bound value within one binding context — reset per use site because the
/// shared declaration text produces site-specific operand values there.
struct SemanticApplicationTables<'a> {
    projections: &'a mut Vec<CheckedFloatMeaningProjection>,
    projection_keys: &'a mut Vec<FloatMeaningProjectionKey>,
    transitional_source_keys: &'a mut Vec<CheckedFloatProjectionSourceKey>,
    applications: &'a mut Vec<CheckedFloatSemanticApplication>,
    application_values: &'a mut Vec<(
        typed_trees::expression::ExpressionHandle,
        CheckedProofValueId,
    )>,
    equalities: &'a mut Vec<CheckedFloatMeaningEqualityProposition>,
}

/// Resolve one `Meaning`-typed application argument to its bound proof value
/// and format. Nested applications already visited post-order rejoin through
/// `application_values`; an argument that is neither a bound invocation nor
/// a bound application binds nothing.
fn semantic_meaning_operand(
    argument: ExpressionHandle,
    site: &SemanticOperandSite<'_>,
    tables: &mut SemanticApplicationTables<'_>,
) -> Result<Option<(CheckedProofValueId, IeeeFloatFormat)>, Vec<Diagnostic>> {
    let operand_format =
        |value: CheckedProofValueId, tables: &SemanticApplicationTables<'_>| -> IeeeFloatFormat {
            let projection = &tables.projections[usize::try_from(value.0).unwrap_or(usize::MAX)];
            match projection.operation {
                FloatProjectionOperation::Meaning32 => IeeeFloatFormat::Binary32,
                FloatProjectionOperation::Meaning64 => IeeeFloatFormat::Binary64,
            }
        };
    if let Some((_, value)) = tables
        .application_values
        .iter()
        .find(|(expression, _)| *expression == argument)
    {
        return Ok(Some((*value, operand_format(*value, tables))));
    }
    match site {
        SemanticOperandSite::Declaration(invocation_values) => Ok(invocation_values
            .iter()
            .find(|(invocation, _)| *invocation == argument)
            .map(|(_, value)| (*value, operand_format(*value, tables)))),
        SemanticOperandSite::UseSite {
            facts,
            invocation_contracts,
            operand_key,
        } => {
            let Some(invocation) = facts
                .iter()
                .find(|invocation| invocation.invocation == argument)
            else {
                return Ok(None);
            };
            let Some(contract) = invocation_contracts
                .iter()
                .find(|(handle, _)| *handle == argument)
                .map(|(_, contract)| *contract)
            else {
                return Ok(None);
            };
            let value = push_float_meaning_projection(
                tables.projections,
                tables.projection_keys,
                tables.transitional_source_keys,
                operand_key(invocation),
                invocation.operation,
                contract,
                invocation.source_primitive,
            )?;
            Ok(Some((value, operand_format(value, tables))))
        }
    }
}

/// Bind one expression-position `FloatSemantics::<name>(...)` call to its
/// checked semantic-application row. Only the sealed toolchain declaration
/// binds, only `Meaning` results occupy the proof-value carrier, and every
/// operand must spell its catalog kind — a `Format` parameter takes the
/// sealed `FloatFormat::BINARY*` const, a `Meaning` parameter an already
/// bound invocation or nested application; unrepresentable operands bind no
/// row rather than approximating.
fn bind_semantic_application_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    site: &SemanticOperandSite<'_>,
    tables: &mut SemanticApplicationTables<'_>,
) -> Result<Option<CheckedProofValueId>, Vec<Diagnostic>> {
    let Some(operator) = resolve_named_expression_call(program, call) else {
        return Ok(None);
    };
    let Some((row, contract)) = exact_toolchain_float_semantic_contract(program, operator) else {
        return Ok(None);
    };
    if row.result != FloatSemanticValueKind::Meaning {
        return Ok(None);
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != row.parameters.len() {
        return Err(vec![Diagnostic::error(
            "validated float-semantic application arity drifted before checked binding",
        )]);
    }
    let mut operands = Vec::with_capacity(arguments.len());
    let mut declared_format = None;
    let mut operand_format = None;
    for (kind, argument) in row.parameters.iter().zip(arguments.iter()) {
        match kind {
            FloatSemanticValueKind::Format => {
                let Some(format) = exact_toolchain_float_format_const(program, *argument) else {
                    return Ok(None);
                };
                declared_format = Some(format);
                operands.push(CheckedFloatSemanticApplicationOperand::Format(format));
            }
            FloatSemanticValueKind::Meaning => {
                let Some((value, format)) = semantic_meaning_operand(*argument, site, tables)?
                else {
                    return Ok(None);
                };
                operand_format = match operand_format {
                    None => Some(format),
                    Some(existing) if existing == format => Some(existing),
                    Some(existing) => {
                        return Err(vec![Diagnostic::error(format!(
                            "float-semantic application meaning operands do not share one exact format ({existing:?} vs {format:?})"
                        ))]);
                    }
                };
                operands.push(CheckedFloatSemanticApplicationOperand::Meaning(value));
            }
            FloatSemanticValueKind::Bool
            | FloatSemanticValueKind::Class
            | FloatSemanticValueKind::Integer(_) => return Ok(None),
        }
    }
    let Some(format) = declared_format.or(operand_format) else {
        return Ok(None);
    };
    let operation = ieee_format_operation(format);
    let value = push_float_meaning_projection(
        tables.projections,
        tables.projection_keys,
        tables.transitional_source_keys,
        CheckedFloatProjectionSourceKey::SemanticApplication {
            contract,
            format,
            operands: operands.clone(),
        },
        operation,
        operation.contract_identity(),
        ieee_format_primitive(format),
    )?;
    if !tables
        .applications
        .iter()
        .any(|application| application.result == value)
    {
        let application = CheckedFloatSemanticApplication {
            result: value,
            contract,
            format,
            operands,
        };
        application.validate().map_err(|_| {
            vec![Diagnostic::error(
                "checked float-semantic application failed exact catalog replay",
            )]
        })?;
        tables.applications.push(application);
    }
    tables.application_values.push((expression, value));
    Ok(Some(value))
}

/// Emit the proof-only equality a `==` proposition between bound meaning
/// values authors. This covers only pairs the validated equality facts do
/// not already carry — a side bound by a semantic application — so a
/// two-invocation equality never duplicates here.
fn bind_semantic_application_equality(
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
    site: &SemanticOperandSite<'_>,
    use_site: Option<CheckedFloatUseSite>,
    tables: &mut SemanticApplicationTables<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let is_application = |expression: ExpressionHandle, tables: &SemanticApplicationTables<'_>| {
        tables
            .application_values
            .iter()
            .any(|(bound, _)| *bound == expression)
    };
    if !is_application(binary.left, tables) && !is_application(binary.right, tables) {
        return Ok(());
    }
    let mut operands = Vec::with_capacity(2);
    for operand in [binary.left, binary.right] {
        let Some((value, _)) = semantic_meaning_operand(operand, site, tables)? else {
            return Ok(());
        };
        operands.push(value);
    }
    let [left, right] = operands.as_slice() else {
        unreachable!("float-semantic equality has exactly two operands")
    };
    let left_row = &tables.projections[usize::try_from(left.0).unwrap_or(usize::MAX)];
    let right_row = &tables.projections[usize::try_from(right.0).unwrap_or(usize::MAX)];
    if left_row.operation != right_row.operation || left_row.contract != right_row.contract {
        return Err(vec![Diagnostic::error(
            "checked float-semantic equality operands do not share one exact format and projection contract",
        )]);
    }
    let id = u32::try_from(tables.equalities.len()).map_err(|_| {
        vec![Diagnostic::error(
            "float-meaning equality plan exceeds its dense identity space",
        )]
    })?;
    tables
        .equalities
        .push(CheckedFloatMeaningEqualityProposition {
            id: CheckedProofPropositionId(id),
            left: CheckedProofValueId(left.0.min(right.0)),
            right: CheckedProofValueId(left.0.max(right.0)),
            source_expression: expression,
            use_site,
        });
    Ok(())
}

/// Post-order walk binding the semantic applications inside one contract
/// expression: inner applications bind before the outer rows referencing
/// them, each `FloatSemantics::*` call gains a checked application row, and
/// each `==` proposition with an application operand emits its equality at
/// the given use-site coordinate (`None` at the declaration).
fn bind_semantic_applications_in_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    site: &SemanticOperandSite<'_>,
    use_site: Option<CheckedFloatUseSite>,
    tables: &mut SemanticApplicationTables<'_>,
    visited: &mut Vec<ExpressionHandle>,
) -> Result<Option<CheckedProofValueId>, Vec<Diagnostic>> {
    if !expression.is_valid() {
        return Ok(None);
    }
    if let Some((_, value)) = tables
        .application_values
        .iter()
        .find(|(bound, _)| *bound == expression)
    {
        return Ok(Some(*value));
    }
    if visited.contains(&expression) {
        return Ok(None);
    }
    visited.push(expression);
    for child in expression_children(program, expression) {
        bind_semantic_applications_in_expression(program, child, site, use_site, tables, visited)?;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            bind_semantic_application_call(program, expression, call, site, tables)
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            bind_semantic_application_equality(expression, binary, site, use_site, tables)?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Bind every semantic application authored in the program's checked contract
/// expressions and each `==` equality that references one — the
/// declaration-level rows a transported `ensures` shares before it
/// instantiates at a use site.
fn bind_declaration_semantic_applications(
    program: &TypedTrees,
    invocation_values: &[(
        typed_trees::expression::ExpressionHandle,
        CheckedProofValueId,
    )],
    tables: &mut SemanticApplicationTables<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let site = SemanticOperandSite::Declaration(invocation_values);
    let mut visited = Vec::new();
    for (_, fact) in program.proof_facts.iter() {
        let roots: &[ExpressionHandle] = match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                std::slice::from_ref(expression)
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                std::slice::from_ref(&membership.value)
            }
            typed_trees::domain::ProofFact::Proposition(application) => program
                .expression_table
                .expression_handles(application.arguments),
        };
        for root in roots {
            bind_semantic_applications_in_expression(
                program,
                *root,
                &site,
                None,
                tables,
                &mut visited,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn bind_float_meaning_projection_facts(
    program: &TypedTrees,
    proof: &mut ProofFacts,
    facts: &[ValidatedFloatMeaningProjectionInvocation],
    equality_facts: &[ValidatedFloatMeaningEqualityProposition],
) -> Result<(), Vec<Diagnostic>> {
    let mut projections = Vec::with_capacity(facts.len());
    let mut transitional_source_keys = Vec::<CheckedFloatProjectionSourceKey>::new();
    let mut projection_keys = Vec::<FloatMeaningProjectionKey>::new();
    let mut invocation_values = Vec::with_capacity(facts.len());
    let mut invocation_contracts = Vec::with_capacity(facts.len());
    let mut occurrences = Vec::with_capacity(facts.len());
    for (index, fact) in facts.iter().copied().enumerate() {
        let contract = replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
        let source_key = projection_source_key(program, proof, fact);
        let value = push_float_meaning_projection(
            &mut projections,
            &mut projection_keys,
            &mut transitional_source_keys,
            source_key,
            fact.operation,
            contract,
            fact.source_primitive,
        )?;
        invocation_values.push((fact.invocation, value));
        invocation_contracts.push((fact.invocation, contract));
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
            use_site: None,
        });
    }
    let mut applications = Vec::new();
    {
        let mut application_values = Vec::new();
        let mut application_tables = SemanticApplicationTables {
            projections: &mut projections,
            projection_keys: &mut projection_keys,
            transitional_source_keys: &mut transitional_source_keys,
            applications: &mut applications,
            application_values: &mut application_values,
            equalities: &mut equalities,
        };
        bind_declaration_semantic_applications(
            program,
            &invocation_values,
            &mut application_tables,
        )?;
    }
    instantiate_transported_ensures(
        program,
        proof,
        facts,
        equality_facts,
        &mut projections,
        &mut projection_keys,
        &mut transitional_source_keys,
        &invocation_contracts,
        &mut equalities,
        &mut applications,
    )?;
    proof.float_meaning_projections = projections;
    proof.float_meaning_projection_occurrences = occurrences;
    proof.float_meaning_equalities = equalities;
    proof.float_semantic_applications = applications;
    Ok(())
}

#[cfg(test)]
mod tests;
