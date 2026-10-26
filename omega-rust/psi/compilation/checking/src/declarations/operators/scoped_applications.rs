//! Operator selection under an authentic lexical scope, independent of flow.
//!
//! Executable occurrences and declaration proof terms supply their own operand
//! carriers and entry contracts. Selection does not create invocation events or
//! establish the selected preconditions. Applied binders remain attached to the
//! public owner for the consumer's separate formation judgment.

use checked_trees::{CheckedOperatorResolutionStatus, CheckedValueOrigin};
use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::operator::{
    BoundTraitOperatorMeaning, OperatorOperandBindings, ScopedTraitOperatorMeaning,
    SelectedTraitOperatorMeaning, SpelledOperator,
};
use typed_trees::types::TypeReferenceHandle;

/// `origin` is present only for a genuine executable occurrence: it allows
/// selection to inspect that occurrence's preceding local initializers. A
/// declaration uses its own owner and already selected declaration Requires.
pub struct OperatorApplicationScope<'scope> {
    pub owner: SymbolHandle,
    pub origin: Option<CheckedValueOrigin>,
    pub requires: &'scope [&'scope ProofFact],
    pub expression_type: &'scope dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
}

pub enum ScopedOperatorApplication<'program> {
    Declared {
        declaration: SpelledOperator<'program>,
        bindings: OperatorOperandBindings,
    },
    BoundTrait(BoundTraitOperatorMeaning<'program>),
    Trait(ScopedTraitOperatorMeaning<'program>),
    SelectedTrait {
        meaning: SelectedTraitOperatorMeaning<'program>,
        bindings: OperatorOperandBindings,
    },
}

impl ScopedOperatorApplication<'_> {
    pub fn public_symbol(&self) -> SymbolHandle {
        match self {
            Self::Declared { declaration, .. } => declaration.operator.symbol,
            Self::BoundTrait(meaning) => meaning.requirement.symbol,
            Self::Trait(meaning) => meaning.requirement.symbol,
            Self::SelectedTrait { meaning, .. } => meaning.requirement.symbol,
        }
    }
}

/// Candidate order is retained for ordinary diagnostics. Only `Resolved`
/// selects a public application; builtin, missing and ambiguous are distinct.
pub struct ScopedOperatorResolution<'program> {
    pub candidates: Vec<ScopedOperatorApplication<'program>>,
    pub status: CheckedOperatorResolutionStatus,
    pub selected_symbol: SymbolHandle,
}

impl<'program> ScopedOperatorResolution<'program> {
    pub fn selected(&self) -> Option<&ScopedOperatorApplication<'program>> {
        (self.status == CheckedOperatorResolutionStatus::Resolved).then_some(())?;
        self.candidates
            .iter()
            .find(|candidate| candidate.public_symbol() == self.selected_symbol)
    }

    /// Move the selected application out of construction scratch. Borrowed
    /// declaration identities keep the program lifetime; owned substitution
    /// rosters travel with the application rather than borrowing this result.
    pub fn into_selected(self) -> Option<ScopedOperatorApplication<'program>> {
        (self.status == CheckedOperatorResolutionStatus::Resolved).then_some(())?;
        self.candidates
            .into_iter()
            .find(|candidate| candidate.public_symbol() == self.selected_symbol)
    }
}

pub fn scoped_operator_application<'program>(
    program: &'program dyn crate::CheckingInput,
    scope: &OperatorApplicationScope<'_>,
    expression: ExpressionHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> ScopedOperatorResolution<'program> {
    if program.expression_table.binary_node(expression).is_some_and(|binary| {
        crate::proof_contracts::bound_expression_meaning::has_exact_typed_case_membership_meaning(
            program, expression, binary, operand_types.first().copied().flatten(),
        )
    }) {
        return ScopedOperatorResolution {
            candidates: Vec::new(),
            status: CheckedOperatorResolutionStatus::BuiltinFallback,
            selected_symbol: SymbolHandle::default(),
        };
    }
    // Wholly anonymous arithmetic has exact rational meaning before any
    // receiving carrier. Wildcard matching against visible primitive overloads
    // cannot choose that carrier. Reuse the operation owner's eligibility,
    // including retained authored selection custody; formation and destination
    // admission remain separate judgments below the selection boundary.
    if typed_trees::closed_numeric::has_anonymous_operator_meaning(program, expression)
        && program
            .expression_table
            .binary_node(expression)
            .is_some_and(|binary| {
                [binary.left, binary.right].into_iter().all(|operand| {
                    crate::value_custody::literals::has_anonymous_numeric_results(program, operand)
                })
            })
    {
        return ScopedOperatorResolution {
            candidates: Vec::new(),
            status: CheckedOperatorResolutionStatus::BuiltinFallback,
            selected_symbol: SymbolHandle::default(),
        };
    }
    let machine = program
        .machine_holding_state(scope.owner)
        .map_or(scope.owner, |machine| machine.symbol);
    let mut candidates = Vec::new();
    if scope.owner.is_valid() {
        let parent = program.symbols.get(scope.owner).parent;
        if let Some(definition) = program.trait_definition_by_symbol(parent)
            && program
                .trait_machine_signature_by_symbol(definition, scope.owner)
                .is_some()
        {
            candidates.extend(
                typed_trees::operator::trait_scope_operator_meanings(
                    program,
                    definition,
                    spelling,
                    operand_types,
                )
                .into_iter()
                .map(ScopedOperatorApplication::Trait),
            );
        }
    }
    if candidates.is_empty() {
        candidates.extend(
            typed_trees::operator::bound_trait_operator_meanings(
                program,
                machine,
                spelling,
                operand_types,
            )
            .into_iter()
            .map(ScopedOperatorApplication::BoundTrait),
        );
    }
    if candidates.is_empty() {
        candidates.extend(
            typed_trees::operator::selected_trait_operator_meanings(
                program,
                machine,
                spelling,
                operand_types,
            )
            .into_iter()
            .filter_map(|meaning| {
                let bindings = typed_trees::operator::trait_operator_operand_bindings(
                    program,
                    meaning.trait_definition,
                    meaning.requirement,
                    operand_types,
                    None,
                )?;
                Some(ScopedOperatorApplication::SelectedTrait { meaning, bindings })
            }),
        );
    }
    if !candidates.is_empty() {
        let (status, selected_symbol) = if let [selected] = candidates.as_slice() {
            (
                CheckedOperatorResolutionStatus::Resolved,
                selected.public_symbol(),
            )
        } else {
            (
                CheckedOperatorResolutionStatus::Ambiguous,
                SymbolHandle::invalid(),
            )
        };
        return ScopedOperatorResolution {
            candidates,
            status,
            selected_symbol,
        };
    }
    let declared = typed_trees::operator::resolve_spelling_for_operands(
        program,
        spelling,
        operand_types,
        program.expression_table.source_span(expression),
    );
    let operands = program
        .expression_table
        .binary_node(expression)
        .map(|binary| vec![binary.left, binary.right])
        .unwrap_or_default();
    let active = |domain| {
        operands
            .iter()
            .zip(operand_types)
            .any(|(operand, reference)| {
                reference.is_some_and(|reference| {
                    typed_trees::domain::type_selects_semantic_domain(program, reference, domain)
                }) || super::selection::scoped_mint_selects_domain(
                    program,
                    scope.origin,
                    *operand,
                    domain,
                ) || super::selection::signature_facts_select_domain(
                    program,
                    scope.requires,
                    *operand,
                    domain,
                )
            })
    };
    let carriers = program
        .expression_table
        .binary_node(expression)
        .is_some_and(|binary| {
            let [left, right] = operand_types else {
                return false;
            };
            crate::value_custody::expression_types::builtin_binary_result(
                program,
                binary,
                [*left, *right],
                &mut |operand| (scope.expression_type)(operand),
            )
            .is_some()
                || [binary.left, binary.right].into_iter().all(|operand| {
                    crate::value_custody::literals::has_anonymous_numeric_results(program, operand)
                })
        });
    let builtin = carriers
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            scope.owner,
            expression,
            spelling,
            operand_types,
        );
    let domain_builtin = carriers
        && typed_trees::closed_numeric::has_builtin_anonymous_operands(
            program, expression, spelling,
        );
    let identities = declared
        .iter()
        .map(|candidate| {
            (
                candidate.operator.symbol,
                candidate
                    .domain
                    .map_or_else(SymbolHandle::invalid, |domain| domain.symbol),
            )
        })
        .collect::<Vec<_>>();
    let (status, selected_symbol) = super::selection::select_scoped_candidates(
        &identities,
        active,
        builtin || (declared.iter().any(|candidate| candidate.domain.is_some()) && domain_builtin),
    );
    for declaration in declared {
        // Retain precisely the bindings from candidate matching. Missing
        // operands may leave a partial roster; consumers must demand the
        // categories their public application actually needs.
        if let Some(bindings) = typed_trees::operator::operator_operand_bindings(
            program,
            declaration.operator,
            operand_types,
        ) {
            candidates.push(ScopedOperatorApplication::Declared {
                declaration,
                bindings,
            });
        }
    }
    ScopedOperatorResolution {
        candidates,
        status,
        selected_symbol,
    }
}

pub(super) fn occurrence_requires(
    program: &dyn crate::CheckingInput,
    origin: CheckedValueOrigin,
) -> Vec<&ProofFact> {
    let CheckedValueOrigin::StateStatement { state_symbol, .. } = origin else {
        return Vec::new();
    };
    program
        .machine_holding_state(state_symbol)
        .into_iter()
        .flat_map(|machine| program.machine_contracts(machine))
        .chain(
            program
                .state_by_symbol(state_symbol)
                .into_iter()
                .flat_map(|state| program.state_contracts(state)),
        )
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .collect()
}
