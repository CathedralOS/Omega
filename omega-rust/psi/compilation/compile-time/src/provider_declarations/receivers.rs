//! Portable applied receiver content shared by source choices and realization.
//!
//! Original Product operands own their graph and region issuer. Projection
//! retains that complete outer application and its original member roster;
//! member invocation tuples never supply receiver arguments. Omega interns the
//! same content in its own schema table, then compares content across issuers.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::trait_definition::AppliedTraitScope;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderSelectionReceiver {
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub declaration: String,
    pub arguments: Vec<String>,
    pub lifetimes: Vec<String>,
    pub requirements: Vec<String>,
}

/// Retain every exact ancestor, refusing an applied diamond whose original
/// declaration would acquire two different outer applications.
pub fn collect_scopes<'program>(
    program: &'program TypedTrees,
    root: AppliedTraitScope<'program>,
) -> Option<Vec<AppliedTraitScope<'program>>> {
    let mut scopes: Vec<AppliedTraitScope<'_>> = Vec::new();
    let mut ambiguous = false;
    root.visit(program, &mut |scope| {
        if let Some(existing) = scopes
            .iter()
            .find(|existing| existing.declaration.symbol == scope.declaration.symbol)
        {
            ambiguous |= !existing.same_application(program, scope);
        } else {
            scopes.push(scope.clone());
        }
    });
    (!ambiguous).then_some(scopes)
}

pub fn receiver_applications(
    program: &TypedTrees,
    scopes: &[AppliedTraitScope<'_>],
) -> Option<Vec<ProviderSelectionReceiver>> {
    let mut applications = Vec::new();
    for scope in scopes.iter().filter(|scope| scope.declaration.is_boundary) {
        let mut arguments = Vec::new();
        for parameter in program.trait_type_parameters(scope.declaration) {
            let Some(identity) = scope.portable_parameter_identity(program, parameter.symbol)
            else {
                return Some(Vec::new());
            };
            arguments.push(program.type_identity_text(identity)?);
        }
        let Some(regions) = scope.portable_lifetime_arguments(program) else {
            return Some(Vec::new());
        };
        let lifetimes = regions
            .iter()
            .map(|identity| program.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()?;
        let mut requirements = Vec::new();
        scope.visit(program, &mut |declaring| {
            if !declaring.declaration.is_boundary {
                return;
            }
            for signature in program.trait_machine_signatures(declaring.declaration) {
                let identity = program
                    .trait_requirement_callable_identity_text(declaring.declaration, signature);
                if !requirements.contains(&identity) {
                    requirements.push(identity);
                }
            }
        });
        requirements.sort();
        applications.push(ProviderSelectionReceiver {
            package: program
                .symbols
                .symbol_package_identity(scope.declaration.symbol),
            declaration: program.trait_declaration_path(scope.declaration),
            arguments,
            lifetimes,
            requirements,
        });
    }
    if let Some(ancestors) = applications.get_mut(1..) {
        ancestors.sort_by(|left, right| {
            left.declaration
                .cmp(&right.declaration)
                .then_with(|| left.package.cmp(&right.package))
        });
    }
    Some(applications)
}

pub(super) fn product_receiver_application(
    operand: &checked_trees::typed_trees::typed_trees::product_applications::ProductDeclarationOperand,
) -> Option<ProviderSelectionReceiver> {
    use checked_trees::typed_trees::typed_trees::product_applications::ProductDeclarationKind;
    use checked_trees::typed_trees::types::TypeReferenceNode;
    if operand.kind != ProductDeclarationKind::ProviderSubject
        || !operand.application.is_valid()
        || !operand.receiver_type.is_valid()
    {
        return None;
    }
    let program = operand.owner.as_ref();
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        lifetime_arguments,
        ..
    } = program
        .type_reference_table
        .type_reference(operand.receiver_type)
    else {
        return None;
    };
    if *base_symbol != operand.declaration {
        return None;
    }
    let definition = program.trait_definition_by_symbol(operand.declaration)?;
    if !definition.is_boundary {
        return None;
    }
    let application = program
        .expression_table
        .static_machine_application(operand.application);
    let root = AppliedTraitScope::new_with_owner(
        program,
        definition,
        checked_trees::typed_trees::trait_definition::TraitApplicationArguments {
            lexical_owner: application.lexical_owner,
            enclosing_data_applications: &[],
            arguments: program
                .type_reference_table
                .type_reference_handles(*arguments),
            lifetimes: lifetime_arguments,
        },
        application.type_identity_owner(program)?,
    )?;
    let scopes = collect_scopes(program, root)?;
    receiver_applications(program, &scopes)?.into_iter().next()
}
