//! The immutable Core entry edges, independently of selected target backing.
//!
//! Exact source and public requirement identity authenticate these two leaves.
//! They do not grant an image/storage root or a selected continuation. Omega
//! separately rejoins the Build row, UEFI profile and installed entry custody.
use crate::TypedTrees;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreEntryIntrinsic {
    ProgramStorageEnter,
    ProgramImageRetainedSuffixStart,
}

impl CoreEntryIntrinsic {
    pub const fn leaf(self) -> &'static str {
        match self {
            Self::ProgramStorageEnter => "program_storage_enter",
            Self::ProgramImageRetainedSuffixStart => "program_image_retained_suffix_start",
        }
    }
    fn catalogue(self) -> (&'static str, &'static [u8], &'static str, &'static str) {
        match self {
            Self::ProgramStorageEnter => (
                "extent.omg",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/extent.omg"
                )),
                "ProgramStorageEntry",
                "enter",
            ),
            Self::ProgramImageRetainedSuffixStart => (
                "program_image.omg",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/program_image.omg"
                )),
                "ProgramImage",
                "retained_suffix_start",
            ),
        }
    }
}

impl TypedTrees {
    pub fn sealed_core_entry_realization(
        &self,
        realization: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<CoreEntryIntrinsic> {
        let machine = self.machine_by_symbol(realization)?;
        let role = [
            CoreEntryIntrinsic::ProgramStorageEnter,
            CoreEntryIntrinsic::ProgramImageRetainedSuffixStart,
        ]
        .into_iter()
        .find(|role| machine.name.as_str() == role.leaf())?;
        if !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::CompilerPrimitive
        ) || machine.body_is_present
            || machine.attached_data.is_some()
            || machine.target_family_interface
            || self.machine_authored_target(machine).is_some()
            || !machine.lifetime_parameters.is_empty()
            || !self.machine_type_parameters(machine).is_empty()
            || machine.blocks
            || machine.suspends
        {
            return None;
        }
        let [conformance] = self.machine_trait_conformances(machine) else {
            return None;
        };
        let crate::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement: signature,
        } = crate::machine::resolve_satisfied_declaration(self, machine, conformance)?
        else {
            return None;
        };
        let (path, bytes, owner_name, requirement_name) = role.catalogue();
        if !definition.is_boundary
            || definition.symbol != conformance.symbol
            || signature.symbol != conformance.requirement_symbol
            || signature.symbol != requirement
            || definition.name.as_str() != owner_name
            || signature.name.as_str() != requirement_name
            || !definition.lifetime_parameters.is_empty()
            || !self.trait_type_parameters(definition).is_empty()
            || !signature.lifetime_parameters.is_empty()
            || !self.state_signature_type_parameters(signature).is_empty()
            || !signature.native_callback_parameters.is_empty()
            || !conformance.trait_lifetime_arguments.is_empty()
            || !self
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            || signature.blocks
            || signature.suspends
        {
            return None;
        }
        let source = |symbol| {
            self.symbols
                .symbol_source_span(symbol)
                .and_then(|span| self.symbols.source_file(span))
        };
        for symbol in [realization, definition.symbol, signature.symbol] {
            let source = source(symbol)?;
            if source.origin != source::SourceOrigin::Toolchain
                || source.package_identity.is_some()
                || source.path.strip_prefix(&source.package_root).ok()?
                    != std::path::Path::new(path)
                || source.source.as_bytes() != bytes
            {
                return None;
            }
        }
        if self.normalized_hermetic_symbol_identity(realization).ok()?
            != format!("toolchain::{}", role.leaf())
            || self
                .normalized_hermetic_symbol_identity(definition.symbol)
                .ok()?
                != format!("toolchain::{owner_name}")
        {
            return None;
        }
        let [entry] = self.machine_states(machine) else {
            return None;
        };
        let parameters = self.state_parameters(entry);
        let required = self.state_signature_parameters(signature);
        // Both signatures originate in the one immutable file. Retain complete
        // type identities rather than comparing only their scalar ABI carriers.
        if parameters.len() != required.len()
            || parameters.iter().zip(required).any(|(actual, required)| {
                actual.is_const != required.is_const
                    || actual.is_mutable != required.is_mutable
                    || self.normalized_type_identity(actual.type_reference)
                        != self.normalized_type_identity(required.type_reference)
            })
            || self.normalized_type_identity(entry.return_type)
                != self.normalized_type_identity(signature.return_type)
        {
            return None;
        }
        Some(role)
    }
}
