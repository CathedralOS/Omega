use psi_arena::Arena;
use psi_checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedNamedOperatorUseFact, CheckedOperatorCandidateFact,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedValueFacts, CheckedValueOrigin,
};
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::float_semantics::FloatFormat;
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
};
use psi_typed_trees::operator::{
    SelectedTraitOperatorMeaning, SpelledOperator, resolve_spelling_for_operands,
    selected_trait_operator_meanings,
};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle};

mod receiver;
mod selection;

pub(crate) use receiver::expression_type_reference_for_origin;
pub(crate) use selection::select_pending_domain_operator_meanings;

pub(crate) fn build_operator_facts(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut named_uses = Arena::default();
    let mut candidates = Arena::default();
    let mut seen = Vec::new();

    for (_, value) in values.values.iter() {
        collect_expression_operator_use(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
            &mut named_uses,
            &mut candidates,
        );
    }

    CheckedOperatorFacts::with_roots(uses, named_uses, candidates)
        .with_operator_crash_contracts(derive_checked_operator_crash_contracts(program))
        .with_operator_realization_contracts(derive_checked_operator_realization_contracts(program))
}

pub(crate) fn derive_checked_operator_realization_contracts(
    program: &TypedTrees,
) -> Vec<psi_checked_trees::CheckedOperatorRealizationContract> {
    let mut rows = Vec::new();
    for machine in program.machines() {
        let Some(entry) = program.machine_states(machine).first() else {
            continue;
        };
        let provider_parameter_names = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        for conformance in program.machine_trait_conformances(machine) {
            if program
                .traits()
                .iter()
                .any(|definition| definition.symbol == conformance.symbol)
            {
                continue;
            }
            let Some(operator) = psi_typed_trees::operator::declaration_by_symbol(
                program,
                conformance.requirement_symbol,
            ) else {
                continue;
            };
            let requirement_parameter_names = program
                .operator_parameters(operator)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            rows.push(psi_checked_trees::CheckedOperatorRealizationContract::new(
                machine.symbol,
                operator.symbol,
                crate::facts::encode_contract_set_canonical(
                    program,
                    program.machine_contracts(machine),
                    &provider_parameter_names,
                    &[],
                    &[],
                    false,
                    true,
                ),
                crate::facts::encode_contract_set_canonical(
                    program,
                    program.operator_contracts(operator),
                    &requirement_parameter_names,
                    &[],
                    &[],
                    false,
                    true,
                ),
                psi_validation::checked_operator_contract_snapshot(
                    program,
                    program.machine_contracts(machine),
                ),
                psi_validation::checked_operator_contract_snapshot(
                    program,
                    program.operator_contracts(operator),
                ),
                operator_realization_admission_snapshot(program, machine, conformance, operator),
            ));
        }
    }
    rows.sort_by_key(|row| {
        (
            row.machine_symbol().arena_index(),
            row.machine_symbol().generation(),
            row.operator_symbol().arena_index(),
            row.operator_symbol().generation(),
        )
    });
    rows
}

fn operator_realization_admission_snapshot(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Vec<u8> {
    use std::fmt::Debug;

    fn append_debug(value: &impl Debug, output: &mut Vec<u8>) {
        let bytes = format!("{value:?}").into_bytes();
        output.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("checked operator admission snapshot length fits u64")
                .to_le_bytes(),
        );
        output.extend(bytes);
    }

    fn append_type(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        visited: &mut std::collections::HashSet<(u32, u32)>,
        output: &mut Vec<u8>,
    ) {
        if !type_reference.is_valid() {
            append_debug(&"<unit>", output);
            return;
        }
        append_debug(&type_reference, output);
        if !visited.insert((type_reference.arena_index(), type_reference.generation())) {
            return;
        }
        let node = program.type_reference_table.type_reference(type_reference);
        append_debug(node, output);
        append_debug(
            &program
                .package_qualified_type_identity(type_reference)
                .into_string(),
            output,
        );
        match node {
            psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                append_type(program, *referee, visited, output);
            }
            psi_typed_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                append_type(program, *base_type, visited, output);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    append_debug(constraint, output);
                    if let psi_typed_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                        for argument in &domain.arguments {
                            append_type(program, *argument, visited, output);
                        }
                    }
                }
            }
            psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
            | psi_typed_trees::types::TypeReferenceNode::Slice { element_type } => {
                append_type(program, *element_type, visited, output);
            }
            psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } => {
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    append_type(program, *argument, visited, output);
                }
            }
            psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            | psi_typed_trees::types::TypeReferenceNode::Named { .. }
            | psi_typed_trees::types::TypeReferenceNode::Unit => {}
        }
    }

    let mut output = Vec::new();
    let mut visited_types = std::collections::HashSet::new();
    append_debug(
        &(
            machine.symbol,
            &machine.name,
            machine.is_public,
            machine.supply_mode,
            machine.body_is_present,
            &machine.lifetime_parameters,
            program.machine_type_parameters(machine),
        ),
        &mut output,
    );
    append_debug(conformance, &mut output);
    for argument in program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    {
        append_type(program, *argument, &mut visited_types, &mut output);
    }
    for state in program.machine_states(machine) {
        append_debug(state, &mut output);
        for parameter in program.state_parameters(state) {
            append_debug(parameter, &mut output);
            append_type(
                program,
                parameter.type_reference,
                &mut visited_types,
                &mut output,
            );
        }
        append_type(program, state.return_type, &mut visited_types, &mut output);
    }
    append_debug(
        &(
            operator.is_public,
            operator.is_boundary,
            operator.symbol,
            program.operator_path_members(operator.name),
            &operator.lifetime_parameters,
            program.operator_type_parameters(operator),
            operator.spelling,
            operator.token_count,
        ),
        &mut output,
    );
    for parameter in program.operator_parameters(operator) {
        append_debug(parameter, &mut output);
        append_type(
            program,
            parameter.type_reference,
            &mut visited_types,
            &mut output,
        );
    }
    append_type(
        program,
        operator.return_type,
        &mut visited_types,
        &mut output,
    );
    output
}

pub(crate) fn derive_checked_operator_crash_contracts(
    program: &TypedTrees,
) -> Vec<psi_checked_trees::CheckedOperatorCrashContract> {
    use psi_typed_trees::{domain::ProofFact, signature::SignatureContractKind};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Bucket {
        unconditional: bool,
        facts: Vec<psi_arena::Handle<ProofFact>>,
    }

    let operators = program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    );
    let mut rows = operators
        .map(|operator| {
            let mut buckets = BTreeMap::<psi_checked_trees::CrashCause, Bucket>::new();
            for contract in program.operator_contracts(operator) {
                let SignatureContractKind::Crashes { cause } = contract.kind else {
                    continue;
                };
                let cause = match cause {
                    psi_typed_trees::signature::CrashCause::Trap => {
                        psi_checked_trees::CrashCause::Trap
                    }
                    psi_typed_trees::signature::CrashCause::Abort => {
                        psi_checked_trees::CrashCause::Abort
                    }
                };
                let bucket = buckets.entry(cause).or_default();
                if contract.facts.is_empty() {
                    bucket.unconditional = true;
                    continue;
                }
                for offset in 0..contract.facts.count() {
                    let fact = psi_arena::Handle::from_parts(
                        contract
                            .facts
                            .start()
                            .arena_index()
                            .checked_add(offset)
                            .expect("operator crash fact handle index overflow"),
                        contract.facts.start().generation(),
                    );
                    let is_true = matches!(
                        program.proof_facts.get(fact),
                        ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Boolean(true)
                            )
                    );
                    if is_true {
                        bucket.unconditional = true;
                    } else {
                        bucket.facts.push(fact);
                    }
                }
            }
            let buckets = buckets
                .into_iter()
                .map(|(cause, bucket)| {
                    psi_checked_trees::CheckedOperatorCrashBucket::new(
                        cause,
                        bucket.unconditional,
                        if bucket.unconditional {
                            Vec::new()
                        } else {
                            bucket.facts
                        },
                    )
                })
                .collect();
            psi_checked_trees::CheckedOperatorCrashContract::new(operator.symbol, buckets)
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| {
        (
            row.operator_symbol().arena_index(),
            row.operator_symbol().generation(),
        )
    });
    rows
}

fn collect_expression_operator_use(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut Vec<(ExpressionHandle, CheckedValueOrigin)>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    named_uses: &mut Arena<CheckedNamedOperatorUseFact>,
    candidates: &mut Arena<CheckedOperatorCandidateFact>,
) {
    if !expression.is_valid() || seen.iter().any(|seen| *seen == (expression, origin)) {
        return;
    }
    seen.push((expression, origin));

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_expression_operator_use(
            program,
            atomic.value,
            origin,
            seen,
            uses,
            named_uses,
            candidates,
        ),
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            let operand_types = indexed_operand_types(program, indexed, origin);
            uses.append(operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                &operand_types,
                candidates,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                indexed.index,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(
                    program, *value, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // Resolve from the complete recoverable operand tuple. A
            // contextual literal or nested binary may not expose an
            // independent left-hand type, while the companion operand still
            // fixes the exact overload and arithmetic policy.
            if let Some(spelling) = binary_operator_spelling(binary.operator)
                && let operand_types = [
                    expression_type_reference_for_origin(program, binary.left, origin),
                    expression_type_reference_for_origin(program, binary.right, origin),
                ]
                && operand_types.iter().any(Option::is_some)
                && let Some(fact) = binary_operator_use_fact(
                    program,
                    expression,
                    origin,
                    spelling,
                    &operand_types,
                    candidates,
                )
            {
                uses.append(fact);
            }
            collect_expression_operator_use(
                program,
                binary.left,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                binary.right,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(
                program, cast.value, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::Call(call) => {
            if let Some(named_use) = named_operator_use_fact(program, expression, origin, call)
                .or_else(|| builtin_float_operator_use_fact(program, expression, origin, call))
            {
                named_uses.append(named_use);
            }
            collect_expression_operator_use(
                program,
                call.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(
                    program, *argument, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_operator_use(
                program,
                inner.target,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_operator_use(
                program,
                unary.operand,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(
                program,
                range.start,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program, range.end, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_operator_use(
                    program,
                    field.value,
                    origin,
                    seen,
                    uses,
                    named_uses,
                    candidates,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// `min`, `max`, and `sqrt` are the source shorthand for the corresponding
/// primitive-format boundary requirements. Retain that semantic identity in
/// Psi exactly as an explicitly named call would. Omega can then attach and
/// execute the selected ProviderPlan without reconstructing one during
/// lowering. This also covers compiler-synthesized calls from `abs` and
/// `clamp`, whose acknowledgement records their generated origin.
fn builtin_float_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedOperatorUseFact> {
    let (_, requirement, arity) = [
        (BuiltinFunction::Min, "minimum", 2),
        (BuiltinFunction::Max, "maximum", 2),
        (BuiltinFunction::Sqrt, "square_root", 1),
    ]
    .into_iter()
    .find(|(function, _, _)| {
        program.symbols.builtin_function_symbol(*function) == Some(call.target_symbol)
    })?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != arity {
        return None;
    }

    let mut format = None;
    for argument in arguments {
        let Some(type_reference) = expression_type_reference_for_origin(program, *argument, origin)
        else {
            continue;
        };
        let candidate = match program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::F32) => FloatFormat::BINARY32,
            Some(PrimitiveType::F64) => FloatFormat::BINARY64,
            _ => continue,
        };
        if format.is_some_and(|format| format != candidate) {
            return None;
        }
        format = Some(candidate);
    }
    let format = format?;
    let namespace = if format == FloatFormat::BINARY32 {
        "F32"
    } else if format == FloatFormat::BINARY64 {
        "F64"
    } else {
        return None;
    };
    let operator = program.operators().iter().find(|operator| {
        let path = program.operator_path_members(operator.name);
        matches!(path, [candidate_namespace, candidate_requirement]
            if candidate_namespace.as_str() == namespace
                && candidate_requirement.as_str() == requirement)
    })?;

    Some(CheckedNamedOperatorUseFact {
        expression,
        origin,
        selected_operator_symbol: operator.symbol,
        policy_adapter: named_float_policy_adapter(program, call, origin, format),
        provider_plan_identity: 0,
    })
}

/// Retain the selected identity of one unambiguously resolved named operator
/// call. Every boundary-operator provider path consumes this common fact;
/// numeric policy adaptation remains specific to the normalized float surface.
/// Policy adaptation is operand-driven for float-returning F32/F64 operations;
/// classification and destination-owned float-to-integer conversions carry no
/// float result adapter.
fn named_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedOperatorUseFact> {
    let operator = psi_typed_trees::operator::resolve_named_expression_call(program, call)?;
    let path = program.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let format = match namespace.as_str() {
        "F32" => Some(FloatFormat::BINARY32),
        "F64" => Some(FloatFormat::BINARY64),
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
            if matches!(requirement.as_str(), "from_f32" | "from_f64") =>
        {
            None
        }
        _ => None,
    };
    let policy_adapter = match format {
        Some(format)
            if matches!(
                (
                    format,
                    program.primitive_type_reference(operator.return_type)
                ),
                (FloatFormat::BINARY32, Some(PrimitiveType::F32))
                    | (FloatFormat::BINARY64, Some(PrimitiveType::F64))
            ) =>
        {
            named_float_policy_adapter(program, call, origin, format)
        }
        _ => CheckedArithmeticPolicyAdapter::None,
    };

    Some(CheckedNamedOperatorUseFact {
        expression,
        origin,
        selected_operator_symbol: operator.symbol,
        policy_adapter,
        provider_plan_identity: 0,
    })
}

fn named_float_policy_adapter(
    program: &TypedTrees,
    call: &TableCallExpression,
    origin: CheckedValueOrigin,
    format: FloatFormat,
) -> CheckedArithmeticPolicyAdapter {
    let mut selected_domain = ArithmeticDomain::Exact;
    for argument in program.expression_table.expression_handles(call.arguments) {
        let Some(type_reference) = expression_type_reference_for_origin(program, *argument, origin)
        else {
            continue;
        };
        let domain = program
            .type_reference_table
            .arithmetic_domain(type_reference);
        if domain == ArithmeticDomain::Exact {
            continue;
        }
        if selected_domain != ArithmeticDomain::Exact && selected_domain != domain {
            // Validation rejects mixed explicit arithmetic policies. Checked
            // evidence fails closed if lowering is invoked without that gate.
            return CheckedArithmeticPolicyAdapter::None;
        }
        selected_domain = domain;
    }
    float_policy_adapter(format, selected_domain)
}

/// The fixed operator spelling for a binary operator, when one exists.
/// Logical/shift operators have no spelling surface (frozen Wave 0 decision
/// #3) and never participate in spelled dispatch.
fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    })
}

/// Records a spelled binary use when spelled candidates match the left
/// operand type. Root-only candidate sets resolve immediately (root spelled
/// operators are the declared surface of the builtin operation). Any
/// domain-owned candidate defers to the binding-site selection pass
/// (`select_pending_domain_operator_meanings`), which reads declarations,
/// mints, and signature `requires` but never flow facts.
fn binary_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> Option<CheckedOperatorUseFact> {
    let trait_candidates = origin_machine_symbol(origin).map_or_else(Vec::new, |machine_symbol| {
        selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
    });
    if !trait_candidates.is_empty() {
        return Some(trait_operator_use_fact(
            program,
            expression,
            origin,
            spelling,
            trait_candidates,
            candidate_facts,
        ));
    }
    let candidates = resolve_spelling_for_operands(program, spelling, operand_types);
    if candidates.is_empty() {
        return None;
    }

    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let (status, selected_operator_symbol) = if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        (
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        )
    } else if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    Some(CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        policy_adapter: arithmetic_policy_adapter(program, spelling, operand_types),
        provider_plan_identity: 0,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    })
}

fn indexed_operator_spelling(program: &TypedTrees, index: ExpressionHandle) -> OperatorSpelling {
    if index.is_valid()
        && matches!(
            program.expression_table.expression(index),
            ExpressionNode::Range(_)
        )
    {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    }
}

fn indexed_operand_types(
    program: &TypedTrees,
    indexed: &TableIndexedExpression,
    origin: CheckedValueOrigin,
) -> Vec<Option<TypeReferenceHandle>> {
    let mut operand_types = vec![expression_type_reference_for_origin(
        program,
        indexed.collection,
        origin,
    )];
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(range) => {
            operand_types.push(expression_type_reference_for_origin(
                program,
                range.start,
                origin,
            ));
            operand_types.push(expression_type_reference_for_origin(
                program, range.end, origin,
            ));
        }
        _ => operand_types.push(expression_type_reference_for_origin(
            program,
            indexed.index,
            origin,
        )),
    }
    operand_types
}

/// Records the typed-trees resolution outcome for one use site as checked
/// evidence. Every known operand position participates, including both range
/// bounds for `[..]`.
fn operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let trait_candidates = origin_machine_symbol(origin).map_or_else(Vec::new, |machine_symbol| {
        selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
    });
    if !trait_candidates.is_empty() {
        return trait_operator_use_fact(
            program,
            expression,
            origin,
            spelling,
            trait_candidates,
            candidate_facts,
        );
    }
    let candidates = resolve_spelling_for_operands(program, spelling, operand_types);
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let (status, selected_operator_symbol) = if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        (
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        )
    } else if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        )
    } else if candidates.is_empty() {
        (
            CheckedOperatorResolutionStatus::Missing,
            SymbolHandle::invalid(),
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        policy_adapter: CheckedArithmeticPolicyAdapter::None,
        provider_plan_identity: 0,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    }
}

fn origin_machine_symbol(origin: CheckedValueOrigin) -> Option<SymbolHandle> {
    match origin {
        CheckedValueOrigin::MachineDecrease { machine_symbol, .. }
        | CheckedValueOrigin::MachineOwnedDataInitializer { machine_symbol, .. }
        | CheckedValueOrigin::StateStatement { machine_symbol, .. } => Some(machine_symbol),
        CheckedValueOrigin::NestedExpression { .. } => None,
    }
}

fn trait_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    candidates: Vec<SelectedTraitOperatorMeaning<'_>>,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(candidates.iter().map(|candidate| {
        CheckedOperatorCandidateFact::trait_backed(
            candidate.requirement.symbol,
            candidate.application.declaration,
            candidate.row.realization_machine,
            candidate.row.realization_state,
            candidate.application.report_fingerprint,
            candidate.application.commitment,
        )
        .with_signature(
            program
                .state_signature_parameters(candidate.requirement)
                .iter()
                .find(|parameter| parameter.is_self)
                .or_else(|| {
                    program
                        .state_signature_parameters(candidate.requirement)
                        .iter()
                        .find(|parameter| !parameter.is_self)
                })
                .map(|parameter| parameter.type_reference)
                .unwrap_or_else(TypeReferenceHandle::invalid),
            candidate.requirement.return_type,
            candidate.requirement.contracts,
            program
                .trait_type_parameters(candidate.trait_definition)
                .len()
                + program
                    .state_signature_type_parameters(candidate.requirement)
                    .len(),
            program
                .state_signature_parameters(candidate.requirement)
                .len(),
            candidate.trait_definition.is_boundary,
        )
    }));
    let (status, selected_operator_symbol) = if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.requirement.symbol,
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        policy_adapter: CheckedArithmeticPolicyAdapter::None,
        provider_plan_identity: 0,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    }
}

fn arithmetic_policy_adapter(
    program: &TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> CheckedArithmeticPolicyAdapter {
    if !matches!(
        spelling,
        OperatorSpelling::Add
            | OperatorSpelling::Subtract
            | OperatorSpelling::Multiply
            | OperatorSpelling::Divide
    ) {
        return CheckedArithmeticPolicyAdapter::None;
    }

    let mut format = None;
    let mut selected_domain = ArithmeticDomain::Exact;
    for type_reference in operand_types.iter().flatten().copied() {
        let candidate_format = match program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::F32) => Some(FloatFormat::BINARY32),
            Some(PrimitiveType::F64) => Some(FloatFormat::BINARY64),
            _ => None,
        };
        if let Some(candidate_format) = candidate_format {
            if format.is_some_and(|format| format != candidate_format) {
                return CheckedArithmeticPolicyAdapter::None;
            }
            format = Some(candidate_format);
        }

        let candidate_domain = program
            .type_reference_table
            .arithmetic_domain(type_reference);
        if candidate_domain == ArithmeticDomain::Exact {
            continue;
        }
        if selected_domain != ArithmeticDomain::Exact && selected_domain != candidate_domain {
            return CheckedArithmeticPolicyAdapter::None;
        }
        selected_domain = candidate_domain;
    }
    let Some(format) = format else {
        return CheckedArithmeticPolicyAdapter::None;
    };
    float_policy_adapter(format, selected_domain)
}

fn float_policy_adapter(
    format: FloatFormat,
    domain: ArithmeticDomain,
) -> CheckedArithmeticPolicyAdapter {
    match domain {
        ArithmeticDomain::Saturating => {
            CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
        }
        ArithmeticDomain::Trapping => {
            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
        }
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
            CheckedArithmeticPolicyAdapter::None
        }
    }
}

fn checked_candidate(
    program: &TypedTrees,
    candidate: &SpelledOperator<'_>,
) -> CheckedOperatorCandidateFact {
    let fact = if let Some(domain) = candidate.domain {
        CheckedOperatorCandidateFact::domain(candidate.operator.symbol, domain.symbol)
    } else {
        CheckedOperatorCandidateFact::root(candidate.operator.symbol)
    };
    fact.with_signature(
        program
            .operator_parameters(candidate.operator)
            .first()
            .map(|parameter| parameter.type_reference)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        candidate.operator.return_type,
        candidate.operator.contracts,
        program.operator_type_parameters(candidate.operator).len(),
        program.operator_parameters(candidate.operator).len(),
        candidate.operator.is_boundary,
    )
}
