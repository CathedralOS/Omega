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
    typed: crate::lowering::PreparedTypedTrees,
) -> Result<crate::lowering::LoweredSourcePublication, Vec<Diagnostic>> {
    let publication = terminal_codec::TerminalPublication::Portable;
    let prepared = {
        let calling = roster.normalization.calling.borrow(typed.trees());
        crate::lowering::prepare_compilation(
            &typed,
            crate::lowering::Compilation {
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
    Ok(crate::lowering::compilation::bind_source(
        typed,
        publication,
        prepared,
        Some(roster),
    ))
}
