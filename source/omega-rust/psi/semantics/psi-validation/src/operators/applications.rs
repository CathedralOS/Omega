use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameterKind;
use psi_typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use psi_typed_trees::operator::OperatorDefinition;
use psi_typed_trees::statement::{StatementHandle, TableCall};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatedBoundaryOperatorApplicationUseSite {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryOperatorApplication {
    pub site: ValidatedBoundaryOperatorApplicationUseSite,
    pub requirement: SymbolHandle,
    pub arguments: Vec<ValidatedBoundaryOperatorApplicationArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedBoundaryOperatorApplicationArgument {
    Type {
        binder_owner: SymbolHandle,
        binder_ordinal: u32,
        binder_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
}

/// Validate and close the currently supported named-operator static
/// application. `None` is a truthful unsupported/open demand, never coverage.
/// The returned bindings are ordered by the operator's declaration telescope.
pub fn validate_named_operator_type_application(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    static_arguments: &[StaticMachineArgument],
    operand_types: &[Option<TypeReferenceHandle>],
) -> Result<Option<Vec<(SymbolHandle, TypeReferenceHandle)>>, Diagnostic> {
    let operator_name = program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let parameters = program.operator_type_parameters(operator);

    if !operator.lifetime_parameters.is_empty()
        || parameters
            .iter()
            .any(|parameter| !matches!(parameter.kind, TypeParameterKind::Type))
        || parameters
            .iter()
            .any(|parameter| parameter.bounds != Default::default())
    {
        if static_arguments.is_empty() {
            return Ok(None);
        }
        return Err(Diagnostic::error(format!(
            "named operator `{operator_name}` has a lifetime, bounded, const, machine, or proposition telescope whose explicit application is not yet supported"
        )));
    }

    if parameters.is_empty() {
        if static_arguments.is_empty() {
            return Ok(Some(Vec::new()));
        }
        return Err(Diagnostic::error(format!(
            "monomorphic named operator `{operator_name}` takes no static arguments, got {}",
            static_arguments.len()
        )));
    }

    let Some(inferred) = psi_typed_trees::operator::closed_operator_type_application_for_operands(
        program,
        operator,
        operand_types,
    ) else {
        if static_arguments.is_empty() {
            return Ok(None);
        }
        return Err(Diagnostic::error(format!(
            "named operator `{operator_name}` cannot validate explicit static arguments because its operand application remains open or unresolved"
        )));
    };

    if static_arguments.is_empty() {
        return Ok(Some(inferred));
    }
    if static_arguments.len() != inferred.len() {
        return Err(Diagnostic::error(format!(
            "named operator `{operator_name}` requires {} static type argument(s), got {}",
            inferred.len(),
            static_arguments.len()
        )));
    }
    for ((parameter, (_, inferred)), supplied) in parameters
        .iter()
        .zip(&inferred)
        .zip(static_arguments.iter())
    {
        if !static_type_argument_matches(program, supplied, *inferred) {
            return Err(Diagnostic::error(format!(
                "named operator `{operator_name}` static type argument for parameter `{}` does not equal the type inferred from its operands",
                parameter.name
            )));
        }
    }
    Ok(Some(inferred))
}

pub fn validated_boundary_operator_application(
    site: ValidatedBoundaryOperatorApplicationUseSite,
    operator: &OperatorDefinition,
    bindings: Vec<(SymbolHandle, TypeReferenceHandle)>,
) -> ValidatedBoundaryOperatorApplication {
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, (binder_symbol, type_reference))| {
            ValidatedBoundaryOperatorApplicationArgument::Type {
                binder_owner: operator.symbol,
                binder_ordinal: u32::try_from(ordinal)
                    .expect("operator static telescope ordinal overflow"),
                binder_symbol,
                type_reference,
            }
        })
        .collect();
    ValidatedBoundaryOperatorApplication {
        site,
        requirement: operator.symbol,
        arguments,
    }
}

pub(crate) fn retain_validated_boundary_operator_application(
    applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    application: ValidatedBoundaryOperatorApplication,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(existing) = applications
        .iter()
        .find(|existing| existing.site == application.site)
    else {
        applications.push(application);
        return;
    };
    if existing != &application {
        diagnostics.push(Diagnostic::error(
            "one operator use produced inconsistent validated static applications",
        ));
    }
}

pub(crate) fn validate_named_statement_operator_application(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement: StatementHandle,
    call: &TableCall,
    applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(operator) =
        psi_typed_trees::operator::declaration_by_symbol(program, call.target_symbol)
            .filter(|operator| operator.spelling.is_none())
    else {
        return;
    };
    let mut operand_types = Vec::new();
    if let Some(receiver_type) = statement_value_receiver_type(program, machine, state, call) {
        operand_types.push(Some(receiver_type));
    }
    operand_types.extend(
        program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .map(|argument| {
                crate::places::declared_place_type(program, machine, Some(state), *argument)
            }),
    );
    match validate_named_operator_type_application(
        program,
        operator,
        &call.machine_arguments,
        &operand_types,
    ) {
        Ok(Some(bindings)) if operator.is_boundary => {
            let application = validated_boundary_operator_application(
                ValidatedBoundaryOperatorApplicationUseSite::Statement(statement),
                operator,
                bindings,
            );
            retain_validated_boundary_operator_application(applications, application, diagnostics);
        }
        Ok(_) => {}
        Err(diagnostic) => diagnostics.push(diagnostic),
    }
}

fn statement_value_receiver_type(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    call: &TableCall,
) -> Option<TypeReferenceHandle> {
    if !call.receiver_symbol.is_valid()
        || !matches!(
            program.symbols.get(call.receiver_symbol).kind,
            psi_symbols::SymbolKind::Field
                | psi_symbols::SymbolKind::Local
                | psi_symbols::SymbolKind::Parameter
        )
    {
        return None;
    }
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    crate::calls::declared_receiver_type_reference(program, machine, state, receiver.as_str())
}

fn static_type_argument_matches(
    program: &TypedTrees,
    supplied: &StaticMachineArgument,
    inferred: TypeReferenceHandle,
) -> bool {
    if supplied.const_literal.is_some() || supplied.evidence_projection.is_some() {
        return false;
    }
    match program.type_reference_table.type_reference(inferred) {
        TypeReferenceNode::Named { symbol, .. } => {
            supplied.application.is_none()
                && symbol.is_valid()
                && supplied.symbol.is_valid()
                && supplied.symbol == *symbol
        }
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => {
            let Some(application) = supplied.application.as_ref() else {
                return false;
            };
            base_symbol.is_valid()
                && supplied.symbol.is_valid()
                && supplied.symbol == *base_symbol
                && lifetime_arguments.is_empty()
                && application.lifetime_arguments.is_empty()
                && application.arguments.len() == arguments.len() as usize
                && application
                    .arguments
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .all(|(supplied, inferred)| {
                        static_type_argument_matches(program, supplied, *inferred)
                    })
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}
