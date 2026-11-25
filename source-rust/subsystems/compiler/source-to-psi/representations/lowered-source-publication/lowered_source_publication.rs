//! What stage 05 was asked to lower, and what it kept about the source beside
//! the lowered program. The driver hands the request to lowering. Publication
//! reads the correspondence after the checker has judged the lowered program.
#![forbid(unsafe_code)]

mod compilation;
mod correspondence;
pub mod provider_selections;
pub mod selection_questions;
pub mod source_roster;

pub use compilation::{CallingPolicySelections, Compilation, ProgramEntryContractSelection};
pub use correspondence::{
    DesignatedOperands, RequirementInvocation, RequirementReview, RequirementReviewMetadata,
    RequirementService, SourceCorrespondence, SourceFormCorrespondence,
};

use lowered_source_publication_issuer::LoweringIssuer;
use source_declarations::{declaration_selection, source_selections};
use terminal_check_tables::compilation::{CompilationForPublication, TerminalCompilation};
use prepared_source::PreparedTypedTrees;
use typed_trees::source_state;

/// A frozen source and its publication mode, bound to the exact terminal
/// compilation and correspondence stage 05 derived from them.
///
/// The checker takes this value, reads only the terminal compilation, and
/// returns the source with its verdicts. Stage 07 accepts a judgment of this
/// input type and no other.
pub struct LoweredSourcePublication {
    program: TerminalCompilation,
    source: PublicationSource,
}

pub struct PublicationSource {
    pub owner: PreparedTypedTrees,
    pub publication: terminal_codec::TerminalPublication,
    pub correspondence: SourceCorrespondence,
    pub roster: Option<source_roster::SourceCompilationRoster>,
}

impl CompilationForPublication for LoweredSourcePublication {
    type Source = PublicationSource;

    fn into_program_and_source(self) -> (TerminalCompilation, Self::Source) {
        (self.program, self.source)
    }
}

impl LoweredSourcePublication {
    /// Bind the lowered program to the source it was lowered from. The issuer
    /// argument is the reason no other crate can call this: only stage 05 links
    /// the crate that supplies one, and `mbx xtask check-boundary` refuses any
    /// other link.
    pub fn from_lowering(
        _issuer: LoweringIssuer,
        program: TerminalCompilation,
        source: PublicationSource,
    ) -> Self {
        Self { program, source }
    }

    pub fn program(&self) -> &TerminalCompilation {
        &self.program
    }

    pub fn source(&self) -> &PublicationSource {
        &self.source
    }
}
