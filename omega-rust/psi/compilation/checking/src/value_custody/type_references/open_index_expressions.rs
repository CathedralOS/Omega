use super::{TypeParameterScope, TypeReferenceOwner, type_reference_label, type_references_match};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use std::collections::HashMap;
use std::fmt;
use symbols::SymbolHandle;
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

/// Bind every retained open index operator to the consuming machine's
/// explicit conformance bound — `Alg: T satisfies IndexAdd<u64>` — whose
/// carrier trait declares the spelled requirement and the checked
/// commutativity/associativity law slots, before type identity or
/// compatibility checking consumes it. The bound's trait argument at the
/// requirement's operand parameter position names the index type the
/// operation is licensed for.
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
        normalizations.push(typed_trees::typed_trees::OpenIndexNormalization {
            expression,
            index_type,
            operations,
            // Version 2 admits single-law algebras. Version 1 canonicalized
            // an open index only when its carrier declared commutativity and
            // associativity together, so a commutativity-only algebra reached
            // the structural form; under version 2 its operands sort. That is
            // a canonical-form change, which contracts.md makes an explicit
            // compatibility event rather than a silent one.
            normalizer_version: 2,
        });
    }
    if diagnostics.is_empty() {
        program.open_index_normalizations = normalizations;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Maps each generic const/value binder symbol to the declaration that owns
/// it. The open index walk sees only shared expression tables, so the owning
/// machine is recovered from the expression's binder leaves — an open index
/// expression is parameterized by exactly one declaration's binders.
fn const_binder_owners(program: &TypedTrees) -> HashMap<SymbolHandle, SymbolHandle> {
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    index_type: TypeReferenceHandle,
    owner: Option<SymbolHandle>,
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
    let Some(owner) = owner else {
        // No declaration's binder parameterizes this arithmetic — it is a
        // concrete index expression, never an algebra selection.
        return;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner)
    else {
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
    // Specialized clones carry cleared bounds; their open sites inherit the
    // license their template declared and resolve the binder to the
    // conformance that specialization selected. The template's own sites
    // resolve the same binder through its specialization's conformance
    // arguments when any exist.
    let (bound_machine, evidence_spec) = program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.instance == machine.symbol && specialization.template != machine.symbol
        })
        .and_then(|specialization| {
            program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == specialization.template)
                .map(|template| (template, Some(specialization)))
        })
        .unwrap_or_else(|| {
            (
                machine,
                program
                    .machine_specializations
                    .iter()
                    .find(|specialization| specialization.template == machine.symbol),
            )
        });
    let mut candidates = Vec::new();
    for (bound_index, bound) in bound_machine.conformance_bounds.iter().enumerate() {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == bound.carrier)
        else {
            continue;
        };
        for requirement in program.trait_machine_signatures(trait_definition) {
            if requirement.spelling != Some(spelling) {
                continue;
            }
            // The requirement's operand is one shared trait parameter; the
            // bound's trait argument at that parameter's position names the
            // index type this binder licenses the spelling for.
            let Some(parameter) =
                binary_requirement_trait_parameter(program, trait_definition, requirement)
            else {
                continue;
            };
            let Some(argument) = bound.arguments.get(parameter) else {
                continue;
            };
            if !type_references_match(program, *argument, index_type) {
                continue;
            }
            candidates.push((bound_index, bound, trait_definition, requirement));
        }
    }
    let [(bound_index, bound, trait_definition, requirement)] = candidates.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires one exact conformance bound on machine `{}` supplying a `{}` operation spelled `{}`, but {} were found",
            spelling.symbol(),
            bound_machine.name,
            type_reference_label(program, index_type),
            spelling.symbol(),
            candidates.len()
        )));
        return;
    };
    let laws = crate::proof_contracts::contract_entailment::declared_index_algebra_laws(
        program,
        trait_definition,
        requirement.name.as_str(),
    );
    // The bound supplies the operation either way; the carrier trait's checked
    // law slots decide which rewrites the selection authorizes. Commutativity
    // licenses reordering and associativity licenses reassociation, and one
    // never implies the other, so record the selection as soon as either slot
    // is declared and carry both answers. With neither the expression keeps
    // its structural identity — the operation stays usable, the rewrites stay
    // unauthorized.
    if !laws.commutativity.is_empty() || !laws.associativity.is_empty() {
        let subject = program.normalized_type_identity(index_type).into_string();
        // Evidence binders are ordered by declaration; the same position in a
        // specialization's `conformance_arguments` names the selected
        // conformance once the call sites have supplied it.
        let evidence_position = bound_machine.conformance_bounds[..*bound_index]
            .iter()
            .filter(|candidate| candidate.binder.is_some())
            .count();
        let selected_conformance = evidence_spec
            .and_then(|specialization| specialization.conformance_arguments.get(evidence_position))
            .and_then(|symbol| {
                program
                    .conformances()
                    .iter()
                    .find(|conformance| conformance.symbol == *symbol)
            });
        operations.push(typed_trees::typed_trees::OpenIndexOperationSelection {
            expression,
            spelling,
            operator: requirement.symbol,
            operation_contract_identity: format!(
                "trait::{}::{}({subject},{subject})->{subject}",
                program.symbols.display_path(trait_definition.symbol, "::"),
                requirement.name,
            ),
            provider: selected_conformance
                .map(|conformance| conformance.symbol)
                .or(bound.binder)
                .unwrap_or(requirement.symbol),
            algebra_trait: trait_definition.symbol,
            algebra_requirement: requirement.name.as_str().to_owned(),
            algebra_alias: selected_conformance
                .and_then(|conformance| conformance.alias.as_ref())
                .or(bound.binder_name.as_ref())
                .map(|name| name.as_str().to_owned()),
            commutativity_licensed: !laws.commutativity.is_empty(),
            associativity_licensed: !laws.associativity.is_empty(),
        });
    }
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

/// A trait requirement spelled for index normalization must be the binary
/// operation on one shared trait parameter: two operands and the result all
/// bind the same parameter. Returns that parameter's position in the trait's
/// parameter list — the bound's trait argument at the position is the index
/// type the binder licenses the spelling for. `Self`-operand requirements
/// name no parameter position, and a bound's subject is always one of the
/// machine's own binders, so they cannot carry an index type.
fn binary_requirement_trait_parameter(
    program: &TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    requirement: &typed_trees::signature::StateSignature,
) -> Option<usize> {
    let trait_parameters = program.trait_type_parameters(trait_definition);
    let parameter_position = |type_reference: TypeReferenceHandle| {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        trait_parameters
            .iter()
            .position(|parameter| parameter.symbol == *symbol)
    };
    let parameters = program.state_signature_parameters(requirement);
    let [left, right] = parameters else {
        return None;
    };
    let operand = parameter_position(left.type_reference)?;
    (parameter_position(right.type_reference) == Some(operand)
        && parameter_position(requirement.return_type) == Some(operand))
    .then_some(operand)
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
            matches!(
                candidate.kind,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            ) && ((symbol.is_valid() && candidate.symbol == *symbol)
                || candidate.name.as_str() == name.as_str())
        });
        let Some(binder) = binder else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} supplies `{name}` as an index for `{}`, but it is neither a canonical named const nor a direct in-scope const binder",
                domain_name
            )));
            continue;
        };
        let (TypeParameterKind::Const {
            type_reference: actual,
        }
        | TypeParameterKind::Value {
            type_reference: actual,
        }) = binder.kind
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
                matches!(
                    candidate.kind,
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                ) && candidate.name.as_str() == name
            });
            let Some(binder) = binder else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `{name}` in open index expression `{}`, but it is not a direct in-scope const binder",
                    program.expression_table.display_name(expression)
                )));
                return;
            };
            let (TypeParameterKind::Const {
                type_reference: actual,
            }
            | TypeParameterKind::Value {
                type_reference: actual,
            }) = binder.kind
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
