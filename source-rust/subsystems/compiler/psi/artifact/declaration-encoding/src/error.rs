use semantic_vocabulary::PropositionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    NonCanonicalOrder(&'static str),
    NonCanonicalEncoding,
    NestedConjunction,
    NestedDisjunction,
    /// A term-table row names its own or a later entry.
    TermTableForwardReference(&'static str),
    /// A term-table row names entries other than the unconsumed ones
    /// immediately before it, or a single-term table has other than one root.
    NonCanonicalTermTable(&'static str),
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedStructuralFoundation(&'static str),
    MalformedWriteFrame(String),
    MalformedCallableResult {
        machine: Option<semantic_vocabulary::MachineId>,
        reason: String,
    },
    MalformedRanking(&'static str),
    MalformedScalarReturnFoundation {
        machine: String,
        domain: String,
        edge: semantic_vocabulary::EdgeId,
        source: semantic_vocabulary::ValueId,
        result: semantic_vocabulary::ValueId,
        reason: String,
    },
    MalformedOperationFoundation {
        machine: String,
        state: String,
        block: semantic_vocabulary::BlockId,
        operation: semantic_vocabulary::OperationId,
        reason: &'static str,
    },
    MalformedStructuralOperation {
        machine: String,
        operation: semantic_vocabulary::OperationId,
        reason: String,
    },
    AccessPlan(String),
    NativeLayout(String),
    AccessViewPlan(terminal_psi::access::AccessViewPlanError),
    AccessRamCorrespondence(String),
    AccessViewInput(terminal_psi::access::AccessViewInputError),
    MalformedMathematicalCertificate(&'static str),
    InvalidIdentityFormation(String),
    MalformedProposition(PropositionError),
    IntegerWidthClosure(semantic_vocabulary::IntegerWidthClosureError),
    ObligationLedgerMismatch,
    /// The encoded input is larger than the receiving host admits. This is a
    /// resource refusal, not malformed input.
    InputTooLarge {
        limit: u64,
        actual: u64,
    },
    /// Loading would exceed the receiving host's overall work budget in the
    /// named phase. This is a resource refusal, not malformed input or a
    /// failed semantic check.
    LoadWorkExhausted {
        phase: &'static str,
        limit: u64,
    },
}

impl CodecError {
    /// Whether a receiving host's resource limit refused the input, as opposed
    /// to the input being malformed or semantically refused.
    pub const fn is_resource_refusal(&self) -> bool {
        matches!(
            self,
            Self::InputTooLarge { .. } | Self::LoadWorkExhausted { .. }
        )
    }
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CodecError {}

pub fn malformed<T>(message: &'static str) -> Result<T, CodecError> {
    Err(CodecError::MalformedStructuralFoundation(message))
}
