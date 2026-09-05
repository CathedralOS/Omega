use super::*;
use symbols::{Symbol, SymbolKind};
use typed_trees::data::{
    DataField, DataMember, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    symbol: SymbolHandle,
    declaration: Symbol,
    name: String,
    source_span: Option<source::SourceSpan>,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    parameters: Vec<StateParameter>,
    fields: Vec<DataField>,
    static_parameters: Vec<TypeParameter>,
    types: Vec<(SymbolHandle, TypeReferenceHandle)>,
    local_mutability: Vec<bool>,
}

pub(super) fn capture(
    builder: &mut Builder<'_>,
    symbol: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    let program = builder.program;
    let declaration = program.symbols.get(symbol);
    let mut symbol_identity = declaration.clone();
    // Later settlement can append sibling declarations under this scope.
    // Only the referenced symbol's identity is custody, not namespace extent.
    symbol_identity.children = arena::HandleSpan::empty();
    let mut snapshot = Snapshot {
        symbol,
        declaration: symbol_identity,
        name: program.symbols.name(symbol).to_owned(),
        source_span: program.symbols.symbol_source_span(symbol),
        package: program.symbols.symbol_package_identity(symbol),
        parameters: Vec::new(),
        fields: Vec::new(),
        static_parameters: Vec::new(),
        types: Vec::new(),
        local_mutability: Vec::new(),
    };
    // Symbol parent links carry scope identity, independently of display paths.
    builder.symbol(declaration.parent)?;
    builder.symbol(declaration.generated_from)?;
    match declaration.kind {
        SymbolKind::Parameter => {
            for (_, parameter) in program
                .state_parameters
                .iter()
                .filter(|(_, parameter)| parameter.symbol == symbol)
            {
                if !snapshot.parameters.is_empty() {
                    return Err(rejected("duplicate operand parameter binding"));
                }
                builder.charge(1)?;
                snapshot.parameters.push(parameter.clone());
            }
            if snapshot.parameters.len() != 1 {
                return Err(rejected(
                    "an operand parameter without one exact typed binding",
                ));
            }
        }
        SymbolKind::Local => {
            for (_, state) in program.machine_states.iter() {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let StatementNode::LocalData(local) = statement
                        && local.symbol == symbol
                    {
                        if !snapshot.types.is_empty() {
                            return Err(rejected("duplicate operand local binding"));
                        }
                        builder.charge(1)?;
                        snapshot.types.push((state.symbol, local.type_reference));
                        snapshot.local_mutability.push(local.is_mutable);
                    }
                }
            }
            if snapshot.types.len() != 1 {
                return Err(rejected("an operand local without one exact typed binding"));
            }
        }
        SymbolKind::Field => {
            for (_, member) in program.data_members.iter() {
                if let DataMember::Field(field) = member
                    && field.symbol == symbol
                {
                    builder.charge(1)?;
                    snapshot.fields.push(field.clone());
                }
            }
            for (_, field) in program
                .data_payload_fields
                .iter()
                .filter(|(_, field)| field.symbol == symbol)
            {
                builder.charge(1)?;
                snapshot.fields.push(field.clone());
            }
            for (_, field) in program
                .machine_owned_data
                .iter()
                .filter(|(_, field)| field.symbol == symbol)
            {
                builder.charge(1)?;
                snapshot.types.push((symbol, field.type_reference));
            }
            if snapshot.fields.is_empty() && snapshot.types.is_empty() {
                attached_field_alias(builder, symbol, &mut snapshot.fields)?;
            }
            if snapshot.fields.len() + snapshot.types.len() != 1 {
                return Err(rejected("an operand field without one exact typed binding"));
            }
        }
        SymbolKind::Machine | SymbolKind::State => {
            let mut found = 0usize;
            for machine in program.machines() {
                for (index, state) in program.machine_states(machine).iter().enumerate() {
                    if state.symbol == symbol || (machine.symbol == symbol && index == 0) {
                        found += 1;
                        if found > 1 {
                            return Err(rejected("duplicate exact called entry signature"));
                        }
                        builder.charge(
                            program
                                .state_parameters(state)
                                .len()
                                .saturating_add(program.machine_type_parameters(machine).len()),
                        )?;
                        snapshot
                            .parameters
                            .extend_from_slice(program.state_parameters(state));
                        snapshot
                            .static_parameters
                            .extend_from_slice(program.machine_type_parameters(machine));
                        snapshot.types.push((state.symbol, state.return_type));
                    }
                }
            }
            if found == 0 {
                for (_, signature) in program
                    .trait_machine_signatures
                    .iter()
                    .filter(|(_, signature)| signature.symbol == symbol)
                {
                    found += 1;
                    if found > 1 {
                        return Err(rejected("duplicate exact called requirement signature"));
                    }
                    builder.charge(
                        program
                            .state_signature_parameters(signature)
                            .len()
                            .saturating_add(
                                program.state_signature_type_parameters(signature).len(),
                            ),
                    )?;
                    snapshot
                        .parameters
                        .extend_from_slice(program.state_signature_parameters(signature));
                    snapshot
                        .static_parameters
                        .extend_from_slice(program.state_signature_type_parameters(signature));
                    snapshot
                        .types
                        .push((signature.symbol, signature.return_type));
                }
            }
            if found != 1 {
                return Err(rejected(
                    "a called symbol without one exact entry signature",
                ));
            }
        }
        SymbolKind::Operator => {
            let operator = typed_trees::operator::declaration_by_symbol(program, symbol)
                .ok_or_else(|| rejected("a called operator without its exact declaration"))?;
            builder.charge(
                program
                    .operator_parameters(operator)
                    .len()
                    .saturating_add(program.operator_type_parameters(operator).len()),
            )?;
            snapshot
                .parameters
                .extend_from_slice(program.operator_parameters(operator));
            snapshot
                .static_parameters
                .extend_from_slice(program.operator_type_parameters(operator));
            snapshot.types.push((operator.symbol, operator.return_type));
        }
        SymbolKind::Domain => {
            let definition = unique(
                program
                    .domain_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == symbol),
                "a domain operand without one exact carrier declaration",
            )?;
            snapshot.types.push((symbol, definition.target_type));
            builder.charge(
                definition
                    .index_arguments
                    .len()
                    .saturating_add(program.domain_type_parameters(definition).len()),
            )?;
            snapshot.types.extend(
                definition
                    .index_arguments
                    .iter()
                    .map(|reference| (symbol, *reference)),
            );
            snapshot
                .static_parameters
                .extend_from_slice(program.domain_type_parameters(definition));
        }
        SymbolKind::Const => {
            for (_, declaration) in program
                .const_declarations
                .iter()
                .filter(|(_, declaration)| declaration.symbol == symbol)
            {
                if !snapshot.types.is_empty() {
                    return Err(rejected("duplicate const operand declaration"));
                }
                builder.charge(1)?;
                snapshot.types.push((symbol, declaration.declared_type));
            }
            if snapshot.types.len() != 1 {
                return Err(rejected(
                    "a const operand without one exact typed declaration",
                ));
            }
        }
        SymbolKind::TypeParameter
        | SymbolKind::MachineParameter
        | SymbolKind::PropositionParameter => {
            for (_, parameter) in program
                .data_type_parameters
                .iter()
                .filter(|(_, parameter)| parameter.symbol == symbol)
            {
                if !snapshot.static_parameters.is_empty() {
                    return Err(rejected("duplicate static operand binder"));
                }
                builder.charge(1)?;
                snapshot.static_parameters.push(parameter.clone());
            }
            if snapshot.static_parameters.len() != 1 {
                return Err(rejected("a static operand without one exact typed binder"));
            }
        }
        SymbolKind::Data => {
            let definition = unique(
                program
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == symbol),
                "a nominal operand type without one exact definition",
            )?;
            builder.charge(program.data_type_parameters(definition).len())?;
            snapshot
                .static_parameters
                .extend_from_slice(program.data_type_parameters(definition));
            builder.charge(program.data_members(definition).len())?;
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => snapshot.fields.push(field.clone()),
                    DataMember::Variant(variant) => {
                        builder.charge(program.data_payload_fields(variant).len())?;
                        snapshot
                            .fields
                            .extend_from_slice(program.data_payload_fields(variant));
                    }
                }
            }
        }
        SymbolKind::Variant => {
            let mut found = 0;
            for (_, member) in program.data_members.iter() {
                if let DataMember::Variant(variant) = member
                    && variant.symbol == symbol
                {
                    found += 1;
                    builder.charge(program.data_payload_fields(variant).len())?;
                    snapshot
                        .fields
                        .extend_from_slice(program.data_payload_fields(variant));
                }
            }
            if found != 1 {
                return Err(rejected("an operand variant without one exact definition"));
            }
        }
        SymbolKind::Root
        | SymbolKind::Module
        | SymbolKind::BuiltinType
        | SymbolKind::BuiltinFunction
        | SymbolKind::Trait
        | SymbolKind::Conformance
        | SymbolKind::ConformanceParameter
        | SymbolKind::PropositionMachineParameter => {}
        _ => return Err(rejected("an unsupported symbol-backed operand category")),
    }
    builder.charge(
        snapshot
            .parameters
            .len()
            .saturating_add(snapshot.fields.len())
            .saturating_add(snapshot.static_parameters.len())
            .saturating_add(snapshot.types.len()),
    )?;
    for parameter in &snapshot.parameters {
        builder.type_reference(parameter.type_reference)?;
    }
    for field in &snapshot.fields {
        builder.type_reference(field.type_reference)?;
    }
    for (_, reference) in &snapshot.types {
        builder.type_reference(*reference)?;
    }
    for parameter in &snapshot.static_parameters {
        static_parameter(builder, parameter)?;
    }
    builder.result.bindings.push(snapshot);
    Ok(())
}

fn attached_field_alias(
    builder: &mut Builder<'_>,
    selected: SymbolHandle,
    fields: &mut Vec<DataField>,
) -> Result<(), Vec<Diagnostic>> {
    let program = builder.program;
    let symbols = &program.symbols;
    let parent = symbols.get(selected).parent;
    let machine = unique(
        program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == parent),
        "an attached field alias without its exact machine",
    )?;
    let Some(selected_span) = symbols
        .symbol_source_span(selected)
        .filter(|span| span.span.start < span.span.end)
    else {
        return Err(rejected(
            "an attached field alias without authored source custody",
        ));
    };
    let definition = unique(
        program.data_definitions().iter().filter(|definition| {
            definition.symbol == machine.attached_data_symbol && definition.symbol.is_valid()
        }),
        "an attached field alias without its exact carrier",
    )?;
    for member in program.data_members(definition) {
        if let DataMember::Field(field) = member
            && symbols.get(field.symbol).parent == definition.symbol
            && symbols.symbol_source_span(field.symbol) == Some(selected_span)
            && symbols.same_symbol_source_package(field.symbol, selected)
        {
            builder.charge(1)?;
            fields.push(field.clone());
            builder.symbol(field.symbol)?;
        }
    }
    Ok(())
}

fn static_parameter(
    builder: &mut Builder<'_>,
    parameter: &TypeParameter,
) -> Result<(), Vec<Diagnostic>> {
    match &parameter.kind {
        TypeParameterKind::Type => {}
        TypeParameterKind::Const { type_reference } => builder.type_reference(*type_reference)?,
        TypeParameterKind::Machine { contract } => match contract {
            MachineParameterContract::RequirementIdentity => {}
            MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            } => {
                builder.symbol(*trait_definition)?;
                builder.symbol(*requirement)?;
            }
            MachineParameterContract::Structural(signature) => {
                for parameter in builder.program.state_signature_parameters(signature) {
                    builder.symbol(parameter.symbol)?;
                    builder.type_reference(parameter.type_reference)?;
                }
                for parameter in builder.program.state_signature_type_parameters(signature) {
                    builder.symbol(parameter.symbol)?;
                }
                builder.type_reference(signature.return_type)?;
            }
        },
        TypeParameterKind::Proposition { contract } => {
            let parameters = builder
                .program
                .state_parameters
                .span_or_empty(contract.parameters);
            builder.charge(parameters.len())?;
            for parameter in parameters {
                builder.symbol(parameter.symbol)?;
                builder.type_reference(parameter.type_reference)?;
            }
        }
    }
    Ok(())
}

fn unique<T>(mut values: impl Iterator<Item = T>, reason: &str) -> Result<T, Vec<Diagnostic>> {
    match (values.next(), values.next()) {
        (Some(value), None) => Ok(value),
        _ => Err(rejected(reason)),
    }
}
