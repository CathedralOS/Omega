//! Consumer-facing result types, independent of an enclosing destination.
//!
//! Declaration-backed leaves retain their complete type. Computed builtin
//! results require the node's actual selected meaning and signature. Direct
//! operators may expose caller-independent declared results; copying a
//! sibling's type could turn a comparison into an integer or export unproved
//! input refinements. Guard candidate lookup remains separate: its conservative
//! operand shells are selection inputs, not computed-result evidence.

use language_core::OperatorSpelling;
use numerics::arithmetic::ArithmeticDomain;
use symbols::BuiltinTypeAtom;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Return an existing result reference, never an expected-type guess. An
/// unresolved result does not establish anonymous numeric meaning. Builtin
/// computed results retain their carrier and policy, not input predicates.
/// Direct operators retain closed builtin carrier/policy result references.
/// Casts retain their normalized semantic-domain result separately from the
/// authored membership target. Operator results needing instantiated semantic
/// domains, predicates or selected traits remain unresolved rather than losing
/// their meaning.
pub fn expression_result_type_reference(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    result_type(
        program,
        ExpressionOwner::State { machine, state },
        expression,
        &mut Vec::new(),
    )
}

/// Interpret an abstract callable's expression in its exact declaration
/// telescope. Parameters and reserved result occurrences retain their owner;
/// computed nodes share the selected-result rules used by executable states.
pub fn parameter_expression_result_type_reference(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !owner_symbol.is_valid() {
        return None;
    }
    declaration_expression_result_type_reference(
        program,
        owner_symbol,
        &|expression| {
            crate::reserved_result_place(program, expression)
                .filter(|place| place.machine_symbol == owner_symbol)
                .map(|place| place.type_reference)
                .or_else(|| {
                    crate::value_custody::places::parameter_scoped_type_reference(
                        program, parameters, expression,
                    )
                })
        },
        expression,
    )
}

/// A declaration supplies its exact place types; the shared result walk owns
/// selected computation results. The explicit callback binds data/domain
/// subjects, parameters and reserved results without creating another abstract
/// expression recognizer or inferring a type from the cast destination.
pub(crate) fn declaration_expression_result_type_reference(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    place_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !owner_symbol.is_valid() {
        return None;
    }
    result_type(
        program,
        ExpressionOwner::Declaration {
            owner_symbol,
            place_type,
        },
        expression,
        &mut Vec::new(),
    )
}

// Domain predicates use the same selected-result rules as state expressions.
// Only declaration-backed leaves differ: self belongs to the domain carrier,
// while static indices retain their own declared types.
pub(crate) fn domain_expression_result_type_reference(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    result_type(
        program,
        ExpressionOwner::Domain(domain),
        expression,
        &mut Vec::new(),
    )
}

#[derive(Clone, Copy)]
enum ExpressionOwner<'program> {
    State {
        machine: &'program Machine,
        state: &'program State,
    },
    Domain(&'program typed_trees::domain::DomainDefinition),
    Declaration {
        owner_symbol: symbols::SymbolHandle,
        place_type: &'program dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    },
}

impl ExpressionOwner<'_> {
    fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::State { machine, .. } => machine.symbol,
            Self::Domain(domain) => domain.symbol,
            Self::Declaration { owner_symbol, .. } => owner_symbol,
        }
    }
}

fn result_type(
    program: &dyn crate::CheckingInput,
    owner: ExpressionOwner<'_>,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return None;
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => result_type(program, owner, atomic.value, active),
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            let references = arms
                .iter()
                .map(|arm| result_type(program, owner, arm.value, active))
                .collect::<Vec<_>>();
            join_result_type_references(
                program,
                &references,
                &arms
                    .iter()
                    .zip(&references)
                    .map(|(arm, reference)| {
                        reference.is_none()
                            && crate::value_custody::literals::has_anonymous_numeric_results(
                                program, arm.value,
                            )
                    })
                    .collect::<Vec<_>>(),
            )
        }
        ExpressionNode::Call(call) => {
            // Inside an abstract trait contract, a sibling requirement's
            // generic result already belongs to this exact telescope. Runtime
            // callers cannot borrow that raw result: they need its applied
            // signature. Keep this owner distinction instead of making the
            // context-free call query guess a generic instantiation.
            let scoped_requirement_result = match owner {
                ExpressionOwner::Declaration { owner_symbol, .. }
                    if call.target_symbol.is_valid() =>
                {
                    let parent = program.symbols.get(owner_symbol).parent;
                    (program.symbols.get(call.target_symbol).parent == parent)
                        .then(|| program.trait_definition_by_symbol(parent))
                        .flatten()
                        .and_then(|definition| {
                            program
                                .trait_machine_signature_by_symbol(definition, call.target_symbol)
                        })
                        .filter(|signature| {
                            signature.type_parameters.is_empty() && signature.return_type.is_valid()
                        })
                        .map(|signature| signature.return_type)
                }
                _ => None,
            };
            scoped_requirement_result
                .or_else(|| crate::machine_calls::calls::resolved_call_result_type(program, call))
                .or_else(|| {
                    typed_trees::operator::resolve_named_expression_call(program, call).and_then(
                        |operator| {
                            let operands: Vec<_> = program
                                .expression_table
                                .expression_handles(call.arguments)
                                .iter()
                                .map(|argument| result_type(program, owner, *argument, active))
                                .collect();
                            bound_result_type_parameter(program, operator, &operands).or_else(
                                || {
                                    (!result_mentions_type_parameters(
                                        program,
                                        operator,
                                        operator.return_type,
                                    ))
                                    .then_some(operator.return_type)
                                },
                            )
                        },
                    )
                })
                .or_else(|| {
                    if typed_trees::operator::resolve_named_expression_call(program, call).is_some()
                    {
                        return None;
                    }
                    program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        .filter(|function| function.is_scalar_computation())?;
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    let references: Vec<_> = arguments
                        .iter()
                        .map(|argument| {
                            result_type(program, owner, *argument, active).and_then(|reference| {
                                arithmetic_result_type_reference(program, reference)
                            })
                        })
                        .collect();
                    let anonymous: Vec<_> = arguments
                        .iter()
                        .map(|argument| {
                            crate::value_custody::literals::has_anonymous_numeric_results(
                                program, *argument,
                            )
                        })
                        .collect();
                    // Computation preserves the carrier and arithmetic policy,
                    // not the operand's range or refinement predicates.
                    join_result_type_references(program, &references, &anonymous)
                })
                .and_then(|result| {
                    // A numeric `as` retains the source operand's operation
                    // policy beneath its selected raw conversion. The authored
                    // cast owns the result qualification; transferring the source
                    // policy here would invent another result operation (or ask
                    // for a nonexistent float Wrapping carrier).
                    if call.exact_coercion_target.is_valid() {
                        return Some(result);
                    }
                    // Spelled boundary machines are rewritten to calls naming
                    // their entry state. That state belongs to the same exact
                    // callable contract as the original operator declaration;
                    // its bare signature result is not an adapted result type.
                    let Some(operator) =
                        typed_trees::operator::resolve_named_expression_call(program, call)
                            .or_else(|| {
                                crate::machine_calls::calls::callee_entry_contract(
                                    program,
                                    call.target_symbol,
                                )
                                .and_then(|contract| {
                                    typed_trees::operator::declaration_by_symbol(
                                        program,
                                        contract.callable,
                                    )
                                })
                            })
                    else {
                        return Some(result);
                    };
                    let operands: Vec<_> = program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| result_type(program, owner, *argument, active))
                        .collect();
                    selected_operator_result_type_reference(
                        program,
                        operator.symbol,
                        result,
                        &operands,
                    )
                })
        }
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol),
        ExpressionNode::Cast(cast) => cast_result_type_reference(program, cast),
        ExpressionNode::Indexed(indexed)
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            (|| {
                let range = program.expression_table.range_node(indexed.index)?;
                if range.end_inclusive {
                    return None;
                }
                let collection = result_type(program, owner, indexed.collection, active)?;
                let operands = [
                    Some(collection),
                    range
                        .start
                        .is_valid()
                        .then(|| result_type(program, owner, range.start, active))
                        .flatten(),
                    range
                        .end
                        .is_valid()
                        .then(|| result_type(program, owner, range.end, active))
                        .flatten(),
                ];
                if !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    program,
                    OperatorSpelling::Range,
                    &operands,
                    program.expression_table.source_span(expression),
                )
                .is_empty()
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        owner.symbol(),
                        expression,
                        OperatorSpelling::Range,
                        &operands,
                    )
                {
                    return None;
                }
                // A fresh descriptor preserves elements and the source loan's
                // access, but carries no source collection value predicates.
                let carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(collection);
                let (access, carrier) = match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } => (*access, *referee),
                    _ => (language_core::ReferenceAccess::Shared, carrier),
                };
                if access == language_core::ReferenceAccess::WriteOnly {
                    return None;
                }
                let carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(carrier);
                let element = match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Slice { element_type }
                    | TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
                    _ => return None,
                };
                let identity = program.normalized_type_identity(element);
                program
                    .type_reference_table
                    .slice_view_references()
                    .find_map(|(reference, candidate_access, candidate_element)| {
                        (candidate_access == access
                            && program.normalized_type_identity(candidate_element) == identity)
                            .then_some(reference)
                    })
            })()
        }
        ExpressionNode::Member(member) => result_type(program, owner, member.receiver, active)
            .and_then(|receiver| {
                let data =
                    crate::value_custody::places::data_definition_for_type(program, receiver)?;
                if let Some(instance) = data.generic_instance {
                    return program
                        .closed_data_member_field(
                            instance,
                            member.member_symbol,
                            member.member.as_str(),
                        )
                        .map(|field| field.type_reference);
                }
                crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|case| case.as_str()),
                )
                .map(|field| field.type_reference)
            }),
        ExpressionNode::Indexed(indexed) => (|| {
            let operands = [indexed.collection, indexed.index]
                .map(|operand| result_type(program, owner, operand, active));
            let candidates = typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                OperatorSpelling::Index,
                &operands,
                program.expression_table.source_span(expression),
            );
            if candidates.is_empty()
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner.symbol(),
                    expression,
                    OperatorSpelling::Index,
                    &operands,
                )
            {
                // Indexing projects the referent's element for both owned
                // collections and borrowed views. Its result type grants no
                // access: the ordinary index/custody checks still own the read.
                operands[0].and_then(|collection| {
                    crate::value_custody::places::collection_element_type_reference(
                        program, collection,
                    )
                })
            } else {
                let [selected] = candidates.as_slice() else {
                    return None;
                };
                if !typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    owner.symbol(),
                    OperatorSpelling::Index,
                    &operands,
                )
                .is_empty()
                {
                    return None;
                }
                bound_result_type_parameter(program, selected.operator, &operands).or_else(|| {
                    (!result_mentions_type_parameters(
                        program,
                        selected.operator,
                        selected.operator.return_type,
                    ))
                    .then_some(selected.operator.return_type)
                })
            }
        })(),
        ExpressionNode::ZeroValue(reference) => Some(*reference),
        // Place lookup strips a Borrow to its target. A value-type query must
        // not turn that view into a by-value operand and hide reference-typed
        // operator candidates, including through a nested Match result.
        ExpressionNode::Borrow(_) => None,
        ExpressionNode::Integer(literal) => {
            let reference = crate::declarations::operators::landed_integer_literal_type_reference(
                program, expression,
            );
            reference.and_then(|reference| {
                let domain = literal.landing()?.domain;
                if domain == ArithmeticDomain::Exact {
                    Some(reference)
                } else {
                    program
                        .type_reference_table
                        .find_arithmetic_result_type_reference(
                            program.type_reference_table.type_symbol(reference),
                            domain,
                        )
                }
            })
        }
        ExpressionNode::Boolean(_) => {
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                program,
                BuiltinTypeAtom::Bool,
            )
        }
        ExpressionNode::UndeterminedNaN(format) => {
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                program,
                match format {
                    numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                    numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
                },
            )
        }
        ExpressionNode::Float(literal) => literal.landing().and_then(|format| {
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                program,
                match format {
                    numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                    numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
                },
            )
        }),
        ExpressionNode::Binary(binary) => {
            let operands = [binary.left, binary.right]
                .map(|operand| result_type(program, owner, operand, active));
            binary_result(program, owner, expression, binary, operands)
        }
        ExpressionNode::Unary(unary) => {
            let operand = result_type(program, owner, unary.operand, active);
            match unary.operator {
                UnaryOperator::BitwiseNot => operand
                    .and_then(|reference| arithmetic_result_type_reference(program, reference))
                    .filter(|reference| integer(program, *reference)),
                UnaryOperator::LogicalNot => operand
                    .filter(|reference| primitive(program, *reference) == Some(PrimitiveType::Bool))
                    .and_then(|_| {
                        crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                            program,
                            BuiltinTypeAtom::Bool,
                        )
                    }),
            }
        }
        _ => match owner {
            ExpressionOwner::Declaration { place_type, .. } => place_type(expression),
            ExpressionOwner::State { machine, state } => {
                crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
            }
            ExpressionOwner::Domain(domain) => {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Name(_)
                        if typed_trees::domain::domain_subject_reference(
                            program, domain, expression,
                        ) =>
                    {
                        Some(domain.target_type)
                    }
                    _ => crate::proof_contracts::proof_embeddings::expression_type_reference(
                        program, expression,
                    ),
                }
            }
        },
    };
    active.pop();
    result.filter(|reference| {
        program
            .type_reference_table
            .contains_type_reference(*reference)
    })
}

/// Join declaration-backed result types without exporting one arm's predicates
/// to another. Missing references require independently checked anonymous shape.
pub fn join_result_type_references(
    program: &dyn crate::CheckingInput,
    references: &[Option<TypeReferenceHandle>],
    anonymous: &[bool],
) -> Option<TypeReferenceHandle> {
    if references.len() != anonymous.len() {
        return None;
    }
    references
        .iter()
        .copied()
        .flatten()
        .next()
        .and_then(|first| {
            // Exact shared identity retains common predicates. Otherwise
            // a numeric join weakens predicate facts, never policy. A range
            // on one arm cannot become a promise about an anonymous peer.
            // Do not compare rendered bounds: equal spellings need not
            // name the same dependent predicate subject.
            if references.iter().all(|reference| *reference == Some(first)) {
                return Some(first);
            }
            // Separately authored qualifications have separate type
            // handles. Their normalized declared-domain identities may
            // still agree (including transparent aliases and indices).
            // Raw range predicates remain on the exact-subject path
            // above: rendered dependent bounds are not interchangeable.
            if has_domain_result_shell(program, first) {
                let identity = program.normalized_type_identity(first);
                if references.iter().all(|reference| {
                    reference.is_some_and(|reference| {
                        has_domain_result_shell(program, reference)
                            && program.normalized_type_identity(reference) == identity
                    })
                }) {
                    return Some(first);
                }
            }
            let carrier = arithmetic_carrier(program, first)?;
            anonymous
                .iter()
                .zip(references)
                .all(|(anonymous, reference)| {
                    reference.map_or_else(
                        || *anonymous,
                        |reference| arithmetic_carrier(program, reference) == Some(carrier),
                    )
                })
                .then(|| arithmetic_result_type_reference(program, first))
                .flatten()
        })
}

fn has_domain_result_shell(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> bool {
    let mut seen = Vec::new();
    let mut domain = false;
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !seen.contains(&reference)
    {
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let Some(constraints) = program.type_reference_table.constraint_span(*constraints)
                else {
                    return false;
                };
                for constraint in constraints {
                    match constraint {
                        TypeConstraintNode::Domain(qualification)
                            if qualification.symbol.is_valid() =>
                        {
                            domain = true
                        }
                        TypeConstraintNode::ArithmeticDomain(_) => {}
                        _ => return false,
                    }
                }
                reference = *base_type;
            }
            TypeReferenceNode::Named { .. } => return domain,
            _ => return false,
        }
    }
    false
}

fn binary_result(
    program: &dyn crate::CheckingInput,
    owner: ExpressionOwner<'_>,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
    operands: [Option<TypeReferenceHandle>; 2],
) -> Option<TypeReferenceHandle> {
    use BinaryOperator::*;
    let spelling = match binary.operator {
        Add => Some(OperatorSpelling::Add),
        Subtract => Some(OperatorSpelling::Subtract),
        Multiply => Some(OperatorSpelling::Multiply),
        Divide => Some(OperatorSpelling::Divide),
        Modulo => Some(OperatorSpelling::Modulo),
        Equal => Some(OperatorSpelling::Equal),
        NotEqual => Some(OperatorSpelling::NotEqual),
        Less => Some(OperatorSpelling::Less),
        LessOrEqual => Some(OperatorSpelling::LessEqual),
        Greater => Some(OperatorSpelling::Greater),
        GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
        And | Or | BitwiseAnd | BitwiseOr | BitwiseXor | ShiftLeft | ShiftRight
        | CaseMembership => None,
    };
    // Unknown operands stay unknown during selection. Substituting the known
    // peer first could hide a heterogeneous or reference-typed declaration.
    if let Some(spelling) = spelling
        && !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner.symbol(),
            expression,
            spelling,
            &operands,
        )
    {
        let candidates = typed_trees::operator::resolve_spelling_for_operands(
            program,
            spelling,
            &operands,
            program.expression_table.source_span(expression),
        );
        let [selected] = candidates.as_slice() else {
            return None;
        };
        if !typed_trees::operator::selected_trait_operator_meanings(
            program,
            owner.symbol(),
            spelling,
            &operands,
        )
        .is_empty()
        {
            return None;
        }
        // A domain-owned meaning applies only where an operand statically
        // selects that domain's facet; declaring a domain operator does not
        // replace the builtin (chapter 8), which is the checked stage's
        // `BuiltinFallback`. Reading an inactive domain declaration's result
        // here would un-qualify an operand's arithmetic policy that the
        // builtin meaning keeps, so the builtin arms below answer instead.
        if selected.domain.is_none_or(|domain| {
            operands.iter().flatten().any(|reference| {
                typed_trees::domain::type_selects_semantic_domain(
                    program,
                    *reference,
                    domain.symbol,
                )
            })
        }) {
            // This is the selected declaration's result, never an operand
            // guess. A concrete result needs no substitution even if operands
            // are generic. Dependent predicates and composite result binders
            // need an instantiated reference; exporting their
            // declaration-local subjects would invent caller facts.
            let result = selected.operator.return_type;
            if let Some(bound) = bound_result_type_parameter(program, selected.operator, &operands)
            {
                return Some(bound);
            }
            qualified_builtin_carrier(program, result, false)?;
            return selected_operator_result_type_reference(
                program,
                selected.operator.symbol,
                result,
                &operands,
            );
        }
    }
    match binary.operator {
        CaseMembership => {
            let builtin = match owner {
                ExpressionOwner::State { machine, .. } =>
                    crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                        program, machine, None, expression, binary),
                ExpressionOwner::Declaration { .. } | ExpressionOwner::Domain(_) => false,
            };
            builtin
                .then(|| {
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        BuiltinTypeAtom::Bool,
                    )
                })
                .flatten()
        }
        And | Or => operands
            .into_iter()
            .all(|reference| {
                reference.and_then(|reference| primitive(program, reference))
                    == Some(PrimitiveType::Bool)
            })
            .then(|| {
                crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    BuiltinTypeAtom::Bool,
                )
            })
            .flatten(),
        Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
            let compatible = if let Some(carrier) = operands.into_iter().flatten().next() {
                let primitive = primitive(program, carrier)?;
                let supported = integer(program, carrier)
                    || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    || (matches!(binary.operator, Equal | NotEqual)
                        && primitive == PrimitiveType::Bool);
                supported && compatible_operands(program, binary, operands, carrier)
            } else {
                [binary.left, binary.right].into_iter().all(|operand| {
                    crate::value_custody::literals::has_anonymous_numeric_results(program, operand)
                })
            };
            compatible
                .then(|| {
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        BuiltinTypeAtom::Bool,
                    )
                })
                .flatten()
        }
        ShiftLeft | ShiftRight => {
            let left = operands[0]
                .and_then(|reference| arithmetic_result_type_reference(program, reference))
                .filter(|reference| integer(program, *reference))?;
            let count = operands[1].map_or_else(
                || {
                    crate::value_custody::literals::has_anonymous_numeric_results(
                        program,
                        binary.right,
                    )
                },
                |reference| integer(program, reference),
            );
            count.then_some(left)
        }
        Add | Subtract | Multiply | Divide => {
            // The difference of two addresses is a distance, and the address
            // model calls a distance a count: there is no referent for it to
            // name, and no width equality relates counts to addresses, so it
            // lands `u64` rather than its operands' carrier. `addr - count`
            // and `addr + count` still offset an address and keep it.
            // `reject_address_arithmetic` admits exactly these three shapes;
            // this types the one whose result is not the left operand's.
            if binary.operator == Subtract
                && operands.iter().all(|reference| {
                    reference.is_some_and(|reference| {
                        primitive(program, reference) == Some(PrimitiveType::Addr)
                    })
                })
            {
                return crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    BuiltinTypeAtom::U64,
                );
            }
            let carrier =
                arithmetic_result_type_reference(program, operands.into_iter().flatten().next()?)?;
            let numeric = integer(program, carrier)
                || matches!(
                    primitive(program, carrier),
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                );
            (numeric && compatible_operands(program, binary, operands, carrier)).then_some(carrier)
        }
        Modulo | BitwiseAnd | BitwiseOr | BitwiseXor => {
            let carrier =
                arithmetic_result_type_reference(program, operands.into_iter().flatten().next()?)?;
            (integer(program, carrier) && compatible_operands(program, binary, operands, carrier))
                .then_some(carrier)
        }
    }
}

/// A `-> T` result instantiates through the same binding operand selection
/// already performed: parameters in normalized order (self first) pair with
/// operand references, and the first position declared as a bare `T` fixes
/// that parameter's bound reference. A result parameter reached only through
/// composite declarations (`Pair<T>`) or bound by no operand stays
/// unresolved: rebuilding instantiated shells or inventing an unbound
/// identity would fabricate caller facts.
fn bound_result_type_parameter(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
    operands: &[Option<TypeReferenceHandle>],
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Named {
        symbol: result_symbol,
        name: result_name,
    } = program
        .type_reference_table
        .type_reference(operator.return_type)
    else {
        return None;
    };
    let result_parameter = program
        .operator_type_parameters(operator)
        .iter()
        .find(|parameter| {
            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                && ((result_symbol.is_valid() && parameter.symbol == *result_symbol)
                    || parameter.name.as_str() == result_name.as_str())
        })?;
    let parameters = program.operator_parameters(operator);
    parameters
        .iter()
        .zip(operands.iter())
        .find_map(|(parameter, operand)| {
            let operand = (*operand)?;
            let TypeReferenceNode::Named { symbol, name } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return None;
            };
            ((symbol.is_valid() && *symbol == result_parameter.symbol)
                || name.as_str() == result_parameter.name.as_str())
            .then_some(operand)
        })
}

/// A declared result that mentions the operator's type parameters — bare,
/// nested inside a composite binder, or inside a constraint's static
/// arguments — is declaration-local until `bound_result_type_parameter`
/// instantiates a bare `T` from its operand. Exporting such a shell through
/// a named call would fabricate caller identity, so it stays unresolved.
fn result_mentions_type_parameters(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
    reference: TypeReferenceHandle,
) -> bool {
    let parameters = program.operator_type_parameters(operator);
    let mut pending = vec![reference];
    let mut seen = Vec::new();
    while let Some(reference) = pending.pop() {
        if seen.contains(&reference) {
            continue;
        }
        seen.push(reference);
        if let TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } = program.type_reference_table.type_reference(reference)
            && parameters
                .iter()
                .any(|parameter| parameter.symbol == *symbol)
        {
            return true;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                if let Some(constraints) =
                    program.type_reference_table.constraint_span(*constraints)
                {
                    for constraint in constraints {
                        if let TypeConstraintNode::Domain(qualification) = constraint {
                            pending.extend(qualification.arguments.iter().copied());
                        }
                    }
                }
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend_from_slice(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                );
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                pending.push(*element_type);
                if let typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } = length
                    && parameters
                        .iter()
                        .any(|parameter| parameter.symbol == *symbol)
                {
                    return true;
                }
            }
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            _ => {}
        }
    }
    false
}

fn compatible_operands(
    program: &dyn crate::CheckingInput,
    binary: &TableBinaryExpression,
    operands: [Option<TypeReferenceHandle>; 2],
    carrier: TypeReferenceHandle,
) -> bool {
    [binary.left, binary.right]
        .into_iter()
        .zip(operands)
        .all(|(expression, reference)| {
            reference.map_or_else(
                || {
                    crate::value_custody::literals::has_anonymous_numeric_results(
                        program, expression,
                    )
                },
                |reference| {
                    arithmetic_carrier(program, reference) == arithmetic_carrier(program, carrier)
                },
            )
        })
}

fn primitive(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    arithmetic_carrier(program, reference)?;
    program.primitive_type_reference(reference)
}

fn integer(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> bool {
    primitive(program, reference).is_some_and(|primitive| primitive.is_integer())
        || arithmetic_carrier(program, reference).is_some_and(|(symbol, _)| {
            // Machine-width `UInt`/`Int` carry no fixed PrimitiveType atom but are
            // integer carriers: `calls = calls + 1` on a `UInt` place is an
            // established arithmetic shape, not a type the evaluator may drop.
            matches!(
                program.symbols.builtin_type_atom(symbol),
                Some(BuiltinTypeAtom::UInt | BuiltinTypeAtom::Int)
            )
        })
}

// Raw operand references reach overload selection above. Only after builtin
// selection may range predicates be forgotten: an operation on [0..=10] can
// produce 11. Arithmetic policy is different: it governs this result's next
// operation and must survive. Unknown domain and reference shells are not
// predicate-only qualifications and cannot use this projection.
pub(super) fn arithmetic_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
    qualified_builtin_carrier(program, reference, true)
}

fn qualified_builtin_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    allow_range_predicates: bool,
) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
    let mut policy = None;
    let mut visited = Vec::new();
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !visited.contains(&reference)
    {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                program.symbols.builtin_type_atom(*symbol)?;
                return Some((*symbol, policy.unwrap_or(ArithmeticDomain::Exact)));
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraint_span(*constraints)? {
                    match constraint {
                        TypeConstraintNode::Range { .. } if allow_range_predicates => {}
                        TypeConstraintNode::ArithmeticDomain(domain) => {
                            if policy.is_some() {
                                return None;
                            }
                            policy = Some(*domain);
                        }
                        TypeConstraintNode::Range { .. }
                        | TypeConstraintNode::Named(_)
                        | TypeConstraintNode::Domain(_) => {
                            return None;
                        }
                    }
                }
                reference = *base_type;
            }
            _ => return None,
        }
    }
    None
}

pub fn arithmetic_result_type_reference(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let (carrier, policy) = arithmetic_carrier(program, reference)?;
    program
        .type_reference_table
        .find_arithmetic_result_type_reference(carrier, policy)
}

/// Cast results retain the normalized policy and semantic-domain qualification;
/// the authored membership target alone does not describe the resulting value.
pub(crate) fn cast_result_type_reference(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
) -> Option<TypeReferenceHandle> {
    // Cast policy and semantic-domain suffixes live outside target_type.
    // Returning that bare target would erase the result qualification
    // before a surrounding operator selects its meaning.
    if program
        .type_reference_table
        .contains_type_reference(cast.result_type)
    {
        Some(cast.result_type)
    } else if !cast.semantic_domain.is_empty() {
        None
    } else if cast.domain == ArithmeticDomain::Exact {
        Some(cast.target_type)
    } else if let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(cast.target_type)
    {
        program.symbols.builtin_type_atom(*symbol).and_then(|_| {
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(*symbol, cast.domain)
        })
    } else {
        program
            .type_reference_table
            .find_policy_qualified_type_reference(cast.target_type, cast.domain)
    }
}

/// Exact operands contribute no policy. Distinct explicit policies have no
/// common selected adapter; validation diagnoses the incompatible operands.
pub(crate) fn operand_arithmetic_domain(
    program: &dyn crate::CheckingInput,
    operands: &[Option<TypeReferenceHandle>],
) -> Option<ArithmeticDomain> {
    let mut selected = ArithmeticDomain::Exact;
    for reference in operands.iter().flatten() {
        let domain = program.type_reference_table.arithmetic_domain(*reference);
        if domain == ArithmeticDomain::Exact {
            continue;
        }
        if selected != ArithmeticDomain::Exact && selected != domain {
            return None;
        }
        selected = domain;
    }
    Some(selected)
}

/// A selected sealed float operation owns its raw result and adapter. Ordinary
/// declarations retain their own result, including Boolean queries and records.
/// Operand refinements are never copied into a computed result.
pub(crate) fn selected_operator_result_type_reference(
    program: &dyn crate::CheckingInput,
    operator: symbols::SymbolHandle,
    declared_result: TypeReferenceHandle,
    operands: &[Option<TypeReferenceHandle>],
) -> Option<TypeReferenceHandle> {
    if !crate::machine_calls::calls::selected_float_operation_retains_argument_policy(
        program, operator,
    ) {
        return Some(declared_result);
    }
    let domain = operand_arithmetic_domain(program, operands)?;
    let (carrier, _) = arithmetic_carrier(program, declared_result)?;
    program
        .type_reference_table
        .find_arithmetic_result_type_reference(carrier, domain)
}
