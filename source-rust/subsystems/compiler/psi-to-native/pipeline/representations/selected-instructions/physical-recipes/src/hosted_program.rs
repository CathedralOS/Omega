//! A hosted entry's chosen instructions, import stubs and matching custody.

use crate::{PhysicalRecipe, hosted_entry::HostedEntryProgram, imports::PhysicalImportProgram};

pub type HostedProgram<Origin, Context> =
    PhysicalRecipe<Origin, HostedProgramParts<Origin, Context>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedProgramParts<Origin, Context> {
    pub context: Context,
    pub entry: HostedEntryProgram<Origin>,
    pub imports: PhysicalImportProgram<Origin>,
}

impl<Origin, Context> HostedProgram<Origin, Context> {
    pub fn into_parts(self) -> HostedProgramParts<Origin, Context> {
        self.instructions
    }
}

/// Storage sizes selected with the hosted entry instructions. The final object
/// supplies the BSS origin when it places the continuation, stack and receivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedStackLayout {
    pub stack_byte_count: u64,
    pub private_ram: Vec<PrivateRamExtent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateRamExtent {
    pub alignment: u64,
    pub contents_bytes: u64,
    pub observation_header_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedStackBacking<R> {
    Loader,
    Owned(HostedStackLayout),
    Receiver {
        receipt: R,
        layout: HostedStackLayout,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedProgramContext<R> {
    pub profile: Option<target::TargetProfile>,
    pub demand: machine_code::physical_stack::StackDemand,
    pub backing: HostedStackBacking<R>,
}

impl<R> HostedProgramContext<R> {
    pub const fn receiver(&self) -> Option<&R> {
        match &self.backing {
            HostedStackBacking::Receiver { receipt, .. } => Some(receipt),
            _ => None,
        }
    }
}
