use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::CanonicalConstIdentity;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameterKind;
use psi_typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use psi_typed_trees::operator::{ClosedOperatorApplicationArgument, OperatorDefinition};
use psi_typed_trees::statement::{StatementHandle, TableCall};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use crate::symbols::TopLevelSymbols;

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
/// realization retained by one concrete machine specialization.
///
/// Rows are a semantic set and sort by their complete canonical encoding. The
/// retained set must equal the operator `satisfies` edges on the concrete
/// machine, and every row must freshly reconstruct from its substituted entry
/// signature before any commitment consumer can use it.
pub fn canonical_closed_operator_realization_bytes(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    retained: &[psi_typed_trees::operator::ClosedOperatorRealizationApplication],
) -> Result<Vec<u8>, &'static str> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .ok_or("operator realization specialization lost its concrete machine")?;
    let expected = program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            psi_typed_trees::operator::declaration_by_symbol(
                program,
                conformance.requirement_symbol,
            )
            .map(|operator| (conformance.requirement_symbol, operator))
        })
        .collect::<Vec<_>>();
    if expected.len() != retained.len() {
        return Err("operator realization specialization has incomplete retained coverage");
    }

    let mut rows = Vec::with_capacity(retained.len());
    for (requirement_symbol, operator) in expected {
        let matching = retained
            .iter()
            .filter(|row| row.requirement_symbol == requirement_symbol)
            .collect::<Vec<_>>();
        let [row] = matching.as_slice() else {
            return Err("operator realization specialization has missing or duplicate exact rows");
        };
        let reconstructed = psi_typed_trees::operator::closed_operator_realization_application(
            program, machine, operator,
        )
        .ok_or("operator realization specialization no longer matches its concrete signature")?;
        if **row != reconstructed {
            return Err("operator realization specialization mismatches fresh reconstruction");
        }

        let parameters = program.operator_type_parameters(operator);
        if parameters.len() != row.arguments.len() {
            return Err("operator realization specialization mismatches its static telescope");
        }
        let mut bytes = Vec::new();
        encode_application_text(
            &operator_symbol_identity(program, requirement_symbol),
            &mut bytes,
        );
        bytes.extend((row.arguments.len() as u64).to_le_bytes());
        for (ordinal, (parameter, argument)) in parameters.iter().zip(&row.arguments).enumerate() {
            bytes.extend((ordinal as u64).to_le_bytes());
            match (&parameter.kind, argument) {
                (
                    TypeParameterKind::Type,
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol,
                        type_reference,
                    },
                ) if *binder_symbol == parameter.symbol && type_reference.is_valid() => {
                    bytes.push(0);
                    encode_application_text(
                        program
                            .package_qualified_type_identity(*type_reference)
                            .as_str(),
                        &mut bytes,
                    );
                }
                (
                    TypeParameterKind::Const {
                        type_reference: expected_carrier,
                    },
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol,
                        declared_carrier,
                        value,
                    },
                ) if *binder_symbol == parameter.symbol
                    && *declared_carrier == *expected_carrier =>
                {
                    crate::type_references::validate_exact_const_identity(
                        program,
                        *declared_carrier,
                        value,
                    )
                    .map_err(|_| {
                        "operator realization specialization has an invalid canonical const value"
                    })?;
                    bytes.push(1);
                    encode_application_text(
                        program
                            .package_qualified_type_identity(*declared_carrier)
                            .as_str(),
                        &mut bytes,
                    );
                    encode_application_text(&value.type_name, &mut bytes);
                    encode_application_text(&value.encoding, &mut bytes);
                }
                _ => {
                    return Err(
                        "operator realization specialization mismatches binder category or identity",
                    );
                }
            }
        }
        rows.push(bytes);
    }
    rows.sort();
    if rows.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("operator realization specialization has duplicate canonical rows");
    }
    let mut bytes = Vec::new();
    bytes.extend((rows.len() as u64).to_le_bytes());
    for row in rows {
        bytes.extend((row.len() as u64).to_le_bytes());
        bytes.extend(row);
    }
    Ok(bytes)
}

/// Compare one checked D29 use demand with one independently retained closed
/// realization by semantic identity rather than private type-table handles.
pub fn checked_operator_application_matches_realization(
    program: &TypedTrees,
    demand: &psi_checked_trees::CheckedBoundaryOperatorApplicationDemand,
    realization: &psi_typed_trees::operator::ClosedOperatorRealizationApplication,
) -> bool {
    demand.requirement_symbol == realization.requirement_symbol
        && demand.arguments.len() == realization.arguments.len()
        && demand
            .arguments
            .iter()
            .zip(&realization.arguments)
            .enumerate()
            .all(
                |(ordinal, (demand_argument, realization))| match (demand_argument, realization) {
                    (
                        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                            binder_owner,
                            binder_ordinal,
                            binder_symbol,
                            type_reference,
                        },
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol: realization_binder,
                            type_reference: realization_type,
                        },
                    ) => {
                        *binder_owner == demand.requirement_symbol
                            && usize::try_from(*binder_ordinal).ok() == Some(ordinal)
                            && *binder_symbol == *realization_binder
                            && program.package_qualified_type_identity(*type_reference)
                                == program.package_qualified_type_identity(*realization_type)
                    }
                    (
                        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                            binder_owner,
                            binder_ordinal,
                            binder_symbol,
                            declared_carrier,
                            value,
                        },
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol: realization_binder,
                            declared_carrier: realization_carrier,
                            value: realization_value,
                        },
                    ) => {
                        *binder_owner == demand.requirement_symbol
                            && usize::try_from(*binder_ordinal).ok() == Some(ordinal)
                            && *binder_symbol == *realization_binder
                            && program.package_qualified_type_identity(*declared_carrier)
                                == program.package_qualified_type_identity(*realization_carrier)
                            && value == realization_value
                    }
                    _ => false,
                },
            )
}

fn operator_symbol_identity(program: &TypedTrees, symbol: SymbolHandle) -> String {
    let path = program.symbols.display_path(symbol, "::");
    let Some(package) = program.symbols.symbol_package_identity(symbol) else {
        return format!("unmanaged::{path}");
    };
    let mut owner = String::with_capacity(package.digest().len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in package.digest() {
        owner.push(char::from(HEX[usize::from(byte >> 4)]));
        owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    format!("package:{owner}::{path}")
}

fn encode_application_text(value: &str, bytes: &mut Vec<u8>) {
    bytes.extend((value.len() as u64).to_le_bytes());
    bytes.extend(value.as_bytes());
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
}

/// Validate and close the currently supported named-operator static
/// application. `None` is a truthful unsupported/open demand, never coverage.
/// The returned bindings are ordered by the operator's declaration telescope.
pub fn validate_named_operator_application(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    operator: &OperatorDefinition,
    static_arguments: &[StaticMachineArgument],
    operand_types: &[Option<TypeReferenceHandle>],
) -> Result<Option<Vec<ClosedOperatorApplicationArgument>>, Diagnostic> {
    let operator_name = program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
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

    let Some(inferred) = psi_typed_trees::operator::closed_operator_application_for_operands(
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

    validate_closed_operator_application(program, symbols, operator, &inferred)?;

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
    symbols: &TopLevelSymbols<'_>,
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
    match validate_named_operator_application(
        program,
        symbols,
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

pub fn validate_closed_operator_application(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    operator: &OperatorDefinition,
    application: &[ClosedOperatorApplicationArgument],
) -> Result<(), Diagnostic> {
    let operator_name = program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
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
                let bounds = crate::properties::declared_property_requirements(&parameter.bounds);
                let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
                for property in bounds {
                    if crate::properties::type_satisfies_declared_property(
                        program,
                        symbols,
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
                TypeParameterKind::Const { type_reference },
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol,
                    declared_carrier,
                    value,
                },
            ) if *binder_symbol == parameter.symbol && *declared_carrier == *type_reference => {
                crate::type_references::validate_exact_const_identity(
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
    program: &TypedTrees,
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
    program: &TypedTrees,
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
