//! Source expressions owe the same operand and result obligations as a body.
//! These witnesses carry no executable publication and assume no body summary.

use language_semantics::const_value::CanonicalConstIdentity;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::{TypedTrees, expression::ExpressionHandle, types::TypeReferenceHandle};

/// Only source-declared input conditions can be premises. An explicit subset
/// is authenticated against that declaration and kept in its authored order.
#[derive(Clone, Copy)]
pub enum SourceExpressionAssumptions<'a> {
    Published,
    Explicit(&'a [ExpressionHandle]),
}

#[derive(Clone, Copy)]
pub enum SourceExpressionValue<'a> {
    Constant(&'a CanonicalConstIdentity),
    Expression(ExpressionHandle),
    Symbol(SymbolHandle),
}

/// A real source formal and its supplied operand. The formal's declaration and
/// carrier are authenticated before reading the operand in the supplier scope.
#[derive(Clone, Copy)]
pub struct SourceExpressionBinding<'a> {
    pub symbol: SymbolHandle,
    pub carrier: TypeReferenceHandle,
    pub value: SourceExpressionValue<'a>,
    /// A foreign static formal belongs to this exact applied row. A symbol
    /// alone cannot identify a copied signature after substitution.
    pub signature: Option<&'a typed_trees::signature::StateSignature>,
}

impl<'a> SourceExpressionBinding<'a> {
    pub fn input(
        symbol: SymbolHandle,
        carrier: TypeReferenceHandle,
        value: SourceExpressionValue<'a>,
    ) -> Self {
        Self {
            symbol,
            carrier,
            value,
            signature: None,
        }
    }
}

/// A scalar projection of an authenticated raw zero subject. The reader's
/// source-zero owner checks the actual declaration path before substitution.
#[derive(Clone)]
pub struct SourceExpressionProjectionBinding {
    pub expression: ExpressionHandle,
    pub carrier: TypeReferenceHandle,
    pub value: CanonicalConstIdentity,
}

#[derive(Clone, Copy)]
pub struct SourceExpressionContext<'a> {
    pub lexical_owner: SymbolHandle,
    pub state: Option<SymbolHandle>,
    /// Copied anonymous signatures require their exact row, not a symbol scan.
    pub signature: Option<&'a typed_trees::signature::StateSignature>,
    pub types: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub scope_starts: &'a [usize],
    pub bindings: &'a [SourceExpressionBinding<'a>],
    pub projections: &'a [SourceExpressionProjectionBinding],
    pub assumptions: SourceExpressionAssumptions<'a>,
}

impl SourceExpressionContext<'_> {
    pub fn declaration(lexical_owner: SymbolHandle) -> Self {
        Self {
            lexical_owner,
            state: None,
            signature: None,
            types: &[],
            scope_starts: &[],
            bindings: &[],
            projections: &[],
            assumptions: SourceExpressionAssumptions::Published,
        }
    }
}

#[derive(Clone, Copy)]
pub enum SourceExpressionGoal {
    Formed { expected: TypeReferenceHandle },
    Establish,
}

#[derive(Clone, Copy)]
pub(crate) enum WitnessGoal {
    Expression(SourceExpressionGoal),
    PremiseFormation,
}

/// The source clauses fixed solely by a signature's static value tuple.
/// Clauses mentioning runtime formals remain invocation obligations instead.
pub fn static_tuple_requirements(
    typed: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Result<Vec<ExpressionHandle>, &'static str> {
    use typed_trees::{
        domain::ProofFact,
        references::{TypeReferenceInputs, visit_expression},
        signature::SignatureContractKind,
    };
    let parameters = typed
        .state_parameters
        .span(signature.parameters)
        .ok_or("static requirement parameter span is stale")?;
    let generics = typed
        .data_type_parameters
        .span(signature.type_parameters)
        .ok_or("static requirement binder span is stale")?;
    let excluded = parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .chain(
            generics
                .iter()
                .filter(|parameter| !parameter.kind.is_const_or_value())
                .map(|parameter| parameter.symbol),
        )
        .collect::<Vec<_>>();
    let contracts = typed
        .signature_contracts
        .span(signature.contracts)
        .ok_or("static requirement clause span is stale")?;
    let mut requirements = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in typed
            .proof_facts
            .span(contract.facts)
            .ok_or("static requirement fact span is stale")?
        {
            let ProofFact::Expression(expression) = fact else {
                return Err(
                    "static tuple proof needs the retained membership or proposition evidence lane",
                );
            };
            if !typed.expression_table.expression_is_valid(*expression) {
                return Err("static tuple requirement expression is stale");
            }
            let mut inputs = TypeReferenceInputs::default();
            visit_expression(typed, &mut inputs, *expression);
            if !inputs
                .symbols
                .iter()
                .any(|symbol| excluded.contains(symbol))
            {
                requirements.push(*expression);
            }
        }
    }
    Ok(requirements)
}

/// Each declared premise is formed using only earlier premises. Consequently a
/// condition cannot justify forming its own subtraction, index or call operand.
pub fn elaborate_source_expression_witnesses<'a>(
    typed: &'a TypedTrees,
    expression: ExpressionHandle,
    goal: SourceExpressionGoal,
    context: SourceExpressionContext<'a>,
) -> Checked<Vec<crate::Elaboration>> {
    let program = crate::read::Program::new(typed, false);
    let premises = program.source_expression_premises(context)?;
    let mut witnesses = Vec::with_capacity(premises.len() + 1);
    for (position, premise) in premises.iter().copied().enumerate() {
        let mut program = crate::read::Program::new(typed, false);
        let (entry, body) = program.source_expression_witness(
            premise,
            WitnessGoal::PremiseFormation,
            context,
            &premises[..position],
        )?;
        let mut bodies = semantic_vocabulary::IdTable::default();
        bodies.push(body);
        witnesses.push(crate::finish_logical_witness(program, entry, bodies)?);
    }
    let mut program = crate::read::Program::new(typed, false);
    let (entry, body) = program.source_expression_witness(
        expression,
        WitnessGoal::Expression(goal),
        context,
        &premises,
    )?;
    let mut bodies = semantic_vocabulary::IdTable::default();
    bodies.push(body);
    witnesses.push(crate::finish_logical_witness(program, entry, bodies)?);
    Ok(witnesses)
}
