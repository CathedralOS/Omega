//! Requirement correspondence uses the same simultaneous application identity
//! as ordinary type substitution. The selected conformance's published
//! commitment remains separate; no display spelling is an equality witness.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameter;
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity, applied_type_identity,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
pub(super) struct ApplicationScope {
    declaration: SymbolHandle,
    arguments: Vec<(SymbolHandle, TypeReferenceHandle, bool)>,
    lifetimes: Vec<(String, Option<String>)>,
}

impl ApplicationScope {
    pub(super) fn new(
        declaration: SymbolHandle,
        parameters: &[TypeParameter],
        arguments: &[TypeReferenceHandle],
        lifetimes: Vec<(String, String)>,
    ) -> Option<Self> {
        if parameters.len() != arguments.len() {
            return None;
        }
        Some(Self {
            declaration,
            arguments: parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.symbol,
                        *argument,
                        parameter.kind.is_const_or_value(),
                    )
                })
                .collect(),
            lifetimes: lifetimes
                .into_iter()
                .map(|(parameter, argument)| (parameter, Some(argument)))
                .collect(),
        })
    }
}

pub(super) struct ApplicationArguments {
    pub(super) declaration: SymbolHandle,
    pub(super) arguments: Vec<TypeReferenceHandle>,
    pub(super) lifetimes: Vec<String>,
    pub(super) scopes: Vec<ApplicationScope>,
}

pub(super) fn matches_declaring_application(
    program: &mut TypedTrees,
    lexical_owner: SymbolHandle,
    declaring_trait: SymbolHandle,
    supplied: &[TypeReferenceHandle],
    initial: ApplicationArguments,
) -> Option<bool> {
    let supplied = argument_identities(program, lexical_owner, declaring_trait, supplied, &[])?;
    let mut pending = vec![(initial, Vec::new())];
    let mut matched = false;
    while let Some((application, mut ancestors)) = pending.pop() {
        if application.declaration == declaring_trait {
            let expected = argument_identities(
                program,
                lexical_owner,
                declaring_trait,
                &application.arguments,
                &application.scopes,
            )?;
            if expected != supplied {
                return Some(false);
            }
            matched = true;
            continue;
        }
        if ancestors.contains(&application.declaration) {
            continue;
        }
        ancestors.push(application.declaration);
        let definition = program.trait_definition_by_symbol(application.declaration)?;
        if definition.lifetime_parameters.len() != application.lifetimes.len() {
            return None;
        }
        let scope = ApplicationScope::new(
            definition.symbol,
            program.trait_type_parameters(definition),
            &application.arguments,
            definition
                .lifetime_parameters
                .iter()
                .map(|parameter| parameter.as_str().to_owned())
                .zip(application.lifetimes)
                .collect(),
        )?;
        let mut scopes = application.scopes;
        scopes.push(scope);
        for parent in program
            .trait_requirements(definition)
            .iter()
            .chain(definition.refines.iter())
        {
            pending.push((
                ApplicationArguments {
                    declaration: parent.symbol,
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(parent.arguments)
                        .to_vec(),
                    lifetimes: parent
                        .lifetime_arguments
                        .iter()
                        .map(|argument| argument.as_str().to_owned())
                        .collect(),
                    scopes: scopes.clone(),
                },
                ancestors.clone(),
            ));
        }
    }
    matched.then_some(true)
}

fn argument_identities(
    program: &mut TypedTrees,
    lexical_owner: SymbolHandle,
    declaration: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    scopes: &[ApplicationScope],
) -> Option<Vec<NormalizedTypeIdentity>> {
    let definition = program.trait_definition_by_symbol(declaration)?;
    let parameters = program.trait_type_parameters(definition).to_vec();
    let final_scope = ApplicationScope::new(declaration, &parameters, arguments, Vec::new())?;
    let owned_scopes = scopes
        .iter()
        .chain(std::iter::once(&final_scope))
        .collect::<Vec<_>>();
    let operands = owned_scopes
        .iter()
        .map(|scope| {
            scope
                .arguments
                .iter()
                .map(|(symbol, reference, value)| {
                    (
                        *symbol,
                        if *value {
                            IndexIdentityArgument::TypeReference(*reference)
                        } else {
                            IndexIdentityArgument::TypeArgument(*reference)
                        },
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let scopes = owned_scopes
        .iter()
        .zip(&operands)
        .map(|(scope, arguments)| AppliedTypeIdentityScope {
            region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
            declaration: scope.declaration,
            arguments,
            lifetimes: &scope.lifetimes,
        })
        .collect::<Vec<_>>();
    parameters
        .iter()
        .map(|parameter| {
            // A view of the existing binder selects its declared operand role;
            // this creates no declaration or inferred application argument.
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter.symbol,
                    name: parameter.name.clone(),
                });
            applied_type_identity(
                program,
                reference,
                typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(lexical_owner),
                &scopes,
                typed_trees::type_identity::TypeIdentityQualification::Ordinary,
            )
        })
        .collect()
}
