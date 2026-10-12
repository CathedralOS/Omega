//! Admit authored type and const operands at the immutable checking boundary.
//!
//! Executable specialization may erase operands and constant folding may erase
//! a type root. Retained applications keep source declarations, payloads and
//! lexical owners; this pass judges those inputs, not clone names or canonical
//! strings. Tuple derivation shares carrier readers so it cannot erase a named
//! const's carrier or a literal's earlier landing.

use diagnostics::Diagnostic;
use numerics::literals::LandedIntegerType;
use std::borrow::Borrow;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use typed_trees::references::StaticArgumentCall;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

mod requirement_predicates;

pub fn validate_authored_static_arguments(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
    let scopes = requirement_predicates::CallScopes::build(program);
    for (handle, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(_) = expression {
            // Executable provider rewriting can retain a different application
            // beside the original source occurrence. Static predicates and
            // operand admission belong to that original complete tuple, just
            // like Requires and argument consent. The physical call remains
            // available through this same occurrence for applied header rejoin.
            let Some(original) = program.expression_table.source_call_node(handle) else {
                diagnostics.push(Diagnostic::error(
                    "static predicate admission lost its original call occurrence",
                ));
                continue;
            };
            validate_call(
                program,
                StaticArgumentCall::Expression(handle, original),
                scopes.context(handle),
                &symbols,
                &mut diagnostics,
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    validate_call(
                        program,
                        StaticArgumentCall::Statement(call),
                        Some((machine, state)),
                        &symbols,
                        &mut diagnostics,
                    );
                }
            }
        }
    }
    // The declaration owns a complete family, including tuples no source call
    // uses. Preparation can derive these rows but cannot admit their operands
    // or substituted predicates on the checker's behalf.
    for application in program.closed_requirement_applications() {
        let source = program
            .expression_table
            .static_machine_application(application.application);
        let arguments = program
            .expression_table
            .static_machine_application_arguments(application.application);
        let Some((owner, requirement)) = program.trait_machine_signature(application.template)
        else {
            diagnostics.push(Diagnostic::error(
                "closed requirement lost its authored declaration",
            ));
            continue;
        };
        // The application receipt owns its source route. Rebuilding only the
        // materialized argument tuple here loses enclosing and inherited
        // simultaneous frames, including captured regions in phantom operands.
        let scope = application
            .trait_scope
            .scope(program)
            .filter(|scope| scope.declaration.symbol == owner.symbol);
        let Some(signature) = scope
            .as_ref()
            .and_then(|scope| program.applied_callable_signature(scope, application.template))
        else {
            diagnostics.push(Diagnostic::error(
                "closed requirement lost its complete applied source telescope",
            ));
            continue;
        };
        if let Err(error) = validate_arguments(
            program,
            requirement.name.as_str(),
            program.state_signature_type_parameters(signature),
            arguments,
        ) {
            diagnostics.push(error);
        }
        requirement_predicates::validate_where_facts(
            program,
            owner,
            signature,
            arguments,
            source.lexical_owner,
            None,
            &mut diagnostics,
        );
    }
    // Source-supply demands retain their issuing owner and original occurrence
    // even when no executable call node exists. Predicate membership consumes
    // that source owner, never the later clone's substituted truth or handles.
    for (_, produced) in program.typed_program().instantiated_static_applications() {
        let source = produced.demand.owner.as_ref();
        if source
            .machine_by_symbol(produced.demand.declaration)
            .is_none()
        {
            continue;
        }
        let application = source
            .expression_table
            .static_machine_application(produced.demand.application);
        requirement_predicates::validate_machine_predicate_application(
            source,
            produced.demand.application,
            produced.demand.declaration,
            application.lexical_owner,
            None,
            &[],
            &mut diagnostics,
        );
    }
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(diagnostics)
    } else {
        Ok(())
    }
}

/// A static application names the authored callable telescope, whether its
/// declaration owns a body or is a trait requirement signature. Executable
/// instance identity cannot substitute for that source admission boundary.
fn source_telescope<'program>(
    program: &'program dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Result<Option<(&'program str, Vec<&'program TypeParameter>)>, &'static str> {
    let application = call.application();
    if application.is_valid()
        && !program
            .expression_table
            .static_machine_application_is_valid(application)
    {
        return Err("source static operands lost their original application");
    }
    let target = call.registration_operation(program);
    let name = if let Some(machine) = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
    {
        machine.name.as_str()
    } else if let Some((_, requirement)) = program.trait_machine_signature(target) {
        requirement.name.as_str()
    } else {
        return Ok(None);
    };
    let original = application.is_valid().then(|| {
        program
            .expression_table
            .static_machine_application(application)
    });
    let declaration =
        typed_trees::static_applications::StaticApplicationDeclaration::for_selected_interface(
            program.typed_program(),
            target,
            original.map_or(SymbolHandle::invalid(), |original| {
                original.selected_interface
            }),
        )?;
    if original.is_some_and(|original| {
        original.binding_declaration.is_valid()
            && original.binding_declaration != declaration.symbol()
    }) {
        return Err("source static operands lost their original receiving declaration");
    }
    let slots = if original.is_some() {
        program
            .expression_table
            .static_machine_application_argument_parameters(application)
    } else {
        &[]
    };
    if original.is_some_and(|original| original.argument_parameters.len() != slots.len()) {
        return Err("source static operands lost their receiving slot storage");
    }
    if slots.is_empty() {
        return Ok(Some((name, declaration.parameters().to_vec())));
    }
    if slots.len() != call.arguments(program).len() {
        return Err("source static operands lost their complete receiving slot correspondence");
    }
    let mut parameters = Vec::with_capacity(slots.len());
    let mut previous = None;
    for slot in slots {
        let (ordinal, parameter) = declaration
            .parameters()
            .iter()
            .enumerate()
            .find(|(_, parameter)| slot.is_valid() && parameter.symbol == *slot)
            .ok_or("source static operand does not belong to its original receiving telescope")?;
        if previous.is_some_and(|previous| previous >= ordinal) {
            return Err("source static operands repeat or reorder an original receiving binder");
        }
        previous = Some(ordinal);
        parameters.push(*parameter);
    }
    Ok(Some((name, parameters)))
}

fn validate_call(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
    context: Option<(&typed_trees::machine::Machine, &typed_trees::state::State)>,
    symbols: &crate::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Err(diagnostic) =
        crate::compiler_operations::validate_declaration_operands(program, call)
    {
        diagnostics.push(diagnostic);
        return;
    }
    if call.application().is_valid()
        && !program
            .expression_table
            .static_machine_application_is_valid(call.application())
    {
        diagnostics.push(Diagnostic::error(
            "source static operands lost their original application",
        ));
        return;
    }
    let arguments = call.arguments(program);
    requirement_predicates::validate(program, call, context, arguments, diagnostics);
    if arguments.is_empty() {
        return;
    }
    match source_telescope(program, call) {
        Ok(Some((name, parameters))) if !parameters.is_empty() => {
            if let Err(error) = validate_argument_telescope(program, name, &parameters, arguments) {
                diagnostics.push(error);
            }
        }
        Err(reason) => diagnostics.push(Diagnostic::error(reason)),
        _ => {}
    }
    // Owned declaration operands are admitted against their relocated payload
    // namespace by static_bindings. Reading them as Build type handles would
    // judge the wrong arena and the wrong dependency role.
    let owner = call.lexical_owner(program);
    let caller = program
        .machine_by_symbol(owner)
        .map(crate::StaticTypeArgumentOwner::Machine)
        .or_else(|| {
            program
                .data_definition_by_symbol(owner)
                .map(crate::StaticTypeArgumentOwner::Data)
        });
    for (index, argument) in arguments.iter().enumerate() {
        if program
            .product_declaration_application(call.application(), index)
            .is_none()
        {
            validate_types(
                program,
                caller,
                std::slice::from_ref(argument),
                symbols,
                diagnostics,
            );
        }
    }
}

/// Rejoin a const graph payload to the existing ordinary static value lane.
/// Missing original integer meaning remains unavailable; carrier/policy and
/// binder-kind admission still belong to the immutable telescope judgment.
pub fn normalized_const_operand(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<StaticMachineArgument> {
    if !argument.type_reference.is_valid()
        || !argument.path.is_empty()
        || argument.const_literal.is_some()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return None;
    }
    let mut operand = program
        .typed_program()
        .static_operand_from_type_reference(argument.type_reference)
        .ok()?;
    if operand.type_reference.is_valid()
        || (typed_trees::typed_trees::static_const_argument_spelling(
            program.typed_program(),
            &operand,
        )
        .is_none()
            && !forwarded_type(program, &operand).is_valid())
    {
        return None;
    }
    operand.authored_position = argument.authored_position;
    Some(operand)
}

pub fn validate_types(
    program: &dyn crate::CheckingInput,
    caller: Option<crate::StaticTypeArgumentOwner<'_>>,
    arguments: &[StaticMachineArgument],
    symbols: &crate::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        if argument.type_reference.is_valid()
            && normalized_const_operand(program, argument).is_none()
        {
            if let Some(caller) = caller {
                crate::validate_static_type_argument(
                    program,
                    caller,
                    argument.type_reference,
                    symbols,
                    diagnostics,
                );
            } else {
                diagnostics.push(Diagnostic::error(
                    "structural static type argument has no retained caller type/lifetime context",
                ));
            }
        }
        if let Some(application) = &argument.application {
            validate_types(
                program,
                caller,
                &application.arguments,
                symbols,
                diagnostics,
            );
        }
    }
}

pub fn spelling(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<String> {
    let projected = normalized_const_operand(program, argument);
    typed_trees::typed_trees::static_const_argument_spelling(
        program.typed_program(),
        projected.as_ref().unwrap_or(argument),
    )
}

pub fn forwarded_type(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> TypeReferenceHandle {
    if !argument.symbol.is_valid() {
        return TypeReferenceHandle::invalid();
    }
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_type_parameters(machine))
        .find_map(|parameter| match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }
                if parameter.symbol == argument.symbol =>
            {
                Some(type_reference)
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// The shape of one ordinary runtime subject admissible into a `Value`
/// binder: a single-segment name carrying no static payload. A spelled name
/// that resolved to no static declaration still qualifies; the caller's value
/// scope decides whether it denotes a subject (`resolve_runtime_subject`).
pub fn is_runtime_value_subject(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.evidence_projection.is_none()
        && argument.const_literal.is_none()
        && argument.path.len() == 1
        && (!argument.symbol.is_valid()
            || matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            ))
}

/// A structural Type operand has exactly one custody lane. Sharing this
/// judgment keeps conformance and machine telescopes from interpreting an
/// already-lowered applied Type as an absent named symbol.
pub(crate) fn validate_structural_type_operand(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Result<(), &'static str> {
    if !argument.type_reference.is_valid()
        || !program
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        || !argument.path.is_empty()
        || argument.symbol.is_valid()
        || argument.const_literal.is_some()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return Err("structural static type argument has invalid or mixed argument custody");
    }
    Ok(())
}

fn validate_arguments(
    program: &dyn crate::CheckingInput,
    name: &str,
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &[StaticMachineArgument],
) -> Result<(), Diagnostic> {
    validate_argument_telescope(program, name, parameters, arguments)
}

/// Authored slot category and const custody judgment shared by calls and
/// non-executing declaration applications. The declaration owns the telescope.
pub fn validate_argument_telescope<Parameter: Borrow<TypeParameter>>(
    program: &dyn crate::CheckingInput,
    name: &str,
    parameters: &[Parameter],
    arguments: &[StaticMachineArgument],
) -> Result<(), Diagnostic> {
    let has_types = parameters
        .iter()
        .any(|parameter| matches!(parameter.borrow().kind, TypeParameterKind::Type));
    let const_parameters: Vec<&TypeParameter> = parameters
        .iter()
        .map(Borrow::borrow)
        .filter(|parameter| parameter.kind.is_const_or_value())
        .collect::<Vec<_>>();
    let mut const_index = 0;
    for (ordinal, argument) in arguments.iter().enumerate() {
        let projected = normalized_const_operand(program, argument);
        if projected.is_some()
            && !parameters
                .get(ordinal)
                .is_some_and(|parameter| parameter.borrow().kind.is_const_or_value())
        {
            return Err(Diagnostic::error(format!(
                "machine `{}` static argument {} requires its declared binder kind, not a const graph argument",
                name,
                ordinal + 1,
            )));
        }
        let argument = projected.as_ref().unwrap_or(argument);
        // Proposals are grouped by kind after validation. Named types must
        // retain their authored slot just like structural types; otherwise
        // `<7, u8>` can silently become `<u8, 7>`. A const-only template keeps
        // its more specific "requires a const value" diagnostic below.
        let named_type = argument.symbol.is_valid()
            && matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::BuiltinType | SymbolKind::Data
            );
        if argument.type_reference.is_valid() || (named_type && has_types) {
            if !parameters
                .get(ordinal)
                .is_some_and(|parameter| matches!(parameter.borrow().kind, TypeParameterKind::Type))
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a type argument",
                    name,
                    ordinal + 1
                )));
            }
        }
        if argument.type_reference.is_valid() {
            validate_structural_type_operand(program, argument).map_err(Diagnostic::error)?;
            continue;
        }
        // An unused type binder is still a type, not an unsupplied data
        // constructor. Check the selected declaration before specialization
        // can erase this argument from the executable call.
        if named_type
            && argument.application.is_none()
            && program.symbols.get(argument.symbol).kind == SymbolKind::Data
            && let Some(definition) = program.data_definition_by_symbol(argument.symbol)
            && (!definition.type_parameters.is_empty()
                || !definition.lifetime_parameters.is_empty())
        {
            return Err(Diagnostic::error(format!(
                "static type argument `{}` selects generic data without its complete type/lifetime arguments",
                definition.name,
            )));
        }
        let declaration = program.const_declarations().iter().find(|declaration| {
            argument.symbol.is_valid() && declaration.symbol == argument.symbol
        });
        let forwarded = forwarded_type(program, argument);
        let forwarded_type = forwarded.is_valid().then_some(forwarded);
        if declaration.is_none()
            && forwarded_type.is_none()
            && spelling(program, argument).is_none()
        {
            let type_shaped = argument.type_reference.is_valid()
                || argument.symbol.is_valid()
                    && matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                    );
            if !const_parameters.is_empty() && !has_types && type_shaped {
                return Err(Diagnostic::error(format!(
                    "machine `{}` requires a const value, not type `{}`",
                    name,
                    argument.display_name()
                )));
            }
            // A `Value` binder admits static arguments through the ordinary
            // const specialization path and ordinary runtime subjects through
            // the dynamic realization path. Anything else spelled here is
            // neither.
            if const_parameters
                .get(const_index)
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
                && argument.application.is_none()
                && argument.evidence_projection.is_none()
                && !type_shaped
            {
                let parameter_name = &const_parameters[const_index].name;
                if is_runtime_value_subject(program, argument) {
                    const_index += 1;
                    continue;
                }
                return Err(Diagnostic::error(format!(
                    "value parameter `{parameter_name}` of machine `{}` received `{}`, which \
                     is neither a static const value nor an ordinary runtime value subject",
                    name,
                    argument.display_name()
                )));
            }
            continue;
        }
        let Some(parameter) = const_parameters.get(const_index) else {
            return Err(Diagnostic::error(format!(
                "machine `{}` has no const parameter for extra argument `{}`",
                name,
                argument.display_name()
            )));
        };
        let parameter_name = &parameter.name;
        let required = match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => type_reference,
            _ => unreachable!("const telescope contains only const/value binders"),
        };
        const_index += 1;
        if argument.type_reference.is_valid()
            || argument.application.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(Diagnostic::error(format!(
                "const argument `{}` must select a closed value or a const binder",
                argument.display_name()
            )));
        }
        if let Some(actual) = declaration
            .map(|declaration| declaration.declared_type)
            .or(forwarded_type)
            && program.normalized_type_identity(actual)
                != program.normalized_type_identity(*required)
        {
            return Err(Diagnostic::error(format!(
                "const parameter `{parameter_name}` of `{}` requires `{}`, but `{}` declares `{}`",
                name,
                program.display_type_reference(*required),
                argument.display_name(),
                program.display_type_reference(actual)
            )));
        }
        // Decimal specialization keys cannot erase a literal's earlier typed
        // landing. Only anonymous literals may land in the binder's carrier.
        if let Some(landing) = argument
            .const_literal
            .as_ref()
            .and_then(|literal| literal.landing())
        {
            let actual = match landing.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => PrimitiveType::Addr,
            };
            if program.type_reference_table.primitive_type(*required) != Some(actual) {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{}` requires `{}`, but its literal already landed as `{}`",
                    name,
                    program.display_type_reference(*required),
                    landing.landed_type.name(),
                )));
            }
            if landing.domain != program.type_reference_table.arithmetic_domain(*required) {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{name}` cannot establish its declared arithmetic policy from the original const value"
                )));
            }
        }
        if let Some(spelling) = spelling(program, argument)
            && let Some(value) =
                language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
            && matches!(
                value.decode_encoding(),
                Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { .. })
            )
        {
            let meaning_matches = match program
                .typed_program()
                .static_const_integer_meaning(argument)
            {
                typed_trees::types::ConstIntegerMeaning::Typed {
                    primitive,
                    arithmetic_domain,
                } => {
                    program.type_reference_table.primitive_type(*required) == Some(primitive)
                        && program.type_reference_table.arithmetic_domain(*required)
                            == arithmetic_domain
                }
                typed_trees::types::ConstIntegerMeaning::Anonymous => true,
                typed_trees::types::ConstIntegerMeaning::Unavailable => false,
            };
            if !meaning_matches {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{name}` cannot establish its declared arithmetic policy from the original const value"
                )));
            }
        }
        // Public declaration identity is broader than proof-static atom identity.
        // Decode through the ordinary specialization reader even for unused
        // binders, whose Unit calls may otherwise need no substitution.
        if declaration.is_some() && spelling(program, argument).is_none() {
            return Err(Diagnostic::error(format!(
                "const argument `{}` has no eligible canonical value",
                argument.display_name()
            )));
        }
    }
    Ok(())
}

/// Derive the complete explicitly supplied tuple without executing the selected
/// declaration. Binding rows remain derivation data until immutable admission.
pub fn derive_explicit_static_bindings(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::data::TypeParameter],
    conformance_bounds: &[typed_trees::machine::GenericConformanceBound],
    arguments: &[StaticMachineArgument],
) -> Option<Vec<typed_trees::static_applications::StaticMachineBinding>> {
    let bindings =
        derive_explicit_static_binding_prefix(program, parameters, conformance_bounds, arguments)?;
    let expected = parameters.len()
        + conformance_bounds
            .iter()
            .filter(|bound| {
                !parameters
                    .iter()
                    .any(|parameter| parameter.symbol == bound.binder)
            })
            .count();
    (bindings.len() == expected).then_some(bindings)
}

/// Derive only supplied original operands, without filling omitted binders.
/// Public-call inference can propose the missing operands from declared runtime
/// carriers; the complete tuple still owes ordinary category and bound admission.
pub fn derive_explicit_static_binding_prefix(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::data::TypeParameter],
    conformance_bounds: &[typed_trees::machine::GenericConformanceBound],
    arguments: &[StaticMachineArgument],
) -> Option<Vec<typed_trees::static_applications::StaticMachineBinding>> {
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    let evidence = conformance_bounds
        .iter()
        .filter(|bound| {
            !parameters
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
        })
        .collect::<Vec<_>>();
    if arguments.len() > parameters.len() + evidence.len() {
        return None;
    }
    let mut bindings = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let named = || {
            program
                .type_reference_table
                .find_named_type_reference(argument.symbol)
        };
        let value = match &parameter.kind {
            TypeParameterKind::Type => {
                let actual = if argument.type_reference.is_valid() {
                    argument.type_reference
                } else if argument.symbol.is_valid()
                    && argument.application.is_none()
                    && matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                    )
                {
                    named()?
                } else {
                    return None;
                };
                StaticMachineBindingValue::Type(actual)
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                let actual = if forwarded_type(program, argument).is_valid()
                    && argument.application.is_none()
                    && argument.evidence_projection.is_none()
                {
                    named()?
                } else {
                    let spelling = spelling(program, argument)?;
                    program
                        .type_reference_table
                        .named_references()
                        .find(|(_, symbol, name)| !symbol.is_valid() && *name == spelling)
                        .map(|(handle, _, _)| handle)?
                };
                StaticMachineBindingValue::Const(actual)
            }
            TypeParameterKind::Machine { .. } => {
                if !argument.symbol.is_valid()
                    || !matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::State | SymbolKind::Machine | SymbolKind::MachineParameter
                    )
                {
                    return None;
                }
                StaticMachineBindingValue::Machine(argument.clone())
            }
            TypeParameterKind::Implementation => {
                if !argument.symbol.is_valid()
                    || (program.conformance_by_symbol(argument.symbol).is_none()
                        && program.symbols.get(argument.symbol).kind
                            != SymbolKind::ConformanceParameter)
                {
                    return None;
                }
                StaticMachineBindingValue::Conformance(argument.clone())
            }
            TypeParameterKind::Proposition { .. } => return None,
        };
        bindings.push(StaticMachineBinding {
            parameter: parameter.symbol,
            value,
        });
    }
    for (bound, argument) in evidence
        .iter()
        .zip(arguments.get(parameters.len()..).unwrap_or(&[]))
    {
        if !argument.symbol.is_valid()
            || (program.conformance_by_symbol(argument.symbol).is_none()
                && program.symbols.get(argument.symbol).kind != SymbolKind::ConformanceParameter)
        {
            return None;
        }
        bindings.push(StaticMachineBinding {
            parameter: bound.binder,
            value: StaticMachineBindingValue::Conformance(argument.clone()),
        });
    }
    Some(bindings)
}
