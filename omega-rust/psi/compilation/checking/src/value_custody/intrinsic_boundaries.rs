//! Exact typed identity of external-leaf boundary realizations.
//!
//! A bodyless leaf `machine leaf(..) satisfies Boundary::requirement via ..;`
//! owns no Omega body: its realization is the compiler intrinsic or the
//! evaluated foreign binding its `via` names. A direct call to such a leaf
//! from checked code (`ConsoleNativeProvider::write_byte(b)`, `darwin_poll(0,
//! 0, ms)`) is therefore a boundary call, and checking plans it against the
//! leaf's exact satisfied requirement: the requirement owns the published
//! contract, reach, crash routes and calling-policy application; Omega settles
//! it through the selected provider row that carries the leaf's realization
//! (a compiler-builtin settlement or a normalized foreign import). Treating
//! the leaf as an ordinary callee instead demands an executable body that no
//! stage can construct, and fabricating one would discard the binding's
//! exact ABI evidence.
//!
//! Two leaf families project, each only through one exact edge:
//!
//! - compiler-intrinsic leaves, identified by the sealed intrinsic catalog's
//!   declaration, signature and provider (`exact_compiler_intrinsic_boundary_requirement`);
//! - evaluated-binding leaves, whose ordinary `via <expression>` evaluates
//!   at build time to a `ForeignBinding` value. Their declared signature is
//!   the boundary signature, so projection requires it to equal the
//!   requirement's runtime signature exactly
//!   (`exact_external_leaf_boundary_requirement`).
//!
//! Call planning, boundary-plan collection, call-source custody, the
//! scalar-computation exclusion and service-reach inference all apply this
//! one join. For reach, an evaluated-binding leaf is an ordinary bodyless
//! machine whose satisfied requirement supplies its published reach, so a
//! call to it propagates that reach like any machine call rather than
//! counting as authored direct boundary use; a leaf call and a field call on
//! one requirement then publish the single reach its boundary plan needs.
//!
//! Honest boundaries:
//!
//! - The projected call resolves through the requirement's selected provider
//!   row, so it reaches the named leaf only while that leaf's family is the
//!   selected realization, the same premise the intrinsic route carries.
//! - A machine holding provider fields cannot yet make a projected leaf call:
//!   stage 05 and Terminal verification count every signature-directed
//!   boundary call there as a provider-field obligation.

use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// The exact boundary requirement a direct call to a bodyless external leaf
/// delegates to, with the leaf's attached provider (the invalid symbol for a
/// free leaf). Checking and lowering use this join for every external leaf;
/// the checked interpreter keeps `exact_compiler_intrinsic_boundary_requirement`
/// because it hosts only the intrinsic catalog.
pub fn exact_external_leaf_boundary_requirement(
    program: &TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    exact_compiler_intrinsic_boundary_requirement(program, target_state_symbol)
        .or_else(|| exact_evaluated_binding_boundary_requirement(program, target_state_symbol))
}

/// An ordinary evaluated-binding leaf's exact boundary-trait requirement.
/// The leaf carries exactly one conformance and it names its `via` producer;
/// the requirement is monomorphic and synchronous, owns no private callback
/// parameters, and publishes the leaf's own runtime signature.
fn exact_evaluated_binding_boundary_requirement(
    program: &TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let machine = program.machine_holding_state(target_state_symbol)?;
    if machine.body_is_present
        || !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization {
                binding: None,
                mechanism: None,
            }
        )
    {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [conformance] = program.machine_trait_conformances(machine) else {
        return None;
    };
    if state.symbol != target_state_symbol
        || conformance.external_binding.is_some()
        || !conformance.via_expression.is_valid()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
    {
        return None;
    }
    let typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)?
    else {
        return None;
    };
    (definition.symbol == conformance.symbol
        && definition.is_boundary
        && definition.lifetime_parameters.is_empty()
        && program.trait_type_parameters(definition).is_empty()
        && requirement.symbol == conformance.requirement_symbol
        && requirement.lifetime_parameters.is_empty()
        && program
            .state_signature_type_parameters(requirement)
            .is_empty()
        && requirement.native_callback_parameters.is_empty()
        && !requirement.suspends
        && same_runtime_signature(
            program,
            (program.state_parameters(state), state.return_type),
            (
                program.state_signature_parameters(requirement),
                requirement.return_type,
            ),
        ))
    .then_some((requirement.symbol, machine.attached_data_symbol))
}

/// Parameter-for-parameter equality of two runtime signatures: position,
/// receiver/const/mutable markers, relevance and normalized type identity,
/// then the normalized result identity. A coercible or merely equally shaped
/// signature does not qualify.
fn same_runtime_signature(
    program: &TypedTrees,
    (leaf_parameters, leaf_result): (
        &[typed_trees::signature::StateParameter],
        typed_trees::types::TypeReferenceHandle,
    ),
    (requirement_parameters, requirement_result): (
        &[typed_trees::signature::StateParameter],
        typed_trees::types::TypeReferenceHandle,
    ),
) -> bool {
    leaf_parameters.len() == requirement_parameters.len()
        && leaf_parameters
            .iter()
            .zip(requirement_parameters)
            .all(|(leaf, requirement)| {
                !leaf.is_self
                    && !requirement.is_self
                    && leaf.is_const == requirement.is_const
                    && leaf.is_mutable == requirement.is_mutable
                    && leaf.relevance == requirement.relevance
                    && program.normalized_type_identity(leaf.type_reference)
                        == program.normalized_type_identity(requirement.type_reference)
            })
        && program.normalized_type_identity(leaf_result)
            == program.normalized_type_identity(requirement_result)
}

/// The sealed compiler-intrinsic catalog's exact requirement edge. This is
/// the narrower identity the checked interpreter hosts; checking and lowering
/// consume `exact_external_leaf_boundary_requirement`.
pub fn exact_compiler_intrinsic_boundary_requirement(
    program: &TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    // State spans are disjoint append-only ranges: at most one machine can
    // contain a state symbol, so the holder is unique by construction.
    let machine = program.machine_holding_state(target_state_symbol)?;
    if !program.machine_type_parameters(machine).is_empty() {
        return None;
    }
    let (authored_binding, inferred_hosted_intrinsic) =
        intrinsic_realization_candidate(program, machine)?;
    let inferred_trait_name = inferred_hosted_intrinsic.map(|(_, trait_name)| trait_name);
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // A concrete byte leaf returns its requirement's nominal sum, not Unit.
    // Keep this separate from the scalar-to-Unit signature: this join also
    // feeds source custody and reach inference, not just host interpretation.
    let byte_result = (machine.name.as_str() == "ConsoleNativeProvider::read_byte"
        && program.state_parameters(state).is_empty())
    .then(|| exact_byte_read_result_type(program, state.return_type))
    .flatten();
    if state.symbol != target_state_symbol
        || !(exact_direct_intrinsic_signature(
            program,
            program.state_parameters(state),
            state.return_type,
        ) || byte_result.is_some())
    {
        return None;
    }

    let mut matches = program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            if ((inferred_trait_name.is_some()
                && (conformance.external_binding.is_some()
                    || conformance.via_expression.is_valid()
                    || conformance.external_binding_source_span.is_some()))
                || (inferred_trait_name.is_none()
                    && conformance.external_binding != authored_binding))
                || conformance.requirement.is_none()
                || !program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .is_empty()
            {
                return None;
            }
            let typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            } = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)?
            else {
                return None;
            };
            let provider_prefix = inferred_hosted_intrinsic
                .map(|(provider_name, _)| provider_name)
                .unwrap_or_default();
            (definition.symbol == conformance.symbol
                && definition.is_boundary
                && inferred_trait_name
                    .is_none_or(|trait_name| definition.name.as_str() == trait_name)
                && definition.lifetime_parameters.is_empty()
                && program.trait_type_parameters(definition).is_empty()
                && requirement.symbol == conformance.requirement_symbol
                && (inferred_trait_name.is_none()
                    || machine
                        .name
                        .as_str()
                        .strip_prefix(provider_prefix)
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        == Some(requirement.name.as_str()))
                && requirement.lifetime_parameters.is_empty()
                && program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
                && requirement.native_callback_parameters.is_empty()
                && !requirement.suspends
                // The hosted byte-input leaf publishes `blocks;` honestly:
                // it may occupy the worker while it waits for input. Every
                // other intrinsic boundary leaf must stay nonblocking.
                && byte_result.is_some() == requirement.blocks
                && ((byte_result.is_none()
                    && exact_direct_intrinsic_signature(
                        program,
                        program.state_signature_parameters(requirement),
                        requirement.return_type,
                    ))
                    || (byte_result.is_some()
                        && definition.name.as_str() == "Console"
                        && requirement.name.as_str() == "read_byte"
                        && program.state_signature_parameters(requirement).is_empty()
                        && exact_byte_read_result_type(program, requirement.return_type)
                            == byte_result)))
                .then_some(requirement.symbol)
        });
    let requirement = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some((requirement, machine.attached_data_symbol))
}

/// The machine-level half of the intrinsic identity: the authored
/// compiler-intrinsic binding, or the exact hosted catalog leaf (realization,
/// provider nominal, boundary trait) admitted without an authored `via`
/// binding. `None` when the machine cannot realize an intrinsic boundary.
fn intrinsic_realization_candidate(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<(
    Option<language_semantics::ExternalBindingId>,
    Option<(&'static str, &'static str)>,
)> {
    if machine.body_is_present || !machine.lifetime_parameters.is_empty() {
        return None;
    }
    let authored_binding = match machine.supply_mode {
        MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        } if program.external_bindings.identity(binding)
            == Some(&language_semantics::ExternalBindingIdentity::CompilerIntrinsic) =>
        {
            Some(binding)
        }
        _ => None,
    };
    let inferred_hosted_intrinsic = match machine.name.as_str() {
        "ConsoleNativeProvider::exit_process" | "ConsoleNativeProvider::write_byte" => {
            Some(("ConsoleNativeProvider", "Console"))
        }
        "ProcessExitNativeProvider::exit_process" => {
            Some(("ProcessExitNativeProvider", "ProcessExit"))
        }
        _ => None,
    }
    .filter(|(provider_name, _)| {
        machine.supply_mode == MachineSupplyMode::Boundary
            && machine.attached_data.as_ref().map(|name| name.as_str()) == Some(*provider_name)
    });
    (authored_binding.is_some() || inferred_hosted_intrinsic.is_some())
        .then_some((authored_binding, inferred_hosted_intrinsic))
}

/// The supported byte-input result, resolved by declaration identity. This is
/// typed shape validation, not selected-package or native execution admission.
pub fn exact_byte_read_result_type(
    program: &TypedTrees,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    use typed_trees::data::DataMember;
    use typed_trees::types::TypeConstraintNode;

    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(result_type)
    else {
        return None;
    };
    let data = crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
    if !symbol.is_valid()
        || data.name.as_str() != "ByteRead"
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || !data.lifetime_parameters.is_empty()
        || !program.data_type_parameters(data).is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
        || !data.where_facts.is_empty()
        || data.zero_gated
        || data.properties.carry.is_some()
        || data.properties.multiplicity == language_semantics::Multiplicity::Linear
    {
        return None;
    }
    let [DataMember::Variant(eof), DataMember::Variant(byte)] = program.data_members(data) else {
        return None;
    };
    if eof.name.as_str() != "Eof" || !eof.payload.is_empty() || byte.name.as_str() != "Byte" {
        return None;
    }
    let [field] = program.data_payload_fields(byte) else {
        return None;
    };
    if field.name.as_str() != "value"
        || field.relevance.is_erased()
        || program.primitive_type_reference(field.type_reference) != Some(PrimitiveType::I32)
    {
        return None;
    }
    // Ranges may narrow the carrier but must include every input byte. Other
    // domain/proof promises cannot be established by this host operation.
    let mut carrier = field.type_reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(carrier)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return None;
            };
            let minimum = crate::closed_integer_range_bound(program, *minimum)?.to_i64()?;
            let maximum =
                crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?.to_i64()?;
            if minimum > 0 || maximum < 255 {
                return None;
            }
        }
        carrier = *base_type;
    }
    Some(*symbol)
}

fn exact_direct_intrinsic_signature(
    program: &TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    return_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    !parameter.is_self
        && !parameter.is_const
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

/// Completion belongs to the admitted requirement declaration, independently
/// of the provider selected to realize it. Only the canonical process-exit
/// requirement and the existing canonical Console adapter end the domain.
pub fn boundary_requirement_completion(
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
    definition: &typed_trees::trait_definition::TraitDefinition,
    requirement: SymbolHandle,
    console_adapter: bool,
) -> bool {
    const CORE_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/process_exit.omg"
    ));
    const CONSOLE_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/console.omg"
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
    declaration: &typed_trees::BoundaryCompletionDeclaration,
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
