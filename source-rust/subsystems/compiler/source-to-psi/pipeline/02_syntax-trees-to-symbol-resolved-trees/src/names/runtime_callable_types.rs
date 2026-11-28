//! Give structural callable contracts their own formals before resolving uses.

use super::scopes::{DeclarationTypeScopes, declaration_type_scope, evidence_type_parameters};
use super::top_level::{SignatureSymbolTables, assign_callable_signature_symbols};
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::callable_values::RuntimeCallableContractHandle;
use crate::source_resolved::data::{TypeParameter, TypeParameterKind};
use crate::source_resolved::types::{TypeReference, TypeReferenceArena};
use crate::source_symbols::SymbolTable;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};

/// A structural callable can contain an operand introduced by an opening at
/// its use. Retain only symbols visible at that occurrence. Alias-owned names
/// are still resolved by the definition-site filter in assign_signatures.
#[derive(Default)]
pub(super) struct CallableOpenings(
    Vec<Option<(RuntimeCallableContractHandle, Vec<TypeParameter>)>>,
);

impl CallableOpenings {
    pub(super) fn of(
        program: &SymbolResolvedTrees,
        contracts: &[RuntimeCallableContractHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let types = &program.tables.declarations.child_type_references;
        let mut captured = Self::default();
        if contracts.is_empty() || types.package_openings.is_empty() {
            return Ok(captured);
        }
        let declarations = &program.tables.declarations.runtime_callable_contracts;
        super::repeated_locals::inspect_bindings(program, &mut |reference, visible| {
            visit_callables(types, reference, &mut |contract| {
                if visible.is_empty() || !declarations.is_valid(contract) {
                    return;
                }
                let parameters = visible
                    .iter()
                    .map(|binding| TypeParameter {
                        symbol: binding.symbol,
                        name: binding.name.clone(),
                        kind: TypeParameterKind::SomeType,
                        bounds: Default::default(),
                    })
                    .collect::<Vec<_>>();
                let index = contract.arena_index() as usize;
                if captured.0.len() <= index {
                    captured.0.resize_with(index + 1, || None);
                }
                captured.0[index] = Some((contract, parameters));
            });
        })?;
        Ok(captured)
    }

    fn parameters(&self, contract: RuntimeCallableContractHandle) -> Option<&[TypeParameter]> {
        let (stored, parameters) = self.0.get(contract.arena_index() as usize)?.as_ref()?;
        (*stored == contract).then_some(parameters.as_slice())
    }
}

fn visit_callables(
    types: &TypeReferenceArena,
    reference: &TypeReference,
    visit: &mut impl FnMut(RuntimeCallableContractHandle),
) {
    match reference {
        TypeReference::Callable { contract, .. } => visit(*contract),
        TypeReference::AliasUse {
            expansion,
            application,
            ..
        } => {
            visit_callables(types, expansion, visit);
            visit_callables(types, application, visit);
        }
        TypeReference::Reference(reference) => {
            visit_callables(types, types.get(reference.referee), visit)
        }
        TypeReference::FixedArray(array) => {
            visit_callables(types, types.get(array.element_type), visit)
        }
        TypeReference::Slice(slice) => visit_callables(types, types.get(slice.element_type), visit),
        TypeReference::Constrained(constrained) => {
            visit_callables(types, types.get(constrained.base_type), visit)
        }
        TypeReference::Generic(reference) => {
            for argument in types.span_or_empty(reference.arguments) {
                visit_callables(types, argument, visit);
            }
        }
        TypeReference::Named { .. } | TypeReference::Unit | TypeReference::ConstExpression(_) => {}
    }
}

pub(super) fn assign_signatures(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    contracts: &[RuntimeCallableContractHandle],
    openings: &CallableOpenings,
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
        let inherited = callable_parameters(program, &captured, openings, handle)
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

pub(super) fn callable_parameters(
    program: &SymbolResolvedTrees,
    captured: &DeclarationTypeScopes,
    openings: &CallableOpenings,
    handle: RuntimeCallableContractHandle,
) -> Result<Vec<TypeParameter>, Diagnostic> {
    let owner = program
        .tables
        .declarations
        .runtime_callable_contracts
        .get(handle)
        .lexical_owner;
    let parameters = openings.parameters(handle).unwrap_or_default().to_vec();
    owner_parameters(program, captured, openings, owner, parameters)
}

pub(super) fn owner_parameters(
    program: &SymbolResolvedTrees,
    captured: &DeclarationTypeScopes,
    openings: &CallableOpenings,
    mut owner: SymbolHandle,
    mut parameters: Vec<TypeParameter>,
) -> Result<Vec<TypeParameter>, Diagnostic> {
    let mut entered = Vec::new();
    while owner.is_valid() && owner != program.symbols.root() {
        if entered.contains(&owner) {
            return Err(Diagnostic::error(
                "runtime callable lexical scopes contain a cycle",
            ));
        }
        entered.push(owner);
        if let Some((enclosing, declaration)) = program
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
            if let Some(opened) = openings.parameters(enclosing) {
                parameters = declaration_type_scope(&parameters, opened);
            }
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
            if let Some(data) = program.data_definition_by_symbol(machine.attached_data_symbol) {
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
                use crate::source_resolved::proposition::PropositionBinderKind;
                TypeParameter {
                    symbol: binder.symbol,
                    name: binder.name.clone(),
                    bounds: binder.bounds,
                    kind: match &binder.kind {
                        PropositionBinderKind::Type => TypeParameterKind::Type,
                        PropositionBinderKind::Const { type_reference } => TypeParameterKind::Const { type_reference: type_reference.clone() },
                        PropositionBinderKind::Machine => TypeParameterKind::Machine { contract: crate::source_resolved::data::MachineParameterContract::RequirementIdentity },
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
            && let Some(signature) = parameter.kind.contract_signature()
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
