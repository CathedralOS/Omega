//! Authored selection occurrences and their checked rebase when a resolved forest grows.
use crate::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionRecordError, AuthoredDeclarationSelectionSuffixRebase,
    AuthoredDeclarationSelectionSuffixRebaseError, AuthoredDeclarationSelections,
    SymbolResolvedTrees, domain, expression, statement,
};
use source_text::SourceSpan;
struct PreparedOccurrenceRebase {
    expressions: expression::PreparedExpressionSelectionRebase,
    tree_arguments: Vec<(
        arena::Handle<statement::Statement>,
        expression::PreparedStaticArgumentRebase,
    )>,
    folded_arguments: Vec<(
        arena::Handle<statement::StatementNode>,
        expression::PreparedStaticArgumentRebase,
    )>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredSelectionExtensionFrontier {
    selections: usize,
    expressions: usize,
    tree_statements: usize,
    statements: usize,
    transition_targets: usize,
    proof_facts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSelectionOccurrenceStore {
    Expressions,
    TreeStatements,
    Statements,
    TransitionTargets,
    ProofFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSelectionExtensionRebaseError {
    Ledger(AuthoredDeclarationSelectionSuffixRebaseError),
    StoreFrontierOutOfRange(AuthoredSelectionOccurrenceStore),
    OccurrenceOutsideOwnerFrontier(AuthoredSelectionOccurrenceStore),
}

impl SymbolResolvedTrees {
    pub fn authored_selection_extension_frontier(&self) -> AuthoredSelectionExtensionFrontier {
        AuthoredSelectionExtensionFrontier {
            selections: self.tables.authored_declaration_selections.len(),
            expressions: self.expression_table().expression_count(),
            tree_statements: self.declarations().state_statements.len(),
            statements: self.statement_table().statement_count(),
            transition_targets: self.statement_table().transition_target_count(),
            proof_facts: self.declarations().proof_facts.len(),
        }
    }

    pub fn rebase_authored_selection_extension(
        self,
        frontier: AuthoredSelectionExtensionFrontier,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<Self, (Self, AuthoredSelectionExtensionRebaseError)> {
        let (ledger, rebase) = match self
            .tables
            .authored_declaration_selections
            .replace_prefix_and_rebase_suffix(frontier.selections, destination_base)
        {
            Ok(joined) => joined,
            Err(error) => {
                return Err((self, AuthoredSelectionExtensionRebaseError::Ledger(error)));
            }
        };
        let expressions = match self.prepare_occurrence_rebase(frontier, rebase) {
            Ok(prepared) => prepared,
            Err(error) => return Err((self, error)),
        };
        let mut trees = self;
        let PreparedOccurrenceRebase {
            expressions,
            tree_arguments,
            folded_arguments,
        } = expressions;
        trees
            .expression_table_mut()
            .apply_authored_selection_rebase(expressions);
        for (handle, prepared) in tree_arguments {
            let arguments = trees
                .expression_table_mut()
                .apply_static_argument_rebase(prepared);
            let statement::Statement::Call(call) =
                trees.declarations_mut().state_statements.get_mut(handle)
            else {
                unreachable!("prepared source call remains current")
            };
            call.machine_arguments = arguments;
        }
        {
            let bodies = &mut trees.tables.bodies;
            bodies
                .statements
                .apply_static_argument_rebases(&mut bodies.expressions, folded_arguments);
        }
        trees.apply_tree_statement_rebase(frontier.tree_statements, rebase);
        trees.statement_table_mut().remap_selection_occurrences(
            frontier.statements,
            frontier.transition_targets,
            rebase,
        );
        trees.apply_proof_fact_rebase(frontier.proof_facts, rebase);
        trees.tables.authored_declaration_selections = ledger;
        Ok(trees)
    }

    fn prepare_occurrence_rebase(
        &self,
        frontier: AuthoredSelectionExtensionFrontier,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<PreparedOccurrenceRebase, AuthoredSelectionExtensionRebaseError> {
        if frontier.expressions > self.expression_table().expression_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::Expressions,
                ),
            );
        }
        let expressions = self
            .expression_table()
            .prepare_authored_selection_rebase(frontier.expressions, rebase)
            .map_err(|()| {
                AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                    AuthoredSelectionOccurrenceStore::Expressions,
                )
            })?;

        self.validate_tree_statement_rebase(frontier.tree_statements, rebase)?;

        if frontier.statements > self.statement_table().statement_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::Statements,
                ),
            );
        }
        if frontier.transition_targets > self.statement_table().transition_target_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::TransitionTargets,
                ),
            );
        }
        self.tables
            .bodies
            .statements
            .validate_selection_occurrence_remap(
                frontier.statements,
                frontier.transition_targets,
                rebase,
            )
            .map_err(|store| {
                AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(match store {
                    statement::AuthoredSelectionStatementStore::Statements => {
                        AuthoredSelectionOccurrenceStore::Statements
                    }
                    statement::AuthoredSelectionStatementStore::TransitionTargets => {
                        AuthoredSelectionOccurrenceStore::TransitionTargets
                    }
                })
            })?;

        self.validate_proof_fact_rebase(frontier.proof_facts, rebase)?;
        let mut tree_arguments = Vec::new();
        for (index, (handle, statement)) in self.declarations().state_statements.iter().enumerate()
        {
            let statement::Statement::Call(call) = statement else {
                continue;
            };
            let prepared = self
                .expression_table()
                .prepare_static_argument_rebase(&call.machine_arguments, |occurrence| {
                    if index < frontier.tree_statements {
                        rebase.retain_base(occurrence)
                    } else {
                        rebase.rebase_appended(occurrence)
                    }
                })
                .map_err(|()| {
                    AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                        AuthoredSelectionOccurrenceStore::TreeStatements,
                    )
                })?;
            if index >= frontier.tree_statements {
                tree_arguments.push((handle, prepared));
            }
        }
        let folded_arguments = self
            .statement_table()
            .prepare_static_argument_rebases(self.expression_table(), frontier.statements, rebase)
            .map_err(|()| {
                AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                    AuthoredSelectionOccurrenceStore::Statements,
                )
            })?;
        Ok(PreparedOccurrenceRebase {
            expressions,
            tree_arguments,
            folded_arguments,
        })
    }

    fn validate_tree_statement_rebase(
        &self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionExtensionRebaseError> {
        let statements = &self.declarations().state_statements;
        if frontier > statements.len() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::TreeStatements,
                ),
            );
        }
        for (index, (_, statement)) in statements.iter().enumerate() {
            for occurrence in tree_statement_occurrences(statement) {
                let valid = if index < frontier {
                    rebase.retain_base(occurrence)
                } else {
                    rebase.rebase_appended(occurrence)
                };
                if valid.is_none() {
                    return Err(
                        AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                            AuthoredSelectionOccurrenceStore::TreeStatements,
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn apply_tree_statement_rebase(
        &mut self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) {
        let statements = &mut self.declarations_mut().state_statements;
        statements.for_each_mut(|handle, statement| {
            if usize::try_from(handle.arena_index()).expect("tree statement index overflow")
                > frontier
            {
                rebase_tree_statement_occurrences(statement, rebase);
            }
        });
    }

    fn validate_proof_fact_rebase(
        &self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionExtensionRebaseError> {
        let facts = &self.declarations().proof_facts;
        if frontier > facts.len() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::ProofFacts,
                ),
            );
        }
        for (index, (_, fact)) in facts.iter().enumerate() {
            let domain::ProofFact::Membership(membership) = fact else {
                continue;
            };
            let Some(occurrence) = membership.authored_domain_selection else {
                continue;
            };
            let valid = if index < frontier {
                rebase.retain_base(occurrence)
            } else {
                rebase.rebase_appended(occurrence)
            };
            if valid.is_none() {
                return Err(
                    AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                        AuthoredSelectionOccurrenceStore::ProofFacts,
                    ),
                );
            }
        }
        Ok(())
    }

    fn apply_proof_fact_rebase(
        &mut self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) {
        let facts = &mut self.declarations_mut().proof_facts;
        facts.for_each_mut(|handle, fact| {
            if usize::try_from(handle.arena_index()).expect("proof fact index overflow") <= frontier
            {
                return;
            }
            let domain::ProofFact::Membership(membership) = fact else {
                return;
            };
            let Some(occurrence) = membership.authored_domain_selection else {
                return;
            };
            membership.authored_domain_selection = Some(
                rebase
                    .rebase_appended(occurrence)
                    .expect("proof membership occurrence was validated before mutation"),
            );
        });
    }

    pub fn record_resolved_authored_declaration_selection(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: symbols::SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables.authored_declaration_selections.record_resolved(
            source_span,
            exposure,
            kind,
            selected_symbol,
        )
    }

    pub fn record_resolved_authored_declaration_selection_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<crate::source_selections::CompilerDerivedSelectionPartition>,
        selected_symbol: symbols::SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_resolved_in_partition(
                source_span,
                exposure,
                kind,
                compiler_partition,
                selected_symbol,
            )
    }

    pub fn record_late_bound_authored_declaration_selection(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_late_bound(source_span, exposure, kind, late_binding)
    }

    pub fn finalize_authored_declaration_selection(
        &mut self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        selected_symbol: symbols::SymbolHandle,
    ) -> Result<(), crate::source_selections::AuthoredDeclarationSelectionFinalizationError> {
        self.tables
            .authored_declaration_selections
            .finalize_late_bound(occurrence, expected_binding, selected_symbol)
    }

    pub fn finalize_intrinsic_authored_declaration_selection(
        &mut self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        intrinsic: source_declarations::declaration_selection::AuthoredDeclarationSelectionIntrinsic,
    ) -> Result<(), crate::source_selections::AuthoredDeclarationSelectionFinalizationError> {
        self.tables
            .authored_declaration_selections
            .finalize_intrinsic(occurrence, expected_binding, intrinsic)
    }

    pub fn record_late_bound_authored_declaration_selection_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<crate::source_selections::CompilerDerivedSelectionPartition>,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_late_bound_in_partition(
                source_span,
                exposure,
                kind,
                compiler_partition,
                late_binding,
            )
    }

    pub fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        &self.tables.authored_declaration_selections
    }

    pub fn retain_top_level_static_argument_position(
        &mut self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
        position: usize,
    ) -> Result<(), crate::source_selections::AuthoredDeclarationSelectionFinalizationError> {
        self.tables
            .authored_declaration_selections
            .retain_top_level_static_argument_position(occurrence, position)
    }
}

fn tree_statement_occurrences(
    statement: &statement::Statement,
) -> impl Iterator<Item = AuthoredDeclarationSelectionOccurrenceId> {
    let mut occurrences = [None, None];
    match statement {
        statement::Statement::Call(call) => occurrences[0] = call.authored_call_selection,
        statement::Statement::Transition(transition) => {
            if let statement::TransitionTarget::Named(target) = &transition.target {
                occurrences[0] = target.authored_call_selection;
            }
            if let Some(statement::TransitionTarget::Named(target)) = &transition.continuation {
                occurrences[1] = target.authored_call_selection;
            }
        }
        _ => {}
    }
    occurrences.into_iter().flatten()
}

fn rebase_tree_statement_occurrences(
    statement: &mut statement::Statement,
    rebase: AuthoredDeclarationSelectionSuffixRebase,
) {
    match statement {
        statement::Statement::Call(call) => {
            if let Some(occurrence) = call.authored_call_selection {
                call.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree call occurrence was validated before mutation"),
                );
            }
        }
        statement::Statement::Transition(transition) => {
            if let statement::TransitionTarget::Named(target) = &mut transition.target
                && let Some(occurrence) = target.authored_call_selection
            {
                target.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree transition occurrence was validated before mutation"),
                );
            }
            if let Some(statement::TransitionTarget::Named(target)) = &mut transition.continuation
                && let Some(occurrence) = target.authored_call_selection
            {
                target.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree continuation occurrence was validated before mutation"),
                );
            }
        }
        _ => {}
    }
}
