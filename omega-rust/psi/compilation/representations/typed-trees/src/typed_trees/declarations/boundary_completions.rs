//! Exact boundary-completion identity of declared requirements.
//!
//! A boundary completion promises how a boundary requirement returns control:
//! normally, or through the canonical process-exit declaration. Completion
//! belongs to the admitted requirement declaration, independently of the
//! provider selected to realize it, so the join is pure typed-program
//! vocabulary plus the canonical toolchain source identity that pins the
//! `ProcessExit` and `Console` declarations. Realization reads the same join
//! through checked custody instead of re-deriving it inside checking.

use crate::types::{PrimitiveType, TypeReferenceNode};
use crate::{BoundaryCompletionDeclaration, TypedTrees};
use symbols::SymbolHandle;

/// Resolve the exact source requirement used by admission and realization.
pub fn completion_declaration(
    typed: &TypedTrees,
    owner: SymbolHandle,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Option<BoundaryCompletionDeclaration> {
    let definition = typed.trait_definition_by_symbol(owner)?;
    let mut requirements = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == "exit_process");
    let requirement = requirements.next()?;
    if requirements.next().is_some() {
        return None;
    }
    let declaration = BoundaryCompletionDeclaration {
        package: Some(package),
        owner,
        owner_path: typed.symbols.display_path(owner, "::"),
        requirement: requirement.symbol,
        requirement_path: typed.symbols.display_path(requirement.symbol, "::"),
        completion: semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        ),
    };
    valid_boundary_completion_declaration(typed, &declaration).then_some(declaration)
}

/// Completion belongs to the admitted requirement declaration, independently
/// of the provider selected to realize it. Only the canonical process-exit
/// requirement and the existing canonical Console adapter end the domain.
pub fn boundary_requirement_completion(
    program: &TypedTrees,
    requirement: SymbolHandle,
) -> semantic_vocabulary::BoundaryCompletion {
    // A portable family head has no satisfaction edges of its own. Its
    // completion must hold for every retained target member, independently
    // of which target Omega will select. Never inherit one prototype's
    // promise or turn a checked body into an external leaf.
    if let Some(family) = program
        .target_machine_families
        .iter()
        .find(|family| family.canonical == requirement)
    {
        let mut members = family.members.iter();
        if let Some(first) = members.next() {
            let completion = machine_completion(program, first.machine);
            if members.all(|member| machine_completion(program, member.machine) == completion) {
                return completion;
            }
        }
        return semantic_vocabulary::BoundaryCompletion::NormalReturn;
    }
    machine_completion(program, requirement)
}

fn machine_completion(
    program: &TypedTrees,
    requirement: SymbolHandle,
) -> semantic_vocabulary::BoundaryCompletion {
    // A direct bodyless realization carries its exact satisfied declaration's
    // completion. Checked adapters do not: their control flow must establish
    // that promise, rather than assuming it from their satisfaction edge.
    if let Some(machine) = program.machine_by_symbol(requirement)
        && !machine.body_is_present
    {
        let mut external = program
            .machine_trait_conformances(machine)
            .iter()
            .filter_map(|edge| {
                let declaration =
                    crate::machine::resolve_satisfied_declaration(program, machine, edge)?;
                let completion = declared_completion(program, declaration.symbol());
                matches!(
                    completion,
                    semantic_vocabulary::BoundaryCompletion::External(_)
                )
                .then_some(completion)
            });
        if let Some(completion) = external.next()
            && external.all(|other| other == completion)
        {
            return completion;
        }
    }
    declared_completion(program, requirement)
}

fn declared_completion(
    program: &TypedTrees,
    requirement: SymbolHandle,
) -> semantic_vocabulary::BoundaryCompletion {
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind};
    let mut declarations = program
        .boundary_completion_declarations
        .iter()
        .map(|(_, declaration)| declaration)
        .filter(|declaration| declaration.requirement == requirement);
    if let Some(declaration) = declarations.next()
        && declarations.next().is_none()
        && valid_boundary_completion_declaration(program, declaration)
    {
        return declaration.completion;
    }
    let external = program.traits().iter().any(|definition| {
        exact_process_exit_requirement(program, definition, requirement, false)
            || exact_process_exit_requirement(program, definition, requirement, true)
    });
    if external {
        BoundaryCompletion::External(ExternalCompletionKind::ProcessExit)
    } else {
        BoundaryCompletion::NormalReturn
    }
}

/// The core terminal-event identity used by both checking and interpretation.
pub fn is_canonical_process_exit_requirement(
    program: &TypedTrees,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    program.traits().iter().any(|definition| {
        definition.symbol == trait_symbol
            && exact_process_exit_requirement(program, definition, requirement, false)
    })
}

fn exact_process_exit_requirement(
    program: &TypedTrees,
    definition: &crate::trait_definition::TraitDefinition,
    requirement: SymbolHandle,
    console_adapter: bool,
) -> bool {
    const CORE_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/process_exit.omg"
    ));
    const CONSOLE_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/std/console.omg"
    ));
    let (trait_name, source_path, source_bytes) = if console_adapter {
        ("Console", "console.omg", CONSOLE_SOURCE)
    } else {
        ("ProcessExit", "process_exit.omg", CORE_SOURCE)
    };
    let exact_source = |symbol| {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new(source_path))
                    && file.source.as_bytes() == source_bytes
            })
    };
    if !requirement.is_valid()
        || !definition.is_boundary
        || !definition.is_public
        || definition.name.as_str() != trait_name
        || !definition.lifetime_parameters.is_empty()
        || !program.trait_type_parameters(definition).is_empty()
        || !exact_source(definition.symbol)
    {
        return false;
    }
    let signatures = program.trait_machine_signatures(definition);
    if !console_adapter && signatures.len() != 1 {
        return false;
    }
    signatures.iter().any(|signature| {
        signature.symbol == requirement
            && signature.name.as_str() == "exit_process"
            && exact_source(signature.symbol)
            && signature.lifetime_parameters.is_empty()
            && program
                .state_signature_type_parameters(signature)
                .is_empty()
            && signature.native_callback_parameters.is_empty()
            && !signature.suspends
            && !signature.blocks
            && exact_direct_intrinsic_signature(
                program,
                program.state_signature_parameters(signature),
                signature.return_type,
            )
    })
}

/// Rejoin a semantic-role receipt to the exact package-owned declaration.
/// Consumer policy supplies the role; declaration spelling alone cannot.
pub fn valid_boundary_completion_declaration(
    program: &TypedTrees,
    declaration: &BoundaryCompletionDeclaration,
) -> bool {
    if declaration.completion
        != semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        )
        || !declaration.requirement.is_valid()
        || declaration.package.is_none()
        || program.symbols.symbol_package_identity(declaration.owner) != declaration.package
        || program
            .symbols
            .symbol_package_identity(declaration.requirement)
            != declaration.package
        || program.symbols.display_path(declaration.owner, "::") != declaration.owner_path
        || program.symbols.display_path(declaration.requirement, "::")
            != declaration.requirement_path
    {
        return false;
    }
    program.traits().iter().any(|definition| {
        definition.symbol == declaration.owner
            && definition.is_boundary
            && definition.is_public
            && definition.lifetime_parameters.is_empty()
            && program.trait_type_parameters(definition).is_empty()
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| {
                    signature.symbol == declaration.requirement
                        && signature.lifetime_parameters.is_empty()
                        && program
                            .state_signature_type_parameters(signature)
                            .is_empty()
                        && signature.native_callback_parameters.is_empty()
                        && !signature.suspends
                        && !signature.blocks
                        && program
                            .state_signature_parameters(signature)
                            .iter()
                            .all(|parameter| !parameter.relevance.is_erased())
                        && exact_direct_intrinsic_signature(
                            program,
                            program.state_signature_parameters(signature),
                            signature.return_type,
                        )
                })
    })
}

/// Malformed source-role input must fail admission, never weaken an external
/// completion promise into an ordinary return by losing its exact join.
pub fn validate_boundary_completion_declarations(program: &TypedTrees) -> Result<(), &'static str> {
    let mut requirements = Vec::new();
    for (_, declaration) in program.boundary_completion_declarations.iter() {
        if !valid_boundary_completion_declaration(program, declaration)
            || requirements.contains(&declaration.requirement)
        {
            return Err(
                "boundary completion declaration lost its exact package or requirement custody",
            );
        }
        requirements.push(declaration.requirement);
    }
    Ok(())
}

/// A direct intrinsic boundary signature is exactly `i32 -> unit`, modulo
/// return-type `constrained` wrappers. Checking's intrinsic catalog projects
/// the same predicate for its own leaf joins.
#[doc(hidden)]
pub fn exact_direct_intrinsic_signature(
    program: &TypedTrees,
    parameters: &[crate::signature::StateParameter],
    return_type: crate::types::TypeReferenceHandle,
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    !parameter.is_const
        && !parameter.is_mutable
        && program.primitive_type_reference(parameter.type_reference) == Some(PrimitiveType::I32)
        && {
            let mut current = return_type;
            loop {
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Unit => break true,
                    _ => break false,
                }
            }
        }
}
