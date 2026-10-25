//! Sealed declaration custody for activation-local storage.
//!
//! A byte view describes a region but cannot establish its backing authority.
//! Before a call may become an activation operation, join the selected machine
//! to the complete sealed core signature and its opaque linear carrier. Keep
//! this join separate from executable admission: recognizing a declaration is
//! not permission to bypass the bodyless-call fence without a realization.

use language_core::ReferenceAccess;
use language_core::activation_storage::ACTIVATION_CORE_SOURCE;
pub use language_core::activation_storage::ActivationStorageOperation;
use language_semantics::{DataSupplyMode, MachineSupplyMode, Multiplicity};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
use typed_trees::machine::Machine;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Resolved declaration handles, never reconstructed from a leaf spelling.
#[derive(Debug, Clone, Copy)]
pub struct ActivationStorageContract {
    pub operation: ActivationStorageOperation,
    pub declaration: SymbolHandle,
    pub claim: SymbolHandle,
    pub attempt: SymbolHandle,
    pub established_case: SymbolHandle,
    pub rejected_case: SymbolHandle,
    pub byte_type: TypeReferenceHandle,
}

fn sealed(program: &dyn crate::CheckingInput, symbol: SymbolHandle, identity: &str) -> bool {
    program
        .symbol_toolchain_relative_path(symbol)
        .is_some_and(|path| path == std::path::Path::new(ACTIVATION_CORE_SOURCE))
        && program
            .normalized_hermetic_symbol_identity(symbol)
            .ok()
            .as_deref()
            == Some(identity)
}

fn plain_data<'a>(
    program: &'a dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    identity: &str,
    supply: DataSupplyMode,
) -> Option<&'a DataDefinition> {
    let symbol = program
        .type_reference_table
        .type_reference(reference)
        .as_named_symbol()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    (sealed(program, symbol, identity)
        && data.is_public
        && data.supply_mode == supply
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.generic_instance.is_none()
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && data.properties.carry.is_none()
        && data.retired_identities.is_empty())
    .then_some(data)
}

fn claim(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let data = plain_data(
        program,
        reference,
        "toolchain::ActivationClaim",
        DataSupplyMode::BoundaryOpaque,
    )?;
    (data.properties.multiplicity == Multiplicity::Linear && program.data_members(data).is_empty())
        .then_some(data.symbol)
}

fn mutable_referee(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        } => Some(*referee),
        _ => None,
    }
}

/// Validate the entire authored core roster. Specializations must retain this
/// exact declaration as their template; callers separately evaluate the bound
/// from the selected static application, not a diagnostic name or string.
pub fn exact_toolchain_activation_storage_contract(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<ActivationStorageContract> {
    let operation = if sealed_machine(program, machine, "toolchain::Activation::try_claim") {
        ActivationStorageOperation::Establish
    } else if sealed_machine(program, machine, "toolchain::ActivationClaim::bytes") {
        ActivationStorageOperation::BorrowBytes
    } else if sealed_machine(program, machine, "toolchain::ActivationClaim::release") {
        ActivationStorageOperation::Release
    } else if sealed_machine(program, machine, "toolchain::ActivationClaim::split") {
        ActivationStorageOperation::Split
    } else if sealed_machine(program, machine, "toolchain::ActivationClaim::merge") {
        ActivationStorageOperation::Merge
    } else {
        return None;
    };
    if matches!(
        operation,
        ActivationStorageOperation::Split | ActivationStorageOperation::Merge
    ) {
        return split_merge_contract(program, machine, operation);
    }
    valid_header(program, machine, operation)?;

    // Resolve the other two declarations by sealed identity, then authenticate
    // the common carrier and case payloads. Names alone select no authority.
    let roster_member = |selected_operation, identity| {
        if operation == selected_operation {
            Some(machine)
        } else {
            program
                .machines()
                .iter()
                .find(|candidate| sealed_machine(program, candidate, identity))
        }
    };
    let establish = roster_member(
        ActivationStorageOperation::Establish,
        "toolchain::Activation::try_claim",
    )?;
    let bytes = roster_member(
        ActivationStorageOperation::BorrowBytes,
        "toolchain::ActivationClaim::bytes",
    )?;
    let release = roster_member(
        ActivationStorageOperation::Release,
        "toolchain::ActivationClaim::release",
    )?;
    let establish_state = valid_header(program, establish, ActivationStorageOperation::Establish)?;
    let bytes_state = valid_header(program, bytes, ActivationStorageOperation::BorrowBytes)?;
    let release_state = valid_header(program, release, ActivationStorageOperation::Release)?;
    let [bytes_parameter] = program.state_parameters(bytes_state) else {
        return None;
    };
    let [release_parameter] = program.state_parameters(release_state) else {
        return None;
    };
    let claim_symbol = claim(program, release_parameter.type_reference)?;
    if claim(
        program,
        mutable_referee(program, bytes_parameter.type_reference)?,
    )? != claim_symbol
        || !matches!(
            program
                .type_reference_table
                .type_reference(release_state.return_type),
            TypeReferenceNode::Unit
        )
    {
        return None;
    }
    let byte_slice = mutable_referee(program, bytes_state.return_type)?;
    let TypeReferenceNode::Slice {
        element_type: byte_type,
    } = program.type_reference_table.type_reference(byte_slice)
    else {
        return None;
    };
    if !plain_primitive(program, *byte_type, PrimitiveType::U8) {
        return None;
    }
    let attempt = plain_data(
        program,
        establish_state.return_type,
        "toolchain::ActivationClaimAttempt",
        DataSupplyMode::CheckedShape,
    )?;
    if attempt.properties.multiplicity != Multiplicity::Affine {
        return None;
    }
    let [
        DataMember::Variant(established),
        DataMember::Variant(rejected),
    ] = program.data_members(attempt)
    else {
        return None;
    };
    let [payload] = program.data_payload_fields(established) else {
        return None;
    };
    if established.name.as_str() != "Established"
        || rejected.name.as_str() != "Rejected"
        || claim(program, payload.type_reference)? != claim_symbol
        || payload.relevance.is_erased()
        || payload.identity.is_some()
        || established.identity.is_some()
        || rejected.identity.is_some()
        || !established.retired_payload_identities.is_empty()
        || !rejected.retired_payload_identities.is_empty()
        || !established.where_facts.is_empty()
        || !rejected.where_facts.is_empty()
        || !rejected.payload.is_empty()
    {
        return None;
    }
    Some(ActivationStorageContract {
        operation,
        declaration: machine.symbol,
        claim: claim_symbol,
        attempt: attempt.symbol,
        established_case: established.symbol,
        rejected_case: rejected.symbol,
        byte_type: *byte_type,
    })
}

// A specialization may reuse the template's symbol. Follow exactly its retained
// template edge once; never guess the authored declaration from a suffix.
fn sealed_machine(program: &dyn crate::CheckingInput, machine: &Machine, identity: &str) -> bool {
    let declaration = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    // Generated instances have no authored source span. Their retained exact
    // template edge authenticates the sealed declaration; signature validation
    // below independently checks the instantiated machine and its substitution.
    sealed(program, declaration, identity)
}

fn valid_header<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    operation: ActivationStorageOperation,
) -> Option<&'a typed_trees::state::State> {
    // Specialization clones are executable implementation details and are
    // deliberately private. Visibility belongs to the authenticated authored
    // template, while the rest of this signature belongs to the selected body.
    let declaration = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    let public_declaration = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == declaration)
        .is_some_and(|candidate| candidate.is_public);
    if !public_declaration
        || machine.body_is_present
        || machine.supply_mode != MachineSupplyMode::Boundary
        || !machine.lifetime_parameters.is_empty()
        || machine.spelling.is_some()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.contracts.is_empty()
        || machine.suspends
        || machine.blocks
        || !machine.conformance_bounds.is_empty()
        || !machine.invokes.is_empty()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.target_family_interface
        || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
        || !machine.owned_data.is_empty()
        || machine.service_reach_is_installation_bound
        || machine.admitted_same_stack_bytes != 0
    {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [parameter] = program.state_parameters(state) else {
        return None;
    };
    if parameter.is_const
        || (parameter.is_mutable && operation != ActivationStorageOperation::BorrowBytes)
        || parameter.relevance.is_erased()
        || !state.contracts.is_empty()
    {
        return None;
    }
    let parameters = program
        .machine_specialization(machine.symbol)
        .map(|application| {
            program
                .data_type_parameters
                .span_or_empty(application.template_parameters)
        })
        .unwrap_or_else(|| program.machine_type_parameters(machine));
    if operation == ActivationStorageOperation::Establish {
        let [bound] = parameters else {
            return None;
        };
        let TypeParameterKind::Const { type_reference } = bound.kind else {
            return None;
        };
        if !plain_primitive(program, type_reference, PrimitiveType::U64)
            || !plain_primitive(program, parameter.type_reference, PrimitiveType::U64)
            || !sealed(
                program,
                machine.attached_data_symbol,
                "toolchain::Activation",
            )
        {
            return None;
        }
    } else if !parameters.is_empty()
        || !sealed(
            program,
            machine.attached_data_symbol,
            "toolchain::ActivationClaim",
        )
    {
        return None;
    }
    Some(state)
}

fn split_merge_contract(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    operation: ActivationStorageOperation,
) -> Option<ActivationStorageContract> {
    let establish = program
        .machines()
        .iter()
        .find(|candidate| sealed_machine(program, candidate, "toolchain::Activation::try_claim"))?;
    let base = exact_toolchain_activation_storage_contract(program, establish)?;
    let state = common_boundary_state(program, machine)?;
    if !sealed(
        program,
        machine.attached_data_symbol,
        "toolchain::ActivationClaim",
    ) {
        return None;
    }
    let parameters = program.state_parameters(state);
    let (identity, covered_fields, rejected_fields) = match operation {
        ActivationStorageOperation::Split => {
            let [claim_parameter, committed] = parameters else {
                return None;
            };
            if claim_parameter.is_mutable
                || committed.is_mutable
                || claim(program, claim_parameter.type_reference)? != base.claim
                || !plain_primitive(program, committed.type_reference, PrimitiveType::U64)
            {
                return None;
            }
            let type_parameters = program
                .machine_specialization(machine.symbol)
                .map(|application| {
                    program
                        .data_type_parameters
                        .span_or_empty(application.template_parameters)
                })
                .unwrap_or_else(|| program.machine_type_parameters(machine));
            let [bound] = type_parameters else {
                return None;
            };
            let TypeParameterKind::Const { type_reference } = bound.kind else {
                return None;
            };
            if !plain_primitive(program, type_reference, PrimitiveType::U64) {
                return None;
            }
            ("toolchain::ActivationClaimSplit", 2, 1)
        }
        ActivationStorageOperation::Merge => {
            let [lower, upper] = parameters else {
                return None;
            };
            if lower.is_mutable
                || upper.is_mutable
                || claim(program, lower.type_reference)? != base.claim
                || claim(program, upper.type_reference)? != base.claim
                || !program.machine_type_parameters(machine).is_empty()
            {
                return None;
            }
            ("toolchain::ActivationClaimMerge", 1, 2)
        }
        _ => return None,
    };
    let outcome = plain_data(
        program,
        state.return_type,
        identity,
        DataSupplyMode::CheckedShape,
    )?;
    if outcome.properties.multiplicity != Multiplicity::Affine {
        return None;
    }
    let [covered, rejected] = program.data_members(outcome) else {
        return None;
    };
    let DataMember::Variant(covered) = covered else {
        return None;
    };
    let DataMember::Variant(rejected) = rejected else {
        return None;
    };
    if covered.name.as_str() != "Covered" || rejected.name.as_str() != "Rejected" {
        return None;
    }
    let covered_payload = program.data_payload_fields(covered);
    let rejected_payload = program.data_payload_fields(rejected);
    if covered_payload.len() != covered_fields || rejected_payload.len() != rejected_fields {
        return None;
    }
    if covered_payload
        .iter()
        .chain(rejected_payload.iter())
        .any(|field| {
            claim(program, field.type_reference) != Some(base.claim)
                || field.relevance.is_erased()
                || field.identity.is_some()
        })
    {
        return None;
    }
    Some(ActivationStorageContract {
        operation,
        declaration: machine.symbol,
        claim: base.claim,
        attempt: outcome.symbol,
        established_case: covered.symbol,
        rejected_case: rejected.symbol,
        byte_type: base.byte_type,
    })
}

fn common_boundary_state<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
) -> Option<&'a typed_trees::state::State> {
    let declaration = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    let public_declaration = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == declaration)
        .is_some_and(|candidate| candidate.is_public);
    if !public_declaration
        || machine.body_is_present
        || machine.supply_mode != MachineSupplyMode::Boundary
        || !machine.lifetime_parameters.is_empty()
        || machine.spelling.is_some()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.contracts.is_empty()
        || machine.suspends
        || machine.blocks
        || !machine.conformance_bounds.is_empty()
        || !machine.invokes.is_empty()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.target_family_interface
        || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
        || !machine.owned_data.is_empty()
        || machine.service_reach_is_installation_bound
        || machine.admitted_same_stack_bytes != 0
    {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !state.contracts.is_empty() {
        return None;
    }
    Some(state)
}

/// Evaluate the selected bound from the retained typed substitution. This
/// never parses callable identity, diagnostic arguments, or rendered recipes.
/// Failure leaves demand open and must reject before backing is provisioned.
pub fn activation_storage_bound(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Result<u64, &'static str> {
    let contract = exact_toolchain_activation_storage_contract(program, machine)
        .ok_or("activation establishment has no exact sealed declaration contract")?;
    if !matches!(
        contract.operation,
        ActivationStorageOperation::Establish | ActivationStorageOperation::Split
    ) {
        return Err("only activation establishment has a static backing bound");
    }
    let application = program
        .machine_specialization(machine.symbol)
        .ok_or("activation establishment requires a closed static bound application")?;
    let [parameter] = program
        .data_type_parameters
        .span_or_empty(application.template_parameters)
    else {
        return Err("activation bound lost its original const binder");
    };
    let Some(typed_trees::static_applications::StaticMachineBindingValue::Const(actual)) =
        program.specialization_binding(application, parameter.symbol)
    else {
        return Err("activation bound has no unique const substitution");
    };
    match program.type_reference_table.type_reference(*actual) {
        // A closed literal's source payload is decimal here. It is not a
        // symbol or normalized-identity rendering; unresolved named constants
        // cannot enter this branch.
        TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => name
            .as_str()
            .parse()
            .map_err(|_| "activation bound is not a closed u64 byte extent"),
        TypeReferenceNode::ConstExpression(expression) => program
            .closed_integer_expression_value(*expression)
            .and_then(|value| value.to_u64())
            .ok_or("activation bound is not a closed u64 byte extent"),
        _ => Err("activation bound is not a closed u64 byte extent"),
    }
}

// Primitive queries intentionally peel value restrictions for other consumers.
// A sealed runtime signature cannot lose an authored constraint that way.
fn plain_primitive(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    expected: PrimitiveType,
) -> bool {
    matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { .. }
    ) && program.primitive_type_reference(reference) == Some(expected)
}

/// Compiler-owned physical carrier of the exact sealed opaque claim. Geometry
/// alone grants nothing: only the establishment operation issues its custody.
pub fn activation_claim_byte_carrier(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    // Every successful contract authenticates this exact claim declaration.
    // Ordinary returned types need no activation-machine roster traversal.
    // The full contract below still supplies the carrier, never this premise.
    if !sealed(program, symbol, "toolchain::ActivationClaim") {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        let contract = exact_toolchain_activation_storage_contract(program, machine)?;
        (contract.claim == symbol).then_some(contract.byte_type)
    })
}

/// The sealed accessor reads the claim descriptor and issues a loan; it does
/// not replace that descriptor or write its backing. Operand evaluation keeps
/// its own effects. An opaque declaration with the same spelling has no such
/// compiler-owned effect summary.
pub fn activation_accessor_has_empty_write_frame(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> bool {
    let Some((machine, state)) = crate::find_machine_by_entry_state(program, target) else {
        return false;
    };
    program.machine_states(machine).len() == 1
        && state.symbol == target
        && exact_toolchain_activation_storage_contract(program, machine)
            .is_some_and(|contract| contract.operation == ActivationStorageOperation::BorrowBytes)
}
