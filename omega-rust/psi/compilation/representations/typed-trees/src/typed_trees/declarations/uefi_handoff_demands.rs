//! Exact source demands for the two firmware handoff mechanics.
//!
//! This attests the immutable SDK declaration and its public requirement only.
//! It supplies no physical invocation, provider, target, machine-state evidence
//! or executable implementation. Unsupported selected native closure still
//! refuses; full physical handoff realization is a separate obligation.

use crate::{TypedTrees, machine::SatisfiedDeclaration, types::TypeReferenceNode};
use symbols::SymbolHandle;

const SOURCE_PATH: &str = "targets/uefi_x86_64/handoff.omg";
const SOURCE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/targets/uefi_x86_64/handoff.omg"
));

impl TypedTrees {
    /// A free catalogue leaf delegates to this exact public boundary slot.
    /// Source checking may retain the demand without claiming it is settled.
    pub fn uefi_handoff_requirement(&self, entry: SymbolHandle) -> Option<SymbolHandle> {
        let requirement = self.uefi_handoff_source_requirement(entry)?;
        let machine = self.machine_holding_state(entry)?;
        self.boundary_catalogue_is_admitted(
            super::boundary_catalogues::BoundaryCatalogueRole::UefiHandoff,
            machine.symbol,
            requirement,
        )
        .then_some(requirement)
    }

    /// Immutable source shape for explicit package role nomination only.
    pub fn uefi_handoff_source_requirement(&self, entry: SymbolHandle) -> Option<SymbolHandle> {
        let machine = self.machine_holding_state(entry)?;
        let requirement_name = match machine.name.as_str() {
            "uefi_transfer_to_entry" => "transfer",
            "uefi_firmware_return" => "firmware_return",
            _ => return None,
        };
        if machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || machine.body_is_present
            || machine.attached_data.is_some()
            || !self.machine_type_parameters(machine).is_empty()
            || !machine.lifetime_parameters.is_empty()
            || machine.blocks
            || machine.suspends
        {
            return None;
        }
        let [state] = self.machine_states(machine) else {
            return None;
        };
        if state.symbol != entry {
            return None;
        }
        let source = self
            .symbols
            .symbol_source_span(machine.symbol)
            .and_then(|span| self.symbols.source_file(span))?;
        if source.path.strip_prefix(&source.package_root).ok()? != std::path::Path::new(SOURCE_PATH)
            || source.source.as_bytes() != SOURCE_BYTES
            || self.symbols.symbol_package_identity(machine.symbol) != source.package_identity
        {
            return None;
        }
        let [conformance] = self.machine_trait_conformances(machine) else {
            return None;
        };
        let SatisfiedDeclaration::Trait {
            definition,
            requirement,
        } = crate::machine::resolve_satisfied_declaration(self, machine, conformance)?
        else {
            return None;
        };
        if !definition.is_boundary
            || definition.name.as_str() != "UefiOsHandoffTermination"
            || requirement.name.as_str() != requirement_name
            || conformance.symbol != definition.symbol
            || conformance.requirement_symbol != requirement.symbol
            || !self.trait_type_parameters(definition).is_empty()
            || !definition.lifetime_parameters.is_empty()
            || !conformance.trait_lifetime_arguments.is_empty()
            || !self
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            || !self.state_signature_type_parameters(requirement).is_empty()
            || !requirement.lifetime_parameters.is_empty()
            || !requirement.native_callback_parameters.is_empty()
            || requirement.blocks
            || requirement.suspends
        {
            return None;
        }
        // The public protocol and private demand must belong to the same exact
        // immutable file and package occurrence, never a same-named import.
        for symbol in [definition.symbol, requirement.symbol] {
            if self.symbols.symbol_source_span(symbol)?.source_id != source.source_id
                || self.symbols.symbol_package_identity(symbol) != source.package_identity
            {
                return None;
            }
        }
        let parameters = self.state_parameters(state);
        let required = self.state_signature_parameters(requirement);
        let valid_parameters =
            |parameters: &[crate::signature::StateParameter]| match requirement_name {
                "transfer" => parameters.is_empty(),
                "firmware_return" => matches!(parameters, [status]
                    if !status.is_const && !status.is_mutable
                        && self.primitive_type_reference(status.type_reference)
                            == Some(crate::types::PrimitiveType::U64)),
                _ => false,
            };
        if !valid_parameters(parameters)
            || !valid_parameters(required)
            || !matches!(
                self.type_reference_table.type_reference(state.return_type),
                TypeReferenceNode::Unit
            )
            || !matches!(
                self.type_reference_table
                    .type_reference(requirement.return_type),
                TypeReferenceNode::Unit
            )
        {
            return None;
        }
        Some(requirement.symbol)
    }
}
