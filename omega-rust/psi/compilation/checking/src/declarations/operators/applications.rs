use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstIdentity;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::operator::{ClosedOperatorApplicationArgument, OperatorDefinition};
use typed_trees::statement::{StatementHandle, TableCall};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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

/// Canonically encode and independently replay every exact closed operator
/// realization retained by one concrete machine specialization. The join is
/// typed-program vocabulary; checking forwards its `CheckingInput` view.
pub fn canonical_closed_operator_realization_bytes(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    retained: &[typed_trees::operator::ClosedOperatorRealizationApplication],
) -> Result<Vec<u8>, &'static str> {
    typed_trees::operator::canonical_closed_operator_realization_bytes(
        program.typed_program(),
        machine_symbol,
        retained,
    )
}

/// Compare one checked D29 use demand with one independently retained closed
/// realization by semantic identity rather than private type-table handles.
/// The join lives beside its checked operand types in checked-trees.
pub fn checked_operator_application_matches_realization(
    program: &dyn crate::CheckingInput,
    demand: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
    realization: &typed_trees::operator::ClosedOperatorRealizationApplication,
) -> bool {
    checked_trees::operators::checked_operator_application_matches_realization(
        program.typed_program(),
        demand,
        realization,
    )
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedBoundaryOperatorApplicationArgument {
    Type {
        binder_owner: SymbolHandle,
        binder_ordinal: u32,
        binder_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
    Const {
        binder_owner: SymbolHandle,
        binder_ordinal: u32,
        binder_symbol: SymbolHandle,
        declared_carrier: TypeReferenceHandle,
        value: CanonicalConstIdentity,
    },
    TypeBinder {
        binder_owner: SymbolHandle,
        binder_ordinal: u32,
        binder_symbol: SymbolHandle,
        machine_owner: SymbolHandle,
        machine_binder_ordinal: u32,
        machine_binder_symbol: SymbolHandle,
    },
}

/// Validate and close the currently supported named-operator static
/// application. `None` is a truthful unsupported/open demand, never coverage.
/// The returned bindings are ordered by the operator's declaration telescope.
pub fn validate_named_operator_application(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
    static_arguments: &[StaticMachineArgument],
    operand_types: &[Option<TypeReferenceHandle>],
) -> Result<Option<Vec<ClosedOperatorApplicationArgument>>, Diagnostic> {
    let operator_name = program.operator_path_string(operator.name);
    let parameters = program.operator_type_parameters(operator);

    if !operator.lifetime_parameters.is_empty()
        || parameters.iter().any(|parameter| {
            !matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Const { .. }
            )
        })
    {
        if static_arguments.is_empty() {
            return Ok(None);
        }
        return Err(Diagnostic::error(format!(
            "named operator `{operator_name}` has a lifetime, machine, or proposition telescope whose explicit application is not yet supported"
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

    let Some(inferred) = typed_trees::operator::closed_operator_application_for_operands(
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

    validate_closed_operator_application(program, operator, &inferred)?;

    if static_arguments.is_empty() {
        return Ok(Some(inferred));
    }
    if static_arguments.len() != inferred.len() {
        return Err(Diagnostic::error(format!(
            "named operator `{operator_name}` requires {} static argument(s), got {}",
            inferred.len(),
            static_arguments.len()
        )));
    }
    for ((parameter, inferred), supplied) in parameters
        .iter()
        .zip(&inferred)
        .zip(static_arguments.iter())
    {
        if !static_argument_matches(program, supplied, inferred) {
            let identity_kind = match inferred {
                ClosedOperatorApplicationArgument::Type { .. } => "type",
                ClosedOperatorApplicationArgument::Const { .. } => "value",
            };
            return Err(Diagnostic::error(format!(
                "named operator `{operator_name}` static argument for parameter `{}` does not equal the {identity_kind} inferred from its operands",
                parameter.name,
            )));
        }
    }
    Ok(Some(inferred))
}

/// Recover the exact builtin type carried by a width-landed integer literal.
///
/// Place lookup deliberately does not classify literals. Generic operator
/// applications still need a concrete operand type before specialization,
/// and a width suffix already provides that source-owned fact. This
/// helper rejoins the landing to the compiler-installed builtin symbol and an
/// existing typed type-reference; spelling alone never selects the type.
pub fn landed_integer_literal_type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let atom = match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            typed_trees::types::builtin_type_atom(literal.landing()?.landed_type)
        }
        _ => return None,
    };
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

pub fn validated_boundary_operator_application(
    site: ValidatedBoundaryOperatorApplicationUseSite,
    operator: &OperatorDefinition,
    bindings: Vec<ClosedOperatorApplicationArgument>,
) -> ValidatedBoundaryOperatorApplication {
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            let binder_ordinal =
                u32::try_from(ordinal).expect("operator static telescope ordinal overflow");
            match argument {
                ClosedOperatorApplicationArgument::Type {
                    binder_symbol,
                    type_reference,
                } => ValidatedBoundaryOperatorApplicationArgument::Type {
                    binder_owner: operator.symbol,
                    binder_ordinal,
                    binder_symbol,
                    type_reference,
                },
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol,
                    declared_carrier,
                    value,
                } => ValidatedBoundaryOperatorApplicationArgument::Const {
                    binder_owner: operator.symbol,
                    binder_ordinal,
                    binder_symbol,
                    declared_carrier,
                    value,
                },
            }
        })
        .collect();
    ValidatedBoundaryOperatorApplication {
        site,
        requirement: operator.symbol,
        arguments,
    }
}

/// Validate and retain the first symbolic D29 demand shape. The mapping is
/// deliberately direct: an operator type binder may name one enclosing
/// machine type binder, while nested symbolic type construction and symbolic
/// const arguments remain unsupported.
pub fn validated_symbolic_boundary_operator_application(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    site: ValidatedBoundaryOperatorApplicationUseSite,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<ValidatedBoundaryOperatorApplication> {
    let bindings = typed_trees::operator::symbolic_operator_type_application_for_operands(
        program,
        machine,
        operator,
        operand_types,
    )?;
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, binding)| {
            Some(ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
                binder_owner: operator.symbol,
                binder_ordinal: u32::try_from(ordinal).ok()?,
                binder_symbol: binding.operator_binder_symbol,
                machine_owner: machine.symbol,
                machine_binder_ordinal: binding.machine_binder_ordinal,
                machine_binder_symbol: binding.machine_binder_symbol,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ValidatedBoundaryOperatorApplication {
        site,
        requirement: operator.symbol,
        arguments,
    })
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
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement: StatementHandle,
    call: &TableCall,
    applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(operator) = typed_trees::operator::declaration_by_symbol(program, call.target_symbol)
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
                crate::value_custody::places::declared_place_type(
                    program,
                    machine,
                    Some(state),
                    *argument,
                )
                .or_else(|| landed_integer_literal_type_reference(program, *argument))
            }),
    );
    match validate_named_operator_application(
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
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &TableCall,
) -> Option<TypeReferenceHandle> {
    if !call.receiver_symbol.is_valid()
        || !matches!(
            program.symbols.get(call.receiver_symbol).kind,
            symbols::SymbolKind::Field
                | symbols::SymbolKind::Local
                | symbols::SymbolKind::Parameter
        )
    {
        return None;
    }
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    crate::machine_calls::calls::declared_receiver_type_reference(
        program,
        machine,
        state,
        receiver.as_str(),
    )
}

pub fn validate_closed_operator_application(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
    application: &[ClosedOperatorApplicationArgument],
) -> Result<(), Diagnostic> {
    let operator_name = program.operator_path_string(operator.name);
    let parameters = program.operator_type_parameters(operator);
    if parameters.len() != application.len() {
        return Err(Diagnostic::error(format!(
            "closed operator application for `{operator_name}` does not match its declaration telescope"
        )));
    }
    for (parameter, argument) in parameters.iter().zip(application) {
        match (&parameter.kind, argument) {
            (
                TypeParameterKind::Type,
                ClosedOperatorApplicationArgument::Type {
                    binder_symbol,
                    type_reference,
                },
            ) if *binder_symbol == parameter.symbol && type_reference.is_valid() => {
                let bounds = crate::proof_contracts::properties::declared_property_requirements(
                    &parameter.bounds,
                );
                let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
                for property in bounds {
                    if crate::proof_contracts::properties::type_satisfies_declared_property(
                        program,
                        &[],
                        *type_reference,
                        property,
                    ) {
                        continue;
                    }
                    return Err(Diagnostic::error(format!(
                        "type parameter `{} [{}]` of operator `{operator_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                        parameter.name,
                        bound_labels.join(", "),
                        program.display_type_reference_with_constraints(*type_reference),
                    )));
                }
            }
            (
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference },
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol,
                    declared_carrier,
                    value,
                },
            ) if *binder_symbol == parameter.symbol && *declared_carrier == *type_reference => {
                crate::value_custody::type_references::validate_exact_const_identity(
                    program,
                    *declared_carrier,
                    value,
                )
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const parameter `{}` of operator `{operator_name}` has an invalid closed value: {reason}",
                        parameter.name
                    ))
                })?;
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "closed operator application for `{operator_name}` does not rejoin parameter `{}` by category and symbol",
                    parameter.name
                )));
            }
        }
    }
    Ok(())
}

fn static_argument_matches(
    program: &dyn crate::CheckingInput,
    supplied: &StaticMachineArgument,
    inferred: &ClosedOperatorApplicationArgument,
) -> bool {
    match inferred {
        ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
            static_type_argument_matches(program, supplied, *type_reference)
        }
        ClosedOperatorApplicationArgument::Const {
            declared_carrier,
            value,
            ..
        } => static_const_argument_identity(program, supplied, *declared_carrier)
            .is_some_and(|supplied| supplied == *value),
    }
}

fn static_const_argument_identity(
    program: &dyn crate::CheckingInput,
    supplied: &StaticMachineArgument,
    declared_carrier: TypeReferenceHandle,
) -> Option<CanonicalConstIdentity> {
    if supplied.application.is_some()
        || supplied.evidence_projection.is_some()
        || supplied.symbol.is_valid()
        || !supplied.path.is_empty()
    {
        return None;
    }
    let literal = supplied.const_literal.as_ref()?;
    let value = literal
        .value_i64()
        .map(i128::from)
        .or_else(|| literal.value_u64().map(i128::from))?;
    let primitive = program
        .type_reference_table
        .primitive_type(declared_carrier)?;
    Some(CanonicalConstIdentity::integer(primitive.name(), value))
}

fn static_type_argument_matches(
    program: &dyn crate::CheckingInput,
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
                && application.arguments.len() == arguments.len()
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
