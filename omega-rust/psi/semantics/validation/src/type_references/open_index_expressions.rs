use super::{TypeParameterScope, TypeReferenceOwner, type_reference_label, type_references_match};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use std::fmt;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn validate_indexed_domain_arguments(
    program: &TypedTrees,
    constraint: &typed_trees::types::DomainConstraint,
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    let Some(definition) = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == constraint.symbol)
    else {
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
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    cast: &typed_trees::expression::TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !cast.semantic_domain_symbol.is_valid() {
        return;
    }
    let Some(definition) = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == cast.semantic_domain_symbol)
    else {
        return;
    };
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments);
    let domain_name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let owner = format!("machine `{}` indexed qualification", machine.name);
    validate_indexed_domain_argument_pack(
        program,
        definition,
        &domain_name,
        arguments,
        TypeParameterScope {
            type_parameters: program.machine_type_parameters(machine),
            lifetime_parameters: &machine.lifetime_parameters,
        },
        diagnostics,
        &owner,
    );
}

/// Bind every retained open index operator to one exact public operator and
/// one proved associative/commutative algebra instance before type identity or
/// compatibility checking consumes it.
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
            let Some(definition) = program
                .domain_definitions()
                .iter()
                .find(|definition| definition.symbol == constraint.symbol)
            else {
                continue;
            };
            let parameters = typed_trees::domain::index_parameters(program, definition);
            for (parameter, argument) in parameters.iter().zip(&constraint.arguments) {
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
        let Some(definition) = program
            .domain_definitions()
            .iter()
            .find(|definition| definition.symbol == domain_symbol)
        else {
            continue;
        };
        let parameters = typed_trees::domain::index_parameters(program, definition);
        for (parameter, argument) in parameters.iter().zip(
            program
                .type_reference_table
                .type_reference_handles(arguments),
        ) {
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

    let mut diagnostics = Vec::new();
    let mut normalizations = Vec::new();
    for (expression, index_type) in sites {
        let mut operations = Vec::new();
        normalize_open_index_expression_operations(
            program,
            expression,
            index_type,
            &mut operations,
            &mut diagnostics,
        );
        normalizations.push(typed_trees::typed_trees::OpenIndexNormalization {
            expression,
            index_type,
            operations,
            normalizer_version: 1,
        });
    }
    if diagnostics.is_empty() {
        program.open_index_normalizations = normalizations;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn normalize_open_index_expression_operations(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    index_type: TypeReferenceHandle,
    operations: &mut Vec<typed_trees::typed_trees::OpenIndexOperationSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use language_core::operator_spelling::OperatorSpelling;
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        _ => return,
    };
    let candidates = typed_trees::operator::resolve_spelling_for_operands(
        program,
        spelling,
        &[Some(index_type), Some(index_type)],
    );
    let [selected] = candidates.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires one exact operator over `{}`, but {} candidates were found",
            spelling.symbol(),
            type_reference_label(program, index_type),
            candidates.len()
        )));
        return;
    };
    if !type_references_match(program, selected.operator.return_type, index_type) {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` returns `{}`, but the indexed domain requires `{}`",
            spelling.symbol(),
            type_reference_label(program, selected.operator.return_type),
            type_reference_label(program, index_type)
        )));
        return;
    }
    let path = program.operator_path_members(selected.operator.name);
    let [namespace, requirement] = path else {
        diagnostics.push(Diagnostic::error(
            "an open index operator must have an exact `Namespace::operation` contract path",
        ));
        return;
    };
    let providers = program
        .machines()
        .iter()
        .filter(|machine| {
            program
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    conformance.name.as_str() == namespace.as_str()
                        && conformance.requirement.as_ref().map(|name| name.as_str())
                            == Some(requirement.as_str())
                })
                && typed_trees::operator::resolve_satisfied_checked_operator(
                    program,
                    machine,
                    namespace.as_str(),
                    requirement.as_str(),
                )
                .is_some_and(|operator| operator.symbol == selected.operator.symbol)
        })
        .collect::<Vec<_>>();
    let [provider] = providers.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires one exact checked provider for `{namespace}::{requirement}`, but {} were found",
            spelling.symbol(),
            providers.len()
        )));
        return;
    };
    let algebras =
        crate::contract_entailment::proved_index_algebras_for_provider(program, provider);
    let [algebra] = algebras.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` provider `{}` requires one exact proved associative/commutative algebra instance, but {} were found",
            spelling.symbol(),
            provider.name,
            algebras.len()
        )));
        return;
    };
    operations.push(typed_trees::typed_trees::OpenIndexOperationSelection {
        expression,
        spelling,
        operator: selected.operator.symbol,
        operation_contract_identity: typed_trees::operator::boundary_operator_requirement_identity(
            program,
            selected.operator,
        ),
        provider: provider.symbol,
        algebra_trait: algebra.trait_symbol,
        algebra_requirement: algebra.requirement.clone(),
        algebra_alias: algebra.alias.clone(),
    });
    normalize_open_index_expression_operations(
        program,
        binary.left,
        index_type,
        operations,
        diagnostics,
    );
    normalize_open_index_expression_operations(
        program,
        binary.right,
        index_type,
        operations,
        diagnostics,
    );
}

fn validate_indexed_domain_argument_pack(
    program: &TypedTrees,
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
            matches!(candidate.kind, TypeParameterKind::Const { .. })
                && ((symbol.is_valid() && candidate.symbol == *symbol)
                    || candidate.name.as_str() == name.as_str())
        });
        let Some(binder) = binder else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} supplies `{name}` as an index for `{}`, but it is neither a canonical named const nor a direct in-scope const binder",
                domain_name
            )));
            continue;
        };
        let TypeParameterKind::Const {
            type_reference: actual,
        } = binder.kind
        else {
            unreachable!();
        };
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

fn validate_open_index_expression(
    program: &TypedTrees,
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
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
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
            let binder = scope.type_parameters.iter().find(|candidate| {
                matches!(candidate.kind, TypeParameterKind::Const { .. })
                    && candidate.name.as_str() == name
            });
            let Some(binder) = binder else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `{name}` in open index expression `{}`, but it is not a direct in-scope const binder",
                    program.expression_table.display_name(expression)
                )));
                return;
            };
            let TypeParameterKind::Const {
                type_reference: actual,
            } = binder.kind
            else {
                unreachable!();
            };
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

fn const_index_type_label(program: &TypedTrees, type_reference: TypeReferenceHandle) -> String {
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
    program: &TypedTrees,
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
