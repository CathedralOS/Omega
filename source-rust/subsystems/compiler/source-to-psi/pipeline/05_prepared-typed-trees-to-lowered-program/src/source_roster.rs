//! Scoped source roots and normalization selected by Build. The source owner
//! prepares and judges this roster before a product can publish its code.

use diagnostics::Diagnostic;

pub use lowered_source_publication::source_roster::{
    SourceCallingContext, SourceCallingInputs, SourceCompilationRoster, SourceNormalizationInputs,
    SourceProgramEntryInput,
};

/// Lower the exact roots and their normalizations over the frozen owner.
/// The lowered program retains the roster so a later publication cannot
/// substitute different roots.
pub fn lower_roster(
    roster: SourceCompilationRoster,
    typed: crate::PreparedTypedTrees,
) -> Result<crate::LoweredSourcePublication, Vec<Diagnostic>> {
    let publication = terminal_codec::TerminalPublication::Portable;
    let prepared = {
        let calling = roster.normalization.calling.borrow(typed.trees());
        crate::prepare_compilation(
            &typed,
            crate::Compilation {
                declaration_metadata: &roster.declaration_metadata,
                entries: roster.publication_entries(),
                subjects: std::slice::from_ref(&roster.subject),
                purpose: source_provenance::DependencyScope::Product,
                selected: &roster.selected,
                bindings: &roster.bindings,
                supplies: &roster.supplies,
                calling: calling.selection(),
                providers: roster.normalization.providers(typed.trees()),
            },
            publication,
        )?
    };
    Ok(crate::compilation::bind_source(
        typed,
        publication,
        prepared,
        Some(roster),
    ))
}
