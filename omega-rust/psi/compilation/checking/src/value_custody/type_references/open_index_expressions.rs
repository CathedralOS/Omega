use super::{TypeParameterScope, TypeReferenceOwner, type_reference_label, type_references_match};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use std::collections::HashMap;
use std::fmt;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn validate_indexed_domain_arguments(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    let Some(definition) = program.domain_definition(constraint.symbol) else {
        return;
    };
    validate_indexed_domain_argument_pack(
        program,
        definition,
        constraint.name.as_str(),
        &constraint.arguments,
        scope,
        diagnostics,
        &owner,
    );
}

pub(crate) fn validate_indexed_qualification_arguments(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    cast: &typed_trees::expression::TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !cast.semantic_domain_symbol.is_valid() {
        return;
    }
    let Some(definition) = program.domain_definition(cast.semantic_domain_symbol) else {
        return;
    };
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments);
    let domain_name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .joined_path();
    let owner = format!("machine `{}` indexed qualification", machine.name);
    validate_indexed_domain_argument_pack(
        program,
        definition,
        &domain_name,
        arguments,
        TypeParameterScope {
            operand_lifetime_owner: symbols::SymbolHandle::invalid(),
            lexical_owner: machine.symbol,
            type_parameters: program.machine_type_parameters(machine),
            lifetime_parameters: &machine.lifetime_parameters,
        },
        diagnostics,
        &owner,
    );
}

/// Bind every retained open index operator to the consuming machine's
/// explicit conformance bound — `Alg: T satisfies IndexAdd<u64>` — whose
/// carrier trait declares the spelled requirement and the checked
/// commutativity/associativity law slots, before type identity or
/// compatibility checking consumes it. The bound's trait argument at the
/// requirement's operand parameter position names the index type the
/// operation is licensed for.
/// Const-typed domain parameters' `(expression, expected)` sites zipped from
/// one argument row — deduped by expression.
fn collect_domain_const_sites<'a>(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::data::TypeParameter],
    arguments: impl Iterator<Item = &'a TypeReferenceHandle>,
    sites: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let TypeParameterKind::Const {
            type_reference: expected,
        } = parameter.kind
        else {
            continue;
        };
        let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(*argument)
        else {
            continue;
        };
        if !sites.iter().any(|(existing, _)| *existing == *expression) {
            sites.push((*expression, expected));
        }
    }
}

pub fn normalize_open_index_expressions(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut sites = Vec::new();
    for (_, _, constraints) in program
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for constraint in program.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Domain(constraint) = constraint else {
                continue;
            };
            let Some(definition) = program.domain_definition(constraint.symbol) else {
                continue;
            };
            let parameters = typed_trees::domain::index_parameters(program, definition);
            collect_domain_const_sites(
                program,
                parameters,
                constraint.arguments.iter(),
                &mut sites,
            );
        }
    }
    let indexed_qualification_sites = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, expression)| {
            let typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            (cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty())
                .then_some((cast.semantic_domain_symbol, cast.semantic_domain_arguments))
        })
        .collect::<Vec<_>>();
    for (domain_symbol, arguments) in indexed_qualification_sites {
        let Some(definition) = program.domain_definition(domain_symbol) else {
            continue;
        };
        let parameters = typed_trees::domain::index_parameters(program, definition);
        collect_domain_const_sites(
            program,
            parameters,
            program
                .type_reference_table
                .type_reference_handles(arguments)
                .iter(),
            &mut sites,
        );
    }

    let binder_owners = const_binder_owners(program);
    let mut diagnostics = Vec::new();
    let mut normalizations = Vec::new();
    for (expression, index_type) in sites {
        let mut operations = Vec::new();
        let owner = open_index_expression_owner(program, expression, &binder_owners);
        normalize_open_index_expression_operations(
            program,
            expression,
            index_type,
            owner,
            &mut operations,
            &mut diagnostics,
        );
        normalizations.push((
            expression,
            typed_trees::expression::OpenIndexNormalization {
                index_type,
                // Version 3 retains exact providers even without AC laws and
                // shares their identity with established fact operands. An
                // alias is diagnostic payload, never algebra identity.
                normalizer_version: 3,
            },
            operations,
        ));
    }
    // Facts and indices must describe the same selected operation before any
    // specialization copies them. Binding only index syntax leaves a fact's
    // `+` unqualified and lets token-only matching borrow a foreign algebra.
    // These are ordinary lexical conformance bounds, not ambient satisfiers.
    let fact_operations = bind_machine_index_operations(program);
    if diagnostics.is_empty() {
        for (expression, normalization, operations) in normalizations {
            program
                .expression_table
                .index_expression_authority_mut(expression)
                .normalization = Some(normalization);
            for operation in operations {
                let expression = operation.expression;
                program
                    .expression_table
                    .index_expression_authority_mut(expression)
                    .operation = Some(operation);
            }
        }
        for operation in fact_operations {
            let expression = operation.expression;
            program
                .expression_table
                .index_expression_authority_mut(expression)
                .operation = Some(operation);
        }
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Maps each generic const/value binder symbol to the declaration that owns
/// it. The open index walk sees only shared expression tables, so the owning
/// machine is recovered from the expression's binder leaves — an open index
/// expression is parameterized by exactly one declaration's binders.
fn const_binder_owners(program: &dyn crate::CheckingInput) -> HashMap<SymbolHandle, SymbolHandle> {
    let mut owners = HashMap::new();
    let mut seed = |parameters: &[typed_trees::data::TypeParameter], owner: SymbolHandle| {
        for parameter in parameters {
            owners.insert(parameter.symbol, owner);
        }
    };
    for machine in program.machines() {
        seed(program.machine_type_parameters(machine), machine.symbol);
    }
    for data in program.data_definitions() {
        seed(program.data_type_parameters(data), data.symbol);
    }
    for domain in program.domain_definitions() {
        seed(program.domain_type_parameters(domain), domain.symbol);
    }
    for operator in program.operators() {
        seed(program.operator_type_parameters(operator), operator.symbol);
    }
    for trait_definition in program.traits() {
        seed(
            program.trait_type_parameters(trait_definition),
            trait_definition.symbol,
        );
        for signature in program.trait_machine_signatures(trait_definition) {
            seed(
                program.state_signature_type_parameters(signature),
                trait_definition.symbol,
            );
        }
    }
    for conformance in program.conformances() {
        seed(
            program.conformance_type_parameters(conformance),
            conformance.symbol,
        );
    }
    owners
}

/// The single declaration whose binder leaves parameterize this expression —
/// its machine's conformance bounds license the spelled operations. `None`
/// when no leaf is a generic binder (a fully concrete arithmetic expression
/// never carries open-index authority).
fn open_index_expression_owner(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    binder_owners: &HashMap<SymbolHandle, SymbolHandle>,
) -> Option<SymbolHandle> {
    use typed_trees::expression::ExpressionNode;

    let mut owner = None;
    let mut leaves = vec![expression];
    while let Some(handle) = leaves.pop() {
        match program.expression_table.expression(handle) {
            ExpressionNode::Name(path) => {
                if let Some(candidate) = binder_owners.get(&path.symbol)
                    && let Some(existing) = owner.replace(*candidate)
                    && existing != *candidate
                {
                    return None;
                }
            }
            ExpressionNode::Binary(binary) => {
                leaves.push(binary.left);
                leaves.push(binary.right);
            }
            ExpressionNode::Unary(unary) => leaves.push(unary.operand),
            _ => {}
        }
    }
    owner
}

fn normalize_open_index_expression_operations(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    index_type: TypeReferenceHandle,
    owner: Option<SymbolHandle>,
    operations: &mut Vec<typed_trees::expression::OpenIndexOperationSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use language_core::operator_spelling::OperatorSpelling;
    use typed_trees::expression::BinaryOperator;

    let Some(binary) = program.expression_table.binary_node(expression) else {
        return;
    };
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        _ => return,
    };
    let Some(owner) = owner else {
        // No declaration's binder parameterizes this arithmetic — it is a
        // concrete index expression, never an algebra selection.
        return;
    };
    let Some(machine) = program.machine(owner) else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires an explicit conformance binder on the machine owning the index expression, but the owner is not a machine",
            spelling.symbol(),
        )));
        return;
    };
    let TypeReferenceNode::Named { .. } = program.type_reference_table.type_reference(index_type)
    else {
        return;
    };
    let candidates = bound_index_operations(program, machine, expression, index_type, spelling);
    let [operation] = candidates.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires one exact conformance bound on machine `{}` supplying a `{}` operation spelled `{}`, but {} were found",
            spelling.symbol(),
            machine.name,
            type_reference_label(program, index_type),
            spelling.symbol(),
            candidates.len()
        )));
        return;
    };
    operations.push(operation.clone());
    normalize_open_index_expression_operations(
        program,
        binary.left,
        index_type,
        Some(owner),
        operations,
        diagnostics,
    );
    normalize_open_index_expression_operations(
        program,
        binary.right,
        index_type,
        Some(owner),
        operations,
        diagnostics,
    );
}

/// One selection operation serves index syntax and ordinary fact operands.
/// A selected operation retains identity even when neither law permits rewriting.
fn bound_index_operations(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    index_type: TypeReferenceHandle,
    spelling: language_core::operator_spelling::OperatorSpelling,
) -> Vec<typed_trees::expression::OpenIndexOperationSelection> {
    typed_trees::operator::bound_trait_operator_meanings(
        program,
        machine.symbol,
        spelling,
        &[Some(index_type), Some(index_type)],
    )
    .into_iter()
    .filter_map(|meaning| {
        // An index algebra is closed over its carrier. Comparisons retain
        // their ordinary requirement meaning, not index-operation laws.
        if meaning.result_identity(program) != program.normalized_type_identity(index_type) {
            return None;
        }
        let trait_definition = meaning.trait_definition;
        let requirement = meaning.requirement;
        let bound = meaning.bound;
        let laws = crate::proof_contracts::contract_entailment::declared_index_algebra_laws(
            program,
            trait_definition,
            requirement.name.as_str(),
        );
        Some(typed_trees::expression::OpenIndexOperationSelection {
            expression,
            spelling,
            operator: requirement.symbol,
            operand_type: index_type,
            provider: bound.binder,
            application: None,
            algebra_trait: trait_definition.symbol,
            algebra_requirement: requirement.name.as_str().to_owned(),
            algebra_alias: Some(bound.binder_name.as_str().to_owned()),
            commutativity_licensed: !laws.commutativity.is_empty(),
            associativity_licensed: !laws.associativity.is_empty(),
        })
    })
    .collect()
}

fn bind_machine_index_operations(
    program: &dyn crate::CheckingInput,
) -> Vec<typed_trees::expression::OpenIndexOperationSelection> {
    let mut operations = Vec::new();
    program.for_each_machine_state_contract_frame(|machine, state, contracts, _| {
        if machine.conformance_bounds.is_empty() {
            return;
        }
        let mut expressions = Vec::new();
        for contract in contracts {
            crate::expression_traversal::collect_contract_facts(
                program,
                contract.facts,
                &mut expressions,
            );
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::expression_traversal::collect_statement_expression_trees(
                program,
                statement,
                &mut expressions,
            );
        }
        for expression in expressions {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                continue;
            };
            let Some(spelling) =
                typed_trees::operator::arithmetic_operator_spelling(binary.operator)
            else {
                continue;
            };
            let operand_type =
                crate::expression_result_type_reference(program, machine, state, binary.left)
                    .or_else(|| {
                        crate::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            binary.right,
                        )
                    });
            let Some(operand_type) = operand_type else {
                continue;
            };
            let candidates =
                bound_index_operations(program, machine, expression, operand_type, spelling);
            if let [operation] = candidates.as_slice() {
                operations.push(operation.clone());
            }
        }
    });
    operations
}

fn validate_indexed_domain_argument_pack(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::domain::DomainDefinition,
    domain_name: &str,
    arguments: &[TypeReferenceHandle],
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &dyn fmt::Display,
) {
    let parameters = typed_trees::domain::index_parameters(program, definition);
    if parameters.is_empty() {
        return;
    }
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let TypeParameterKind::Const {
            type_reference: expected,
        } = parameter.kind
        else {
            continue;
        };
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            if let TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(*argument)
            {
                validate_open_index_expression(
                    program,
                    *expression,
                    expected,
                    scope,
                    domain_name,
                    owner,
                    diagnostics,
                );
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} supplies a noncanonical argument for indexed domain `{}`",
                    domain_name
                )));
            }
            continue;
        };
        if let Some(value) =
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
        {
            let expected_name = const_index_type_label(program, expected);
            if value.type_name != expected_name {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} supplies indexed-domain argument type `{}`, but `{}` requires `{expected_name}`",
                    value.type_name, parameter.name
                )));
            }
            continue;
        }
        if let Ok(value) = name.as_str().parse::<i128>() {
            if !integer_const_fits_type(program, expected, value) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} supplies integer index `{value}` outside the declared `{}` type",
                    const_index_type_label(program, expected)
                )));
            }
            continue;
        }
        let binder = scope.type_parameters.iter().find(|candidate| {
            candidate.kind.is_const_or_value()
                && if symbol.is_valid() {
                    candidate.symbol == *symbol
                } else {
                    candidate.name.as_str() == name.as_str()
                }
        });
        let Some(binder) = binder else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} supplies `{name}` as an index for `{}`, but it is neither a canonical named const nor a direct in-scope const binder",
                domain_name
            )));
            continue;
        };
        let actual = const_binder_type_reference(binder);
        if !type_references_match(program, actual, expected) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} forwards const binder `{}` of type `{}` into `{}`, which requires `{}`",
                binder.name,
                type_reference_label(program, actual),
                domain_name,
                type_reference_label(program, expected)
            )));
        }
    }
}

/// A const or value binder's declared type reference — the only kinds the
/// candidate filters above admit.
fn const_binder_type_reference(binder: &typed_trees::data::TypeParameter) -> TypeReferenceHandle {
    let (TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }) =
        binder.kind
    else {
        unreachable!();
    };
    type_reference
}

fn validate_open_index_expression(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    expected: TypeReferenceHandle,
    scope: TypeParameterScope<'_>,
    domain_name: &str,
    owner: &dyn fmt::Display,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .joined_path();
            if let Some(value) = CanonicalConstValue::from_atom(&name) {
                let expected_name = const_index_type_label(program, expected);
                if value.type_name != expected_name {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses closed index atom of type `{}` in `{domain_name}`, whose expression requires `{expected_name}`",
                        value.type_name
                    )));
                }
                return;
            }
            if let Ok(value) = name.parse::<i128>() {
                if !integer_const_fits_type(program, expected, value) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses integer index `{value}` outside the declared `{}` type in `{domain_name}`",
                        const_index_type_label(program, expected)
                    )));
                }
                return;
            }
            // Copied leaves keep authored display text after exact lexical
            // remapping. A resolved declaration, never that old spelling,
            // decides whether this scope owns the binder.
            let symbol = if path.symbol.is_valid() { path.symbol } else { path.head_symbol };
            let binder = scope.type_parameters.iter().find(|candidate| {
                candidate.kind.is_const_or_value()
                    && if symbol.is_valid() {
                    candidate.symbol == symbol
                } else {
                    candidate.name.as_str() == name
                }
            });
            let Some(binder) = binder else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `{name}` in open index expression `{}`, but it is not a direct in-scope const binder",
                    program.expression_table.display_name(expression)
                )));
                return;
            };
            let actual = const_binder_type_reference(binder);
            if !type_references_match(program, actual, expected) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses const binder `{}` of type `{}` in `{domain_name}`, whose index expression requires `{}`",
                    binder.name,
                    type_reference_label(program, actual),
                    type_reference_label(program, expected)
                )));
            }
        }
        ExpressionNode::Integer(value) => {
            let Some(value) = value
                .value_i64()
                .map(i128::from)
                .or_else(|| value.value_u64().map(i128::from))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses an integer outside the const-index envelope in `{domain_name}`"
                )));
                return;
            };
            if !integer_const_fits_type(program, expected, value) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses integer index `{value}` outside the declared `{}` type in `{domain_name}`",
                    const_index_type_label(program, expected)
                )));
            }
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
            ) =>
        {
            validate_open_index_expression(
                program,
                binary.left,
                expected,
                scope,
                domain_name,
                owner,
                diagnostics,
            );
            validate_open_index_expression(
                program,
                binary.right,
                expected,
                scope,
                domain_name,
                owner,
                diagnostics,
            );
        }
        _ => diagnostics.push(Diagnostic::error(format!(
            "{owner} uses unsupported open index expression `{}` in `{domain_name}`; only direct const binders combined with `+`, `-`, `*`, or `/` are in the proof-static algebra fragment",
            program.expression_table.display_name(expression)
        ))),
    }
}

fn const_index_type_label(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => name.as_str().to_owned(),
        TypeReferenceNode::Constrained { base_type, .. } => {
            const_index_type_label(program, *base_type)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => format!(
            "[{}; {length}]",
            const_index_type_label(program, *element_type)
        ),
        TypeReferenceNode::Unit => "()".to_owned(),
        _ => type_reference_label(program, type_reference),
    }
}

fn integer_const_fits_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    value: i128,
) -> bool {
    let label = const_index_type_label(program, type_reference);
    let (minimum, maximum) = match label.as_str() {
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "u64" | "addr" => (0, i128::from(u64::MAX)),
        _ => return false,
    };
    value >= minimum && value <= maximum
}
