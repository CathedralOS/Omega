//! Normalize declared callable shape and facts under the supplying binder scope.
//!
//! Callers supply the normalization callbacks because a type application must
//! retain its outer substitutions, while checking may replace a conservation
//! expression with its established plan. Reach, progress and the other signature
//! envelope fields remain with the complete signature that owns this projection.

use crate::source_state::TypedTrees;
use crate::source_state::domain::ProofFact;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::signature::{SignatureContract, SignatureContractKind, StateParameter};
use crate::source_state::type_identity::NormalizedTypeIdentity;
use crate::source_state::types::TypeReferenceHandle;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeclaredContractFact {
    Expression(String),
    Membership {
        value: String,
        domain: String,
        arguments: Vec<NormalizedTypeIdentity>,
    },
    Proposition(String),
    ContentConservation(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclaredContractKind<Cause = semantic_vocabulary::CrashCause> {
    Requires,
    Ensures,
    EnsuresForResultCase { data: String, case: String },
    Crashes(Cause),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredContractClause<Cause = semantic_vocabulary::CrashCause> {
    pub kind: DeclaredContractKind<Cause>,
    pub fact: Option<DeclaredContractFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredContractParameter {
    pub mutable: bool,
    pub constant: bool,
    pub relevance: language_core::BindingRelevance,
    pub type_identity: NormalizedTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredCallableContract<Cause = semantic_vocabulary::CrashCause> {
    pub parameters: Vec<DeclaredContractParameter>,
    pub result: NormalizedTypeIdentity,
    pub clauses: Vec<DeclaredContractClause<Cause>>,
}

/// The supplying operation owns the context captured by these callbacks.
pub struct ContractIdentityContext<Type, Fact, Symbol, Cause> {
    pub type_identity: Type,
    pub fact_identity: Fact,
    pub symbol_identity: Symbol,
    pub crash_cause: Cause,
}

pub fn generic_binders(
    parameters: &[crate::source_state::data::TypeParameter],
) -> Vec<(SymbolHandle, String)> {
    parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| (parameter.symbol, format!("$G{position}")))
        .collect()
}

pub fn callable_binders(
    generics: &[(SymbolHandle, String)],
    parameters: &[StateParameter],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = generics.to_vec();
    binders.extend(
        parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| (parameter.symbol, format!("$V{position}"))),
    );
    binders
}

/// Project with the complete binder list supplied by the enclosing operation.
pub fn declared_callable_with<Cause, Error, Type, Fact, Symbol, CauseProjection>(
    program: &TypedTrees,
    parameters: arena::RootRows<'_, StateParameter>,
    result: TypeReferenceHandle,
    contracts: arena::RootRows<'_, SignatureContract>,
    binders: &[(SymbolHandle, String)],
    include_crashes: bool,
    projection: &mut ContractIdentityContext<Type, Fact, Symbol, CauseProjection>,
) -> Result<DeclaredCallableContract<Cause>, Error>
where
    Cause: Clone + Ord,
    Type: FnMut(
        TypeReferenceHandle,
        &[(SymbolHandle, String)],
    ) -> Result<NormalizedTypeIdentity, Error>,
    Fact: FnMut(&ProofFact, &[(SymbolHandle, String)]) -> Result<DeclaredContractFact, Error>,
    Symbol: FnMut(SymbolHandle, &[(SymbolHandle, String)]) -> String,
    CauseProjection: FnMut(crate::source_state::signature::CrashCause) -> Cause,
{
    let parameters = parameters
        .iter()
        .map(|parameter| {
            Ok(DeclaredContractParameter {
                mutable: parameter.is_mutable,
                constant: parameter.is_const,
                relevance: parameter.relevance,
                type_identity: (projection.type_identity)(parameter.type_reference, binders)?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let result = (projection.type_identity)(result, binders)?;
    let clauses =
        declared_contracts_with(program, contracts, binders, include_crashes, projection)?;
    Ok(DeclaredCallableContract {
        parameters,
        result,
        clauses,
    })
}

pub fn declared_contracts_with<Cause, Error, Type, Fact, Symbol, CauseProjection>(
    program: &TypedTrees,
    contracts: arena::RootRows<'_, SignatureContract>,
    binders: &[(SymbolHandle, String)],
    include_crashes: bool,
    projection: &mut ContractIdentityContext<Type, Fact, Symbol, CauseProjection>,
) -> Result<Vec<DeclaredContractClause<Cause>>, Error>
where
    Cause: Clone + Ord,
    Fact: FnMut(&ProofFact, &[(SymbolHandle, String)]) -> Result<DeclaredContractFact, Error>,
    Symbol: FnMut(SymbolHandle, &[(SymbolHandle, String)]) -> String,
    CauseProjection: FnMut(crate::source_state::signature::CrashCause) -> Cause,
{
    let mut clauses = Vec::new();
    let mut crashes = Vec::<(Cause, bool, Vec<DeclaredContractFact>)>::new();
    for contract in contracts {
        let facts = program.proof_facts.span_or_empty(contract.facts);
        let mut project = || {
            facts
                .iter()
                .map(|fact| (projection.fact_identity)(fact, binders))
                .collect::<Result<Vec<_>, _>>()
        };
        let kind = match contract.kind {
            SignatureContractKind::Requires => DeclaredContractKind::Requires,
            SignatureContractKind::Ensures => DeclaredContractKind::Ensures,
            SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } => DeclaredContractKind::EnsuresForResultCase {
                data: (projection.symbol_identity)(result_data, binders),
                case: (projection.symbol_identity)(result_case, binders),
            },
            SignatureContractKind::Crashes { cause } => {
                if !include_crashes {
                    continue;
                }
                let cause = (projection.crash_cause)(cause);
                let position = crashes
                    .iter()
                    .position(|bucket| bucket.0 == cause)
                    .unwrap_or_else(|| {
                        crashes.push((cause, false, Vec::new()));
                        crashes.len() - 1
                    });
                let bucket = &mut crashes[position];
                let projected = project()?;
                if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                    bucket.1 = true;
                } else {
                    bucket.2.extend(projected);
                }
                continue;
            }
        };
        clauses.extend(project()?.into_iter().map(|fact| DeclaredContractClause {
            kind: kind.clone(),
            fact: Some(fact),
        }));
    }
    for (cause, unconditional, mut routes) in crashes {
        let kind = DeclaredContractKind::Crashes(cause);
        if unconditional {
            clauses.push(DeclaredContractClause { kind, fact: None });
        } else {
            routes.sort();
            routes.dedup();
            clauses.extend(routes.into_iter().map(|fact| DeclaredContractClause {
                kind: kind.clone(),
                fact: Some(fact),
            }));
        }
    }
    clauses.sort();
    Ok(clauses)
}

/// Project one fact using the caller's current type and expression context.
pub fn project_declared_fact_with(
    program: &TypedTrees,
    fact: &ProofFact,
    mut expression_identity: impl FnMut(ExpressionHandle) -> Option<String>,
    mut type_identity: impl FnMut(TypeReferenceHandle) -> Option<NormalizedTypeIdentity>,
    mut symbol_identity: impl FnMut(SymbolHandle) -> String,
    mut content_conservation: impl FnMut(ExpressionHandle) -> Option<Vec<u8>>,
) -> Option<DeclaredContractFact> {
    Some(match fact {
        ProofFact::Expression(expression) => {
            if let Some(plan) = content_conservation(*expression) {
                return Some(DeclaredContractFact::ContentConservation(plan));
            }
            DeclaredContractFact::Expression(expression_identity(*expression)?)
        }
        ProofFact::Membership(membership) => DeclaredContractFact::Membership {
            value: expression_identity(membership.value)?,
            domain: symbol_identity(membership.domain_symbol),
            arguments: program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments)
                .iter()
                .map(|argument| type_identity(*argument))
                .collect::<Option<Vec<_>>>()?,
        },
        ProofFact::Proposition(application) => {
            let binder_labels = application
                .binder_arguments
                .iter()
                .map(|argument| {
                    if let Some(literal) = &argument.const_literal {
                        return literal.value_bignum().map(|value| value.to_string());
                    }
                    let identity = symbol_identity(argument.symbol);
                    if let Some(projection) = &argument.evidence_projection {
                        Some(format!("{identity}.{}", projection.member))
                    } else {
                        Some(identity)
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            let argument_labels = program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| expression_identity(*argument))
                .collect::<Option<Vec<_>>>()?;
            let formula = program.normalize_proposition_application(
                application,
                Some(crate::source_state::proposition::PropositionLabels {
                    binder_labels: &binder_labels,
                    argument_labels: &argument_labels,
                }),
            )?;
            DeclaredContractFact::Proposition(formula.identity_label())
        }
    })
}

pub fn is_true_crash_route(program: &TypedTrees, fact: &ProofFact) -> bool {
    matches!(
        fact,
        ProofFact::Expression(expression)
            if matches!(
                program.expression_table.expression(*expression),
                crate::source_state::expression::ExpressionNode::Boolean(true)
            )
    )
}

/// Retain one contract subject by its resolved root and declared members.
/// This forms an identity path only. Its consumer still checks the path against
/// the root's actual type and proves any required case or index formation.
/// The caller supplies the permitted roots: a normal-result promise permits
/// `result`, while a content clause may also name its ordinary parameters.
pub fn structural_subject_with(
    program: &TypedTrees,
    owner: SymbolHandle,
    mut expression: ExpressionHandle,
    mut root: impl FnMut(
        &crate::source_state::expression::TableNamePath,
    ) -> Result<declaration_contracts::content::ContentPlaceRoot, &'static str>,
) -> Result<declaration_contracts::content::ContentStructuralPlace, &'static str> {
    use crate::source_state::expression::ExpressionNode;
    use declaration_contracts::content::{
        ContentCaseSegment, ContentFieldSegment, ContentPlaceSegment, ContentPlaceVersion,
        ContentStructuralPlace,
    };
    let mut reversed = Vec::new();
    let mut remaining = program.expression_table.expression_count();
    loop {
        if remaining == 0 || !program.expression_table.expression_is_valid(expression) {
            return Err("contract subject expression is missing or cyclic");
        }
        remaining -= 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(name) if !name.static_application.is_valid() => {
                reversed.reverse();
                return Ok(ContentStructuralPlace {
                    version: ContentPlaceVersion::Current,
                    root: root(name)?,
                    segments: reversed,
                });
            }
            ExpressionNode::Member(member) => {
                let symbol = member.selection.field_symbol();
                let field = program
                    .unique_declared_field_by_symbol(symbol)
                    .ok_or("contract subject field has no exact declaration")?;
                if !field.member_identity().is_valid() {
                    return Err("contract subject field lost its member identity");
                }
                reversed.push(ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol,
                    name: field.path_identity(),
                }));
                let parent = program.symbols.get(symbol).parent;
                if !program.symbols.symbols().nodes().is_valid(parent) {
                    return Err("contract subject field lost its owner");
                }
                if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                    let (_, case) = program
                        .variant_owner(parent)
                        .ok_or("contract subject field lost its exact case")?;
                    reversed.push(ContentPlaceSegment::Case(ContentCaseSegment {
                        symbol: parent,
                        name: case.path_identity(),
                    }));
                }
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let index = program
                    .closed_integer_expression_value_in(indexed.index, owner)
                    .and_then(|value| value.to_u64())
                    .ok_or("contract subject index is not an exact closed element")?;
                reversed.push(ContentPlaceSegment::FixedIndex(index));
                expression = indexed.collection;
            }
            _ => return Err("contract subject is not a resolved root or structural projection"),
        }
    }
}
