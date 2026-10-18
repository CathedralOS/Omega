//! The source producer checks and reduces applications once. Checking borrows
//! these terms alongside their source captures; neither kernel Int typing nor
//! an uninterpreted assumption supplies a source operation's value law.
//! This invocation-owned view is discarded after checking, not encoded as a
//! second program or proof bundle.

use arena::{Arena, HandleSpan};
use numerics::bignum::BigInt;
use proof_admission::{TermArena, TermHandle};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[derive(Debug, Default)]
pub struct PreparedMathematicalApplications {
    pub applications: Arena<PreparedApplication>,
    pub captures: Arena<PreparedCapture>,
    pub constants: Arena<PreparedConstant>,
}

#[derive(Debug, Default)]
pub struct PreparedApplication {
    pub expression: ExpressionHandle,
    pub term: TermHandle,
    pub captures: HandleSpan<PreparedCapture>,
}

#[derive(Debug, Default)]
pub struct PreparedCapture {
    pub expression: ExpressionHandle,
    pub source_type: TypeReferenceHandle,
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
}

impl<'program> PreparedMathematicalView<'program> {
    pub fn application(
        self,
        expression: ExpressionHandle,
    ) -> Option<&'program PreparedApplication> {
        self.applications
            .applications
            .iter()
            .find_map(|(_, application)| {
                (application.expression == expression).then_some(application)
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

/// Read an already reduced constant application without redoing conversion.
pub(crate) fn application_spine(
    view: PreparedMathematicalView<'_>,
    mut term: TermHandle,
) -> Option<(u32, Vec<TermHandle>)> {
    let mut arguments = Vec::new();
    loop {
        match view.arena.get(term) {
            proof_admission::Term::Apply { function, argument } => {
                arguments.push(argument);
                term = function;
            }
            proof_admission::Term::Constant { declaration, .. } => {
                arguments.reverse();
                return Some((declaration, arguments));
            }
            _ => return None,
        }
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
