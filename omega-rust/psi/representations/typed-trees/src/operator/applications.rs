use language_semantics::const_value::CanonicalConstIdentity;
use symbols::{SymbolHandle, SymbolKind};

use crate::TypedTrees;
use crate::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{
    OperatorConstBinding, OperatorDefinition, declared_domain_constraints_match,
    normalized_operand_parameters, type_reference_matches_with_policy,
};

/// One declaration-ordered, closed operator telescope argument inferred from
/// the exact operand tuple. Const identity deliberately excludes display text;
/// the independently retained declared carrier is rechecked by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedOperatorApplicationArgument {
    Type {
        binder_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
    Const {
        binder_symbol: SymbolHandle,
        declared_carrier: TypeReferenceHandle,
        value: CanonicalConstIdentity,
    },
}

/// One direct mapping from a boundary operator's type telescope into the
/// enclosing generic machine's type telescope. This is an open demand, not a
/// closed application or realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicOperatorTypeApplicationArgument {
    pub operator_binder_symbol: SymbolHandle,
    pub machine_binder_symbol: SymbolHandle,
    pub machine_binder_ordinal: u32,
}

/// One concrete checked-body realization of an exact operator requirement.
///
/// The requirement owns the binder telescope. Argument order therefore
/// retains declaration ordinal, while each argument retains the exact binder
/// symbol/category and structural type or canonical const custody needed for
/// independent replay after generic specialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedOperatorRealizationApplication {
    pub requirement_symbol: SymbolHandle,
    pub arguments: Vec<ClosedOperatorApplicationArgument>,
}

/// Derive one complete closed type/const application for an operator use from
/// the same operand unification used by spelling resolution. Lifetime,
/// machine, and proposition binders remain fail-closed until their exact
/// category-specific identities exist.
pub fn closed_operator_application_for_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<Vec<ClosedOperatorApplicationArgument>> {
    closed_application_for_operands(program, operator, operand_types, false)
}

/// Reconstruct the same shared collection adaptation used by indexed spelling
/// selection, retaining the exact closed element binder for boundary demands.
pub fn closed_indexed_operator_application_for_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<Vec<ClosedOperatorApplicationArgument>> {
    if !matches!(
        operator.spelling,
        Some(super::OperatorSpelling::Index | super::OperatorSpelling::Range)
    ) {
        return None;
    }
    closed_application_for_operands(program, operator, operand_types, true)
}

fn closed_application_for_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
    indexed_collection: bool,
) -> Option<Vec<ClosedOperatorApplicationArgument>> {
    if !operator.lifetime_parameters.is_empty() {
        return None;
    }
    let type_parameters = program.operator_type_parameters(operator);
    if type_parameters.iter().any(|parameter| {
        !matches!(
            parameter.kind,
            crate::data::TypeParameterKind::Type | crate::data::TypeParameterKind::Const { .. }
        )
    }) {
        return None;
    }
    if type_parameters.is_empty() {
        return Some(Vec::new());
    }
    let parameters = program.operator_parameters(operator);
    if parameters.len() != operand_types.len() {
        return None;
    }
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    let matches = operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .enumerate()
        .all(|(position, (actual, expected))| {
            actual.is_none_or(|actual| {
                let (matched_actual, matched_expected) = if indexed_collection && position == 0 {
                    super::indexing::shared_collection_elements(
                        program,
                        actual,
                        expected.type_reference,
                    )
                    .unwrap_or((actual, expected.type_reference))
                } else {
                    (actual, expected.type_reference)
                };
                type_reference_matches_with_policy(
                    program,
                    matched_actual,
                    matched_expected,
                    None,
                    type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                    false,
                ) && declared_domain_constraints_match(program, actual, expected.type_reference)
                    && declared_domain_constraints_match(program, matched_actual, matched_expected)
            })
        });
    if !matches {
        return None;
    }
    let application = type_parameters
        .iter()
        .map(|parameter| match parameter.kind {
            crate::data::TypeParameterKind::Type => bindings
                .iter()
                .find_map(|(symbol, argument)| {
                    (*symbol == parameter.symbol).then_some(
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol: parameter.symbol,
                            type_reference: *argument,
                        },
                    )
                })
                .filter(|argument| match argument {
                    ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                        closed_boundary_application_type(program, *type_reference)
                    }
                    ClosedOperatorApplicationArgument::Const { .. } => false,
                }),
            crate::data::TypeParameterKind::Const { type_reference } => const_bindings
                .iter()
                .find_map(|OperatorConstBinding { symbol, value }| {
                    (*symbol == parameter.symbol).then_some(
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol: parameter.symbol,
                            declared_carrier: type_reference,
                            value: value.clone(),
                        },
                    )
                })
                .filter(|_| closed_boundary_application_type(program, type_reference)),
            crate::data::TypeParameterKind::Machine { .. }
            | crate::data::TypeParameterKind::Proposition { .. } => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(application)
}

/// Derive the first supported symbolic D29 application shape: every operator
/// type binder maps directly to one type binder on the enclosing generic
/// machine. Nested type construction and symbolic const expressions remain
/// unsupported so this fact cannot overstate what final substitution can
/// close and independently validate.
pub fn symbolic_operator_type_application_for_operands(
    program: &TypedTrees,
    machine: &crate::machine::Machine,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<Vec<SymbolicOperatorTypeApplicationArgument>> {
    if !operator.lifetime_parameters.is_empty() {
        return None;
    }
    let operator_parameters = program.operator_type_parameters(operator);
    if operator_parameters.is_empty()
        || operator_parameters
            .iter()
            .any(|parameter| !matches!(parameter.kind, crate::data::TypeParameterKind::Type))
    {
        return None;
    }
    let parameters = program.operator_parameters(operator);
    if parameters.len() != operand_types.len() {
        return None;
    }

    let mut type_bindings = Vec::new();
    let mut const_bindings = Vec::new();
    if !operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .all(|(actual, expected)| {
            actual.is_some_and(|actual| {
                type_reference_matches_with_policy(
                    program,
                    actual,
                    expected.type_reference,
                    None,
                    operator_parameters,
                    &mut type_bindings,
                    &mut const_bindings,
                    false,
                ) && declared_domain_constraints_match(program, actual, expected.type_reference)
            })
        })
        || !const_bindings.is_empty()
    {
        return None;
    }

    let machine_parameters = program.machine_type_parameters(machine);
    operator_parameters
        .iter()
        .map(|operator_parameter| {
            let type_reference = type_bindings.iter().find_map(|(symbol, argument)| {
                (*symbol == operator_parameter.symbol).then_some(*argument)
            })?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            let (machine_binder_ordinal, machine_parameter) = machine_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == *symbol)?;
            if !matches!(machine_parameter.kind, crate::data::TypeParameterKind::Type) {
                return None;
            }
            Some(SymbolicOperatorTypeApplicationArgument {
                operator_binder_symbol: operator_parameter.symbol,
                machine_binder_symbol: machine_parameter.symbol,
                machine_binder_ordinal: u32::try_from(machine_binder_ordinal).ok()?,
            })
        })
        .collect()
}

/// Reconstruct one specialized machine's exact closed realization of an
/// operator requirement from its concrete entry signature.
///
/// Operand types derive the application; the result may corroborate those
/// bindings but never fill a return-only binder. Parameter modes and the
/// substituted result are checked here so retaining this row cannot turn a
/// merely same-arity machine into a realization.
pub fn closed_operator_realization_application(
    program: &TypedTrees,
    machine: &crate::machine::Machine,
    operator: &OperatorDefinition,
) -> Option<ClosedOperatorRealizationApplication> {
    let state = program.machine_states(machine).first()?;
    let actual_parameters = program.state_parameters(state);
    let required_parameters = program.operator_parameters(operator);
    if actual_parameters.len() != required_parameters.len()
        || actual_parameters
            .iter()
            .zip(required_parameters)
            .any(|(actual, required)| {
                actual.is_self != required.is_self
                    || actual.is_const != required.is_const
                    || actual.is_mutable != required.is_mutable
            })
    {
        return None;
    }

    let operand_types = actual_parameters
        .iter()
        .map(|parameter| Some(parameter.type_reference))
        .collect::<Vec<_>>();
    let arguments = closed_operator_application_for_operands(program, operator, &operand_types)?;

    if state.return_type.is_valid() != operator.return_type.is_valid() {
        return None;
    }
    if state.return_type.is_valid() {
        let mut type_bindings = arguments
            .iter()
            .filter_map(|argument| match argument {
                ClosedOperatorApplicationArgument::Type {
                    binder_symbol,
                    type_reference,
                } => Some((*binder_symbol, *type_reference)),
                ClosedOperatorApplicationArgument::Const { .. } => None,
            })
            .collect::<Vec<_>>();
        let mut const_bindings = arguments
            .iter()
            .filter_map(|argument| match argument {
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol,
                    value,
                    ..
                } => Some(OperatorConstBinding {
                    symbol: *binder_symbol,
                    value: value.clone(),
                }),
                ClosedOperatorApplicationArgument::Type { .. } => None,
            })
            .collect::<Vec<_>>();
        if !type_reference_matches_with_policy(
            program,
            state.return_type,
            operator.return_type,
            None,
            program.operator_type_parameters(operator),
            &mut type_bindings,
            &mut const_bindings,
            false,
        ) || !declared_domain_constraints_match(program, state.return_type, operator.return_type)
        {
            return None;
        }
    }

    Some(ClosedOperatorRealizationApplication {
        requirement_symbol: operator.symbol,
        arguments,
    })
}

fn closed_boundary_application_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => lifetime.is_none() && closed_boundary_application_type(program, *referee),
        // Constraint expressions and declared-domain arguments need their own
        // exact closedness replay. The first D29 cohort does not erase them.
        TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. } => false,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => closed_boundary_application_type(program, *element_type),
        TypeReferenceNode::FixedArray { .. } => false,
        TypeReferenceNode::Slice { element_type } => {
            closed_boundary_application_type(program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            lifetime_arguments.is_empty()
                && closed_boundary_application_nominal(program, *base_symbol, base_name.as_str())
                && program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|argument| closed_boundary_application_type(program, *argument))
        }
        TypeReferenceNode::Named { symbol, name } => {
            closed_boundary_application_nominal(program, *symbol, name.as_str())
        }
        TypeReferenceNode::Unit => true,
    }
}

fn closed_boundary_application_nominal(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> bool {
    if PrimitiveType::from_name(name).is_some()
        || language_semantics::const_value::CanonicalConstValue::from_atom(name).is_some()
        || name.parse::<i128>().is_ok()
    {
        return true;
    }
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinType | SymbolKind::Data
        )
}
