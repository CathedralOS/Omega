/*
Typing entrance (`lower_symbol_resolved_trees`), cohort typing and settlement
(`type_resolved_cohort` and `close_typed_cohort`), terminating-record progress
normalization (`progress`), target-machine family telescopes, and append-only
seeded continuation (`seeded_continuation`).
*/

use crate::contracts::proof_facts::normalize_membership_selections;
use crate::declarations::data::lower_data_definition;
use crate::declarations::domain::lower_domain_definition;
use crate::declarations::machine::{
    lower_machine, lower_token_binding_view, settle_satisfied_declarations_from,
};
use crate::declarations::mathematical::lower_mathematical_definition;
use crate::declarations::operator::lower_operator_definition;
use crate::declarations::proposition::lower_proposition_definition;
use crate::declarations::provider_defaults::finalize_provider_default_calls;
use crate::declarations::trait_definition::lower_trait_definition;
use crate::declarations::type_aliases::lower_type_alias_definition;
use crate::expressions::equatable::validate_equatable_conformances_in;
use crate::expressions::exhaustiveness::validate_case_dispatch_exhaustiveness_in;
use crate::expressions::expression::lower_expression_handle;
use crate::lowerer::progress::normalize_machine_progress_premises;
use crate::tables::resolved_expressions;
use crate::type_reference::domain_constraints::normalize_domain_constraints;
use crate::type_reference::{
    lower_type_reference_into_table, normalize_const_argument_carriers, validate_computed_arguments,
};
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::signature::AuthoredServiceReachTarget as ReachTarget;

pub(crate) mod name {
    use resolved::name::DiagnosticName;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub(crate) fn lower_name(name: &DiagnosticName) -> typed::name::Identifier {
        typed::name::Identifier::generated(name.as_str())
    }

    pub(crate) fn lower_names(names: &[DiagnosticName]) -> Vec<typed::name::Identifier> {
        names.iter().map(lower_name).collect()
    }
}
pub(crate) mod progress {
    use diagnostics::Diagnostic;
    use language_semantics::{
        ProgressPremise, ProgressSubject, TerminationGuarantee, TerminationInterface,
    };
    use typed::machine::resolve_satisfied_declaration;
    use typed_trees as typed;

    pub(crate) fn normalize_trait_progress_premises(
        program: &mut typed::TypedTrees,
    ) -> Result<(), Diagnostic> {
        let signature_updates = program
            .traits()
            .iter()
            .flat_map(|trait_definition| program.trait_machine_signatures(trait_definition))
            .map(|signature| {
                let guarantee = if signature.termination_guarantee.promises_termination() {
                    TerminationGuarantee::Terminates {
                        premises: authored_premises(
                            program,
                            program.state_signature_contracts(signature),
                            program.state_signature_parameters(signature),
                        )?,
                    }
                } else {
                    TerminationGuarantee::NoGuarantee
                };
                Ok((signature.symbol, guarantee))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        program
            .tables
            .trait_machine_signatures
            .for_each_mut(|_, signature| {
                if let Some((_, guarantee)) = signature_updates
                    .iter()
                    .find(|(symbol, _)| *symbol == signature.symbol)
                {
                    signature.termination_guarantee = guarantee.clone();
                }
            });

        Ok(())
    }

    pub(crate) fn normalize_machine_progress_premises(
        program: &mut typed::TypedTrees,
        machine_frontier: usize,
    ) -> Result<(), Diagnostic> {
        let authored_updates = program
            .machines()
            .iter()
            .skip(machine_frontier)
            .filter(|machine| {
                matches!(
                    machine.termination_plan.interface,
                    TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
                )
            })
            .map(|machine| {
                let parameters = program
                    .machine_states(machine)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default();
                let interface = TerminationInterface::Published(TerminationGuarantee::Terminates {
                    premises: authored_premises(
                        program,
                        program.machine_contracts(machine),
                        parameters,
                    )?,
                });
                Ok((machine.symbol, interface))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        for machine in program.machines_mut().iter_mut().skip(machine_frontier) {
            if let Some((_, interface)) = authored_updates
                .iter()
                .find(|(symbol, _)| *symbol == machine.symbol)
            {
                machine.termination_plan.interface = interface.clone();
            }
        }

        let inherited_updates = program
            .machines()
            .iter()
            .skip(machine_frontier)
            .filter(|machine| {
                !machine
                    .termination_plan
                    .interface
                    .published()
                    .is_some_and(TerminationGuarantee::promises_termination)
            })
            .map(|machine| Ok((machine.symbol, inherited_guarantee(program, machine)?)))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        for machine in program.machines_mut().iter_mut().skip(machine_frontier) {
            if let Some((_, Some(guarantee))) = inherited_updates
                .iter()
                .find(|(symbol, _)| *symbol == machine.symbol)
            {
                machine.termination_plan.interface =
                    TerminationInterface::Published(guarantee.clone());
            }
        }

        Ok(())
    }

    pub(crate) fn settle_machine_ranking_default(
        program: &typed::TypedTrees,
        symbols: &symbols::SymbolTable,
        machine: &mut typed::machine::Machine,
    ) {
        if let Some(view) = pending_ranking_default(program, symbols, machine)
            && let Some(witness) = &mut machine.termination_plan.implementation_witness
        {
            witness.ranking_view = view;
            witness.view_path = view.canonical_path().unwrap_or_default().to_string();
        }
    }

    fn pending_ranking_default(
        program: &typed::TypedTrees,
        symbols: &symbols::SymbolTable,
        machine: &typed::machine::Machine,
    ) -> Option<language_semantics::RankingViewId> {
        use language_semantics::RankingViewId;
        use typed::types::TypeReferenceNode;

        let witness = machine.termination_plan.implementation_witness.as_ref()?;
        if witness.ranking_view != RankingViewId::NULL
            || !witness.view_path.is_empty()
            || witness.subjects.len() != 1
            || !witness.view_arguments.is_empty()
        {
            return None;
        }
        let custody = program.ranking_expression_custody_for(machine.symbol)?;
        let [subject] = custody.subjects.as_slice() else {
            return None;
        };
        let typed::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(*subject)
        else {
            return None;
        };
        if !path.symbol.is_valid() || path.head_symbol != path.symbol {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        let mut reference = program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                let parameter = program
                    .machine_type_parameters(machine)
                    .iter()
                    .find(|parameter| parameter.symbol == path.symbol)?;
                match parameter.kind {
                    typed::data::TypeParameterKind::Value { type_reference }
                    | typed::data::TypeParameterKind::Const { type_reference } => {
                        Some(type_reference)
                    }
                    _ => None,
                }
            })?;
        for _ in 0..128 {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => {
                    return Some(RankingViewId::SLICE_LENGTH);
                }
                TypeReferenceNode::Named { symbol, .. }
                    if matches!(
                        symbols.builtin_type_atom(*symbol),
                        Some(
                            symbols::BuiltinTypeAtom::U8
                                | symbols::BuiltinTypeAtom::U16
                                | symbols::BuiltinTypeAtom::U32
                                | symbols::BuiltinTypeAtom::U64
                        )
                    ) =>
                {
                    return Some(RankingViewId::NAT_DESCENDING);
                }
                _ => return None,
            }
        }
        None
    }

    fn inherited_guarantee(
        program: &typed::TypedTrees,
        machine: &typed::machine::Machine,
    ) -> Result<Option<TerminationGuarantee>, Diagnostic> {
        for conformance in program.machine_trait_conformances(machine) {
            let Some(declaration) = resolve_satisfied_declaration(program, machine, conformance)
            else {
                continue;
            };
            let (mut guarantee, required_parameters, requirement_name) = match declaration {
                typed::machine::SatisfiedDeclaration::Trait { requirement, .. } => (
                    requirement.termination_guarantee.clone(),
                    program.state_signature_parameters(requirement),
                    &requirement.name,
                ),
                typed::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) => (
                    requirement
                        .termination_plan
                        .interface
                        .published()
                        .cloned()
                        .unwrap_or(TerminationGuarantee::NoGuarantee),
                    program
                        .machine_states(requirement)
                        .first()
                        .map(|state| program.state_parameters(state))
                        .unwrap_or_default(),
                    &requirement.name,
                ),
                typed::machine::SatisfiedDeclaration::Operator(_) => continue,
            };
            if let TerminationGuarantee::Terminates { premises } = &mut guarantee {
                let actual_parameters = program
                    .machine_states(machine)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default();
                for premise in premises {
                    let Some(index) = typed_trees::signature::state_parameter_position(
                        required_parameters,
                        premise.subject.root,
                    ) else {
                        return Err(Diagnostic::error(format!(
                            "progress premise inherited by `{}` is not rooted in requirement `{}`'s parameter telescope",
                            machine.name, requirement_name
                        )));
                    };
                    let Some(actual) = actual_parameters.get(index) else {
                        return Err(Diagnostic::error(format!(
                            "progress premise inherited by `{}` has no corresponding implementation parameter at position {index}",
                            machine.name
                        )));
                    };
                    premise.subject.root = actual.symbol;
                }
            }
            return Ok(Some(guarantee));
        }
        Ok(None)
    }

    fn authored_premises(
        program: &typed::TypedTrees,
        contracts: &[typed::signature::SignatureContract],
        parameters: &[typed::signature::StateParameter],
    ) -> Result<Vec<ProgressPremise>, Diagnostic> {
        let mut premises = Vec::new();
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == typed::signature::SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                    continue;
                };
                if domain.classification
                    != Some(language_semantics::DomainClassification::ProgressProfile)
                {
                    continue;
                }
                let Some(subject) = subject_path(program, membership.value, parameters) else {
                    return Err(Diagnostic::error(format!(
                        "progress premise for `{}` must name one identity-preserving parameter or field path",
                        domain.name
                    )));
                };
                let premise = ProgressPremise {
                    profile: domain.semantic_id,
                    subject,
                };
                if !premises.contains(&premise) {
                    premises.push(premise);
                }
            }
        }
        Ok(premises)
    }

    fn subject_path(
        program: &typed::TypedTrees,
        expression: typed::expression::ExpressionHandle,
        parameters: &[typed::signature::StateParameter],
    ) -> Option<ProgressSubject> {
        match program.expression_table.expression(expression) {
            typed::expression::ExpressionNode::Name(path)
                if program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
            {
                let identifier = &program.expression_table.name_path_members(path.members)[0];
                let name = identifier.as_str();
                let root = parameters
                    .iter()
                    .find(|parameter| path.symbol.is_valid() && parameter.symbol == path.symbol)
                    .or_else(|| {
                        parameters
                            .iter()
                            .find(|parameter| parameter.name.as_str() == name)
                    })
                    .map(|parameter| parameter.symbol)?;
                Some(ProgressSubject {
                    root,
                    projections: Vec::new(),
                })
            }
            typed::expression::ExpressionNode::Member(member) => {
                let mut subject = subject_path(program, member.receiver, parameters)?;
                let symbol = member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
                    .or_else(|| {
                        resolve_subject_member_symbol(
                            program,
                            &subject,
                            member.member.as_str(),
                            parameters,
                        )
                    })?;
                subject.projections.push(symbol);
                Some(subject)
            }
            _ => None,
        }
    }

    fn resolve_subject_member_symbol(
        program: &typed::TypedTrees,
        subject: &ProgressSubject,
        member_name: &str,
        parameters: &[typed::signature::StateParameter],
    ) -> Option<symbols::SymbolHandle> {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == subject.root)?;
        let mut position = parameter.type_reference;
        let mut substitution = Vec::new();
        for projection in &subject.projections {
            let field = match replay_subject_position(program, position, &substitution) {
                Some((data, bound)) => {
                    substitution = bound;
                    declared_field_with_symbol(program, data, *projection)?
                }
                None => {
                    substitution = Vec::new();
                    crate::typing_queries::exact_declared_field(program, *projection)?
                }
            };
            position = field.type_reference;
        }

        let (data, _) = replay_subject_position(program, position, &substitution)?;
        program.data_members(data).iter().find_map(|member| {
            let field = member.as_field()?;
            (field.name.as_str() == member_name).then_some(field.symbol)
        })
    }

    fn replay_subject_position<'program>(
        program: &'program typed::TypedTrees,
        position: typed::types::TypeReferenceHandle,
        substitution: &[(symbols::SymbolHandle, typed::types::TypeReferenceHandle)],
    ) -> Option<(
        &'program typed::data::DataDefinition,
        Vec<(symbols::SymbolHandle, typed::types::TypeReferenceHandle)>,
    )> {
        use typed::types::TypeReferenceNode;
        let mut reference = position;
        let mut substituted = Vec::new();
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if base_symbol.is_valid() => {
                    let data = program.data_definition_by_symbol(*base_symbol)?;
                    let bound = program
                        .data_type_parameters(data)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments)
                                .iter()
                                .copied(),
                        )
                        .collect();
                    return Some((data, bound));
                }
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::DynamicTrait { symbol, .. }
                    if symbol.is_valid() =>
                {
                    if !substituted.contains(symbol)
                        && let Some(argument) = substitution
                            .iter()
                            .find(|(parameter, _)| parameter == symbol)
                            .map(|(_, argument)| *argument)
                    {
                        substituted.push(*symbol);
                        reference = argument;
                        continue;
                    }
                    let data = program.data_definition_by_symbol(*symbol)?;
                    return Some((data, Vec::new()));
                }
                _ => return None,
            }
        }
    }

    fn declared_field_with_symbol<'program>(
        program: &'program typed::TypedTrees,
        data: &'program typed::data::DataDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'program typed::data::DataField> {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed::data::DataMember::Field(field) => (field.symbol == symbol).then_some(field),
                typed::data::DataMember::Variant(variant) => {
                    program.data_payload_field_by_symbol(variant, symbol)
                }
            })
    }
}
pub(crate) mod seeded_continuation {
    use super::{
        Lowerer, TypingCohort, append_authored_service_reach_rows, close_typed_cohort,
        lower_evidence_forwarding, lower_symbol_resolved_trees, type_resolved_cohort,
    };
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use language_semantics::{SemanticDomainId, SemanticDomainTable};
    use source::SourceMap;
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::machine::MachineBodyStatus;
    use symbols::{SourceScopedTopLevelBinding, SymbolHandle};
    use syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier;
    use typed_trees::TypedTrees;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SeededContinuationError {
        RetainedResolvedRootsChanged,
        CrossPairedResolvedBase,
        AuthoredSelectionPrefixChanged,
        Lowering(Diagnostic),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SeededTypingBase {
        resolved: Arc<SymbolResolvedTrees>,
        typed: Arc<TypedTrees>,
    }

    impl SeededTypingBase {
        pub fn resolved(&self) -> &SymbolResolvedTrees {
            &self.resolved
        }

        pub fn shared_resolved(&self) -> Arc<SymbolResolvedTrees> {
            Arc::clone(&self.resolved)
        }

        pub fn activate_body_completion(
            mut self,
            frontier: DeferredResolvedFrontier,
            sources: Arc<SourceMap>,
            bindings: Vec<SourceScopedTopLevelBinding>,
        ) -> Result<(Self, DeferredResolvedFrontier), Vec<Diagnostic>> {
            let frontier = self
                .join_prepared_allocations(frontier)?
                .activate_sources(sources, bindings)?;
            self.resolved = frontier.shared_trees();
            Arc::make_mut(&mut self.typed).symbols = self.resolved.symbols.clone();
            Ok((self, frontier))
        }

        pub fn append_deferred(
            self,
            extension: syntax_trees_to_symbol_resolved_trees::DeferredResolvedExtension,
        ) -> Result<(Self, DeferredResolvedFrontier), Vec<Diagnostic>> {
            let (source, frontier) = extension
                .rebase_for_typed_continuation(self.typed.authored_declaration_selections())?;
            // A source diagnostic found while typing the extension belongs to
            // the author as written; only a retained-base mismatch is internal.
            let typed = lower_seeded_extension(source, self).map_err(|reason| match reason {
                SeededContinuationError::Lowering(diagnostic) => vec![diagnostic],
                reason => vec![Diagnostic::error(format!(
                    "authored declaration typing could not extend its retained source: {reason:?}"
                ))],
            })?;
            Ok((typed, frontier))
        }

        /// Narrow this retained typing to the program written in `sources`'
        /// selected files, then give its symbols `current_sources`.
        ///
        /// The retained base was typed once for every source it holds. The
        /// program being compiled keeps the declarations of its selected
        /// files and the declarations the compiler derived for them; the
        /// resolved roots, service rows and evidence forwardings follow the
        /// typed declarations that remain, so a later continuation extends
        /// exactly this program. A selection that still names a declaration
        /// of an unselected file is the caller's to reject.
        pub fn activate_sources(
            mut self,
            sources: Arc<SourceMap>,
            current_sources: Arc<SourceMap>,
            bindings: Vec<SourceScopedTopLevelBinding>,
        ) -> Result<Self, Vec<Diagnostic>> {
            let mut resolved = Arc::unwrap_or_clone(self.resolved_base_for_extension()?);
            if resolved
                .symbols
                .source_map()
                .is_none_or(|previous| !sources.retains_source_storage(&previous))
            {
                return Err(vec![Diagnostic::error(
                    "source activation does not retain its paired source coordinates",
                )]);
            }
            let mut typed = typed_trees::declaration_projection::project_source_declarations(
                &self.typed,
                &sources,
            );
            let selection = typed_trees::references::DeclarationSelection::from_program(&typed);
            macro_rules! select_roots {
                ($($field:ident),+ $(,)?) => {
                    $(resolved.roots.$field.retain(|root| selection.contains(root.symbol));)+
                };
            }
            select_roots!(
                const_declarations,
                data_definitions,
                type_aliases,
                domain_definitions,
                machines,
                measures,
                operators,
                propositions,
                mathematical_definitions,
                traits,
                conformances,
                wire_schemas,
            );
            let retained_owner = |mut owner: SymbolHandle| {
                while owner.is_valid() {
                    if selection.contains(owner) {
                        return true;
                    }
                    owner = resolved.symbols.get(owner).parent;
                }
                false
            };
            resolved
                .authored_service_reach_rows
                .retain(|row| retained_owner(row.owner));
            resolved
                .evidence_forwardings
                .retain(|row| selection.contains(row.machine_symbol));
            let symbols = typed
                .symbols
                .clone()
                .begin_source_activation(sources, bindings.clone())
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                .finish()
                .begin_source_activation(current_sources, bindings)
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                .finish();
            resolved.symbols = symbols.clone();
            typed.symbols = symbols;
            resolved.resolution_indexes = Default::default();
            Ok(Self {
                resolved: Arc::new(resolved),
                typed: Arc::new(typed),
            })
        }

        pub fn join_prepared_allocations(
            &mut self,
            frontier: DeferredResolvedFrontier,
        ) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
            let frontier = frontier.with_prepared_allocations(
                &self.resolved,
                &self.typed.symbols,
                &self.typed.semantic_domains,
                &self.typed.service_reach_rows,
            )?;
            self.resolved = frontier.shared_trees();
            Ok(frontier)
        }

        pub fn complete_bodies(
            self,
            source: syntax_trees_to_symbol_resolved_trees::ResolvedBodyCompletion,
        ) -> Result<(Self, DeferredResolvedFrontier), Vec<Diagnostic>> {
            let source = source
            .rebase_authored_selections_for_typed_completion(
                self.typed.authored_declaration_selections(),
            )
            .map_err(|(_, error)| vec![Diagnostic::error(format!(
                "authored-body completion could not join its retained typed selection ledger: {error:?}"
            ))])?;
            lower_body_completion(source, self).map_err(|error| vec![error])
        }

        pub fn lower_extension(
            self,
            source: syntax_trees_to_symbol_resolved_trees::SeededSymbolResolvedTrees,
        ) -> Result<Self, Vec<Diagnostic>> {
            let rebased = source
            .rebase_authored_selections_for_typed_continuation(
                self.typed.authored_declaration_selections(),
            )
            .map_err(|(_, error)| {
                vec![Diagnostic::error(format!(
                    "generated-source authored-selection suffix could not join the retained typed base: {error:?}"
                ))]
            })?;
            lower_seeded_extension(rebased, self).map_err(|error| match error {
                SeededContinuationError::Lowering(diagnostic) => vec![diagnostic],
                SeededContinuationError::RetainedResolvedRootsChanged => vec![Diagnostic::error(
                    "source continuation changed a retained declaration root",
                )],
                error => vec![Diagnostic::error(format!(
                    "generated-source continuation violated its retained-base invariant: {error:?}"
                ))],
            })
        }

        pub fn resolved_base_for_extension(
            &mut self,
        ) -> Result<Arc<SymbolResolvedTrees>, Vec<Diagnostic>> {
            if !self
                .typed
                .symbols
                .is_generated_extension_of(&self.resolved.symbols)
            {
                return Err(vec![Diagnostic::error(
                    "generated-source continuation changed retained source custody or appended authored symbols",
                )]);
            }
            if !domain_table_retains(
                &self.resolved.semantic_domains,
                &self.typed.semantic_domains,
            ) || !self
                .typed
                .service_reach_rows
                .starts_with(&self.resolved.service_reach_rows)
            {
                return Err(vec![Diagnostic::error(
                    "generated-source continuation changed retained domain or service-row allocations",
                )]);
            }
            if self.typed.symbols != self.resolved.symbols
                || self.typed.semantic_domains != self.resolved.semantic_domains
                || self.typed.service_reach_rows != self.resolved.service_reach_rows
            {
                let prepared = Arc::make_mut(&mut self.resolved);
                prepared.symbols = self.typed.symbols.clone();
                prepared.semantic_domains = self.typed.semantic_domains.clone();
                prepared.service_reach_rows = self.typed.service_reach_rows.clone();
            }
            Ok(Arc::clone(&self.resolved))
        }

        pub fn typed(&self) -> &TypedTrees {
            &self.typed
        }

        pub fn typed_mut(&mut self) -> &mut TypedTrees {
            Arc::make_mut(&mut self.typed)
        }

        pub fn into_typed(self) -> TypedTrees {
            Arc::unwrap_or_clone(self.typed)
        }
    }

    pub fn lower_symbol_resolved_trees_to_seeded_base(
        resolved: impl Into<Arc<SymbolResolvedTrees>>,
    ) -> Result<SeededTypingBase, Diagnostic> {
        let resolved = resolved.into();
        let typed = lower_symbol_resolved_trees(&resolved)?;
        Ok(SeededTypingBase {
            resolved,
            typed: Arc::new(typed),
        })
    }

    pub fn lower_seeded_extension(
        source: syntax_trees_to_symbol_resolved_trees::RebasedSeededSymbolResolvedTrees,
        retained: SeededTypingBase,
    ) -> Result<SeededTypingBase, SeededContinuationError> {
        let (source, resolved_base) = source.into_typing_continuation_parts();
        if resolved_base != retained.resolved {
            return Err(SeededContinuationError::CrossPairedResolvedBase);
        }
        if !resolved_root_prefix_is_retained(&source, &resolved_base) {
            return Err(SeededContinuationError::RetainedResolvedRootsChanged);
        }
        let destination_ledger = retained.typed.authored_declaration_selections();
        if !source
            .authored_declaration_selections()
            .as_slice()
            .starts_with(destination_ledger.as_slice())
        {
            return Err(SeededContinuationError::AuthoredSelectionPrefixChanged);
        }

        let cohort = TypingCohort::continuing(&resolved_base, &retained.typed);
        if let Err(error) = super::validate_resolved_semantics_in(&source, &cohort) {
            return Err(SeededContinuationError::Lowering(error));
        }
        let mut candidate = Arc::unwrap_or_clone(retained.typed);
        if let Err(error) = join_resolved_semantics(&source, &resolved_base, &mut candidate) {
            return Err(SeededContinuationError::Lowering(error));
        }
        let mut lowerer = Lowerer {
            typed_trees: candidate,
            source_trees: &source,
            equality_scope: None,
            type_reference_exposure: Some(
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
            ),
            admits_contract_policy_operators: false,
        };
        if let Err(error) = type_resolved_cohort(&mut lowerer, &cohort) {
            return Err(SeededContinuationError::Lowering(error));
        }
        for forwarding in source
            .evidence_forwardings
            .iter()
            .skip(resolved_base.evidence_forwardings.len())
        {
            lowerer
                .typed_trees
                .evidence_forwardings
                .push(lower_evidence_forwarding(
                    forwarding,
                    &lowerer.typed_trees.proof_output_calls,
                ));
        }
        if let Err(error) = close_typed_cohort(&mut lowerer, &cohort) {
            return Err(SeededContinuationError::Lowering(error));
        }
        let typed = lowerer.typed_trees;
        Ok(SeededTypingBase {
            resolved: Arc::new(source),
            typed: Arc::new(typed),
        })
    }

    fn lower_body_completion(
        source: syntax_trees_to_symbol_resolved_trees::RebasedResolvedBodyCompletion,
        retained: SeededTypingBase,
    ) -> Result<(SeededTypingBase, DeferredResolvedFrontier), Diagnostic> {
        let (frontier, predecessor, selected) = source.into_typing_completion_parts();
        let source = frontier.shared_trees();
        if predecessor != retained.resolved {
            return Err(Diagnostic::error(
                "authored-body completion does not match the exact retained resolved predecessor",
            ));
        }
        if retained.typed.symbols != predecessor.symbols {
            return Err(Diagnostic::error(
                "authored-body completion found changed typed predecessor symbols",
            ));
        }
        if !source
            .authored_declaration_selections()
            .as_slice()
            .starts_with(retained.typed.authored_declaration_selections().as_slice())
        {
            return Err(Diagnostic::error(
                "authored-body completion did not retain the typed selection ledger prefix",
            ));
        }
        if !resolved_completion_retains_declarations(&predecessor, &source, &selected) {
            return Err(Diagnostic::error(
                "authored-body completion changed retained resolved declaration or unselected body storage",
            ));
        }
        if !resolved_normalized_prefix_is_retained(&predecessor, &source) {
            return Err(Diagnostic::error(
                "authored-body completion changed retained normalized statement, transition target or type storage",
            ));
        }
        let cohort = TypingCohort::continuing(&predecessor, &retained.typed).completing(&selected);
        super::validate_resolved_semantics_in(&source, &cohort)?;
        let mut candidate = Arc::unwrap_or_clone(retained.typed);
        join_resolved_semantics(&source, &predecessor, &mut candidate)?;
        let mut lowerer = Lowerer {
            typed_trees: candidate,
            source_trees: &source,
            equality_scope: None,
            type_reference_exposure: Some(
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
            ),
            admits_contract_policy_operators: false,
        };
        type_resolved_cohort(&mut lowerer, &cohort)?;
        for forwarding in source
            .evidence_forwardings
            .iter()
            .skip(predecessor.evidence_forwardings.len())
        {
            lowerer
                .typed_trees
                .evidence_forwardings
                .push(lower_evidence_forwarding(
                    forwarding,
                    &lowerer.typed_trees.proof_output_calls,
                ));
        }
        close_typed_cohort(&mut lowerer, &cohort)?;
        let typed = lowerer.typed_trees;
        Ok((
            SeededTypingBase {
                resolved: source,
                typed: Arc::new(typed),
            },
            frontier,
        ))
    }

    fn resolved_normalized_prefix_is_retained(
        base: &SymbolResolvedTrees,
        candidate: &SymbolResolvedTrees,
    ) -> bool {
        let old_statements = &base.tables.bodies.statements;
        let new_statements = &candidate.tables.bodies.statements;
        let old_types = &base.tables.types.references;
        let new_types = &candidate.tables.types.references;
        new_statements.statement_count() >= old_statements.statement_count()
            && new_statements.transition_target_count() >= old_statements.transition_target_count()
            && new_types.type_reference_count() >= old_types.type_reference_count()
            && (1..=old_statements.statement_count()).all(|ordinal| {
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    return false;
                };
                let handle =
                    symbol_resolved_trees::statement::StatementHandle::from_arena_index(ordinal);
                old_statements.statement(handle) == new_statements.statement(handle)
            })
            && (1..=old_statements.transition_target_count()).all(|ordinal| {
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    return false;
                };
                let handle =
                    symbol_resolved_trees::statement::TransitionTargetHandle::from_arena_index(
                        ordinal,
                    );
                old_statements.transition_target(handle) == new_statements.transition_target(handle)
            })
            && (1..=old_types.type_reference_count()).all(|ordinal| {
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    return false;
                };
                let handle =
                    symbol_resolved_trees::types::TypeReferenceHandle::from_arena_index(ordinal);
                old_types.type_reference(handle) == new_types.type_reference(handle)
            })
    }

    fn domain_table_retains(base: &SemanticDomainTable, candidate: &SemanticDomainTable) -> bool {
        candidate.len() >= base.len()
            && (1..=base.len()).all(|ordinal| {
                let identity = SemanticDomainId(ordinal as u32);
                candidate.name(identity) == base.name(identity)
            })
    }

    fn resolved_completion_retains_declarations(
        base: &SymbolResolvedTrees,
        candidate: &SymbolResolvedTrees,
        selected: &[SymbolHandle],
    ) -> bool {
        if candidate.machines.len() < base.machines.len()
            || selected
                .iter()
                .enumerate()
                .any(|(ordinal, symbol)| selected[..ordinal].contains(symbol))
            || selected.iter().any(|symbol| {
                !base
                    .machines
                    .iter()
                    .any(|machine| machine.symbol == *symbol)
            })
        {
            return false;
        }
        for (old, new) in base.machines.iter().zip(candidate.machines.iter()) {
            if !selected.contains(&old.symbol) {
                if old != new {
                    return false;
                }
                if base
                    .machine_state_handles(old.states)
                    .iter()
                    .any(|state| base.machine_state(*state) != candidate.machine_state(*state))
                    || base.machine_owned_data(old.owned_data)
                        != candidate.machine_owned_data(new.owned_data)
                    || base.machine_trait_conformances(old.satisfies)
                        != candidate.machine_trait_conformances(new.satisfies)
                {
                    return false;
                }
                continue;
            }
            if old.body_status != MachineBodyStatus::Deferred
                || new.body_status != MachineBodyStatus::Complete
            {
                return false;
            }
            let mut header = new.clone();
            header.body_status = old.body_status;
            header.termination_plan.implementation_witness =
                old.termination_plan.implementation_witness.clone();
            header.ranking_subjects = old.ranking_subjects;
            header.ranking_view = old.ranking_view;
            header.ranking_view_arguments = old.ranking_view_arguments;
            header.ranking_range = old.ranking_range;
            header.states = old.states;
            header.owned_data = old.owned_data;
            header.satisfies = old.satisfies;
            if header != *old {
                return false;
            }
            let old_states = base.machine_state_handles(old.states);
            let new_states = candidate.machine_state_handles(new.states);
            if !new_states.starts_with(old_states) {
                return false;
            }
            for &state in old_states {
                let old_state = base.machine_state(state);
                let mut new_state = candidate.machine_state(state).clone();
                new_state.statements = old_state.statements;
                new_state.statement_nodes = old_state.statement_nodes;
                if new_state != *old_state {
                    return false;
                }
            }
            let old_owned = base.machine_owned_data(old.owned_data);
            let new_owned = candidate.machine_owned_data(new.owned_data);
            if old_owned.len() != new_owned.len() {
                return false;
            }
            for (old, new) in old_owned.iter().zip(new_owned) {
                let mut header = new.clone();
                header.initial_value = old.initial_value;
                if header != *old {
                    return false;
                }
            }
            let old_conformances = base.machine_trait_conformances(old.satisfies);
            let new_conformances = candidate.machine_trait_conformances(new.satisfies);
            if old_conformances.len() != new_conformances.len() {
                return false;
            }
            for (old, new) in old_conformances.iter().zip(new_conformances) {
                let mut header = new.clone();
                header.via_expression = old.via_expression;
                if header != *old {
                    return false;
                }
            }
        }
        candidate.const_declarations.len() >= base.const_declarations.len()
            && candidate
                .const_declarations
                .iter()
                .take(base.const_declarations.len())
                .eq(base.const_declarations.iter())
            && candidate.data_definitions.len() >= base.data_definitions.len()
            && candidate
                .data_definitions
                .iter()
                .take(base.data_definitions.len())
                .eq(base.data_definitions.iter())
            && candidate.traits.len() >= base.traits.len()
            && candidate
                .traits
                .iter()
                .take(base.traits.len())
                .eq(base.traits.iter())
            && retained_declaration_families_are_exact(base, candidate)
    }

    fn join_resolved_semantics(
        source: &SymbolResolvedTrees,
        base: &SymbolResolvedTrees,
        candidate: &mut TypedTrees,
    ) -> Result<(), Diagnostic> {
        if !source
            .service_reaches
            .definitions()
            .starts_with(base.service_reaches.definitions())
            || !source
                .service_reaches
                .definitions()
                .starts_with(candidate.service_reaches.definitions())
        {
            return Err(Diagnostic::error(
                "source continuation changed retained service definitions",
            ));
        }
        if !source
            .service_reach_rows
            .starts_with(&base.service_reach_rows)
            || !source
                .service_reach_rows
                .starts_with(&candidate.service_reach_rows)
        {
            return Err(Diagnostic::error(
                "source continuation changed retained service-row identities",
            ));
        }
        if !domain_table_retains(&base.semantic_domains, &source.semantic_domains) {
            return Err(Diagnostic::error(
                "source continuation changed retained semantic domains",
            ));
        }
        if !source
            .external_bindings
            .identities()
            .take(base.external_bindings.identities().len())
            .eq(base.external_bindings.identities())
            || !source
                .external_bindings
                .identities()
                .take(candidate.external_bindings.identities().len())
                .eq(candidate.external_bindings.identities())
        {
            return Err(Diagnostic::error(
                "source continuation changed retained external-binding identities",
            ));
        }
        if !source
            .evidence_forwardings
            .starts_with(&base.evidence_forwardings)
        {
            return Err(Diagnostic::error(
                "source continuation changed retained evidence forwardings",
            ));
        }
        for row in &base.authored_service_reach_rows {
            if source
                .authored_service_reach_rows
                .iter()
                .find(|next| next.owner == row.owner)
                != Some(row)
            {
                return Err(Diagnostic::error(
                    "source continuation changed a retained authored service ceiling",
                ));
            }
        }
        for ordinal in 1..=source.semantic_domains.len() {
            let identity = SemanticDomainId(ordinal as u32);
            let Some(name) = source.semantic_domains.name(identity) else {
                return Err(Diagnostic::error(
                    "source continuation lost a semantic domain identity",
                ));
            };
            if candidate.semantic_domains.intern(name) != identity {
                return Err(Diagnostic::error(
                    "source continuation conflicts with a prepared semantic domain identity",
                ));
            }
        }
        candidate.symbols = source.symbols.clone();
        candidate.service_reaches = source.service_reaches.clone();
        candidate.service_reach_rows = source.service_reach_rows.clone();
        candidate.external_bindings = source.external_bindings.clone();
        candidate.retain_authored_declaration_selections(
            source.authored_declaration_selections().clone(),
        );
        append_authored_service_reach_rows(candidate, source, &base.authored_service_reach_rows);
        Ok(())
    }

    fn resolved_root_prefix_is_retained(
        source: &SymbolResolvedTrees,
        base: &SymbolResolvedTrees,
    ) -> bool {
        macro_rules! retains {
        ($($family:ident),* $(,)?) => {
            $(source.$family.iter().take(base.$family.len()).eq(base.$family.iter()))&&*
        };
    }
        retains!(
            const_declarations,
            data_definitions,
            type_aliases,
            domain_definitions,
            machines,
            measures,
            operators,
            propositions,
            mathematical_definitions,
            traits,
            conformances,
            wire_schemas,
        )
    }

    fn retained_declaration_families_are_exact(
        base: &SymbolResolvedTrees,
        candidate: &SymbolResolvedTrees,
    ) -> bool {
        candidate.type_aliases == base.type_aliases
            && candidate.domain_definitions == base.domain_definitions
            && candidate.measures == base.measures
            && candidate.operators == base.operators
            && candidate.propositions == base.propositions
            && candidate.mathematical_definitions == base.mathematical_definitions
            && candidate.conformances == base.conformances
            && candidate.wire_schemas == base.wire_schemas
    }
}
pub(crate) mod target_machine_families {
    use crate::lowerer::name::lower_name;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use typed_trees::TypedTrees;
    use typed_trees::deferred_refusals::DeferredRequest;
    use typed_trees::machine::{Machine, TargetMachineFamily, TargetMachineFamilyMember};

    /// `linux_arm64 machine write_byte(byte: i32)` and its siblings for the
    /// other targets are the bodies of one machine. Resolution gave them one
    /// interface declaration; typing records the family and holds every body
    /// to that interface's signature, so a call typed against the interface is
    /// typed against whichever body a target later selects.
    pub(crate) fn append(
        source: &SymbolResolvedTrees,
        typed: &mut TypedTrees,
        machine_frontier: usize,
    ) -> Result<(), Diagnostic> {
        let mut declarations = source
            .machines
            .iter()
            .skip(machine_frontier)
            .filter(|machine| machine.target_family.is_valid())
            .collect::<Vec<_>>();
        declarations.sort_by_key(|machine| machine.target_family.arena_index());
        let mut families = Vec::new();
        let mut uncompared = Vec::new();
        for declarations in
            declarations.chunk_by(|left, right| left.target_family == right.target_family)
        {
            let canonical = declarations[0].target_family;
            let Some(interface) = typed.machine_by_symbol(canonical) else {
                return Err(Diagnostic::error(
                    "target-machine family interface did not survive typing",
                ));
            };
            if typed
                .target_machine_families
                .iter()
                .any(|family| family.canonical == canonical)
            {
                return Err(Diagnostic::error(
                    "seeded typing cannot append a body to a retained target-machine family",
                ));
            }
            let mut members = Vec::new();
            for declaration in declarations {
                let Some(target) = declaration.authored_target.as_ref() else {
                    continue;
                };
                let Some(member) = typed.machine_by_symbol(declaration.symbol) else {
                    return Err(Diagnostic::error(
                        "target-machine family member did not survive typing",
                    ));
                };
                let span = declaration.name.source_span();
                match require_shared_signature(typed, interface, member)
                    .map_err(|refusal| refusal.with_source_span(span))?
                {
                    SharedSignature::Held => {}
                    // The body was not held to the interface, so neither it
                    // nor the interface a call is typed against may be read:
                    // the row is on both, and whoever demands one raises it.
                    SharedSignature::NotCompared => {
                        uncompared.extend([(canonical, span), (declaration.symbol, span)]);
                    }
                }
                members.push(TargetMachineFamilyMember {
                    target: lower_name(target),
                    machine: declaration.symbol,
                });
            }
            if members.is_empty() {
                return Err(Diagnostic::error(
                    "target-machine family has no retained members",
                ));
            }
            members.sort_by(|left, right| left.target.as_str().cmp(right.target.as_str()));
            families.push(TargetMachineFamily {
                name: interface.name.clone(),
                canonical,
                purpose: interface.purpose,
                members,
            });
        }
        families.sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        typed.target_machine_families.extend(families);
        for (declaration, span) in uncompared {
            typed.record_deferred_refusal(
                declaration,
                DeferredRequest::TargetFamilyGenericSignature,
                span,
            );
        }
        Ok(())
    }

    /// What comparing one body with its family's interface concluded.
    #[derive(Debug, PartialEq, Eq)]
    enum SharedSignature {
        /// The body has the interface's signature.
        Held,
        /// The family binds generic parameters, which typing does not
        /// compare yet. Nothing was concluded about this body.
        NotCompared,
    }

    /// Whether the body's contracts hold is not asked here: resolution already
    /// made the interface require what each body requires, and each body is
    /// checked on its own.
    fn require_shared_signature(
        typed: &TypedTrees,
        interface: &Machine,
        member: &Machine,
    ) -> Result<SharedSignature, Diagnostic> {
        let incompatible = |detail: &str| {
            Diagnostic::error(format!(
                "machine `{}` has an incompatible signature for `{}` --                  every target's body of one machine shares its signature; incompatible {detail}",
                member.name,
                member
                    .target
                    .as_ref()
                    .map_or("<missing target>", |target| target.as_str()),
            ))
        };
        // A generic body binds its own parameters, so comparing it with the
        // interface means matching the two telescopes binder for binder,
        // bounds and nested callable contracts included. Typing has no such
        // comparison yet, and an unequal count is already a mismatch.
        if typed.machine_type_parameters(interface).len()
            != typed.machine_type_parameters(member).len()
            || interface.lifetime_parameters.len() != member.lifetime_parameters.len()
        {
            return Err(incompatible("generic/lifetime parameter count"));
        }
        if !typed.machine_type_parameters(interface).is_empty()
            || !interface.lifetime_parameters.is_empty()
            || !interface.conformance_bounds.is_empty()
            || !member.conformance_bounds.is_empty()
        {
            return Ok(SharedSignature::NotCompared);
        }
        let (Some(required), Some(actual)) = (
            typed.machine_states(interface).first(),
            typed.machine_states(member).first(),
        ) else {
            return Err(incompatible("entry signature"));
        };
        let required_parameters = typed.state_parameters(required);
        let actual_parameters = typed.state_parameters(actual);
        if required_parameters.len() != actual_parameters.len()
            || required_parameters
                .iter()
                .zip(actual_parameters)
                .any(|(required, actual)| required.relevance != actual.relevance)
        {
            return Err(incompatible("parameter count or relevance"));
        }
        let required_identity = typed.machine_callable_identity(interface);
        let actual_identity = typed.machine_callable_identity(member);
        if !required_identity.is_valid()
            || !actual_identity.is_valid()
            || required_identity != actual_identity
        {
            return Err(incompatible("parameter types or calling shape"));
        }
        if typed.machine_entry_result_type_identity(interface)
            != typed.machine_entry_result_type_identity(member)
        {
            return Err(incompatible("result type"));
        }
        if interface.purpose != member.purpose {
            return Err(incompatible("purpose"));
        }
        Ok(SharedSignature::Held)
    }

    #[cfg(test)]
    mod tests {
        use super::{SharedSignature, require_shared_signature};
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::TypedTrees;
        use typed_trees::data::TypeParameter;
        use typed_trees::machine::Machine;
        use typed_trees::name::Identifier;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceNode;

        /// The interface `write(byte: i32)`, a body with that signature, a
        /// body taking `u8` instead, and a body with a type parameter.
        fn family() -> (TypedTrees, [SymbolHandle; 4]) {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = builder.insert_children(
                root,
                symbols::builtin_type_symbols()
                    .into_iter()
                    .chain([(SymbolKind::Machine, SymbolNameRef::Static("write")); 4]),
            );
            let machines: [SymbolHandle; 4] = std::array::from_fn(|position| {
                children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + position as u32)
            });
            let formal = builder
                .insert_children(
                    machines[3],
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
                )
                .start();
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let builtin =
                |program: &mut TypedTrees, atom: symbols::BuiltinTypeAtom, name: &'static str| {
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: children.handle_at(atom.ordinal() as u32),
                            name: Identifier::generated(name),
                        })
                };
            let signed = builtin(&mut program, symbols::BuiltinTypeAtom::I32, "i32");
            let byte = builtin(&mut program, symbols::BuiltinTypeAtom::U8, "u8");
            for (symbol, parameter_type) in machines.into_iter().zip([signed, signed, byte, signed])
            {
                let parameters = program.state_parameters.insert_many([StateParameter {
                    name: Identifier::generated("byte"),
                    type_reference: parameter_type,
                    ..Default::default()
                }]);
                let states = program.machine_states.insert_many([State {
                    name: Identifier::generated("entry"),
                    parameters,
                    ..Default::default()
                }]);
                let type_parameters = if symbol == machines[3] {
                    program.data_type_parameters.insert_many([TypeParameter {
                        symbol: formal,
                        name: Identifier::generated("T"),
                        ..Default::default()
                    }])
                } else {
                    Default::default()
                };
                program.push_machine(Machine {
                    symbol,
                    name: Identifier::generated("write"),
                    states,
                    type_parameters,
                    ..Default::default()
                });
            }
            (program, machines)
        }

        #[test]
        fn a_target_body_shares_its_family_interface_signature_or_is_refused() {
            let (program, [interface, same, other, generic]) = family();
            let machine = |symbol| program.machine_by_symbol(symbol).expect("a typed machine");
            let compare =
                |member| require_shared_signature(&program, machine(interface), machine(member));
            compare(same).expect("a body with the interface's signature belongs to the family");
            let mismatch = compare(other).expect_err("a body taking another type is refused");
            assert!(
                mismatch
                    .message
                    .contains("every target's body of one machine shares its signature"),
                "{}",
                mismatch.message
            );
            // A generic body against a plain interface is a count mismatch; a
            // generic family is not compared at all yet.
            assert!(
                compare(generic)
                    .expect_err("an extra type parameter is refused")
                    .message
                    .contains("generic/lifetime parameter count")
            );
            assert_eq!(
                require_shared_signature(&program, machine(generic), machine(generic)),
                Ok(SharedSignature::NotCompared),
                "a generic family has no comparison yet"
            );
        }
    }
}

pub(crate) fn validate_resolved_semantics(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    validate_resolved_semantics_in(symbol_resolved_trees, &TypingCohort::default())
}

pub(crate) fn validate_resolved_semantics_in(
    symbol_resolved_trees: &SymbolResolvedTrees,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    crate::expressions::equality::validate_equality_operands_in(symbol_resolved_trees, cohort)?;

    validate_equatable_conformances_in(symbol_resolved_trees, cohort)?;

    validate_case_dispatch_exhaustiveness_in(symbol_resolved_trees, cohort)
}

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    validate_resolved_semantics(symbol_resolved_trees)?;
    let mut lowerer = Lowerer {
        // Type arguments are classified while expressions are lowered, so the
        // initial pass needs the resolved symbols from the start.
        typed_trees: TypedTrees {
            symbols: symbol_resolved_trees.symbols.clone(),
            ..TypedTrees::default()
        },
        source_trees: symbol_resolved_trees,
        equality_scope: None,
        type_reference_exposure:
            Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
        admits_contract_policy_operators: false,
    };
    lowerer.typed_trees.service_reaches = symbol_resolved_trees.service_reaches.clone();
    lowerer.typed_trees.service_reach_rows = symbol_resolved_trees.service_reach_rows.clone();
    lowerer.typed_trees.authored_service_reach_rows = symbol_resolved_trees
        .authored_service_reach_rows
        .iter()
        .map(lower_authored_service_reach_row)
        .collect();
    lowerer.typed_trees.semantic_domains = symbol_resolved_trees.semantic_domains.clone();
    lowerer.typed_trees.external_bindings = symbol_resolved_trees.external_bindings.clone();
    lowerer.typed_trees.retain_authored_declaration_selections(
        symbol_resolved_trees
            .authored_declaration_selections()
            .clone(),
    );
    lowerer.typed_trees.evidence_forwardings = symbol_resolved_trees
        .evidence_forwardings
        .iter()
        .map(|forwarding| lower_evidence_forwarding(forwarding, &[]))
        .collect();
    let cohort = TypingCohort::default();
    type_resolved_cohort(&mut lowerer, &cohort)?;
    lowerer.finish(&cohort)
}

#[derive(Default)]
pub(crate) struct TypingCohort<'completed> {
    pub(crate) const_declarations: usize,
    pub(crate) data_definitions: usize,
    pub(crate) type_aliases: usize,
    pub(crate) domain_definitions: usize,
    pub(crate) propositions: usize,
    pub(crate) mathematical_definitions: usize,
    pub(crate) machines: usize,
    pub(crate) measures: usize,
    pub(crate) operators: usize,
    pub(crate) traits: usize,
    pub(crate) conformances: usize,
    pub(crate) wire_schemas: usize,
    pub(crate) generic_application_origins: usize,
    completed_machines: &'completed [SymbolHandle],
    completed_keys: Vec<(u32, u32)>,
    pub(crate) typed_machines: usize,
    pub(crate) typed_type_parameters: usize,
    pub(crate) typed_type_references: usize,
    pub(crate) typed_expressions: usize,
}

impl<'completed> TypingCohort<'completed> {
    pub(crate) fn continuing(resolved: &SymbolResolvedTrees, typed: &TypedTrees) -> Self {
        Self {
            const_declarations: resolved.const_declarations.len(),
            data_definitions: resolved.data_definitions.len(),
            type_aliases: resolved.type_aliases.len(),
            domain_definitions: resolved.domain_definitions.len(),
            propositions: resolved.propositions.len(),
            mathematical_definitions: resolved.mathematical_definitions.len(),
            machines: resolved.machines.len(),
            measures: resolved.measures.len(),
            operators: resolved.operators.len(),
            traits: resolved.traits.len(),
            conformances: resolved.conformances.len(),
            wire_schemas: resolved.wire_schemas.len(),
            generic_application_origins: resolved.tables.types.generic_application_origins.len(),
            completed_machines: &[],
            completed_keys: Vec::new(),
            typed_machines: typed.machines().len(),
            typed_type_parameters: typed.data_type_parameters.len(),
            typed_type_references: typed.type_reference_table.type_reference_count(),
            typed_expressions: typed.expression_table.expression_count(),
        }
    }

    pub(crate) fn completing(mut self, machines: &'completed [SymbolHandle]) -> Self {
        self.completed_machines = machines;
        self.completed_keys = machines.iter().map(|symbol| symbol_key(*symbol)).collect();
        self.completed_keys.sort_unstable();
        self
    }

    pub(crate) fn completes(&self, symbol: SymbolHandle) -> bool {
        self.completed_keys
            .binary_search(&symbol_key(symbol))
            .is_ok()
    }
}

pub(crate) fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

pub(crate) fn type_resolved_cohort(
    lowerer: &mut Lowerer<'_>,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    let source = lowerer.source_trees;
    crate::runtime_callables::lower_contracts(lowerer)?;
    for declaration in source
        .const_declarations
        .iter()
        .skip(cohort.const_declarations)
    {
        lowerer.lower_const_declaration(declaration)?;
    }
    for data_definition in source.data_definitions.iter().skip(cohort.data_definitions) {
        lowerer.lower_data_declaration(data_definition)?;
    }
    for alias in source.type_aliases.iter().skip(cohort.type_aliases) {
        lowerer.lower_type_alias_declaration(alias)?;
    }
    for domain_definition in source
        .domain_definitions
        .iter()
        .skip(cohort.domain_definitions)
    {
        let domain_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(domain_definition.is_public),
            |lowerer| lower_domain_definition(lowerer, domain_definition),
        )?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }
    for proposition in source.propositions.iter().skip(cohort.propositions) {
        let proposition = lowerer.with_type_reference_exposure(
            declaration_exposure(proposition.is_public),
            |lowerer| lower_proposition_definition(lowerer, proposition),
        )?;
        lowerer.typed_trees.push_proposition(proposition);
    }
    for definition in source
        .mathematical_definitions
        .iter()
        .skip(cohort.mathematical_definitions)
    {
        let definition = lowerer.with_type_reference_exposure(
            declaration_exposure(definition.is_public),
            |lowerer| lower_mathematical_definition(lowerer, definition),
        )?;
        lowerer.typed_trees.push_mathematical_definition(definition);
    }
    let completed_positions = source
        .machines
        .iter()
        .take(cohort.machines)
        .map(|machine| {
            cohort.completes(machine.symbol).then(|| {
                lowerer
                    .typed_trees
                    .machine_position_by_symbol(machine.symbol)
            })
        })
        .collect::<Vec<_>>();
    for (ordinal, machine) in source.machines.iter().enumerate() {
        if ordinal < cohort.machines {
            if let Some(position) = completed_positions[ordinal] {
                complete_retained_machine_body(lowerer, machine.symbol, position)?;
            }
            continue;
        }
        lowerer.lower_machine_declaration(machine)?;
    }
    for measure in source.measures.iter().skip(cohort.measures) {
        let measure = crate::declarations::measure::lower_measure_definition(lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }
    for operator in source.operators.iter().skip(cohort.operators) {
        let operator = lowerer
            .with_type_reference_exposure(declaration_exposure(operator.is_public), |lowerer| {
                lower_operator_definition(lowerer, operator)
            })?;
        lowerer.typed_trees.push_operator(operator);
    }
    for trait_definition in source.traits.iter().skip(cohort.traits) {
        lowerer.lower_trait_declaration(trait_definition)?;
    }
    for conformance in source.conformances.iter().skip(cohort.conformances) {
        crate::declarations::conformance::lower_conformance(lowerer, conformance)?;
    }
    for wire_schema in source.wire_schemas.iter().skip(cohort.wire_schemas) {
        let wire_schema = lowerer.with_type_reference_exposure(
            declaration_exposure(wire_schema.is_public),
            |lowerer| crate::declarations::wire::lower_wire_schema(lowerer, wire_schema),
        )?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }
    lowerer.lower_const_initializer_evidence(cohort.const_declarations)
}

fn complete_retained_machine_body(
    lowerer: &mut Lowerer<'_>,
    symbol: SymbolHandle,
    position: Option<usize>,
) -> Result<(), Diagnostic> {
    let lost_base = || {
        Diagnostic::error(
            "authored-body completion changed its retained declaration or implementation frontier",
        )
    };
    let ordinal = position.ok_or_else(lost_base)?;
    let mut declaration = lowerer
        .typed_trees
        .machines()
        .get(ordinal)
        .filter(|declaration| declaration.symbol == symbol)
        .ok_or_else(lost_base)?
        .clone();
    let implementation = lowerer.source_trees.machine(symbol).ok_or_else(lost_base)?;
    crate::declarations::machine::complete_machine_body(lowerer, implementation, &mut declaration)?;
    lowerer.typed_trees.machines_mut()[ordinal] = declaration;
    Ok(())
}

pub(crate) fn close_typed_cohort(
    lowerer: &mut Lowerer<'_>,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    // Result and effect queries must see the signature of the evaluated callee,
    // including calls in declaration contracts and initializers.
    let mut callable_admission = crate::runtime_callables::bind_calls(
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
        cohort.typed_type_references,
    )?;
    let bound_expressions = lowerer.typed_trees.expression_table.expression_count();
    let bound_types = lowerer
        .typed_trees
        .type_reference_table
        .type_reference_count();
    settle_satisfied_declarations_from(&mut lowerer.typed_trees, cohort.typed_machines)?;
    normalize_machine_progress_premises(&mut lowerer.typed_trees, cohort.typed_machines)?;
    lowerer
        .typed_trees
        .retain_static_argument_contexts()
        .map_err(Diagnostic::error)?;
    lowerer
        .typed_trees
        .instantiate_nominal_machine_parameter_contracts(cohort.typed_type_parameters)
        .and_then(|_| lowerer.typed_trees.prepare_applied_callable_signatures())
        .map_err(|message| Diagnostic::error(message.to_owned()))?;
    normalize_const_argument_carriers(&mut lowerer.typed_trees, cohort.typed_type_references)?;
    normalize_domain_constraints(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.typed_type_references,
    )?;
    normalize_membership_selections(lowerer.source_trees, &mut lowerer.typed_trees)?;
    crate::contracts::proof_facts::intern_proof_membership_instances(&mut lowerer.typed_trees)?;
    crate::runtime_callables::validate_pending_token_shapes(
        lowerer.source_trees,
        &lowerer.typed_trees,
    )?;
    lowerer
        .typed_trees
        .prepare_data_application_views(cohort.typed_type_references)
        .map_err(Diagnostic::error)?;
    let cast_destinations = crate::expressions::construction_destinations::prepare(
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
        cohort.typed_machines,
        cohort.completed_machines,
    )?;
    crate::expressions::qualification_casts::normalize_qualification_casts(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
        &cast_destinations,
    )?;
    // Cast normalization can complete a destination's declared type. Reuse
    // the receiving-type walk so its value retains the complete constructor
    // application; assignment authority and value obligations stay with the
    // lowered checker.
    crate::expressions::construction_destinations::prepare(
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
        cohort.typed_machines,
        cohort.completed_machines,
    )?;
    if !(cohort.generic_application_origins > 0
        && cohort.generic_application_origins
            == lowerer
                .source_trees
                .tables
                .types
                .generic_application_origins
                .len()
        && cohort.operators == lowerer.source_trees.operators.len()
        && cohort.data_definitions == lowerer.source_trees.data_definitions.len())
    {
        validate_computed_arguments(lowerer.source_trees)?;
    }
    finalize_provider_default_calls(&mut lowerer.typed_trees)?;
    target_machine_families::append(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.machines,
    )?;
    callable_admission.extend(crate::runtime_callables::bind_calls(
        &mut lowerer.typed_trees,
        bound_expressions,
        bound_types,
    )?);
    crate::runtime_callables::admit_types(&lowerer.typed_trees, &callable_admission)?;
    // Constructor and cast preparation may finish a callee's retained type.
    // Bind those values before attributing any still-unserved call request.
    crate::typing_queries::defer_unbound_calls(
        &mut lowerer.typed_trees,
        cohort.typed_machines,
        cohort.completed_machines,
    )
}

fn lower_authored_service_reach_row(
    row: &symbol_resolved_trees::signature::AuthoredServiceReachRow,
) -> typed_trees::signature::AuthoredServiceReachRow {
    typed_trees::signature::AuthoredServiceReachRow {
        owner: row.owner,
        keyword_source_spans: row.keyword_source_spans.clone(),
        targets: row
            .targets
            .iter()
            .map(|target| ReachTarget {
                service: target.service,
                source_span: target.source_span,
            })
            .collect(),
        installation_bound: row.installation_bound,
    }
}

pub(crate) fn append_authored_service_reach_rows(
    target: &mut TypedTrees,
    source: &SymbolResolvedTrees,
    retained: &[symbol_resolved_trees::signature::AuthoredServiceReachRow],
) {
    target.authored_service_reach_rows.extend(
        source
            .authored_service_reach_rows
            .iter()
            .filter(|row| !retained.iter().any(|retained| retained.owner == row.owner))
            .map(lower_authored_service_reach_row),
    );
}

pub(crate) fn lower_evidence_forwarding(
    forwarding: &symbol_resolved_trees::statement::EvidenceForwarding,
    proof_output_calls: &[typed_trees::typed_trees::ProofOutputCall],
) -> typed_trees::typed_trees::EvidenceForwarding {
    let erased_before = proof_output_calls
        .iter()
        .filter(|package| {
            package.machine_symbol == forwarding.machine_symbol
                && package.state_symbol == forwarding.state_symbol
                && package.source_statement_index < forwarding.statement_index
        })
        .count();
    typed_trees::typed_trees::EvidenceForwarding {
        machine_symbol: forwarding.machine_symbol,
        state_symbol: forwarding.state_symbol,
        statement_index: forwarding.statement_index.saturating_sub(erased_before),
        source_statement_index: forwarding.statement_index,
        target: crate::lowerer::name::lower_name(&forwarding.target),
        source: crate::lowerer::name::lower_name(&forwarding.source),
        source_conformance: forwarding.source_conformance,
    }
}

pub(crate) fn exact_top_level_data_symbol(
    source: &SymbolResolvedTrees,
    definition: &symbol_resolved_trees::data::DataDefinition,
) -> bool {
    definition.symbol.is_valid()
        && source.symbols.get(definition.symbol).kind == symbols::SymbolKind::Data
        && source.symbols.get(definition.symbol).parent == source.symbols.root()
        && source.symbols.name(definition.symbol) == definition.name.as_str()
}

pub(crate) fn exact_field_symbol(
    source: &SymbolResolvedTrees,
    owner: symbols::SymbolHandle,
    field: &symbol_resolved_trees::data::DataField,
) -> bool {
    field.symbol.is_valid()
        && source.symbols.get(field.symbol).kind == symbols::SymbolKind::Field
        && source.symbols.get(field.symbol).parent == owner
        && source.symbols.name(field.symbol) == field.name.as_str()
}

pub(crate) fn declaration_exposure(
    is_public: bool,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;

    if is_public {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
    pub(crate) equality_scope: Option<crate::expressions::equatable::EqualityScope>,
    pub(crate) type_reference_exposure:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    /// Set while the contracts of a machine or one of its states are lowered.
    /// Preparation types a suffixed policy operator in those contracts against
    /// the machine's parameters; no other proof fact has that owner, so every
    /// other fact still rejects the suffix.
    pub(crate) admits_contract_policy_operators: bool,
}

impl Lowerer<'_> {
    fn lower_const_declaration(
        &mut self,
        declaration: &symbol_resolved_trees::constant::ConstDeclaration,
    ) -> Result<(), Diagnostic> {
        let declared_type = self.with_type_reference_exposure(
            declaration_exposure(declaration.is_public),
            |lowerer| lower_type_reference_into_table(lowerer, &declaration.declared_type),
        )?;
        self.typed_trees
            .push_const_declaration(typed_trees::constant::ConstDeclaration {
                symbol: declaration.symbol,
                is_public: declaration.is_public,
                declared_type,
                initializer_source_span: declaration.initializer_source_span,
                pending_target_observation: declaration.pending_target_observation,
                evaluated_value: declaration.evaluated_value.clone(),
                authored_initializer: typed_trees::expression::ExpressionHandle::invalid(),
                materialized_initializer: typed_trees::expression::ExpressionHandle::invalid(),
                evaluation_entry: Default::default(),
                evaluation_expression: Default::default(),
            });
        Ok(())
    }

    fn lower_data_declaration(
        &mut self,
        data_definition: &symbol_resolved_trees::data::DataDefinition,
    ) -> Result<(), Diagnostic> {
        let data_definition = self.with_type_reference_exposure(
            declaration_exposure(data_definition.is_public),
            |lowerer| lower_data_definition(lowerer, data_definition),
        )?;
        self.typed_trees.push_data_definition(data_definition);
        Ok(())
    }

    fn lower_type_alias_declaration(
        &mut self,
        alias: &symbol_resolved_trees::type_aliases::TypeAliasDefinition,
    ) -> Result<(), Diagnostic> {
        let alias = self
            .with_type_reference_exposure(declaration_exposure(alias.is_public), |lowerer| {
                lower_type_alias_definition(lowerer, alias)
            })?;
        self.typed_trees.push_type_alias_definition(alias);
        Ok(())
    }

    fn lower_machine_declaration(
        &mut self,
        machine: &symbol_resolved_trees::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let typed_machine = self
            .with_type_reference_exposure(Some(machine.interface_exposure), |lowerer| {
                lower_machine(lowerer, machine)
            })?;
        if let Some(view) = lower_token_binding_view(self, machine, &typed_machine)? {
            self.typed_trees.push_machine_token_binding(view);
        }
        self.typed_trees.push_machine(typed_machine);
        Ok(())
    }

    fn lower_trait_declaration(
        &mut self,
        trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
    ) -> Result<(), Diagnostic> {
        let trait_definition = self.with_type_reference_exposure(
            declaration_exposure(trait_definition.is_public),
            |lowerer| lower_trait_definition(lowerer, trait_definition),
        )?;
        self.typed_trees.push_trait_definition(trait_definition);
        Ok(())
    }

    fn lower_const_initializer_evidence(&mut self, frontier: usize) -> Result<(), Diagnostic> {
        for ordinal in frontier..self.source_trees.const_declarations.len() {
            let declaration = &self.source_trees.const_declarations[ordinal];
            let (symbol, original, materialized) = (
                declaration.symbol,
                declaration.authored_initializer,
                declaration.initializer,
            );
            let handle = self
                .typed_trees
                .tables
                .const_declarations
                .iter()
                .find_map(|(handle, declaration)| (declaration.symbol == symbol).then_some(handle))
                .ok_or_else(|| {
                    Diagnostic::error("constant fold lost its exact typed declaration")
                })?;
            if !resolved_expressions(self.source_trees).expression_is_valid(materialized) {
                return Err(Diagnostic::error(
                    "constant lost its materialized initializer",
                ));
            }
            let materialized_initializer = lower_expression_handle(self, materialized)?;
            self.typed_trees
                .tables
                .const_declarations
                .get_mut(handle)
                .materialized_initializer = materialized_initializer;
            if !original.is_valid() {
                continue;
            }
            let authored_initializer = lower_expression_handle(self, original)?;
            let declaration = self.typed_trees.tables.const_declarations.get_mut(handle);
            declaration.authored_initializer = authored_initializer;
        }
        Ok(())
    }

    pub(crate) fn with_type_reference_exposure<T>(
        &mut self,
        exposure: impl Into<
            Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
        >,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous = std::mem::replace(&mut self.type_reference_exposure, exposure.into());
        let result = operation(self);
        self.type_reference_exposure = previous;
        result
    }

    /// Lowers the contracts of a machine or state, the proof facts that may
    /// use a suffixed policy operator.
    pub(crate) fn admitting_contract_policy_operators<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous = std::mem::replace(&mut self.admits_contract_policy_operators, true);
        let result = operation(self);
        self.admits_contract_policy_operators = previous;
        result
    }

    pub(crate) fn finish(mut self, cohort: &TypingCohort<'_>) -> Result<TypedTrees, Diagnostic> {
        crate::lowerer::progress::normalize_trait_progress_premises(&mut self.typed_trees)?;
        self.typed_trees =
            rebuild_fresh_typed_trees(std::mem::take(&mut self.typed_trees), self.source_trees);
        close_typed_cohort(&mut self, cohort)?;
        Ok(self.typed_trees)
    }
}

fn rebuild_fresh_typed_trees(typed: TypedTrees, source: &SymbolResolvedTrees) -> TypedTrees {
    let TypedTrees {
        identity: _,
        product_declaration_frontier: _,
        excluded_authored_selection_occurrences,
        roots,
        tables,
        symbols,
        service_reaches,
        service_reach_rows,
        authored_service_reach_rows,
        semantic_domains,
        applied_domain_applications: _,
        applied_data_cleanup_selections: _,
        external_bindings,
        plan_laid_layouts: _,
        plan_laid_target_layouts: _,
        plan_laid_schema_fields: _,
        plan_laid_retired_field_identities: _,
        access_view_plans: _,
        access_lifecycle_plans: _,
        access_policy_invocations: _,
        access_borrow_factories,
        wire_placements: _,
        wire_encode_obligations: _,
        wire_schema_plans: _,
        machine_specializations: _,
        captured_machine_applications: _,
        captured_machine_states: _,
        trait_implementation_applications: _,
        data_implementation_applications: _,
        dynamic_slots: _,
        target_machine_families: _,
        deferred_refusals,
        boundary_completion_declarations: _,
        evidence_forwardings: _,
        proof_output_calls,
        declaration_indexes: _,
        callable_identities: _,
        callable_identity_memo: _,
        ranking_expression_custody,
        pending_const_range_endpoints: _,
        constant_recipes: _,
        constant_expression_contexts: _,
        constant_recipe_closure: _,
        type_identities,
    } = typed;
    let mut trees = TypedTrees::with_roots(roots, tables, symbols);
    trees.type_identities = type_identities;
    // A request recorded on a declaration stays recorded while the program
    // is extended: its declaration is still unserved, and recording again
    // adds nothing.
    trees.deferred_refusals = deferred_refusals;
    trees.access_borrow_factories = access_borrow_factories;
    trees.excluded_authored_selection_occurrences = excluded_authored_selection_occurrences;
    trees.service_reaches = service_reaches;
    trees.service_reach_rows = service_reach_rows;
    trees.authored_service_reach_rows = authored_service_reach_rows;
    trees.semantic_domains = semantic_domains;
    trees.external_bindings = external_bindings;
    trees.evidence_forwardings = source
        .evidence_forwardings
        .iter()
        .map(|forwarding| lower_evidence_forwarding(forwarding, &proof_output_calls))
        .collect();
    trees.proof_output_calls = proof_output_calls;
    trees.ranking_expression_custody = ranking_expression_custody;
    trees
}
