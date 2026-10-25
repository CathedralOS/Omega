//! Actual host-owned entry backing, not values invented from a parameter list.

use native_artifact::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalEstablishedErasedField, TerminalProgram, TerminalStructuralInputs,
    TerminalStructuralValue,
};

use super::TerminalRuntimeError;

pub(super) struct EstablishedEntry {
    arguments: Vec<TerminalStructuralValue>,
    zero_initialized: Vec<u32>,
    erased_fields: Vec<TerminalEstablishedErasedField>,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    establishments: Vec<terminal_installation::TerminalPlacedViewEstablishment>,
}

impl EstablishedEntry {
    pub(super) fn inputs(&self) -> TerminalStructuralInputs<'_> {
        TerminalStructuralInputs {
            arguments: &self.arguments,
            zero_initialized_arguments: &self.zero_initialized,
            established_erased_fields: &self.erased_fields,
            private_entry_ram: &self.private_entry_ram,
            placed_view_establishments: &self.establishments,
            ..TerminalStructuralInputs::default()
        }
    }
}

pub(super) fn establish(
    program: &TerminalProgram,
    installation: &terminal_interpreter::TerminalRuntimeSupplies,
    provisions: Option<&TerminalRuntimeProvisions>,
) -> Result<EstablishedEntry, TerminalRuntimeError> {
    let entry = program
        .entry_installed(installation)
        .map_err(TerminalRuntimeError::Requirements)?;
    let recipes = provisions
        .map(TerminalRuntimeProvisions::private_entry_ram)
        .unwrap_or_default();
    if !entry.scalar_parameters.is_empty() || entry.placed_view_inputs().count() != recipes.len() {
        return Err(TerminalRuntimeError::missing(
            "program-entry-arrival-values",
            "isolated hosted entry does not supply scalar arguments or borrowed placed-view inputs",
        ));
    }
    if provisions.is_some_and(|provisions| {
        provisions.target_slot().schema != target::ProgramEntrySchema::HostedApplication
    }) {
        return Err(TerminalRuntimeError::missing(
            "program-entry-arrival-values",
            "this isolated runtime establishes a hosted application root, not freestanding image/storage arrivals",
        ));
    }
    let mut arrival = EstablishedEntry {
        arguments: Vec::new(),
        zero_initialized: Vec::new(),
        erased_fields: Vec::new(),
        private_entry_ram: recipes.to_vec(),
        establishments: Vec::new(),
    };
    // Every retained argument has an explicit installed private-RAM recipe.
    // No ordinary reference receives zeroed backing from its signature.
    for (ordinal, parameter) in entry.structural_parameters.iter().enumerate() {
        let index = recipes
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or(TerminalRuntimeError::InvalidProvision(
                "entry parameter has no checked private RAM construction",
            ))?;
        let recipe = &recipes[index];
        if recipe.terminal() != program.terminal_psi()
            || recipe.input().machine
                != program
                    .entry_installed(installation)
                    .map_err(TerminalRuntimeError::Requirements)?
                    .machine
            || recipe.correspondence().carrier != parameter.structural_type
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "private entry RAM recipe differs from its actual invocation",
            ));
        }
        let (original_identity, descriptor_identity) =
            terminal_installation::private_entry_ram_identity_pair(program.module(), index).ok_or(
                TerminalRuntimeError::InvalidProvision(
                    "private entry RAM identity namespace is exhausted",
                ),
            )?;
        arrival.arguments.push(TerminalStructuralValue::minted(
            descriptor_identity,
            parameter.structural_type,
            Vec::new(),
        ));
        arrival
            .establishments
            .push(terminal_installation::TerminalPlacedViewEstablishment {
                input: recipe.input().clone(),
                referent: TerminalStructuralValue::minted(
                    original_identity,
                    recipe.schema(),
                    Vec::new(),
                ),
            });
    }
    Ok(arrival)
}
