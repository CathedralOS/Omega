//! Give structural callable contracts their own formals before resolving uses.

use super::scope::{DeclarationTypeScopes, declaration_type_scope, evidence_type_parameters};
use super::top_level::{SignatureSymbolTables, assign_callable_signature_symbols};
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::callable_values::RuntimeCallableContractHandle;
use symbol_resolved_trees::data::{TypeParameter, TypeParameterKind};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

pub(super) fn assign_signatures(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    contracts: &[RuntimeCallableContractHandle],
) -> Result<(), Vec<Diagnostic>> {
    let (_, proposition_slots, machine_identity_slots) =
        super::top_level::trait_parameter_tables(program);
    let captured = DeclarationTypeScopes::of(program)?;
    for &handle in contracts {
        let mut declaration = program
            .tables
            .declarations
            .runtime_callable_contracts
            .get(handle)
            .clone();
        let inherited = lexical_parameters(program, &captured, declaration.lexical_owner)
            .map_err(|error| vec![error])?;
        let signature_symbol = declaration.signature.symbol;
        let tables = &mut program.tables;
        assign_callable_signature_symbols(
            &mut SignatureSymbolTables {
                symbols,
                data_type_parameters: &mut tables.declarations.data_type_parameters,
                state_parameters: &mut tables.declarations.state_parameters,
                child_type_references: &mut tables.declarations.child_type_references,
                type_constraints: &tables.types.constraints,
                trait_proposition_slots: &proposition_slots,
                trait_machine_identity_slots: &machine_identity_slots,
            },
            &mut declaration.signature,
            signature_symbol,
            &inherited,
        );
        // An expanded alias keeps the declaration source of its type names.
        // Resolve those names with the same filter as ordinary alias operands;
        // a caller's unrelated binder must not replace the alias's declaration.
        if !declaration.definition_references.is_empty() {
            let parameters = declaration_type_scope(
                tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(declaration.signature.type_parameters),
                &inherited,
            );
            for parameter in tables
                .declarations
                .state_parameters
                .span_mut_or_empty(declaration.signature.parameters)
            {
                super::type_references::assign_type_reference_with_definitions(
                    symbols,
                    &mut tables.declarations.child_type_references,
                    &tables.types.constraints,
                    &parameters,
                    &mut parameter.type_reference,
                    &declaration.definition_references,
                );
            }
            if let Some(result) = &mut declaration.signature.return_type {
                super::type_references::assign_type_reference_with_definitions(
                    symbols,
                    &mut tables.declarations.child_type_references,
                    &tables.types.constraints,
                    &parameters,
                    result,
                    &declaration.definition_references,
                );
            }
        }
        *tables
            .declarations
            .runtime_callable_contracts
            .get_mut(handle) = declaration;
    }
    Ok(())
}

pub(super) fn lexical_parameters(
    program: &SymbolResolvedTrees,
    captured: &DeclarationTypeScopes,
    mut owner: SymbolHandle,
) -> Result<Vec<TypeParameter>, Diagnostic> {
    let mut parameters = Vec::new();
    let mut entered = Vec::new();
    while owner.is_valid() && owner != program.symbols.root() {
        if entered.contains(&owner) {
            return Err(Diagnostic::error(
                "runtime callable lexical scopes contain a cycle",
            ));
        }
        entered.push(owner);
        if let Some((_, declaration)) = program
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .find(|(_, declaration)| declaration.signature.symbol == owner)
        {
            parameters = declaration_type_scope(
                &parameters,
                program.data_type_parameters(declaration.signature.type_parameters),
            );
            owner = declaration.lexical_owner;
            continue;
        }
        if let Some(machine) = program
            .machines
            .iter()
            .find(|machine| machine.symbol == owner)
        {
            let local = declaration_type_scope(
                program.machine_type_parameters(machine),
                captured.parameters(owner),
            );
            let evidence =
                evidence_type_parameters(&machine.conformance_bounds).collect::<Vec<_>>();
            let local = declaration_type_scope(&local, &evidence);
            parameters = declaration_type_scope(&parameters, &local);
            if let Some(data) = program
                .data_definitions
                .iter()
                .find(|data| data.symbol == machine.attached_data_symbol)
            {
                parameters = declaration_type_scope(
                    &parameters,
                    program.data_type_parameters(data.type_parameters),
                );
            }
            return Ok(parameters);
        }
        macro_rules! declaration_parameters {
            ($($roster:ident),* $(,)?) => {$(
                if let Some(declaration) = program.$roster.iter().find(|declaration| declaration.symbol == owner) {
                    return Ok(declaration_type_scope(
                        &parameters,
                        program.data_type_parameters(declaration.type_parameters),
                    ));
                }
            )*};
        }
        declaration_parameters!(
            data_definitions,
            type_aliases,
            domain_definitions,
            traits,
            conformances,
            operators
        );
        if let Some(definition) = program
            .mathematical_definitions
            .iter()
            .find(|definition| definition.symbol == owner)
        {
            return Ok(declaration_type_scope(
                &parameters,
                program.data_type_parameters(definition.binders),
            ));
        }
        if let Some(definition) = program
            .propositions
            .iter()
            .find(|definition| definition.symbol == owner)
        {
            let local = program.tables.declarations.proposition_binders.span_or_empty(definition.binders).iter().map(|binder| {
                use symbol_resolved_trees::proposition::PropositionBinderKind;
                TypeParameter {
                    symbol: binder.symbol,
                    name: binder.name.clone(),
                    bounds: binder.bounds,
                    kind: match &binder.kind {
                        PropositionBinderKind::Type => TypeParameterKind::Type,
                        PropositionBinderKind::Const { type_reference } => TypeParameterKind::Const { type_reference: type_reference.clone() },
                        PropositionBinderKind::Machine => TypeParameterKind::Machine { contract: symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity },
                    },
                }
            }).collect::<Vec<_>>();
            return Ok(declaration_type_scope(&parameters, &local));
        }
        if program
            .measures
            .iter()
            .any(|definition| definition.symbol == owner)
        {
            return Ok(parameters);
        }
        if let Some((_, signature)) = program
            .tables
            .declarations
            .trait_machine_signatures
            .iter()
            .find(|(_, signature)| signature.symbol == owner)
        {
            parameters = declaration_type_scope(
                &parameters,
                program.data_type_parameters(signature.type_parameters),
            );
        } else if let Some((_, parameter)) = program
            .tables
            .declarations
            .data_type_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == owner)
            && let TypeParameterKind::Machine { contract } = &parameter.kind
            && let Some(signature) = contract.structural()
        {
            parameters = declaration_type_scope(
                &parameters,
                program.data_type_parameters(signature.type_parameters),
            );
        } else if !matches!(
            program.symbols.get(owner).kind,
            SymbolKind::State | SymbolKind::Parameter
        ) {
            return Err(Diagnostic::error(
                "runtime callable lost its supplying type-parameter scope",
            ));
        }
        owner = program.symbols.get(owner).parent;
    }
    if !owner.is_valid() {
        return Err(Diagnostic::error(
            "runtime callable has no lexical declaration owner",
        ));
    }
    Ok(parameters)
}
