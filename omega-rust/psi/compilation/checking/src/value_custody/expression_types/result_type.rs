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
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_computation_results {
    use super::{ExpressionNode, PrimitiveType, expression_result_type_reference};

    #[test]
    fn numeric_scalar_calls_reject_boolean_guards() {
        for subject in [
            "min(value, 0)",
            "max(value, 0)",
            "clamp(value, 0, 10)",
            "min(max(value, 0), 10)",
            "max(min(value, 10), 0)",
            "min(1, 0)",
        ] {
            let source = format!(
                "machine evaluate(value: i32) -> i32 {{
                    transition {subject} {{ true -> (7) _ -> (90) }}
                }}"
            );
            let program = crate::front_end::typed_program(&source);
            let diagnostics = crate::validate_program(&program)
                .expect_err("a numeric result cannot select a Boolean guard");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("numeric value and a boolean")),
                "{source}: {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn scalar_call_results_retain_numeric_carriers_without_input_bounds() {
        for (primitive, subject) in [
            (PrimitiveType::I32, "min(max(value, 0), 10)"),
            (PrimitiveType::I32, "clamp(value, 0, 10)"),
            (PrimitiveType::I32, "min(0, value)"),
            (PrimitiveType::I32, "max(0, value)"),
            (PrimitiveType::U64, "max(value, 20u64)"),
            (PrimitiveType::F64, "sqrt(value)"),
            (PrimitiveType::F64, "clamp(value, 0.0, 10.0)"),
            (PrimitiveType::F64, "abs(value)"),
        ] {
            let parameter = if primitive == PrimitiveType::U64 {
                "value: u64 [0..=10]"
            } else if primitive == PrimitiveType::F64 {
                "value: f64"
            } else {
                "value: i32"
            };
            let result = if primitive == PrimitiveType::U64 {
                "u64"
            } else if primitive == PrimitiveType::F64 {
                "f64"
            } else {
                "i32"
            };
            let source = format!("machine evaluate({parameter}) -> {result} {{ ({subject}) }}");
            let program = crate::front_end::typed_program(&source);
            crate::validate_program(&program)
                .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"));
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[0];
            let calls: Vec<_> = program
                .expression_table
                .iter_expressions()
                .filter_map(|(handle, expression)| {
                    matches!(expression, ExpressionNode::Call(_)).then_some(handle)
                })
                .collect();
            assert!(!calls.is_empty());
            for value in calls {
                let reference = expression_result_type_reference(&program, machine, state, value)
                    .expect("the selected numeric call has an operand-derived carrier");
                assert_eq!(program.primitive_type_reference(reference), Some(primitive));
                assert_eq!(
                    program.normalized_type_identity(reference),
                    program.normalized_type_identity(state.return_type)
                );
            }
        }
    }

    #[test]
    fn static_arithmetic_named_callbacks_keep_their_boolean_contracts() {
        for name in ["Work", "abs", "clamp", "min", "max", "sqrt"] {
            let (parameters, arguments) = if name == "clamp" {
                ("input: i32, lower: i32, upper: i32", "value, 0, 10")
            } else {
                ("input: i32", "value")
            };
            let source = format!(
                "machine evaluate<machine {name}>(value: i32) -> i32
                 where machine {name}({parameters}) -> bool;
                 {{ transition {name}({arguments}) {{ true -> (7) _ -> (90) }} }}"
            );
            let program = crate::front_end::typed_program(&source);
            crate::validate_program(&program)
                .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"));
        }
    }
}

#[cfg(test)]
mod domain_carrier_subjects {}

#[cfg(test)]
mod semantic_nan_types {
    use super::{ExpressionNode, PrimitiveType, expression_result_type_reference};
    use numerics::literals::FloatFormat;

    #[test]
    fn semantic_nan_retains_its_own_format_in_results_and_arguments() {
        let mut program =
            crate::front_end::typed_program("machine run(left: f32, right: f64) -> f32 { left }");
        let binary32 = program
            .expression_table
            .insert(ExpressionNode::UndeterminedNaN(FloatFormat::F32));
        let binary64 = program
            .expression_table
            .insert(ExpressionNode::UndeterminedNaN(FloatFormat::F64));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let reference32 = expression_result_type_reference(&program, machine, state, binary32)
            .expect("f32 semantic carrier");
        let reference64 = expression_result_type_reference(&program, machine, state, binary64)
            .expect("f64 semantic carrier");
        assert_eq!(
            program.primitive_type_reference(reference32),
            Some(PrimitiveType::F32)
        );
        assert_eq!(
            program.primitive_type_reference(reference64),
            Some(PrimitiveType::F64)
        );
        for (value, own, other) in [
            (binary32, reference32, reference64),
            (binary64, reference64, reference32),
        ] {
            assert!(super::super::argument_matches_type_reference_handle(
                &program, value, own
            ));
            assert!(!super::super::argument_matches_type_reference_handle(
                &program, value, other
            ));
        }
    }
}

#[cfg(test)]
mod named_call_result_fabrication {
    use super::{ExpressionHandle, TypedTrees, expression_result_type_reference};

    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;

    fn match_arm_value(program: &TypedTrees, arm: usize) -> ExpressionHandle {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(result)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("outer result expression");
        };
        let mut root = *result;
        if let ExpressionNode::Cast(cast) = program.expression_table.expression(root) {
            root = cast.value;
        }
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(root) else {
            panic!("match result");
        };
        program.expression_table.match_arms(dispatch.arms)[arm].value
    }

    fn first_match_arm_value(program: &TypedTrees) -> ExpressionHandle {
        match_arm_value(program, 0)
    }

    #[test]
    fn named_call_composite_parameter_results_remain_unresolved() {
        let program = crate::front_end::typed_program(
            "data Pair<T> { first: T; second: T; }
             operator + Math::pair<T>(left: T, right: T) -> Pair<T>;
             machine run(left: u8, right: u8) -> Pair<u8> {
                 match true { true -> Math::pair(left, right), false -> Math::pair(right, left) }
             }",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let call = first_match_arm_value(&program);
        assert!(matches!(
            program.expression_table.expression(call),
            ExpressionNode::Call(_)
        ));
        assert_eq!(
            expression_result_type_reference(&program, machine, state, call),
            None,
            "a declaration-local `Pair<T>` shell must not stand in for `Pair<u8>`"
        );
    }

    #[test]
    fn named_call_constrained_parameter_results_remain_unresolved() {
        let program = crate::front_end::typed_program(
            "domain<T> T::NonZero;
             operator + Math::clamp<T>(input: T, bound: T) -> T in NonZero;
             machine run(left: u8, right: u8) -> u8 {
                 match true { true -> Math::clamp(left, right), false -> right }
             }",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let call = first_match_arm_value(&program);
        assert!(matches!(
            program.expression_table.expression(call),
            ExpressionNode::Call(_)
        ));
        assert_eq!(
            expression_result_type_reference(&program, machine, state, call),
            None,
            "a declaration-local `T in NonZero` shell must not stand in for `u8 in NonZero`"
        );
    }
}

/// Return an existing result reference, never an expected-type guess. An
/// unresolved result does not establish anonymous numeric meaning. Builtin
/// computed results retain their carrier and policy, not input predicates.
/// Direct operators retain closed builtin carrier/policy result references.
/// Casts retain their normalized semantic-domain result separately from the
/// authored membership target. Operator results needing instantiated semantic
/// domains, predicates or selected traits remain unresolved rather than losing
/// their meaning.
pub fn expression_result_type_reference(
    program: &TypedTrees,
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
    program: &TypedTrees,
    owner_symbol: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !owner_symbol.is_valid() {
        return None;
    }
    result_type(
        program,
        ExpressionOwner::Parameters {
            owner_symbol,
            parameters,
        },
        expression,
        &mut Vec::new(),
    )
}

// Domain predicates use the same selected-result rules as state expressions.
// Only declaration-backed leaves differ: self belongs to the domain carrier,
// while static indices retain their own declared types.
pub(crate) fn domain_expression_result_type_reference(
    program: &TypedTrees,
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
    Parameters {
        owner_symbol: symbols::SymbolHandle,
        parameters: &'program [typed_trees::signature::StateParameter],
    },
}

impl ExpressionOwner<'_> {
    fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::State { machine, .. } => machine.symbol,
            Self::Domain(domain) => domain.symbol,
            Self::Parameters { owner_symbol, .. } => owner_symbol,
        }
    }
}

fn result_type(
    program: &TypedTrees,
    owner: ExpressionOwner<'_>,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return None;
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
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
            crate::machine_calls::calls::resolved_call_result_type(program, call)
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
        }
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol),
        ExpressionNode::Cast(cast) => {
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
        ExpressionNode::Indexed(indexed)
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            (|| {
                let ExpressionNode::Range(range) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
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
                let mut carrier = collection;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                {
                    carrier = *base_type;
                }
                let (access, carrier) = match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } => (*access, *referee),
                    _ => (language_core::ReferenceAccess::Shared, carrier),
                };
                if access == language_core::ReferenceAccess::WriteOnly {
                    return None;
                }
                let mut carrier = carrier;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                {
                    carrier = *base_type;
                }
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
        ExpressionNode::ZeroValue(reference) => Some(*reference),
        // Place lookup strips a Borrow to its target. A value-type query must
        // not turn that view into a by-value operand and hide reference-typed
        // operator candidates, including through a nested Match result.
        ExpressionNode::Borrow(_) => None,
        ExpressionNode::Integer(_) => {
            crate::declarations::operators::landed_integer_literal_type_reference(
                program, expression,
            )
        }
        ExpressionNode::Boolean(_) => builtin_reference(program, BuiltinTypeAtom::Bool),
        ExpressionNode::UndeterminedNaN(format) => builtin_reference(
            program,
            match format {
                numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
            },
        ),
        ExpressionNode::Float(literal) => literal.landing().and_then(|format| {
            builtin_reference(
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
                    .and_then(|_| builtin_reference(program, BuiltinTypeAtom::Bool)),
            }
        }
        _ => match owner {
            ExpressionOwner::Parameters {
                owner_symbol,
                parameters,
            } => crate::reserved_result_place(program, expression)
                .filter(|place| place.machine_symbol == owner_symbol)
                .map(|place| place.type_reference)
                .or_else(|| {
                    crate::value_custody::places::parameter_scoped_type_reference(
                        program, parameters, expression,
                    )
                }),
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
                    ExpressionNode::Name(path)
                        if !path.symbol.is_valid()
                            && !path.head_symbol.is_valid()
                            && matches!(program.expression_table.name_path_members(path.members),
                            [name] if name.is_self_receiver()) =>
                    {
                        Some(domain.target_type)
                    }
                    // Membership subjects project the declared carrier:
                    // `self.num in NonZero` reads `num`'s declared field type,
                    // `self[i] in Utf8` the declared element type. The shared
                    // embedding resolver cannot see the domain telescope, so
                    // the receiver recurses through `result_type` (which owns
                    // bare `self`) and the projection lands on its result.
                    ExpressionNode::Member(member) => {
                        result_type(program, owner, member.receiver, active).and_then(|receiver| {
                            if let Some(data) =
                                crate::value_custody::places::data_definition_for_type(
                                    program, receiver,
                                )
                            {
                                program.data_members(data).iter().find_map(|data_member| {
                                    match data_member {
                                        typed_trees::data::DataMember::Field(field)
                                            if field.name == member.member =>
                                        {
                                            Some(field.type_reference)
                                        }
                                        _ => None,
                                    }
                                })
                            } else {
                                program
                                    .data_definitions()
                                    .iter()
                                    .flat_map(|data| program.data_members(data))
                                    .find_map(|data_member| match data_member {
                                        typed_trees::data::DataMember::Field(field)
                                            if member.member_symbol.is_valid()
                                                && field.symbol == member.member_symbol =>
                                        {
                                            Some(field.type_reference)
                                        }
                                        _ => None,
                                    })
                            }
                        })
                    }
                    ExpressionNode::Indexed(indexed) => {
                        result_type(program, owner, indexed.collection, active).and_then(
                            |collection| match program.type_reference_table.type_reference(
                                crate::value_custody::places::unwrapped_type_reference(
                                    program, collection,
                                )?,
                            ) {
                                TypeReferenceNode::FixedArray { element_type, .. }
                                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                                _ => None,
                            },
                        )
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
    program: &TypedTrees,
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

fn has_domain_result_shell(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
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
    program: &TypedTrees,
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
            return Some(result);
        }
    }
    match binary.operator {
        CaseMembership => {
            let builtin = match owner {
                ExpressionOwner::State { machine, .. } =>
                    crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                        program, machine, None, expression, binary),
                ExpressionOwner::Parameters { .. } | ExpressionOwner::Domain(_) => false,
            };
            builtin
                .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
                .flatten()
        }
        And | Or => operands
            .into_iter()
            .all(|reference| {
                reference.and_then(|reference| primitive(program, reference))
                    == Some(PrimitiveType::Bool)
            })
            .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
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
                .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
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
                return builtin_reference(program, BuiltinTypeAtom::U64);
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
    program: &TypedTrees,
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
        .filter(|parameter| parameter.is_self)
        .chain(parameters.iter().filter(|parameter| !parameter.is_self))
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
    program: &TypedTrees,
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
    program: &TypedTrees,
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

fn primitive(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    arithmetic_carrier(program, reference)?;
    program.primitive_type_reference(reference)
}

fn integer(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    matches!(
        primitive(program, reference),
        Some(
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    ) || arithmetic_carrier(program, reference).is_some_and(|(symbol, _)| {
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
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
    qualified_builtin_carrier(program, reference, true)
}

fn qualified_builtin_carrier(
    program: &TypedTrees,
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
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let (carrier, policy) = arithmetic_carrier(program, reference)?;
    program
        .type_reference_table
        .find_arithmetic_result_type_reference(carrier, policy)
}

fn builtin_reference(program: &TypedTrees, atom: BuiltinTypeAtom) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}
