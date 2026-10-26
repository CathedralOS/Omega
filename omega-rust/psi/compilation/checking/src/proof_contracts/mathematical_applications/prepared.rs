//! The source producer checks and reduces applications once. Checking borrows
//! these terms alongside their source captures; neither kernel Int typing nor
//! an uninterpreted assumption supplies a source operation's value law.
//! This invocation-owned view is discarded after checking, not encoded as a
//! second program or proof bundle.

use arena::{Arena, HandleSpan};
use numerics::bignum::BigInt;
use proof_admission::{Declaration, TermArena, TermHandle};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[derive(Debug, Default)]
pub struct PreparedMathematicalApplications {
    pub applications: Arena<PreparedApplication>,
    pub captures: Arena<PreparedCapture>,
    pub constants: Arena<PreparedConstant>,
    pub source_evidence: Arena<PreparedSourceEvidence>,
    pub source_constructors: Arena<PreparedSourceConstructor>,
    pub source_carriers: Arena<PreparedSourceCarrier>,
    pub indexed_theorems: Arena<PreparedIndexedTheorem>,
    pub safety_assumptions: Vec<u32>,
}

/// Kernel induction associated with an exact source contract occurrence.
/// Body validation and safety receiver admission are separate judgments.
#[derive(Debug, Default)]
pub struct PreparedIndexedTheorem {
    pub machine: SymbolHandle,
    pub expression: ExpressionHandle,
    pub ensure_ordinal: u32,
    pub evidence_formal: SymbolHandle,
    pub source_type: TypeReferenceHandle,
    pub theorem: TermHandle,
    pub ty: TermHandle,
    pub assumption_closure: Vec<u32>,
}

/// An independently typed source evidence occurrence in this view's signature.
#[derive(Debug, Default)]
pub struct PreparedSourceEvidence {
    pub expression: ExpressionHandle,
    pub source_type: TypeReferenceHandle,
    pub type_identity: typed_trees::type_identity::TypeIdentityId,
    pub term: TermHandle,
    pub ty: TermHandle,
}

/// A constructor derivation abstracted over its exact source values, erased
/// children and entry premises. These are checked function terms, not new
/// assumptions and not closed evidence actuals for an arbitrary destination.
#[derive(Debug, Default, Clone)]
pub struct PreparedSourceConstructor {
    pub expression: ExpressionHandle,
    pub constructor: ExpressionHandle,
    pub source_type: TypeReferenceHandle,
    pub type_identity: typed_trees::type_identity::TypeIdentityId,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub schema: TermHandle,
    pub ty: TermHandle,
    pub parameters: Vec<SymbolHandle>,
    pub predicate_expressions: Vec<ExpressionHandle>,
}

/// Source carrier formation retained under its exact value telescope.
#[derive(Debug, Default)]
pub struct PreparedSourceCarrier {
    pub source_type: TypeReferenceHandle,
    pub type_identity: typed_trees::type_identity::TypeIdentityId,
    pub ty: TermHandle,
    pub domains: Vec<TermHandle>,
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
    /// Authenticated IEEE comparison, never the kernel's reflexive identity.
    FloatRelation {
        operator: BinaryOperator,
        format: numerics::literals::FloatFormat,
        reference: TypeReferenceHandle,
    },
    FloatLiteral {
        format: numerics::literals::FloatFormat,
        bits: u64,
    },
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
    pub declarations: &'program [Declaration],
    pub applications: &'program PreparedMathematicalApplications,
}

impl<'program> PreparedMathematicalView<'program> {
    pub fn source_constructor(
        self,
        expression: ExpressionHandle,
    ) -> Option<&'program PreparedSourceConstructor> {
        self.applications
            .source_constructors
            .iter()
            .find_map(|(_, row)| (row.expression == expression).then_some(row))
    }

    pub fn source_evidence(
        self,
        expression: ExpressionHandle,
    ) -> Option<&'program PreparedSourceEvidence> {
        self.applications
            .source_evidence
            .iter()
            .find_map(|(_, evidence)| (evidence.expression == expression).then_some(evidence))
    }

    pub fn source_carrier(
        self,
        reference: TypeReferenceHandle,
    ) -> Option<&'program PreparedSourceCarrier> {
        self.applications
            .source_carriers
            .iter()
            .find_map(|(_, carrier)| (carrier.source_type == reference).then_some(carrier))
    }

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
