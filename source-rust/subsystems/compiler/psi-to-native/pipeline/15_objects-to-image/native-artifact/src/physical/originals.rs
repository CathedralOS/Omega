//! Original accepted code is an explicit input to the physical proof checker.
//! Object and image producers never receive these executable graphs. The checker
//! keeps each private body's namespace and coverage when it rejoins final bytes.

use boundary_applications::TerminalBoundaryApplicationCoverage;
use function_identity::MachineFunctionIdentity;
use terminal_codec::{RetainedTerminalCode, TerminalExecutionView};
use terminal_psi::TerminalPsiIdentity;

#[derive(Clone)]
pub struct PrivatePhysicalOriginal {
    code: RetainedTerminalCode,
    coverage: TerminalBoundaryApplicationCoverage,
}

impl std::fmt::Debug for PrivatePhysicalOriginal {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("PrivatePhysicalOriginal")
            .field("identity", &self.code.view().custody_identity())
            .field("coverage", &self.coverage)
            .finish()
    }
}

impl PrivatePhysicalOriginal {
    /// Retaining an original grants no authority to its object. The independent
    /// checker still checks the linked body's identity, scope and physical spans.
    pub fn new(
        expected: TerminalPsiIdentity,
        code: RetainedTerminalCode,
        coverage: TerminalBoundaryApplicationCoverage,
    ) -> Result<Self, &'static str> {
        if expected != code.view().identity() {
            return Err("private physical original differs from its emitted namespace");
        }
        coverage.validate_for_terminal(expected)?;
        Ok(Self { code, coverage })
    }

    pub(crate) fn view(&self) -> TerminalExecutionView<'_> {
        self.code.view()
    }
    pub(crate) fn coverage(&self) -> &TerminalBoundaryApplicationCoverage {
        &self.coverage
    }
}

/// Verification inputs are separate from the program consumed by object and
/// image construction. A missing original leaves the existing explicit proof
/// gap; duplicates, foreign namespaces and originals for absent bodies refuse.
#[derive(Debug, Clone, Default)]
pub struct NativePhysicalOriginals {
    private: Vec<(MachineFunctionIdentity, PrivatePhysicalOriginal)>,
}

impl NativePhysicalOriginals {
    pub fn new(
        private: Vec<(MachineFunctionIdentity, PrivatePhysicalOriginal)>,
    ) -> Result<Self, &'static str> {
        for (position, (identity, _)) in private.iter().enumerate() {
            if private[..position]
                .iter()
                .any(|(previous, _)| previous == identity)
            {
                return Err("physical verification repeats a private body original");
            }
        }
        Ok(Self { private })
    }

    pub(crate) fn private(
        &self,
        identity: MachineFunctionIdentity,
    ) -> Option<&PrivatePhysicalOriginal> {
        self.private
            .iter()
            .find(|(key, _)| *key == identity)
            .map(|(_, original)| original)
    }

    pub(crate) fn require_roster(
        &self,
        object: &image_emission::ObjectArtifact,
    ) -> Result<(), &'static str> {
        if self.private.iter().any(|(identity, _)| {
            !object
                .private_functions()
                .iter()
                .any(|body| body.identity == *identity)
        }) {
            return Err("physical verification retains an original for an absent private body");
        }
        Ok(())
    }
}
