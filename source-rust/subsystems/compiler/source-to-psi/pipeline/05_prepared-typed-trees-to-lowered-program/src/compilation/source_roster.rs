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
    Ok(lower_roster_closure(roster, &typed)?.complete())
}

/// A roster lowered as far as its closure. Product provider settlement reads
/// the closure to decide whether the roster needs other providers or layouts;
/// only the roster it keeps is completed into a lowered program, so the
/// conformance witnesses of a discarded roster are never prepared.
pub struct LoweredRosterClosure<'source> {
    owner: &'source crate::PreparedTypedTrees,
    roster: SourceCompilationRoster,
    prepared: crate::compilation::PreparedClosure<'source>,
}

/// Lower the closure of the exact roots and their normalizations over the
/// frozen owner (`LoweredRosterClosure`).
pub fn lower_roster_closure(
    roster: SourceCompilationRoster,
    typed: &crate::PreparedTypedTrees,
) -> Result<LoweredRosterClosure<'_>, Vec<Diagnostic>> {
    let prepared = {
        let calling = roster.normalization.calling.borrow(typed.trees());
        crate::compilation::prepare_closure(
            typed,
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
            terminal_codec::TerminalPublication::Portable,
        )?
    };
    Ok(LoweredRosterClosure {
        owner: typed,
        roster,
        prepared,
    })
}

impl<'source> LoweredRosterClosure<'source> {
    /// The frozen source the closure was lowered from.
    pub fn source(&self) -> &'source crate::PreparedTypedTrees {
        self.owner
    }

    pub fn roster(&self) -> &SourceCompilationRoster {
        &self.roster
    }

    /// The lowered closure, when the bound shapes formed and reading it
    /// refused nothing.
    pub fn closure(&self) -> Option<&crate::Elaboration> {
        self.prepared.closure()
    }

    /// The roster, for lowering again with more supplies.
    pub fn into_roster(self) -> SourceCompilationRoster {
        self.roster
    }

    /// Prepare everything the closure owes besides itself and bind the
    /// result to its source and roster. This is the lowered program
    /// `lower_roster` produces for the same roster.
    pub fn complete(self) -> crate::LoweredSourcePublication {
        let publication = terminal_codec::TerminalPublication::Portable;
        let purpose = if self.roster.inspection_root.is_some() {
            terminal_check_tables::RootPurpose::Inspection
        } else {
            terminal_check_tables::RootPurpose::Execution
        };
        crate::compilation::bind_source(
            self.owner.clone(),
            publication,
            self.prepared.complete(purpose),
            Some(self.roster),
        )
    }
}
