//! Immutable hosted leaf declarations, without provider or target authority.
//!
//! This describes an exact checked catalogue edge. Ordinary package source
//! still requires its independently accepted semantic binding and selected
//! provider plan before Omega can realize any of these operations.

use crate::{TypedTrees, machine::Machine, signature::StateSignature};
use symbols::SymbolHandle;

pub const CATALOGUE_PATH: &str = "native/hosted.omg";
pub const CATALOGUE_MODULE: &str = "native::hosted";
pub const CATALOGUE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/native/hosted.omg"
));
const CONSOLE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/console.omg"
));
const PROCESS_EXIT_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/process_exit.omg"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedIntrinsicRole {
    ReadByte,
    WriteByte,
    ConsoleExit,
    ProcessExit,
}

impl HostedIntrinsicRole {
    pub const fn leaf(self) -> &'static str {
        match self {
            Self::ReadByte => "console_read_byte",
            Self::WriteByte => "console_write_byte",
            Self::ConsoleExit => "console_exit_process",
            Self::ProcessExit => "hosted_process_exit",
        }
    }

    pub const fn service(self) -> &'static str {
        match self {
            Self::ReadByte | Self::WriteByte | Self::ConsoleExit => "Console",
            Self::ProcessExit => "ProcessExit",
        }
    }

    pub const fn requirement(self) -> &'static str {
        match self {
            Self::ReadByte => "read_byte",
            Self::WriteByte => "write_byte",
            Self::ConsoleExit | Self::ProcessExit => "exit_process",
        }
    }
}

pub struct HostedIntrinsicDeclaration<'program> {
    pub role: HostedIntrinsicRole,
    pub machine: &'program Machine,
    pub service: &'program crate::trait_definition::TraitDefinition,
    pub requirement: &'program StateSignature,
    pub code_owner: SymbolHandle,
}

impl TypedTrees {
    /// Authenticate the whole immutable source, code owner, exact public edge
    /// and signature. This does not select an implementation, admit a package,
    /// classify a target, or supply execution authority.
    pub fn hosted_intrinsic_declaration(
        &self,
        realization: SymbolHandle,
    ) -> Option<HostedIntrinsicDeclaration<'_>> {
        let declaration = self.hosted_intrinsic_source_declaration(realization)?;
        use super::boundary_catalogues::BoundaryCatalogueRole;
        let role = match declaration.role {
            HostedIntrinsicRole::ProcessExit => BoundaryCatalogueRole::HostedProcessExit,
            _ => BoundaryCatalogueRole::HostedConsole,
        };
        self.boundary_catalogue_is_admitted(role, realization, declaration.requirement.symbol)
            .then_some(declaration)
    }

    /// Immutable declaration description for confined source-role nomination.
    /// This raw shape view does not permit calling or executing the leaf.
    pub fn hosted_intrinsic_source_declaration(
        &self,
        realization: SymbolHandle,
    ) -> Option<HostedIntrinsicDeclaration<'_>> {
        let machine = self.machine_by_symbol(realization)?;
        let role = [
            HostedIntrinsicRole::ReadByte,
            HostedIntrinsicRole::WriteByte,
            HostedIntrinsicRole::ConsoleExit,
            HostedIntrinsicRole::ProcessExit,
        ]
        .into_iter()
        .find(|role| role.leaf() == machine.name.as_str())?;
        if machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || machine.body_is_present
            || machine.attached_data.is_some()
            || !machine.lifetime_parameters.is_empty()
            || !self.machine_type_parameters(machine).is_empty()
            || self.machine_authored_target(machine).is_some()
            || machine.target_family_interface
        {
            return None;
        }
        let source = self
            .symbols
            .symbol_source_span(realization)
            .and_then(|span| self.symbols.source_file(span))?;
        if source.path.strip_prefix(&source.package_root).ok()?
            != std::path::Path::new(CATALOGUE_PATH)
            || source.source.as_bytes() != CATALOGUE_SOURCE
        {
            return None;
        }
        let code_owner = self.machine_code_owner_symbol(machine)?;
        if self.symbols.get(code_owner).kind != symbols::SymbolKind::Module
            || self.symbols.display_path(code_owner, "::") != CATALOGUE_MODULE
            || self.symbols.symbol_package_identity(code_owner) != source.package_identity
        {
            return None;
        }
        let [conformance] = self.machine_trait_conformances(machine) else {
            return None;
        };
        let crate::machine::SatisfiedDeclaration::Trait {
            definition: service,
            requirement,
        } = crate::machine::resolve_satisfied_declaration(self, machine, conformance)?
        else {
            return None;
        };
        if service.symbol != conformance.symbol
            || requirement.symbol != conformance.requirement_symbol
            || !service.is_boundary
            || service.name.as_str() != role.service()
            || requirement.name.as_str() != role.requirement()
            || !service.lifetime_parameters.is_empty()
            || !self.trait_type_parameters(service).is_empty()
            || !requirement.lifetime_parameters.is_empty()
            || !self.state_signature_type_parameters(requirement).is_empty()
            || !requirement.native_callback_parameters.is_empty()
            || !conformance.trait_lifetime_arguments.is_empty()
            || !self
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
        {
            return None;
        }
        let public_source = self
            .symbols
            .symbol_source_span(service.symbol)
            .and_then(|span| self.symbols.source_file(span))?;
        let (path, bytes) = match role {
            HostedIntrinsicRole::ProcessExit => ("process_exit.omg", PROCESS_EXIT_SOURCE),
            _ => ("console.omg", CONSOLE_SOURCE),
        };
        if public_source
            .path
            .strip_prefix(&public_source.package_root)
            .ok()?
            != std::path::Path::new(path)
            || public_source.source.as_bytes() != bytes
            || self
                .symbols
                .symbol_source_span(requirement.symbol)?
                .source_id
                != public_source.source_id
        {
            return None;
        }
        if role == HostedIntrinsicRole::ProcessExit {
            if public_source.origin != source::SourceOrigin::Toolchain
                || public_source.package_identity.is_some()
            {
                return None;
            }
        } else if public_source.package_identity != source.package_identity
            || public_source.package_root != source.package_root
        {
            return None;
        }
        let [entry] = self.machine_states(machine) else {
            return None;
        };
        let read = role == HostedIntrinsicRole::ReadByte;
        if machine.suspends
            || requirement.suspends
            || machine.blocks != read
            || requirement.blocks != read
        {
            return None;
        }
        let parameters = self.state_parameters(entry);
        let required = self.state_signature_parameters(requirement);
        if read {
            let result = self
                .type_reference_table
                .type_reference(entry.return_type)
                .as_named_symbol()?;
            let required_result = self
                .type_reference_table
                .type_reference(requirement.return_type)
                .as_named_symbol()?;
            if !parameters.is_empty()
                || !required.is_empty()
                || result != required_result
                || self.symbols.symbol_source_span(result)?.source_id != public_source.source_id
                || self
                    .data_definition_by_symbol(result)
                    .is_none_or(|definition| definition.name.as_str() != "ByteRead")
            {
                return None;
            }
        } else {
            let exact = |parameters: &[crate::signature::StateParameter], result| {
                let [parameter] = parameters else {
                    return false;
                };
                !parameter.is_const
                    && !parameter.is_mutable
                    && self.primitive_type_reference(parameter.type_reference)
                        == Some(crate::types::PrimitiveType::I32)
                    && matches!(
                        self.type_reference_table.type_reference(result),
                        crate::types::TypeReferenceNode::Unit
                    )
            };
            if !exact(parameters, entry.return_type) || !exact(required, requirement.return_type) {
                return None;
            }
        }
        Some(HostedIntrinsicDeclaration {
            role,
            machine,
            service,
            requirement,
            code_owner,
        })
    }
}
