//! The source producer checks and reduces applications once. Checking borrows
//! these terms alongside their source captures; neither kernel Int typing nor
//! an uninterpreted assumption supplies a source operation's value law.
//! Normalized terms and operation meanings remain invocation-owned scratch.
//! Original checked applications/captures transfer to the ordinary proof ledger
//! beside their signature; no second program or accepted proof is fabricated.

use arena::{Arena, Handle};
use numerics::bignum::BigInt;
use proof_admission::{TermArena, TermHandle};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub use checked_trees::CheckedMathematicalApplicationCapture as PreparedCapture;

#[derive(Debug, Default)]
pub struct PreparedMathematicalApplications {
    pub applications: Arena<PreparedApplication>,
    pub source_applications: Arena<checked_trees::CheckedMathematicalApplication>,
    pub captures: Arena<PreparedCapture>,
    pub constants: Arena<PreparedConstant>,
}

#[derive(Debug, Default)]
pub struct PreparedApplication {
    /// Exact original formed application; normalization cannot replace source
    /// occurrence or capture custody with a second scratch copy.
    pub source_application: Handle<checked_trees::CheckedMathematicalApplication>,
    pub term: TermHandle,
}

/// One original prepared read, including the precise source application
/// coordinate a judgment-use producer can retain. Reading it is not acceptance
/// of the application proposition or a complete proof dependency receipt.
#[derive(Clone, Copy, Debug)]
pub struct PreparedApplicationView<'program> {
    pub source_application: Handle<checked_trees::CheckedMathematicalApplication>,
    pub original: &'program checked_trees::CheckedMathematicalApplication,
    pub term: TermHandle,
}

#[derive(Debug, Default)]
pub struct PreparedConstant {
    pub declaration: u32,
    pub meaning: PreparedConstantMeaning,
}

#[derive(Debug, Default)]
pub enum PreparedConstantMeaning {
    /// A selected source operation with no arithmetic law in this consumer.
    /// Its function spine still retains every input on which its value depends.
    #[default]
    Opaque,
    Integer(BigInt),
    Boolean(bool),
    IntegerCarrier,
    BooleanCarrier,
    AddressCarrier,
    AddressBound(SymbolHandle),
    IntegerOperation(BinaryOperator),
    IntegerRelation(BinaryOperator),
    Negation,
    IntegerEmbedding {
        primitive: PrimitiveType,
        minimum: BigInt,
        maximum: Option<BigInt>,
    },
    /// Selected carrier and policy remain source custody even when the kernel
    /// uses the same Int domain for several fixed integer carriers.
    SourceOperation {
        expression: ExpressionHandle,
        source_type: TypeReferenceHandle,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct PreparedMathematicalView<'program> {
    pub arena: &'program TermArena,
    pub applications: &'program PreparedMathematicalApplications,
    reads: Option<&'program super::uses::MathematicalApplicationReads>,
}

impl<'program> PreparedMathematicalView<'program> {
    /// Borrow the original prepared owner without a source judgment read scope.
    pub fn new(
        arena: &'program TermArena,
        applications: &'program PreparedMathematicalApplications,
    ) -> Self {
        Self {
            arena,
            applications,
            reads: None,
        }
    }

    pub(super) fn observing(
        mut self,
        reads: &'program super::uses::MathematicalApplicationReads,
    ) -> Self {
        self.reads = Some(reads);
        self
    }

    pub fn application(
        self,
        expression: ExpressionHandle,
    ) -> Option<PreparedApplicationView<'program>> {
        self.applications
            .applications
            .iter()
            .find_map(|(_, application)| {
                let original = self
                    .applications
                    .source_applications
                    .get(application.source_application);
                if original.expression != expression {
                    return None;
                }
                if let Some(reads) = self.reads {
                    reads.record(application.source_application);
                }
                Some(PreparedApplicationView {
                    source_application: application.source_application,
                    original,
                    term: application.term,
                })
            })
    }

    pub fn constant(self, declaration: u32) -> Option<&'program PreparedConstantMeaning> {
        self.applications
            .constants
            .iter()
            .find_map(|(_, constant)| {
                (constant.declaration == declaration).then_some(&constant.meaning)
            })
    }
}

/// The exact selected embedding owns these bounds. A source spelling or the
/// shared kernel Int domain cannot synthesize an address or fixed-carrier law.
pub fn integer_embedding_meaning(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<PreparedConstantMeaning> {
    let (_, argument) =
        crate::proof_contracts::proof_embeddings::integer_embedding_argument(program, expression)?;
    let embedding = crate::proof_contracts::proof_embeddings::integer_embedding(program, argument)?;
    Some(PreparedConstantMeaning::IntegerEmbedding {
        primitive: embedding.primitive,
        minimum: embedding.minimum,
        maximum: embedding.maximum,
    })
}
