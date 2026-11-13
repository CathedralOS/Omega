/*
    What checking publishes beside the typed program: `facts` (CheckFacts, the
    checked program's knowledge of its own values), `proof` (evidence,
    proof-output and outcome facts), `semantic` (semantic facts), `places` (the
    place a value expression denotes in one state) and `labels` (the ordered
    label rosters those tables are keyed by).
*/

#[path = "published_facts.rs"]
pub(crate) mod facts;
pub(crate) mod labels {
    /*
        Labels: the text the checking stage uses for machines, call targets,
        symbols, places and facts, and the conversion of checked-tree fact kinds
        into the fact plan's kinds.

        There is no entry function. Each child is a set of queries that the
        checks, fact construction and flow facts call directly:

        - `boolean_facts` matches a retained true fact or domain clause to an
          exact leaf label within one caller-selected context;
        - `names` names a machine, a call target (a free machine's implicit
          `entry` state takes the machine's name) and any symbol;
        - `places` labels a borrow access, a place with extra segments, and the
          requirement or Boolean expression a semantic fact states;
        - `operators` renders an operator contract expression with each formal
          parameter replaced by a given operand label;
        - `kinds` converts checked-tree contract and proof fact kinds into the
          fact plan's kinds.

        Most uses are diagnostic text, but some labels are compared. Flow facts
        store the operator rendering as the instantiated expression of an
        operator `ensures` fact, the operator `requires` check and named operator
        crash routes compare a rendered requirement with the text of Boolean
        facts, and `semantic::places` matches parameters by `symbol_name`.
    */
    mod boolean_facts {
        /*
            Per-context Boolean fact queries shared by selected operator requires
            and named crash-route discharge. Callers select contexts, polarity and
            incoming rows; this module only matches a retained true fact's label.
            Payload-domain expansion additionally requires selected cases in that exact
            incoming row and consumer scope; conditional validity is not a true clause.
        */
        use facts::{FactContextHandle, FactPayload, FactPlan};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

        use super::semantic_boolean_fact_label;
        use crate::checks::contracts::labels::domain_proves_expression_label;

        /* A retained true clause or domain requirement proves an exact rendered leaf. */
        pub(crate) fn context_proves_boolean_label(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            context: FactContextHandle,
            contexts: &[FactContextHandle],
            point: facts::ProgramPoint,
            required_label: &str,
        ) -> bool {
            let availability =
                crate::published::facts::boolean_predicates::CasePremiseAvailability::new(
                    program, semantic, contexts, point,
                );
            let context = semantic.contexts.get(context);
            semantic
                .context_view(context)
                .facts()
                .any(|fact| match fact.payload {
                    FactPayload::BooleanValue {
                        expression,
                        value: true,
                    }
                    | FactPayload::BooleanExpression(expression) => {
                        true_expression_proves_label(program, expression, required_label)
                    }
                    FactPayload::ContractBooleanExpression {
                        expression,
                        instantiated,
                        ..
                    } => {
                        (!instantiated.is_valid()
                            && true_expression_proves_label(program, expression, required_label))
                            || semantic_boolean_fact_label(program, semantic, fact)
                                .is_some_and(|candidate| candidate == required_label)
                    }
                    FactPayload::DomainMembership {
                        domain_symbol,
                        value,
                        ..
                    }
                    | FactPayload::ContractDomainMembership {
                        domain_symbol,
                        value,
                        ..
                    } => {
                        let Some(place) = fact.place.as_place() else {
                            return false;
                        };
                        let Some(subject) = crate::flow::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(place),
                        ) else {
                            return false;
                        };
                        if !availability.allows(&subject) {
                            return false;
                        }
                        let canonical_base = semantic.place_label(program, place);
                        let display_base = program.expression_table.display_name(value);
                        domain_proves_expression_label(
                            program,
                            domain_symbol,
                            &canonical_base,
                            required_label,
                        ) || domain_proves_expression_label(
                            program,
                            domain_symbol,
                            &display_base,
                            required_label,
                        )
                    }
                    _ => false,
                })
        }

        fn true_expression_proves_label(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            required_label: &str,
        ) -> bool {
            if program.expression_table.display_name(expression) == required_label {
                return true;
            }
            matches!(program.expression_table.expression(expression),
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And
                    && (true_expression_proves_label(program, binary.left, required_label)
                        || true_expression_proves_label(program, binary.right, required_label)))
        }
    }
    mod kinds {
        use checked_trees::{ContractProofFactKind, ProofFactKind};
        use facts::{
            ContractFactKind as SemanticContractFactKind,
            ProofObligationKind as SemanticProofObligationKind,
        };

        pub(crate) fn semantic_contract_fact_kind(
            kind: ContractProofFactKind,
        ) -> SemanticContractFactKind {
            match kind {
                ContractProofFactKind::Requires => SemanticContractFactKind::Requires,
                ContractProofFactKind::Ensures => SemanticContractFactKind::Ensures,
            }
        }

        pub(crate) fn semantic_proof_obligation_kind(
            kind: ProofFactKind,
        ) -> SemanticProofObligationKind {
            match kind {
                ProofFactKind::BoundedAssignment => SemanticProofObligationKind::BoundedAssignment,
                ProofFactKind::BoundedCallArgument => {
                    SemanticProofObligationKind::BoundedCallArgument
                }
                ProofFactKind::BoundedInitializer => {
                    SemanticProofObligationKind::BoundedInitializer
                }
                ProofFactKind::BoundedStateReturn => {
                    SemanticProofObligationKind::BoundedStateReturn
                }
                ProofFactKind::BoundedValue => SemanticProofObligationKind::BoundedValue,
                ProofFactKind::BoundedTransitionArgument => {
                    SemanticProofObligationKind::BoundedTransitionArgument
                }
                ProofFactKind::GuardedTransition => SemanticProofObligationKind::GuardedTransition,
            }
        }
    }
    mod names {
        use symbols::SymbolHandle;

        pub(crate) fn machine_name(
            program: &dyn ::validation::CheckingInput,
            machine_symbol: SymbolHandle,
        ) -> String {
            program
                .machine_by_symbol(machine_symbol)
                .map(|machine| machine.name.as_str().to_owned())
                .unwrap_or_else(|| symbol_name(program, machine_symbol))
        }

        pub(crate) fn call_target_label(
            program: &dyn ::validation::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> String {
            program
                .machines()
                .iter()
                .find_map(|machine| {
                    let state = program.state_in_machine(machine, target_symbol)?;
                    let machine = authored_machine(program, machine);
                    if machine.attached_data.is_none() && state.name.as_str() == "entry" {
                        Some(machine.name.as_str().to_owned())
                    } else {
                        Some(state.name.as_str().to_owned())
                    }
                })
                .unwrap_or_else(|| symbol_name(program, target_symbol))
        }

        /*
            The declaration the author named. Specialization clones are instances of
            that declaration; diagnostics name the template, not the clone.
        */
        fn authored_machine<'program>(
            program: &'program dyn ::validation::CheckingInput,
            machine: &'program typed_trees::machine::Machine,
        ) -> &'program typed_trees::machine::Machine {
            program
                .machine_specializations
                .iter()
                .find(|specialization| specialization.instance == machine.symbol)
                .and_then(|specialization| program.machine_by_symbol(specialization.template))
                .filter(|template| template.symbol != machine.symbol)
                .unwrap_or(machine)
        }

        pub(crate) fn symbol_name(
            program: &dyn ::validation::CheckingInput,
            symbol: SymbolHandle,
        ) -> String {
            if !symbol.is_valid() {
                return "unknown".to_owned();
            }

            if let Some(machine) = program.machine_by_symbol(symbol) {
                return machine.name.as_str().to_owned();
            }

            if let Some(state) = program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_states(machine).iter())
                .find(|state| state.symbol == symbol)
            {
                return state.name.as_str().to_owned();
            }

            if let Some(field) = program
                .data_definitions()
                .iter()
                .flat_map(|data| program.data_members(data).iter())
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                        Some(field.name.as_str().to_owned())
                    }
                    _ => None,
                })
            {
                return field;
            }

            program.symbols.name(symbol).to_string()
        }
    }
    mod operators {
        use crate::flow::expressions;
        use language_semantics::ReferenceAccess;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, UnaryOperator,
            display_call_label, display_match_pattern, display_name_path,
        };
        use typed_trees::proposition::ProofSubstitutions;
        use typed_trees::signature::StateParameter;

        /*
            Render an operator contract expression with each formal parameter replaced
            by its concrete operand label. This is shared by requires discharge and by
            flow introduction of ensures facts so both sides use one canonical
            caller-term representation. Callers supply the operand labels directly: a
            spelled use renders its operands as written, while a named call names the
            referent under a reference formal.
        */
        pub(crate) fn instantiate_operator_contract_expression_label(
            program: &dyn ::validation::CheckingInput,
            parameters: &[StateParameter],
            operand_labels: &[String],
            expression: ExpressionHandle,
        ) -> String {
            let instantiate = |expression: ExpressionHandle| {
                instantiate_operator_contract_expression_label(
                    program,
                    parameters,
                    operand_labels,
                    expression,
                )
            };

            match program.expression_table.expression(expression) {
                ExpressionNode::MachineValue(value) => program.typed_program()
                    .runtime_machine_value_identity(value, &[])
                    .expect("check_program validates machine value identity before labels read frozen source"),
                ExpressionNode::SavedSubject(saved) => format!(
                    "$saved-subject#{}:{}",
                    saved.capture.arena_index(),
                    saved.capture.generation()
                ),
                ExpressionNode::Match(dispatch) => {
                    let arms = expressions(program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| {
                            let pattern = match arm.pattern {
                                MatchPattern::Value(pattern) => instantiate(pattern),
                                MatchPattern::Wildcard => "_".to_owned(),
                                MatchPattern::Destructure(_) => display_match_pattern(
                                    &program.expression_table,
                                    arm.pattern,
                                    instantiate,
                                ),
                            };
                            format!("{pattern} -> {}", instantiate(arm.value))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("match {} {{ {arms} }}", instantiate(dispatch.subject))
                }
                ExpressionNode::Atomic(atomic) => format!(
                    "atomic[{:?}]({})",
                    atomic.ordering,
                    instantiate(atomic.value)
                ),
                ExpressionNode::ArrayLiteral(values) => {
                    let values = expressions(program)
                        .expression_handles(*values)
                        .iter()
                        .map(|value| instantiate(*value))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("[{values}]")
                }
                ExpressionNode::Binary(binary) => format!(
                    "{} {} {}",
                    instantiate(binary.left),
                    binary.operator.display_name(),
                    instantiate(binary.right)
                ),
                ExpressionNode::Boolean(value) => value.to_string(),
                ExpressionNode::Cast(cast) => format!(
                    "{} as {}",
                    instantiate(cast.value),
                    display_name_path(
                        expressions(program).name_path_members(cast.target_label),
                        "::",
                    )
                ),
                ExpressionNode::Call(call) => {
                    display_call_label(&program.expression_table, call, instantiate)
                }
                ExpressionNode::Float(value) => value.to_string(),
                ExpressionNode::UndeterminedNaN(format) => {
                    format!("<{} NaN without payload>", format.name())
                }
                ExpressionNode::Indexed(indexed) => format!(
                    "{}[{}]",
                    instantiate(indexed.collection),
                    instantiate(indexed.index)
                ),
                ExpressionNode::Range(range) => {
                    match (range.start.is_valid(), range.end.is_valid()) {
                        (true, true) => {
                            format!("{}..{}", instantiate(range.start), instantiate(range.end))
                        }
                        (true, false) => format!("{}..", instantiate(range.start)),
                        (false, true) => format!("..{}", instantiate(range.end)),
                        (false, false) => "..".to_owned(),
                    }
                }
                ExpressionNode::Integer(value) => value.to_string(),
                ExpressionNode::Member(member) => format!(
                    "{}{}{}",
                    instantiate(member.receiver),
                    language_core::PLACE_MEMBER_SEPARATOR,
                    member.member
                ),
                ExpressionNode::Borrow(inner) => match inner.access {
                    ReferenceAccess::Shared => instantiate(inner.target),
                    ReferenceAccess::Mutable => {
                        format!("mut {}", instantiate(inner.target))
                    }
                    ReferenceAccess::WriteOnly => {
                        format!("write {}", instantiate(inner.target))
                    }
                },
                ExpressionNode::Unary(unary) => format!(
                    "{}{}",
                    unary.operator.display_name(),
                    instantiate(unary.operand)
                ),
                ExpressionNode::Name(path) => {
                    let members = program.expression_table.name_path_members(path.members);
                    let first_member = members.first().map(|member| member.as_str());
                    let operands_include_self = operand_labels.len() == parameters.len();
                    let mut positional_operand_index = 0usize;

                    for (parameter_index, parameter) in parameters.iter().enumerate() {
                        let operand = if operands_include_self {
                            operand_labels.get(parameter_index)
                        } else {
                            let operand = operand_labels.get(positional_operand_index);
                            positional_operand_index = positional_operand_index.saturating_add(1);
                            operand
                        };

                        let parameter_matches = first_member == Some(parameter.name.as_str())
                            || path.head_symbol == parameter.symbol
                            || path.symbol == parameter.symbol;
                        if parameter_matches {
                            return operand
                                .cloned()
                                .unwrap_or_else(|| parameter.name.to_string());
                        }
                    }

                    program.render_proof_expression(expression, ProofSubstitutions::None)
                }
                ExpressionNode::StructLiteral(literal) => {
                    program.render_proof_constructor_value(literal, instantiate)
                }
                ExpressionNode::String(value) => format!("{value:?}"),
                ExpressionNode::TypeExpression(type_reference) => {
                    program.display_type_reference(*type_reference).to_string()
                }
                ExpressionNode::ZeroValue(type_reference) => format!(
                    "zero_value<{}>()",
                    program.display_type_reference(*type_reference)
                ),
            }
        }

        /*
            The Boolean clause's polarity decomposition: `!x` flips, `and`/`or`
            distribute, every other leaf defers to the site's proof rule. Both the
            operator-use discharge and the statement-entry discharge decompose the
            clause tree identically before judging a leaf.
        */
        pub(crate) fn boolean_expression_has_polarity(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            polarity: bool,
            leaf: &mut impl FnMut(ExpressionHandle, bool) -> bool,
        ) -> bool {
            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(value) => *value == polarity,
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    boolean_expression_has_polarity(program, unary.operand, !polarity, leaf)
                }
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
                {
                    let left =
                        boolean_expression_has_polarity(program, binary.left, polarity, leaf);
                    let right =
                        boolean_expression_has_polarity(program, binary.right, polarity, leaf);
                    if (binary.operator == BinaryOperator::And) == polarity {
                        left && right
                    } else {
                        left || right
                    }
                }
                _ => leaf(expression, polarity),
            }
        }
    }
    mod places {
        use facts::{Fact, FactPayload, FactPlan};

        use super::names::symbol_name;

        pub(crate) fn borrow_access_label(
            program: &dyn ::validation::CheckingInput,
            borrow: &checked_trees::BorrowFacts,
            access: &checked_trees::BorrowArgumentAccessFact,
        ) -> String {
            facts::canonical_place_label_from_parts(
                program,
                access.root,
                borrow.access_segments(access),
            )
        }

        pub(crate) fn semantic_fact_requirement_label(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            fact: &Fact,
        ) -> String {
            match fact.payload {
                FactPayload::ContractDomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                }
                | FactPayload::DomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                } => {
                    let place = match fact.place {
                        facts::FactPlace::Place(place) => place,
                        _ => return "unknown domain membership".to_owned(),
                    };
                    let domain = if program.domain_definitions().iter().any(|domain| {
                        domain.symbol == domain_symbol
                            && !typed_trees::domain::index_parameters(program, domain).is_empty()
                    }) {
                        program
                            .semantic_domains
                            .name(semantic_domain)
                            .map(str::to_owned)
                            .unwrap_or_else(|| symbol_name(program, domain_symbol))
                    } else {
                        symbol_name(program, domain_symbol)
                    };
                    format!("{} in {}", semantic.place_label(program, place), domain)
                }
                facts::carry_permission_pair_pattern!(permission, ..) => {
                    let place = match fact.place {
                        facts::FactPlace::Place(place) => place,
                        _ => return "unknown carry permission".to_owned(),
                    };
                    format!(
                        "{} in {}",
                        semantic.place_label(program, place),
                        permission.name()
                    )
                }
                FactPayload::ContractBooleanExpression { .. }
                | FactPayload::BooleanExpression(_) => {
                    semantic_boolean_fact_label(program, semantic, fact)
                        .unwrap_or_else(|| "unknown boolean expression".to_owned())
                }
                FactPayload::ContractPropositionApplication { .. }
                | FactPayload::PropositionApplication { .. } => semantic
                    .proposition_fact_label(program, fact)
                    .unwrap_or_else(|| "unknown proposition application".to_owned()),
                _ => "unknown contract fact".to_owned(),
            }
        }

        /*
            Canonical caller-term label for a semantic boolean fact. Declaration facts
            render their typed expression directly; flow-instantiated operator/call
            facts use the substitution record owned by the fact plan.
        */
        pub(crate) fn semantic_boolean_fact_label(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            fact: &Fact,
        ) -> Option<String> {
            semantic.boolean_fact_label(program, fact)
        }

        pub(crate) fn joined_place_label(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            place: &facts::Place,
            extra_segments: &[facts::PlaceSegment],
        ) -> String {
            let mut segments: Vec<_> = semantic
                .place_segments
                .span_or_empty(place.segments)
                .to_vec();
            segments.extend(extra_segments.iter().copied());
            facts::canonical_place_label_from_parts(program, place.root, &segments)
        }
    }
    mod propositions {
        /*
            Proposition application label sources.
        */
        use crate::flow::expressions;

        /*
            The label sources every proposition application label instantiation
            prepares identically: the binder arguments' display names plus the
            argument expression handles callers then label positionally.
        */
        pub(crate) fn proposition_label_sources<'a>(
            application: &'a typed_trees::proposition::PropositionApplication,
            program: &'a dyn ::validation::CheckingInput,
        ) -> (Vec<String>, &'a [typed_trees::expression::ExpressionHandle]) {
            (
                application
                    .binder_arguments
                    .iter()
                    .map(|argument| argument.display_name())
                    .collect(),
                expressions(program).expression_handles(application.arguments),
            )
        }
    }

    pub(crate) use boolean_facts::context_proves_boolean_label;
    pub(crate) use kinds::{semantic_contract_fact_kind, semantic_proof_obligation_kind};
    pub(crate) use names::{call_target_label, machine_name, symbol_name};
    pub(crate) use operators::{
        boolean_expression_has_polarity, instantiate_operator_contract_expression_label,
    };
    pub(crate) use places::{
        borrow_access_label, joined_place_label, semantic_boolean_fact_label,
        semantic_fact_requirement_label,
    };
    pub(crate) use propositions::proposition_label_sources;
}
pub(crate) mod places {
    /*
        The place a value expression denotes inside one state.

        Checking asks this question at every operand of every comparison, write and
        transfer, so the answer carries one spelling here instead of repeating the
        full state-local projection call at each site.
    */
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use validation::CanonicalPlace;
    use validation::canonical_place_from_expression_in_state;

    /*
        The canonical place `expression` denotes in `state_symbol` at
        `statement_index`, when it denotes one at all.
    */
    pub(crate) fn place_at(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        canonical_place_from_expression_in_state(program, state_symbol, statement_index, expression)
    }
}
pub(crate) mod proof {
    /*
        Checked proof facts: the evidence, proof-output and outcome facts bound
        onto checked trees from proof plans.

        This file builds the proof facts. `evidence_forwarding.rs` binds
        forwarded and projected evidence, `proof_output_calls.rs` binds
        proof-output call facts and named witness lanes, `outcome_arms.rs` binds
        outcome-specific arm facts, `proposition_vocabulary.rs` builds the
        checked proposition vocabulary and lowers applications, `contracts.rs`,
        `float_meaning.rs` and `obligations.rs` carry contracts, float meaning and
        obligation lowering.
        Mathematical declaration elaboration belongs to the independent proof
        package; this owner only transfers its completed records into `ProofFacts`.
    */
    pub(crate) mod contracts {
        use crate::published::proof::proposition_vocabulary::{
            contract_fact_kind, fact_handles, lower_checked_evidence_interface,
            lower_checked_proposition_application,
        };
        use checked_trees::CheckedEvidenceTerm;
        use checked_trees::{ContractProofFact, ContractProofFactKind, ContractProofFactOwner};
        use symbols::SymbolHandle;
        pub(crate) mod call_signatures {
            /*
                Public logical signatures belong to exact source invocations. The nominal
                requirement can have several simultaneous applied views; its symbol is not
                enough to own a named evidence lane. Retention copies the whole checked
                signature and grants neither provider selection nor normal completion.
            */
            use crate::published::proof::proposition_vocabulary::fact_handles;
            use arena::{Arena, Handle};
            use checked_trees::{
                CallContractSignature, CheckedEvidenceTerm, ContractProofFact,
                ContractProofFactOwner,
            };
            use diagnostics::Diagnostic;
            use symbols::SymbolHandle;

            fn source_identity(
                source: &validation::CallContractSource<'_>,
            ) -> checked_trees::CallContractSignatureSource {
                match source {
                    validation::CallContractSource::Requirement { declaration, .. } => {
                        checked_trees::CallContractSignatureSource::Requirement {
                            declaring_trait: declaration.symbol,
                        }
                    }
                    validation::CallContractSource::Runtime { binding, contract } => {
                        checked_trees::CallContractSignatureSource::Runtime {
                            contract: binding.contract,
                            lexical_owner: contract.lexical_owner,
                            callee: binding.callee,
                        }
                    }
                }
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn retain(
                program: &dyn validation::CheckingInput,
                applications: &mut Arena<CallContractSignature>,
                contract_facts: &mut Arena<ContractProofFact>,
                evidence_terms: &mut Arena<CheckedEvidenceTerm>,
                caller_machine_symbol: SymbolHandle,
                caller_state_symbol: SymbolHandle,
                statement_index: usize,
                call_ordinal: usize,
                site: &validation::CallSite<'_>,
            ) -> Result<Handle<CallContractSignature>, Diagnostic> {
                let source_target = site.target_symbol(program);
                let Some(source) =
                    validation::call_contract_source(program, caller_machine_symbol, site)
                        .map_err(Diagnostic::error)?
                else {
                    return Ok(Handle::invalid());
                };
                let signature = source.signature();
                let source_owner = source.lexical_owner();
                let retained_source = source_identity(&source);
                let state = program
                    .state_by_symbol(caller_state_symbol)
                    .filter(|_| {
                        program
                            .machine_holding_state(caller_state_symbol)
                            .is_some_and(|machine| machine.symbol == caller_machine_symbol)
                    })
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "logical requirement contract lost its source caller state",
                        )
                    })?;
                let source_statement = if statement_index < state.statement_nodes.len() {
                    state.statement_nodes.handle_at(statement_index as u32)
                } else {
                    Default::default()
                };
                let source_expression = match site {
                    validation::CallSite::Expression { expression, .. } => *expression,
                    _ => Default::default(),
                };
                if !source_statement.is_valid() && !source_expression.is_valid() {
                    return Err(Diagnostic::error(
                        "logical requirement contract has no exact source occurrence",
                    ));
                }
                let record = CallContractSignature {
                    caller_machine_symbol,
                    caller_state_symbol,
                    statement_index,
                    call_ordinal,
                    source_statement,
                    source_expression,
                    source_target,
                    source: retained_source,
                    signature: signature.clone(),
                };
                if let Some((handle, _)) = applications
                    .iter()
                    .find(|(_, previous)| **previous == record)
                {
                    return Ok(handle);
                }
                let application = applications.append(record);
                super::append_signature_contract_facts(
                    program,
                    source_owner,
                    signature,
                    ContractProofFactOwner::CallSignature {
                        owner_symbol: source_owner,
                        state_symbol: signature.symbol,
                        application,
                    },
                    contract_facts,
                    evidence_terms,
                )?;
                Ok(application)
            }

            pub(crate) fn owner(
                applications: &Arena<CallContractSignature>,
                application: Handle<CallContractSignature>,
            ) -> Option<ContractProofFactOwner> {
                if !applications.is_valid(application) {
                    return None;
                }
                let selected = applications.get(application);
                let owner_symbol = match selected.source {
                    checked_trees::CallContractSignatureSource::Invalid => return None,
                    checked_trees::CallContractSignatureSource::Requirement { declaring_trait } => {
                        declaring_trait
                    }
                    checked_trees::CallContractSignatureSource::Runtime {
                        lexical_owner, ..
                    } => lexical_owner,
                };
                Some(ContractProofFactOwner::CallSignature {
                    owner_symbol,
                    state_symbol: selected.signature.symbol,
                    application,
                })
            }

            pub(crate) fn source_is_current(
                program: &dyn validation::CheckingInput,
                proof: &checked_trees::ProofFacts,
                call: &checked_trees::ContractCallFact,
                site: &validation::CallSite<'_>,
            ) -> bool {
                if owner(&proof.call_contract_signatures, call.signature).is_none() {
                    return false;
                }
                let retained = proof.call_contract_signatures.get(call.signature);
                if retained.caller_machine_symbol != call.caller_machine_symbol
                    || retained.caller_state_symbol != call.caller_state_symbol
                    || retained.statement_index != call.statement_index
                    || retained.call_ordinal != call.call_ordinal
                {
                    return false;
                }
                let Some(state) = program.state_by_symbol(call.caller_state_symbol) else {
                    return false;
                };
                if program
                    .machine_holding_state(call.caller_state_symbol)
                    .is_none_or(|machine| machine.symbol != call.caller_machine_symbol)
                {
                    return false;
                }
                let statement = if call.statement_index < state.statement_nodes.len() {
                    state.statement_nodes.handle_at(call.statement_index as u32)
                } else {
                    Default::default()
                };
                let expression = match site {
                    validation::CallSite::Expression { expression, .. } => *expression,
                    _ => Default::default(),
                };
                if statement != retained.source_statement
                    || expression != retained.source_expression
                    || site.target_symbol(program) != retained.source_target
                {
                    return false;
                }
                let Ok(Some(source)) =
                    validation::call_contract_source(program, call.caller_machine_symbol, site)
                else {
                    return false;
                };
                source_identity(&source) == retained.source
                    && *source.signature() == retained.signature
            }

            pub(crate) fn owns_contract(
                program: &dyn validation::CheckingInput,
                proof: &checked_trees::ProofFacts,
                call: &checked_trees::ContractCallFact,
                contract: &ContractProofFact,
            ) -> bool {
                if !call.signature.is_valid() {
                    return !matches!(contract.owner, ContractProofFactOwner::CallSignature { .. });
                }
                if owner(&proof.call_contract_signatures, call.signature) != Some(contract.owner) {
                    return false;
                }
                let Some(site) = validation::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    return false;
                };
                if !source_is_current(program, proof, call, &site) {
                    return false;
                }
                let signature = &proof.call_contract_signatures.get(call.signature).signature;
                program
                    .state_signature_contracts(signature)
                    .iter()
                    .any(|clause| {
                        matches!(
                            (&clause.kind, contract.kind),
                            (
                                typed_trees::signature::SignatureContractKind::Requires,
                                checked_trees::ContractProofFactKind::Requires
                            ) | (
                                typed_trees::signature::SignatureContractKind::Ensures,
                                checked_trees::ContractProofFactKind::Ensures
                            )
                        ) && fact_handles(clause.facts).any(|fact| fact == contract.fact)
                    })
            }

            pub(crate) fn signature<'program>(
                program: &'program dyn validation::CheckingInput,
                proof: &'program checked_trees::ProofFacts,
                selected: ContractProofFactOwner,
            ) -> Option<&'program typed_trees::signature::StateSignature> {
                if let ContractProofFactOwner::RuntimeCallableContract { contract } = selected {
                    return program
                        .runtime_callable_contracts
                        .is_valid(contract)
                        .then(|| &program.runtime_callable_contracts.get(contract).signature);
                }
                let ContractProofFactOwner::CallSignature { application, .. } = selected else {
                    return None;
                };
                (owner(&proof.call_contract_signatures, application) == Some(selected))
                    .then(|| &proof.call_contract_signatures.get(application).signature)
            }
        }
        pub(crate) mod calls {
            use validation::machine_state_count;

            use arena::HandleSpan;
            use checked_trees::statement::StatementNode;
            use checked_trees::{
                BorrowFacts, ContractCallFact, ContractExitFact, ContractProofFact,
                ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef,
            };
            use symbols::SymbolHandle;

            pub(crate) fn build_contract_call_facts(
                program: &dyn ::validation::CheckingInput,
                borrow: &BorrowFacts,
                contract_facts: &mut arena::Arena<ContractProofFact>,
                evidence_terms: &mut arena::Arena<checked_trees::CheckedEvidenceTerm>,
            ) -> Result<
                (
                    arena::Arena<ContractProofFactRef>,
                    arena::Arena<ContractCallFact>,
                    arena::Arena<checked_trees::CallContractSignature>,
                ),
                Vec<diagnostics::Diagnostic>,
            > {
                let mut fact_refs = arena::Arena::with_capacity(contract_facts.len());
                let mut calls = arena::Arena::with_capacity(borrow.calls.len());
                let mut signatures = arena::Arena::with_capacity(borrow.calls.len());

                for state in borrow.states.iter().map(|(_, state)| state) {
                    for call in borrow.calls.span_or_empty(state.calls) {
                        let call_site = validation::find_call_site(
                            program,
                            state.machine_symbol,
                            state.state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        );
                        if call_site.as_ref().and_then(|site| site.runtime_call())
                            != call.runtime_call
                        {
                            return Err(vec![diagnostics::Diagnostic::error(
                                "call contract lost its exact borrowed runtime callee and contract row",
                            )]);
                        }
                        let runtime = call_site
                            .as_ref()
                            .filter(|site| site.runtime_call().is_some())
                            .map(|site| {
                                validation::call_contract_source(
                                    program,
                                    state.machine_symbol,
                                    site,
                                )
                            })
                            .transpose()
                            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?
                            .flatten();
                        let contract_owner = if let Some(source) = &runtime {
                            (source.lexical_owner(), source.signature().symbol)
                        } else if let Some(owner) =
                            contract_target_from_state_symbol(program, call.target_symbol)
                        {
                            owner
                        } else {
                            continue;
                        };
                        let (target_machine_symbol, target_state_symbol) = if runtime.is_some() {
                            (SymbolHandle::invalid(), SymbolHandle::invalid())
                        } else if program.dynamic_slot_by_symbol(call.target_symbol).is_some() {
                            (call.target_symbol, call.target_symbol)
                        } else {
                            contract_owner
                        };
                        if call_site.is_none()
                            && validation::selected_requirement_signature(
                                program,
                                call.target_symbol,
                            )
                            .is_some()
                        {
                            return Err(vec![diagnostics::Diagnostic::error(
                                "public requirement contract has no exact source invocation",
                            )]);
                        }
                        let signature = if let Some(site) = &call_site {
                            super::call_signatures::retain(
                                program,
                                &mut signatures,
                                contract_facts,
                                evidence_terms,
                                state.machine_symbol,
                                state.state_symbol,
                                call.statement_index,
                                call.call_ordinal,
                                site,
                            )
                            .map_err(|error| vec![error])?
                        } else {
                            Default::default()
                        };
                        let signature_owner = super::call_signatures::owner(&signatures, signature);
                        let contract_target = call_site
                            .as_ref()
                            .and_then(validation::CallSite::static_requirement_dispatch)
                            .map(|dispatch| (dispatch.declaring_trait, dispatch.contract_symbol()))
                            .or_else(|| {
                                program
                                    .machine_parameter_signature(call.target_symbol)
                                    .map(|(_, signature)| (call.target_symbol, signature.symbol))
                            })
                            .unwrap_or(contract_owner);
                        append_contract_call(
                            contract_facts,
                            &mut fact_refs,
                            &mut calls,
                            call_site.as_ref().is_some_and(|site| {
                                !validation::call_site_evidence_arguments(site).is_empty()
                                    || !crate::published::semantic::calls::call_parameter_domains(
                                        program,
                                        state.machine_symbol,
                                        state.state_symbol,
                                        target_machine_symbol,
                                        target_state_symbol,
                                        site,
                                    )
                                    .is_empty()
                            }),
                            ContractCallSite {
                                caller_machine_symbol: state.machine_symbol,
                                caller_state_symbol: state.state_symbol,
                                statement_index: call.statement_index,
                                call_ordinal: call.call_ordinal,
                                target_machine_symbol,
                                target_state_symbol,
                                contract_machine_symbol: contract_target.0,
                                contract_state_symbol: contract_target.1,
                                signature,
                                signature_owner,
                                targets_machine_entry: program.machines().iter().any(|machine| {
                                    machine.symbol == target_machine_symbol
                                        && program.machine_states(machine).first().is_some_and(
                                            |entry| entry.symbol == target_state_symbol,
                                        )
                                }),
                                is_state_transfer: call_site.as_ref().is_some_and(|site| {
                                    site.is_internal_state_transfer(program, state.machine_symbol)
                                }),
                            },
                        );
                    }
                }

                Ok((fact_refs, calls, signatures))
            }

            #[derive(Debug, Clone, Copy)]
            struct ContractCallSite {
                caller_machine_symbol: SymbolHandle,
                caller_state_symbol: SymbolHandle,
                statement_index: usize,
                call_ordinal: usize,
                target_machine_symbol: SymbolHandle,
                target_state_symbol: SymbolHandle,
                contract_machine_symbol: SymbolHandle,
                contract_state_symbol: SymbolHandle,
                signature: arena::Handle<checked_trees::CallContractSignature>,
                signature_owner: Option<ContractProofFactOwner>,
                targets_machine_entry: bool,
                is_state_transfer: bool,
            }

            fn append_contract_call(
                contract_facts: &arena::Arena<ContractProofFact>,
                fact_refs: &mut arena::Arena<ContractProofFactRef>,
                calls: &mut arena::Arena<ContractCallFact>,
                has_call_obligations: bool,
                site: ContractCallSite,
            ) {
                let requires = append_contract_fact_refs(
                    contract_facts,
                    fact_refs,
                    site.contract_machine_symbol,
                    Some(site.contract_state_symbol),
                    ContractProofFactKind::Requires,
                    !site.is_state_transfer || site.targets_machine_entry,
                    site.signature_owner,
                );
                let ensures = if site.is_state_transfer {
                    HandleSpan::empty()
                } else {
                    append_contract_fact_refs(
                        contract_facts,
                        fact_refs,
                        site.contract_machine_symbol,
                        Some(site.contract_state_symbol),
                        ContractProofFactKind::Ensures,
                        true,
                        site.signature_owner,
                    )
                };

                if requires.is_empty()
                    && ensures.is_empty()
                    && !has_call_obligations
                    && !site.signature.is_valid()
                {
                    return;
                }

                calls.append(ContractCallFact {
                    caller_machine_symbol: site.caller_machine_symbol,
                    caller_state_symbol: site.caller_state_symbol,
                    statement_index: site.statement_index,
                    call_ordinal: site.call_ordinal,
                    target_machine_symbol: site.target_machine_symbol,
                    target_state_symbol: site.target_state_symbol,
                    signature: site.signature,
                    requires,
                    ensures,
                    evidence_arguments: HandleSpan::empty(),
                });
            }

            pub(crate) fn build_contract_exit_facts(
                program: &dyn ::validation::CheckingInput,
                contract_facts: &arena::Arena<ContractProofFact>,
                fact_refs: &mut arena::Arena<ContractProofFactRef>,
            ) -> arena::Arena<ContractExitFact> {
                let mut exits = arena::Arena::with_capacity(machine_state_count(program));

                for machine in program.machines() {
                    if !machine.body_is_present {
                        continue;
                    }
                    for state in program.machine_states(machine) {
                        let statements = program.statement_table.statements(state.statement_nodes);
                        let ensures = append_contract_fact_refs(
                            contract_facts,
                            fact_refs,
                            machine.symbol,
                            Some(state.symbol),
                            ContractProofFactKind::Ensures,
                            true,
                            None,
                        );

                        let mut has_transition = false;
                        for (statement_index, statement) in statements.iter().enumerate() {
                            let Some(transition) = statement.as_transition() else {
                                continue;
                            };
                            has_transition = true;
                            if transition.exit == typed_trees::statement::TransitionExit::Ordinary {
                                for target in transition.targets() {
                                    let returns_from_machine = match program
                                        .statement_table
                                        .transition_target(target)
                                    {
                                        typed_trees::statement::TransitionTargetNode::Terminal => {
                                            true
                                        }
                                        typed_trees::statement::TransitionTargetNode::Value(
                                            expression,
                                        ) => {
                                            validation::expression_normal_completion_possible(
                                                program,
                                                *expression,
                                            ) != Some(false)
                                        }
                                        typed_trees::statement::TransitionTargetNode::Named {
                                            path,
                                            static_machine_parameter,
                                            arguments,
                                            evidence_arguments,
                                            source_span,
                                            authored_call_selection,
                                            ..
                                        } => {
                                            let site = validation::CallSite::TransitionNamed {
                                                path,
                                                static_machine_parameter: *static_machine_parameter,
                                                arguments: *arguments,
                                                evidence_arguments,
                                                source_span: *source_span,
                                                authored_call_selection: *authored_call_selection,
                                            };
                                            contract_target_from_state_symbol(program, path.symbol)
                                                .is_some_and(|(_, state)| {
                                                    !site.is_internal_state_transfer(
                                                        program,
                                                        machine.symbol,
                                                    ) && validation::call_target_state(
                                                        program, state,
                                                    )
                                                    .is_none_or(|target| {
                                                        validation::normal_completion_possible(
                                                            program, target,
                                                        ) != Some(false)
                                                    })
                                                })
                                        }
                                    };
                                    if returns_from_machine {
                                        exits.append(ContractExitFact {
                                            machine_symbol: machine.symbol,
                                            state_symbol: state.symbol,
                                            statement_index,
                                            transition_target: target,
                                            ensures,
                                        });
                                    }
                                }
                            }
                            if transition.continuation.is_valid()
                                || !matches!(
                                    statements.get(statement_index + 1),
                                    Some(StatementNode::Transition(_))
                                )
                            {
                                break;
                            }
                        }
                        if !has_transition
                            && validation::normal_completion_possible(program, state) != Some(false)
                        {
                            exits.append(ContractExitFact {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                statement_index: if matches!(
                                    statements.last(),
                                    Some(StatementNode::Expression(_))
                                ) {
                                    statements.len() - 1
                                } else {
                                    statements.len()
                                },
                                transition_target: Default::default(),
                                ensures,
                            });
                        }
                    }
                }

                exits
            }

            fn append_contract_fact_refs(
                contract_facts: &arena::Arena<ContractProofFact>,
                fact_refs: &mut arena::Arena<ContractProofFactRef>,
                machine_symbol: SymbolHandle,
                state_symbol: Option<SymbolHandle>,
                kind: ContractProofFactKind,
                include_machine_contracts: bool,
                signature_owner: Option<ContractProofFactOwner>,
            ) -> HandleSpan<ContractProofFactRef> {
                let mut span = HandleSpan::empty();

                for (handle, fact) in contract_facts.iter() {
                    let owner_matches = if let Some(owner) = signature_owner {
                        fact.owner == owner
                    } else {
                        match fact.owner {
                            ContractProofFactOwner::Machine {
                                machine_symbol: owner_symbol,
                            } => include_machine_contracts && owner_symbol == machine_symbol,
                            ContractProofFactOwner::MachineState {
                                machine_symbol: owner_machine_symbol,
                                state_symbol: owner_state_symbol,
                            } => {
                                owner_machine_symbol == machine_symbol
                                    && state_symbol.is_some_and(|state_symbol| {
                                        state_symbol == owner_state_symbol
                                    })
                            }
                            ContractProofFactOwner::StateSignature {
                                owner_symbol,
                                state_symbol: owner_state_symbol,
                            } => {
                                owner_symbol == machine_symbol
                                    && state_symbol.is_some_and(|state_symbol| {
                                        state_symbol == owner_state_symbol
                                    })
                            }
                            ContractProofFactOwner::Unknown
                            | ContractProofFactOwner::RuntimeCallableContract { .. }
                            | ContractProofFactOwner::CallSignature { .. }
                            | ContractProofFactOwner::OperatorDeclaration { .. }
                            | ContractProofFactOwner::OperatorUse { .. } => false,
                        }
                    };

                    if owner_matches && fact.kind == kind {
                        fact_refs.append_to_span(&mut span, ContractProofFactRef { fact: handle });
                    }
                }

                span
            }

            pub(crate) fn contract_target_from_state_symbol(
                program: &dyn ::validation::CheckingInput,
                target_state_symbol: SymbolHandle,
            ) -> Option<(SymbolHandle, SymbolHandle)> {
                if !target_state_symbol.is_valid() {
                    return None;
                }

                if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
                    return Some((slot.declaring_trait, slot.requirement));
                }

                if let Some(machine) = program.machine_by_symbol(target_state_symbol) {
                    let entry = program.machine_states(machine).first()?;
                    return Some((machine.symbol, entry.symbol));
                }

                if let Some((target_machine, _)) =
                    validation::find_state(program, target_state_symbol)
                {
                    return Some((target_machine.symbol, target_state_symbol));
                }

                if program
                    .machine_parameter_signature(target_state_symbol)
                    .is_some()
                {
                    return Some((target_state_symbol, target_state_symbol));
                }

                if let Some((declaration, signature)) =
                    validation::selected_requirement_signature(program, target_state_symbol)
                {
                    return Some((declaration.symbol, signature.symbol));
                }

                None
            }
        }
        mod inherited {
            use super::{CheckedEvidenceTerm, ContractEvidenceLanes};
            use checked_trees::{ContractProofFact, ContractProofFactOwner};
            use symbols::SymbolHandle;

            /* Facts carried by one state signature's contracts. */
            fn signature_fact_total(
                program: &dyn ::validation::CheckingInput,
                signature: &typed_trees::signature::StateSignature,
            ) -> usize {
                program
                    .state_signature_contracts(signature)
                    .iter()
                    .map(|contract| contract.facts.len())
                    .sum()
            }

            pub(crate) fn estimated_contract_fact_capacity(
                program: &dyn ::validation::CheckingInput,
            ) -> usize {
                program
                    .machines()
                    .iter()
                    .map(|machine| {
                        let machine_facts = program
                            .machine_contracts(machine)
                            .iter()
                            .map(|contract| contract.facts.len())
                            .sum::<usize>();
                        let state_facts = program
                            .machine_states(machine)
                            .iter()
                            .flat_map(|state| program.state_contracts(state))
                            .map(|contract| contract.facts.len())
                            .sum::<usize>();
                        machine_facts + state_facts
                    })
                    .chain(program.traits().iter().map(|trait_definition| {
                        program
                            .trait_machine_signatures(trait_definition)
                            .iter()
                            .map(|signature| signature_fact_total(program, signature))
                            .sum::<usize>()
                    }))
                    .chain(program.all_operators().map(|operator| {
                        program
                            .operator_contracts(operator)
                            .iter()
                            .map(|contract| contract.facts.len())
                            .sum::<usize>()
                    }))
                    .chain(program.machines().iter().map(|machine| {
                        program
                            .machine_type_parameters(machine)
                            .iter()
                            .filter_map(|parameter| match &parameter.kind {
                                typed_trees::data::TypeParameterKind::Machine { contract } => {
                                    program.machine_parameter_contract_view(contract).map(
                                        typed_trees::data::MachineParameterContractView::signature,
                                    )
                                }
                                _ => None,
                            })
                            .map(|signature| signature_fact_total(program, signature))
                            .sum::<usize>()
                    }))
                    .chain(program.machines().iter().map(|machine| {
                        estimated_inherited_trait_contract_fact_capacity(program, machine)
                    }))
                    .sum()
            }

            pub(crate) fn append_inherited_trait_contract_facts(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                contract_facts: &mut arena::Arena<ContractProofFact>,
                inherited_scopes: &mut arena::Arena<checked_trees::InheritedContractScope>,
                evidence_terms: &arena::Arena<CheckedEvidenceTerm>,
            ) {
                let mut visited_traits = Vec::new();
                for conformance in program.machine_trait_conformances(machine) {
                    let Some(trait_definition) =
                        trait_definition_by_symbol(program, conformance.symbol)
                    else {
                        continue;
                    };
                    let effective_arguments = program
                        .type_reference_table
                        .type_reference_handles(conformance.arguments);
                    append_trait_contract_facts_for_machine(
                        program,
                        machine,
                        trait_definition,
                        effective_arguments,
                        contract_facts,
                        inherited_scopes,
                        evidence_terms,
                        &mut visited_traits,
                    );
                }
            }

            fn append_trait_contract_facts_for_machine(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                trait_definition: &typed_trees::trait_definition::TraitDefinition,
                effective_arguments: &[typed_trees::types::TypeReferenceHandle],
                contract_facts: &mut arena::Arena<ContractProofFact>,
                inherited_scopes: &mut arena::Arena<checked_trees::InheritedContractScope>,
                evidence_terms: &arena::Arena<CheckedEvidenceTerm>,
                visited_traits: &mut Vec<SymbolHandle>,
            ) {
                if visited_traits.contains(&trait_definition.symbol) {
                    return;
                }

                visited_traits.push(trait_definition.symbol);

                for signature in program.trait_machine_signatures(trait_definition) {
                    let exact_direct_requirement = machine_selects_exact_requirement(
                        program,
                        machine,
                        trait_definition.symbol,
                        signature,
                    );
                    let Some((target_machine_symbol, target_state_symbol)) =
                        trait_requirement_state_symbols(
                            program,
                            machine,
                            trait_definition.symbol,
                            signature,
                        )
                    else {
                        continue;
                    };

                    let mut lanes = ContractEvidenceLanes::default();
                    for contract in program.state_signature_contracts(signature) {
                        let Some(kind) =
                            crate::published::proof::proposition_vocabulary::contract_fact_kind(
                                &contract.kind,
                            )
                        else {
                            continue;
                        };
                        for fact in crate::published::proof::proposition_vocabulary::fact_handles(
                            contract.facts,
                        ) {
                            let evidence_term = contract.binding.as_ref().and_then(|_| {
                                if !exact_direct_requirement {
                                    return None;
                                }
                                let lane_position = lanes.next(kind);
                                evidence_terms.iter().find_map(|(handle, term)| {
                                    (term.owner
                                        == ContractProofFactOwner::Machine {
                                            machine_symbol: target_machine_symbol,
                                        }
                                        && term.kind == kind
                                        && term.lane_position == lane_position)
                                        .then_some(handle)
                                })
                            });
                            let qualification_authorization =
                                ::validation::boundary_qualification_authorization(
                                    program,
                                    trait_definition.symbol,
                                    signature,
                                    contract.kind.clone(),
                                    program.proof_facts.get(fact),
                                );
                            let scope =
                                inherited_scopes.append(checked_trees::InheritedContractScope {
                                    conformance_machine: machine.symbol,
                                    declaring_trait: trait_definition.symbol,
                                    requirement: signature.symbol,
                                    trait_arguments: effective_arguments.to_vec(),
                                });
                            contract_facts.append(ContractProofFact {
                                kind,
                                owner: ContractProofFactOwner::MachineState {
                                    machine_symbol: target_machine_symbol,
                                    state_symbol: target_state_symbol,
                                },
                                fact,
                                evidence_term,
                                nominal_argument_roots: None,
                                qualification_authorization,
                                inherited_scope: Some(scope),
                            });
                        }
                    }
                }

                for requirement in program.trait_requirements(trait_definition) {
                    let Some(required_trait) =
                        trait_definition_by_symbol(program, requirement.symbol)
                    else {
                        continue;
                    };
                    let parent_arguments = program
                        .type_reference_table
                        .type_reference_handles(requirement.arguments);
                    let composed_arguments = validation::compose_forwarded_trait_arguments(
                        program,
                        trait_definition,
                        effective_arguments,
                        parent_arguments,
                    );
                    append_trait_contract_facts_for_machine(
                        program,
                        machine,
                        required_trait,
                        &composed_arguments,
                        contract_facts,
                        inherited_scopes,
                        evidence_terms,
                        visited_traits,
                    );
                }

                visited_traits.pop();
            }

            fn estimated_inherited_trait_contract_fact_capacity(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
            ) -> usize {
                let mut visited_traits = Vec::new();
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter_map(|conformance| {
                        trait_definition_by_symbol(program, conformance.symbol)
                    })
                    .map(|trait_definition| {
                        estimated_trait_contract_fact_capacity_for_machine(
                            program,
                            machine,
                            trait_definition,
                            &mut visited_traits,
                        )
                    })
                    .sum()
            }

            fn estimated_trait_contract_fact_capacity_for_machine(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                trait_definition: &typed_trees::trait_definition::TraitDefinition,
                visited_traits: &mut Vec<SymbolHandle>,
            ) -> usize {
                if visited_traits.contains(&trait_definition.symbol) {
                    return 0;
                }

                visited_traits.push(trait_definition.symbol);

                let direct = program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .filter(|signature| {
                        trait_requirement_state_symbols(
                            program,
                            machine,
                            trait_definition.symbol,
                            signature,
                        )
                        .is_some()
                    })
                    .map(|signature| {
                        program
                            .state_signature_contracts(signature)
                            .iter()
                            .map(|contract| contract.facts.len())
                            .sum::<usize>()
                    })
                    .sum::<usize>();
                let inherited = program
                    .trait_requirements(trait_definition)
                    .iter()
                    .filter_map(|requirement| {
                        trait_definition_by_symbol(program, requirement.symbol)
                    })
                    .map(|required_trait| {
                        estimated_trait_contract_fact_capacity_for_machine(
                            program,
                            machine,
                            required_trait,
                            visited_traits,
                        )
                    })
                    .sum::<usize>();

                visited_traits.pop();
                direct.saturating_add(inherited)
            }

            /*
                The state that owes `requirement`'s contract facts through `machine`'s
                own conformance row. Every row names one exact requirement, so each
                implementing machine appends its own requirement's facts when its turn
                comes; a sibling requirement of the same trait is that sibling's row to
                publish. Appending it here as well, located through a same-named sibling
                state, restated one requirement once per conforming machine of the
                attached type (ten rows for one clause on a ten-leaf provider) under a
                conformance edge the sibling never authored.
            */
            fn trait_requirement_state_symbols(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                trait_symbol: SymbolHandle,
                requirement: &typed_trees::signature::StateSignature,
            ) -> Option<(SymbolHandle, SymbolHandle)> {
                machine_selects_exact_requirement(program, machine, trait_symbol, requirement)
                    .then(|| {
                        program
                            .machine_states(machine)
                            .first()
                            .map(|state| (machine.symbol, state.symbol))
                    })
                    .flatten()
            }

            fn machine_selects_exact_requirement(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                trait_symbol: SymbolHandle,
                requirement: &typed_trees::signature::StateSignature,
            ) -> bool {
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        conformance.symbol == trait_symbol
                            && conformance
                                .requirement
                                .as_ref()
                                .is_some_and(|name| *name == requirement.name)
                    })
            }

            fn trait_definition_by_symbol(
                program: &dyn ::validation::CheckingInput,
                symbol: SymbolHandle,
            ) -> Option<&typed_trees::trait_definition::TraitDefinition> {
                if !symbol.is_valid() {
                    return None;
                }

                program.trait_definition_by_symbol(symbol)
            }
        }
        mod operators {
            use crate::published::proof::proposition_vocabulary::{
                contract_fact_kind, fact_handles,
            };
            use arena::HandleSpan;
            use checked_trees::{
                CheckedOperatorContractUse, CheckedOperatorFacts, ContractOperatorUseFact,
                ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
                ContractProofFactRef,
            };

            pub(crate) fn build_contract_operator_use_facts(
                program: &dyn ::validation::CheckingInput,
                operators: &CheckedOperatorFacts,
                contract_facts: &mut arena::Arena<ContractProofFact>,
                fact_refs: &mut arena::Arena<ContractProofFactRef>,
            ) -> arena::Arena<ContractOperatorUseFact> {
                let mut operator_uses =
                    arena::Arena::with_capacity(operators.resolved_contract_uses().count());

                for operator_use in operators.resolved_contract_uses() {
                    if operator_use.operator_use.occurrence
                        != checked_trees::CheckedOperatorOccurrence::Expression
                    {
                        continue;
                    }
                    append_contract_operator_use(
                        program,
                        contract_facts,
                        fact_refs,
                        &mut operator_uses,
                        operator_use,
                    );
                }

                operator_uses
            }

            fn append_contract_operator_use(
                program: &dyn ::validation::CheckingInput,
                contract_facts: &mut arena::Arena<ContractProofFact>,
                fact_refs: &mut arena::Arena<ContractProofFactRef>,
                operator_uses: &mut arena::Arena<ContractOperatorUseFact>,
                operator_use: CheckedOperatorContractUse<'_>,
            ) {
                let mut requires = HandleSpan::empty();
                let mut ensures = HandleSpan::empty();

                for contract in program
                    .signature_contracts
                    .span_or_empty(operator_use.contracts())
                {
                    let Some(kind) = contract_fact_kind(&contract.kind) else {
                        continue;
                    };
                    let target = match kind {
                        ContractProofFactKind::Requires => &mut requires,
                        ContractProofFactKind::Ensures => &mut ensures,
                    };

                    for fact in fact_handles(contract.facts) {
                        let contract_fact = contract_facts.append(ContractProofFact {
                            kind,
                            owner: ContractProofFactOwner::OperatorUse {
                                expression: operator_use.operator_use.expression,
                                origin: operator_use.operator_use.origin,
                                operator_symbol: operator_use.operator_symbol(),
                            },
                            fact,
                            evidence_term: None,
                            nominal_argument_roots: None,
                            qualification_authorization: None,
                            inherited_scope: None,
                        });
                        fact_refs.append_to_span(
                            target,
                            ContractProofFactRef {
                                fact: contract_fact,
                            },
                        );
                    }
                }

                if requires.is_empty() && ensures.is_empty() {
                    return;
                }

                operator_uses.append(ContractOperatorUseFact {
                    expression: operator_use.operator_use.expression,
                    origin: operator_use.operator_use.origin,
                    operator_symbol: operator_use.operator_symbol(),
                    requires,
                    ensures,
                });
            }
        }

        pub(crate) use calls::{
            build_contract_call_facts, build_contract_exit_facts, contract_target_from_state_symbol,
        };
        pub(crate) use inherited::{
            append_inherited_trait_contract_facts, estimated_contract_fact_capacity,
        };
        pub(crate) use operators::build_contract_operator_use_facts;

        /*
            The named-evidence lanes of one contract owner's clause list.

            A lane position counts that owner's named bindings of that kind in authored
            clause order. Erased call lanes and review-evidence projection both address
            evidence terms by it, so a published term and its position must always
            agree; `requires` and `ensures` count independently.
        */
        #[derive(Default)]
        pub(crate) struct ContractEvidenceLanes {
            requires: usize,
            ensures: usize,
        }

        impl ContractEvidenceLanes {
            pub(crate) fn next(&mut self, kind: ContractProofFactKind) -> usize {
                let counter = match kind {
                    ContractProofFactKind::Requires => &mut self.requires,
                    ContractProofFactKind::Ensures => &mut self.ensures,
                };
                let position = *counter;
                *counter += 1;
                position
            }
        }

        /*
            Publish the checked evidence term one named contract binding owns.

            Validation guarantees the binding names a proposition whose nominal
            endpoint carries witness evidence; a row that lost that endpoint is
            reported here rather than re-derived.
        */
        pub(crate) fn append_named_contract_evidence_term(
            program: &dyn ::validation::CheckingInput,
            owner: ContractProofFactOwner,
            kind: ContractProofFactKind,
            lane_position: usize,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
            binding: &str,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<arena::Handle<CheckedEvidenceTerm>, diagnostics::Diagnostic> {
            let Some(application) = program.proof_facts.get(fact).as_proposition() else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "named contract binding `{binding}` is not a validated proposition"
                )));
            };
            let Some(normalized) =
                program.normalize_nominal_proposition_application(application, None)
            else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "named contract binding `{binding}` lost its nominal proposition endpoint"
                )));
            };
            let typed_trees::proposition::PropositionEvidenceClassification::Witness {
                evidence,
                interface,
            } = &normalized.classification
            else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "named contract binding `{binding}` must bind witness evidence"
                )));
            };
            let evidence_type = evidence.clone();
            let evidence_interface = interface.as_ref().map(lower_checked_evidence_interface);
            Ok(evidence_terms.append(CheckedEvidenceTerm {
                name: binding.to_owned(),
                owner,
                kind,
                lane_position,
                proposition: lower_checked_proposition_application(normalized),
                evidence_type,
                evidence_interface,
            }))
        }

        /*
            Publish the checked facts one owner's contract clause list owes.

            Every clause names its own kind; a named binding additionally publishes its
            checked evidence term in this list's lane order. `boundary_signature` is
            the owner's boundary-authorization policy: a public logical signature
            carries one, machine and state contract rows carry none.
        */
        fn append_contract_clause_facts(
            program: &dyn ::validation::CheckingInput,
            contracts: &[typed_trees::signature::SignatureContract],
            owner: ContractProofFactOwner,
            boundary_signature: Option<(SymbolHandle, &typed_trees::signature::StateSignature)>,
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            let mut lanes = ContractEvidenceLanes::default();
            for contract in contracts {
                let Some(kind) = contract_fact_kind(&contract.kind) else {
                    continue;
                };
                for fact in fact_handles(contract.facts) {
                    let evidence_term = match contract.binding.as_ref() {
                        Some(binding) => Some(append_named_contract_evidence_term(
                            program,
                            owner,
                            kind,
                            lanes.next(kind),
                            fact,
                            binding.as_str(),
                            evidence_terms,
                        )?),
                        None => None,
                    };
                    let qualification_authorization =
                        boundary_signature.and_then(|(owner_symbol, signature)| {
                            ::validation::boundary_qualification_authorization(
                                program,
                                owner_symbol,
                                signature,
                                contract.kind.clone(),
                                program.proof_facts.get(fact),
                            )
                        });
                    contract_facts.append(ContractProofFact {
                        kind,
                        owner,
                        fact,
                        evidence_term,
                        nominal_argument_roots: None,
                        qualification_authorization,
                        inherited_scope: None,
                    });
                }
            }
            Ok(())
        }

        /*
            Collect the callable signatures that need binder-owned checked evidence.

            Top-level machine parameters retain the historical behavior for both
            structural and nominal contracts. Nested parameters are different: an
            inline structural contract owns another binder contract and therefore
            needs its own evidence rows, while a nominal contract merely references a
            trait requirement whose evidence is emitted with that trait.

            The traversal is iterative and deduplicates arena-backed parameter
            identities. Besides avoiding call-stack growth, this makes a malformed
            cyclic signature span terminate without repeatedly emitting the same row.
        */
        pub(crate) fn machine_parameter_evidence_signatures<'program>(
            program: &'program dyn ::validation::CheckingInput,
            parameters: &'program [typed_trees::data::TypeParameter],
        ) -> Vec<(
            SymbolHandle,
            SymbolHandle,
            &'program typed_trees::signature::StateSignature,
        )> {
            let mut signatures = Vec::new();
            let mut pending = parameters
                .iter()
                .rev()
                .map(|parameter| (parameter, true))
                .collect::<Vec<_>>();
            let mut visited = std::collections::HashSet::new();

            while let Some((parameter, is_top_level)) = pending.pop() {
                if !visited.insert(parameter as *const typed_trees::data::TypeParameter) {
                    continue;
                }
                let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
                else {
                    continue;
                };
                match contract {
                    typed_trees::data::MachineParameterContract::RequirementIdentity => {}
                    typed_trees::data::MachineParameterContract::Structural(signature) => {
                        let target_state = if is_top_level {
                            parameter.symbol
                        } else {
                            signature.symbol
                        };
                        signatures.push((parameter.symbol, target_state, signature));
                        pending.extend(
                            program
                                .state_signature_type_parameters(signature)
                                .iter()
                                .rev()
                                .map(|nested| (nested, false)),
                        );
                    }
                    typed_trees::data::MachineParameterContract::Nominal { .. } if is_top_level => {
                        let signature = program
                            .machine_parameter_contract_view(contract)
                            .expect(
                                "typed machine-parameter contract must retain a valid requirement identity",
                            )
                            .signature();
                        signatures.push((parameter.symbol, parameter.symbol, signature));
                    }
                    typed_trees::data::MachineParameterContract::Nominal { .. } => {}
                }
            }

            signatures
        }

        /* Publish a machine's own contract clause facts. */
        pub(crate) fn append_machine_contract_facts(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            append_contract_clause_facts(
                program,
                program.machine_contracts(machine),
                ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                None,
                contract_facts,
                evidence_terms,
            )
        }

        pub(crate) fn append_operator_declaration_contract_facts(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            contract_facts: &mut arena::Arena<ContractProofFact>,
        ) {
            let owner = ContractProofFactOwner::OperatorDeclaration {
                operator_symbol: operator.symbol,
            };
            for contract in program.operator_contracts(operator) {
                let Some(kind) = contract_fact_kind(&contract.kind) else {
                    continue;
                };
                for fact in fact_handles(contract.facts) {
                    contract_facts.append(ContractProofFact {
                        kind,
                        owner,
                        fact,
                        evidence_term: None,
                        nominal_argument_roots: None,
                        qualification_authorization: None,
                        inherited_scope: None,
                    });
                }
            }
        }

        /*
            Publish a state's arrival contract clause facts.

            The syntax stage admits `requires` only on states, so a state clause list
            carries no exit guarantees to number.
        */
        pub(crate) fn append_state_contract_facts(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            append_contract_clause_facts(
                program,
                program.state_contracts(state),
                ContractProofFactOwner::MachineState {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                None,
                contract_facts,
                evidence_terms,
            )
        }

        /*
            Append the contract facts owed by every evidence signature reachable
            from `parameters` through machine-parameter evidence.
        */
        pub(crate) fn append_parameter_evidence_contract_facts(
            program: &dyn ::validation::CheckingInput,
            parameters: &[typed_trees::data::TypeParameter],
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            for (owner_symbol, _, contract) in
                machine_parameter_evidence_signatures(program, parameters)
            {
                append_state_signature_contract_facts(
                    program,
                    owner_symbol,
                    std::slice::from_ref(contract),
                    contract_facts,
                    evidence_terms,
                )?;
            }
            Ok(())
        }

        pub(crate) fn append_state_signature_contract_facts(
            program: &dyn ::validation::CheckingInput,
            owner_symbol: SymbolHandle,
            signatures: &[typed_trees::signature::StateSignature],
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            for signature in signatures {
                append_signature_contract_facts(
                    program,
                    owner_symbol,
                    signature,
                    ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol: signature.symbol,
                    },
                    contract_facts,
                    evidence_terms,
                )?;
            }
            Ok(())
        }

        /*
            One builder retains the entire logical signature and both ordered evidence
            lanes. An invocation-qualified owner changes identity, never its clauses.
        */
        pub(crate) fn append_runtime_callable_contract_facts(
            program: &dyn validation::CheckingInput,
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            for (contract, row) in program.runtime_callable_contracts.iter() {
                append_signature_contract_facts(
                    program,
                    row.lexical_owner,
                    &row.signature,
                    ContractProofFactOwner::RuntimeCallableContract { contract },
                    contract_facts,
                    evidence_terms,
                )?;
            }
            Ok(())
        }

        pub(crate) fn append_signature_contract_facts(
            program: &dyn ::validation::CheckingInput,
            owner_symbol: SymbolHandle,
            signature: &typed_trees::signature::StateSignature,
            owner: ContractProofFactOwner,
            contract_facts: &mut arena::Arena<ContractProofFact>,
            evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
        ) -> Result<(), diagnostics::Diagnostic> {
            append_contract_clause_facts(
                program,
                program.state_signature_contracts(signature),
                owner,
                Some((owner_symbol, signature)),
                contract_facts,
                evidence_terms,
            )
        }
    }
    mod evidence_forwarding {
        /*
            Forwarded and projected evidence facts with definite-assignment
            validation.
        */
        use crate::published::proof::outcome_arms::{
            exact_result_case, fact_label, outcome_specific_assignment_matches_result,
            outcome_specific_fact_is_proved,
        };
        use crate::published::proof::proof_output_calls::{
            call_parameter_substitutions, intake_call_ensures_propositions,
            intake_checked_proof_output_propositions,
        };
        use crate::published::proof::proposition_vocabulary::{
            contract_proposition_labels, fact_handles, lower_checked_proposition_application,
        };
        use checked_trees::{
            CheckedEvidenceTerm, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
        };
        use symbols::SymbolHandle;

        pub(crate) fn bind_evidence_forwarding_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &mut ProofFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            let mut diagnostics = Vec::new();
            let mut forwardings = arena::Arena::default();

            for forwarding in &program.evidence_forwardings {
                let output = evidence_term_named(
                    &proof.evidence_terms,
                    forwarding.machine_symbol,
                    forwarding.target.as_str(),
                    ContractProofFactKind::Ensures,
                );
                let source = evidence_term_named(
                    &proof.evidence_terms,
                    forwarding.machine_symbol,
                    forwarding.source.as_str(),
                    ContractProofFactKind::Requires,
                )
                .or_else(|| {
                    proof
                        .proof_output_calls
                        .iter()
                        .flat_map(|(_, invocation)| {
                            (invocation.caller_machine_symbol == forwarding.machine_symbol
                                && invocation.caller_state_symbol == forwarding.state_symbol
                                && invocation.source_statement_index
                                    < forwarding.source_statement_index)
                                .then_some(
                                    invocation.outputs.iter().filter_map(|output| output.output),
                                )
                                .into_iter()
                                .flatten()
                        })
                        .find(|term| {
                            proof.evidence_terms.get(*term).name == forwarding.source.as_str()
                        })
                });
                let Some(output) = output else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "evidence forwarding target `{}` is not a named ensures binding of this machine",
                        forwarding.target
                    )));
                    continue;
                };
                let output_term = proof.evidence_terms.get(output);
                let guarded_output = proof
                    .outcome_specific_guarantees
                    .iter()
                    .any(|(_, row)| row.evidence_term == Some(output));
                let source = if let Some(source) = source {
                    let source_term = proof.evidence_terms.get(source);
                    if output_term.evidence_interface != source_term.evidence_interface
                        || (!guarded_output && output_term.proposition != source_term.proposition)
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "cannot forward evidence term `{}` into `{}` because their proposition identities differ",
                            forwarding.source, forwarding.target
                        )));
                        continue;
                    }
                    checked_trees::EvidenceAssignmentSource::Forwarded { term: source }
                } else if let Some(conformance_symbol) = forwarding.source_conformance {
                    let Some(source) = checked_evidence_producer(
                        program,
                        output_term,
                        conformance_symbol,
                        forwarding.source.as_str(),
                    ) else {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "named implementation `{}` does not provide the exact `{}` evidence interface required by `{}`",
                            forwarding.source, output_term.evidence_type, forwarding.target
                        )));
                        continue;
                    };
                    source
                } else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "evidence forwarding source `{}` is not a named requires binding of this machine, a named implementation, or an available proof-output term",
                        forwarding.source
                    )));
                    continue;
                };
                forwardings.append(checked_trees::EvidenceForwardingFact {
                    machine_symbol: forwarding.machine_symbol,
                    state_symbol: forwarding.state_symbol,
                    statement_index: forwarding.statement_index,
                    output,
                    source,
                });
            }

            if diagnostics.is_empty() {
                proof.evidence_forwardings = forwardings;
                validate_evidence_forwarding_definite_assignment(program, proof)
            } else {
                Err(diagnostics)
            }
        }

        pub(crate) fn bind_evidence_projection_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &mut ProofFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            use checked_trees::CheckedEvidenceProjection;

            let mut diagnostics = Vec::new();
            let mut applications = Vec::new();
            for (fact_handle, fact) in typed_trees::references::program_proof_facts(program) {
                let Some(application) = fact.as_proposition() else {
                    continue;
                };
                let Some(normalized) =
                    program.normalize_nominal_proposition_application(application, None)
                else {
                    continue;
                };
                let projections = application
                    .binder_arguments
                    .iter()
                    .filter(|argument| argument.evidence_projection.is_some())
                    .count();
                if projections == 0 {
                    applications.push(lower_checked_proposition_application(normalized));
                    continue;
                }
                let mut owners = Vec::new();
                for (_, contract) in proof.contract_facts.iter() {
                    if contract.fact == fact_handle && !owners.contains(&contract.owner) {
                        owners.push(contract.owner);
                    }
                }
                if owners.is_empty() {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "carrierless evidence projection in proposition `{}` has no named-contract scope",
                        application.name
                    )));
                    continue;
                }
                let prepared = lower_checked_proposition_application(normalized);
                for owner in owners {
                    let mut checked = prepared.clone();
                    let mut bound_projections = Vec::new();
                    for typed_argument in application.binder_arguments.iter() {
                        let Some(projection) = &typed_argument.evidence_projection else {
                            continue;
                        };
                        let Some(term) = evidence_term_in_scope(
                            &proof.evidence_terms,
                            owner,
                            projection.term.as_str(),
                        ) else {
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "carrierless evidence projection `{}.{}` names no retained evidence term in this contract scope",
                            projection.term, projection.member
                        )));
                            continue;
                        };
                        let term_definition = proof.evidence_terms.get(term);
                        let Some(interface) = &term_definition.evidence_interface else {
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "carrierless evidence projection `{}.{}` uses an unresolved evidence interface",
                            projection.term, projection.member
                        )));
                            continue;
                        };
                        let matching_rows = interface
                            .requirements
                            .iter()
                            .filter(|row| {
                                program.symbols.name(row.requirement) == projection.member.as_str()
                            })
                            .collect::<Vec<_>>();
                        let [row] = matching_rows.as_slice() else {
                            let reason = if matching_rows.is_empty() {
                                "does not contain"
                            } else {
                                "contains more than one requirement named"
                            };
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                                "carrierless evidence interface `{}` {reason} `{}` for projection `{}.{}`",
                                term_definition.evidence_type,
                                projection.member,
                                projection.term,
                                projection.member
                            )));
                            continue;
                        };
                        bound_projections.push((
                            format!("{}.{}", projection.term, projection.member),
                            CheckedEvidenceProjection {
                                term,
                                declaring_trait: row.declaring_trait,
                                declaring_trait_arguments: row.declaring_trait_arguments.clone(),
                                requirement: row.requirement,
                            },
                        ));
                    }
                    for (label, projection) in bound_projections {
                        let mut matched = false;
                        for checked_argument in &mut checked.binder_arguments {
                            if checked_argument.diagnostic_label == label {
                                checked_argument.diagnostic_label.clear();
                                checked_argument.evidence_projection = Some(projection.clone());
                                matched = true;
                            }
                        }
                        if !matched {
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "carrierless evidence projection `{label}` is not retained by the normalized proposition endpoint"
                        )));
                        }
                    }
                    let terms = proof
                        .contract_facts
                        .iter()
                        .filter_map(|(_, contract)| {
                            (contract.fact == fact_handle && contract.owner == owner)
                                .then_some(contract.evidence_term)
                                .flatten()
                        })
                        .collect::<Vec<_>>();
                    for term in terms {
                        proof.evidence_terms.get_mut(term).proposition = checked.clone();
                    }
                    applications.push(checked);
                }
            }
            if diagnostics.is_empty() {
                proof.proposition_vocabulary.applications = applications;
                Ok(())
            } else {
                Err(diagnostics)
            }
        }

        fn evidence_term_in_scope(
            terms: &arena::Arena<CheckedEvidenceTerm>,
            owner: ContractProofFactOwner,
            name: &str,
        ) -> Option<arena::Handle<CheckedEvidenceTerm>> {
            let local = terms.iter().find_map(|(handle, term)| {
                (term.owner == owner
                    && term.kind == ContractProofFactKind::Requires
                    && term.name == name)
                    .then_some(handle)
            });
            if local.is_some() {
                return local;
            }
            let ContractProofFactOwner::MachineState { machine_symbol, .. } = owner else {
                return None;
            };
            terms.iter().find_map(|(handle, term)| {
                (term.owner == ContractProofFactOwner::Machine { machine_symbol }
                    && term.kind == ContractProofFactKind::Requires
                    && term.name == name)
                    .then_some(handle)
            })
        }

        fn checked_evidence_producer(
            program: &dyn ::validation::CheckingInput,
            output: &CheckedEvidenceTerm,
            conformance_symbol: SymbolHandle,
            source_name: &str,
        ) -> Option<checked_trees::EvidenceAssignmentSource> {
            use typed_trees::trait_definition::ConformanceRowSource;

            let expected_interface = output.evidence_interface.as_ref()?;
            let conformance = program.conformance_by_symbol(conformance_symbol)?;
            if conformance.alias.as_ref()?.as_str() != source_name {
                return None;
            }
            let evidence_trait = program
                .traits()
                .iter()
                .find(|candidate| candidate.name == conformance.trait_name)?;
            let arguments = program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .map(|argument| program.normalized_type_identity(*argument))
                .collect::<Vec<_>>();
            if evidence_trait.symbol != expected_interface.trait_symbol
                || arguments != expected_interface.arguments
            {
                return None;
            }
            let rows = &conformance.rows;
            let rows = rows
                .iter()
                .map(|row| {
                    let (requirement_identity, realization_identity) =
                        crate::published::facts::normalized_dynamic_row_identities(program, row)
                            .ok()?;
                    Some(checked_trees::DynamicConformanceRowFact {
                        declaring_trait: row.declaring_trait,
                        requirement: row.requirement,
                        requirement_identity,
                        realization_machine: row.realization_machine,
                        realization_state: row.realization_state,
                        realization_identity,
                        family_tuple: Box::default(),
                        source: match row.source {
                            ConformanceRowSource::Inline | ConformanceRowSource::Synthesized => {
                                checked_trees::DynamicConformanceRowSource::Inline
                            }
                            ConformanceRowSource::Reference => {
                                checked_trees::DynamicConformanceRowSource::Reference
                            }
                            ConformanceRowSource::TraitDefault => {
                                checked_trees::DynamicConformanceRowSource::TraitDefault
                            }
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()?;

            Some(
                checked_trees::EvidenceAssignmentSource::ProducerConformance {
                    conformance: conformance.symbol,
                    evidence_trait: evidence_trait.symbol,
                    rows,
                },
            )
        }

        fn validate_evidence_forwarding_definite_assignment(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            use std::collections::{BTreeMap, BTreeSet, VecDeque};
            use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

            let mut diagnostic_messages = BTreeSet::new();

            for machine in program.machines() {
                let guarded_output_handles = proof
                    .outcome_specific_guarantees
                    .iter()
                    .filter_map(|(_, row)| {
                        (row.machine_symbol == machine.symbol)
                            .then_some(row.evidence_term)
                            .flatten()
                    })
                    .map(|handle| handle.arena_index())
                    .collect::<BTreeSet<_>>();
                let unconditional_outputs = proof
                    .evidence_terms
                    .iter()
                    .filter_map(|(handle, term)| {
                        (term.owner
                            == (ContractProofFactOwner::Machine {
                                machine_symbol: machine.symbol,
                            })
                            && term.kind == ContractProofFactKind::Ensures)
                            .then_some(handle)
                            .filter(|handle| {
                                !guarded_output_handles.contains(&handle.arena_index())
                            })
                    })
                    .collect::<Vec<_>>();
                let has_guarded_rows = proof
                    .outcome_specific_guarantees
                    .iter()
                    .any(|(_, row)| row.machine_symbol == machine.symbol);
                if unconditional_outputs.is_empty() && !has_guarded_rows {
                    continue;
                }
                let states = program.machine_states(machine);
                let Some(entry) = states.first() else {
                    continue;
                };
                let initial_known = contract_proposition_labels(
                    program,
                    program.machine_contracts(machine),
                    typed_trees::signature::SignatureContractKind::Requires,
                    &[],
                );
                let mut work = VecDeque::from([(
                    entry.symbol,
                    BTreeMap::<u32, Option<arena::Handle<CheckedEvidenceTerm>>>::new(),
                    initial_known,
                )]);
                let mut seen = BTreeSet::new();

                while let Some((state_symbol, mut assigned, mut known)) = work.pop_front() {
                    let key = (
                        state_symbol.arena_index(),
                        assigned
                            .iter()
                            .map(|(output, source)| {
                                (*output, source.map(|source| source.arena_index()))
                            })
                            .collect::<Vec<_>>(),
                        known.iter().cloned().collect::<Vec<_>>(),
                    );
                    if !seen.insert(key) {
                        continue;
                    }
                    let Some(state) = typed_trees::state::state_by_symbol(states, state_symbol)
                    else {
                        continue;
                    };
                    known.extend(contract_proposition_labels(
                        program,
                        program.state_contracts(state),
                        typed_trees::signature::SignatureContractKind::Requires,
                        &[],
                    ));

                    let assignments = proof
                        .evidence_forwardings
                        .iter()
                        .filter_map(|(_, forwarding)| {
                            (forwarding.machine_symbol == machine.symbol
                                && forwarding.state_symbol == state.symbol)
                                .then_some(forwarding)
                        })
                        .collect::<Vec<_>>();
                    let statements = program.statement_table.statements(state.statement_nodes);
                    if assignments
                        .iter()
                        .any(|forwarding| forwarding.statement_index > statements.len())
                    {
                        diagnostic_messages.insert(format!(
                            "named ensures evidence assignment coordinate is outside {}::{}",
                            machine.name, state.name
                        ));
                        continue;
                    }

                    let mut stops_fallthrough = false;
                    for statement_index in 0..=statements.len() {
                        intake_checked_proof_output_propositions(
                            proof,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            &mut known,
                        );
                        for forwarding in assignments
                            .iter()
                            .filter(|forwarding| forwarding.statement_index == statement_index)
                        {
                            let source = match &forwarding.source {
                                checked_trees::EvidenceAssignmentSource::Forwarded { term } => {
                                    Some(*term)
                                }
                                checked_trees::EvidenceAssignmentSource::ProducerConformance {
                                    ..
                                } => None,
                            };
                            if assigned
                                .insert(forwarding.output.arena_index(), source)
                                .is_some()
                            {
                                let term = proof.evidence_terms.get(forwarding.output);
                                diagnostic_messages.insert(format!(
                                    "named ensures evidence `{}` is assigned more than once on a reachable path through {}::{}",
                                    term.name, machine.name, state.name
                                ));
                            }
                        }

                        let Some(statement) = statements.get(statement_index) else {
                            continue;
                        };
                        if let Some(call) = statement.as_call() {
                            intake_call_ensures_propositions(program, call, &mut known);
                        }
                        let Some(transition) = statement.as_transition() else {
                            continue;
                        };
                        if transition.exit == TransitionExit::Ordinary {
                            for target_handle in transition.targets() {
                                match program.statement_table.transition_target(target_handle) {
                                    TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } => {
                                        let target = if path.symbol == machine.symbol {
                                            entry.symbol
                                        } else {
                                            path.symbol
                                        };
                                        if typed_trees::state::state_by_symbol(states, target)
                                            .is_some()
                                        {
                                            work.push_back((
                                                target,
                                                assigned.clone(),
                                                known.clone(),
                                            ));
                                        } else {
                                            append_missing_evidence_diagnostics(
                                                program,
                                                proof,
                                                machine,
                                                state,
                                                &unconditional_outputs,
                                                &assigned,
                                                ExitResult::TerminalCall(
                                                    terminal_call_guarantees(
                                                        program,
                                                        proof,
                                                        path.symbol,
                                                        None,
                                                        program
                                                            .statement_table
                                                            .expression_handles(*arguments),
                                                    )
                                                    .as_ref(),
                                                ),
                                                &known,
                                                &mut diagnostic_messages,
                                            );
                                        }
                                    }
                                    TransitionTargetNode::Value(_)
                                    | TransitionTargetNode::Terminal => {
                                        append_missing_evidence_diagnostics(
                                            program,
                                            proof,
                                            machine,
                                            state,
                                            &unconditional_outputs,
                                            &assigned,
                                            ExitResult::Value(
                                                match program
                                                    .statement_table
                                                    .transition_target(target_handle)
                                                {
                                                    TransitionTargetNode::Value(value) => {
                                                        Some(*value)
                                                    }
                                                    TransitionTargetNode::Terminal => None,
                                                    _ => unreachable!(),
                                                },
                                            ),
                                            &known,
                                            &mut diagnostic_messages,
                                        );
                                    }
                                }
                            }
                        }
                        let next_coordinate_has_assignment = assignments
                            .iter()
                            .any(|forwarding| forwarding.statement_index == statement_index + 1);
                        let run_ends = transition.continuation.is_valid()
                            || next_coordinate_has_assignment
                            || !matches!(
                                statements.get(statement_index + 1),
                                Some(StatementNode::Transition(_))
                            );
                        if run_ends {
                            stops_fallthrough = true;
                            break;
                        }
                    }

                    if !stops_fallthrough {
                        append_missing_evidence_diagnostics(
                            program,
                            proof,
                            machine,
                            state,
                            &unconditional_outputs,
                            &assigned,
                            ExitResult::Value(statements.last().and_then(
                                |statement| match statement {
                                    StatementNode::Expression(expression) => Some(*expression),
                                    _ => None,
                                },
                            )),
                            &known,
                            &mut diagnostic_messages,
                        );
                    }
                }
            }

            if diagnostic_messages.is_empty() {
                Ok(())
            } else {
                Err(diagnostic_messages
                    .into_iter()
                    .map(diagnostics::Diagnostic::error)
                    .collect())
            }
        }

        fn append_missing_evidence_diagnostics(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            outputs: &[arena::Handle<CheckedEvidenceTerm>],
            assigned: &std::collections::BTreeMap<u32, Option<arena::Handle<CheckedEvidenceTerm>>>,
            exit: ExitResult<'_>,
            known: &std::collections::BTreeSet<String>,
            messages: &mut std::collections::BTreeSet<String>,
        ) {
            // A call in result position is a terminal call: the callee, not
            // this machine, produces the result case.
            let call_value = match exit {
                ExitResult::Value(Some(value)) => {
                    match program.expression_table.expression(value) {
                        typed_trees::expression::ExpressionNode::Call(call) => {
                            terminal_call_guarantees(
                                program,
                                proof,
                                call.target_symbol,
                                call.runtime_call,
                                program.expression_table.expression_handles(call.arguments),
                            )
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let exit = match &call_value {
                Some(callee) => ExitResult::TerminalCall(Some(callee)),
                None => exit,
            };
            let result = match exit {
                ExitResult::Value(result) => result,
                ExitResult::TerminalCall(Some(callee)) => {
                    append_terminal_call_discharge_diagnostics(
                        program, proof, machine, state, outputs, assigned, callee, messages,
                    );
                    return;
                }
                ExitResult::TerminalCall(None) => None,
            };
            for output in outputs {
                if !assigned.contains_key(&output.arena_index()) {
                    messages.insert(format!(
                        "named ensures evidence `{}` is not definitely assigned on the ordinary exit through {}::{}",
                        proof.evidence_terms.get(*output).name,
                        machine.name,
                        state.name
                    ));
                }
            }

            let guarded_rows = proof
                .outcome_specific_guarantees
                .iter()
                .filter_map(|(_, row)| (row.machine_symbol == machine.symbol).then_some(row))
                .collect::<Vec<_>>();
            if guarded_rows.is_empty() {
                return;
            }
            let Some((result_data, result_case)) =
                result.and_then(|result| exact_result_case(program, result))
            else {
                messages.insert(format!(
                    "cannot classify the ordinary result case on exit through {}::{}; outcome-specific guarantees require an exact nominal result constructor",
                    machine.name, state.name
                ));
                return;
            };
            for row in guarded_rows {
                if row.result_data != result_data {
                    messages.insert(format!(
                        "ordinary exit through {}::{} produces a result outside the declared outcome-specific result sum",
                        machine.name, state.name
                    ));
                    continue;
                }
                let Some(output) = row.evidence_term else {
                    if row.result_case == result_case
                        && !outcome_specific_fact_is_proved(program, row.fact, result, known)
                    {
                        messages.insert(format!(
                            "cannot prove outcome-specific guarantee on the matching ordinary exit through {}::{} after substituting its concrete result",
                            machine.name, state.name
                        ));
                    }
                    continue;
                };
                let term = proof.evidence_terms.get(output);
                let assigned_source = assigned.get(&output.arena_index());
                let is_assigned = assigned_source.is_some();
                if row.result_case == result_case && !is_assigned {
                    messages.insert(format!(
                        "outcome-specific evidence `{}` is not definitely assigned on the matching ordinary exit through {}::{}",
                        term.name, machine.name, state.name
                    ));
                } else if row.result_case == result_case {
                    if let Some(Some(source)) = assigned_source
                        && !outcome_specific_assignment_matches_result(
                            program, proof, row.fact, result, *source,
                        )
                    {
                        messages.insert(format!(
                            "outcome-specific evidence `{}` does not inhabit its guarantee after substituting the concrete result on exit through {}::{}",
                            term.name, machine.name, state.name
                        ));
                    }
                } else if is_assigned {
                    messages.insert(format!(
                        "outcome-specific evidence `{}` is assigned on a nonmatching ordinary exit through {}::{}",
                        term.name, machine.name, state.name
                    ));
                }
            }
        }

        /// How an ordinary exit produces the machine's result: an expression
        /// evaluated in this machine, or a terminal call whose callee produces
        /// it. `TerminalCall(None)` is a call to something that is not a
        /// checked machine, which publishes no guarantees to discharge from.
        #[derive(Clone, Copy)]
        enum ExitResult<'a> {
            Value(Option<typed_trees::expression::ExpressionHandle>),
            TerminalCall(Option<&'a TerminalCallGuarantees>),
        }

        /// The guarantees a terminal call's callee publishes, instantiated at
        /// the call's arguments. Only the declaring machine assigns its named
        /// evidence outputs (wiki/spec/language/machines.md,
        /// `nested-evidence-output-establishment`), so a caller whose exit is a
        /// terminal call discharges its own guarantees from these and the
        /// callee's result case. The call's result is the caller's result, so
        /// `result` stays unsubstituted on both sides.
        struct TerminalCallGuarantees {
            callee_name: String,
            rows: Vec<TerminalCallGuarantee>,
            /// The result cases the callee's ordinary exits can produce, or
            /// `None` when some exit is not classifiable. A caller guarantee
            /// on a case outside this set holds vacuously at this call.
            result_cases: Option<Vec<(SymbolHandle, SymbolHandle)>>,
        }

        struct TerminalCallGuarantee {
            /// `None` for an unconditional guarantee, which holds for every
            /// result case.
            result_case: Option<(SymbolHandle, SymbolHandle)>,
            label: String,
            /// The guarantee names an evidence output the callee assigns.
            supplies_evidence: bool,
        }

        fn terminal_call_guarantees(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            target_symbol: SymbolHandle,
            runtime_call: Option<typed_trees::callable_values::RuntimeCallBinding>,
            arguments: &[typed_trees::expression::ExpressionHandle],
        ) -> Option<TerminalCallGuarantees> {
            if let Some(binding) = runtime_call {
                let signature = binding.signature(program.typed_program()).ok()?;
                let parameters = program.state_signature_parameters(signature);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let substitutions = parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| {
                        (
                            parameter.symbol,
                            parameter.name.to_string(),
                            program.render_proof_expression(
                                *argument,
                                typed_trees::proposition::ProofSubstitutions::None,
                            ),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut rows = Vec::new();
                for contract in program.state_signature_contracts(signature) {
                    let result_case = match contract.kind {
                        typed_trees::signature::SignatureContractKind::Ensures => None,
                        typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                            result_data,
                            result_case,
                        } => Some((result_data, result_case)),
                        _ => continue,
                    };
                    for fact in fact_handles(contract.facts) {
                        rows.push(TerminalCallGuarantee {
                            result_case,
                            label: fact_label(program, fact, &substitutions)?,
                            supplies_evidence: contract.binding.is_some(),
                        });
                    }
                }
                return Some(TerminalCallGuarantees {
                    callee_name: "runtime callable".to_owned(),
                    rows,
                    result_cases: None,
                });
            }
            let (callee, entry) = program
                .machine_by_symbol(target_symbol)
                .and_then(|machine| {
                    program
                        .machine_states(machine)
                        .first()
                        .map(|state| (machine, state))
                })
                .or_else(|| validation::find_state(program, target_symbol))?;
            let substitutions = call_parameter_substitutions(program, entry, arguments);
            let mut rows = proof
                .outcome_specific_guarantees
                .iter()
                .filter(|(_, row)| row.machine_symbol == callee.symbol)
                .filter_map(|(_, row)| {
                    Some(TerminalCallGuarantee {
                        result_case: Some((row.result_data, row.result_case)),
                        label: fact_label(program, row.fact, &substitutions)?,
                        supplies_evidence: row.evidence_term.is_some(),
                    })
                })
                .collect::<Vec<_>>();
            for contract in program.machine_contracts(callee) {
                if !matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Ensures
                ) {
                    continue;
                }
                rows.extend(fact_handles(contract.facts).filter_map(|fact| {
                    Some(TerminalCallGuarantee {
                        result_case: None,
                        label: fact_label(program, fact, &substitutions)?,
                        supplies_evidence: contract.binding.is_some(),
                    })
                }));
            }
            Some(TerminalCallGuarantees {
                callee_name: callee.name.as_str().to_owned(),
                rows,
                result_cases: possible_result_cases(program, callee, &mut Vec::new()),
            })
        }

        /// Over-approximate the nominal result cases a machine's ordinary
        /// exits can produce: every transition target value and trailing
        /// expression of every state counts, and a terminal call contributes
        /// its callee's cases. This is a body-derived summary of a callee in
        /// the same verification unit (`callee-result-body-summary`). Any exit
        /// that is neither an exact constructor nor a call to a checked machine
        /// makes the set unknown, so the caller falls back to requiring a
        /// guarantee for every case. A machine already being summarized adds
        /// no cases of its own: cases only come from constructors.
        fn possible_result_cases(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            visiting: &mut Vec<SymbolHandle>,
        ) -> Option<Vec<(SymbolHandle, SymbolHandle)>> {
            use typed_trees::expression::ExpressionNode;
            use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

            if visiting.contains(&machine.symbol) {
                return Some(Vec::new());
            }
            visiting.push(machine.symbol);
            let states = program.machine_states(machine);
            let mut cases = Vec::new();
            let call_cases = |target_symbol: SymbolHandle,
                              cases: &mut Vec<(SymbolHandle, SymbolHandle)>,
                              visiting: &mut Vec<SymbolHandle>|
             -> Option<()> {
                if target_symbol == machine.symbol
                    || typed_trees::state::state_by_symbol(states, target_symbol).is_some()
                {
                    return Some(());
                }
                let callee = program.machine_by_symbol(target_symbol).or_else(|| {
                    validation::find_state(program, target_symbol).map(|(machine, _)| machine)
                })?;
                for case in possible_result_cases(program, callee, visiting)? {
                    if !cases.contains(&case) {
                        cases.push(case);
                    }
                }
                Some(())
            };
            let value_cases = |value: typed_trees::expression::ExpressionHandle,
                               cases: &mut Vec<(SymbolHandle, SymbolHandle)>,
                               visiting: &mut Vec<SymbolHandle>|
             -> Option<()> {
                if let ExpressionNode::Call(call) = program.expression_table.expression(value) {
                    return call_cases(call.target_symbol, cases, visiting);
                }
                let case = exact_result_case(program, value)?;
                if !cases.contains(&case) {
                    cases.push(case);
                }
                Some(())
            };
            let summary = states.iter().try_for_each(|state| {
                let statements = program.statement_table.statements(state.statement_nodes);
                for statement in statements {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    if transition.exit != TransitionExit::Ordinary {
                        continue;
                    }
                    for target in transition.targets() {
                        match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Named { path, .. } => {
                                call_cases(path.symbol, &mut cases, visiting)?
                            }
                            TransitionTargetNode::Value(value) => {
                                value_cases(*value, &mut cases, visiting)?
                            }
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                }
                if let Some(StatementNode::Expression(expression)) = statements.last() {
                    value_cases(*expression, &mut cases, visiting)?;
                }
                Some(())
            });
            visiting.pop();
            summary.map(|()| cases)
        }

        /// Discharge each guarantee the caller owes on a terminal-call exit
        /// from a callee guarantee with the same instantiated fact on the same
        /// result case (or on every case). A caller output that the callee
        /// does not guarantee stays unassigned: the caller cannot assign it
        /// either, because the callee alone decides the result case.
        #[allow(clippy::too_many_arguments)]
        fn append_terminal_call_discharge_diagnostics(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            outputs: &[arena::Handle<CheckedEvidenceTerm>],
            assigned: &std::collections::BTreeMap<u32, Option<arena::Handle<CheckedEvidenceTerm>>>,
            callee: &TerminalCallGuarantees,
            messages: &mut std::collections::BTreeSet<String>,
        ) {
            let discharged =
                |case: Option<(SymbolHandle, SymbolHandle)>, label: &str, needs_evidence: bool| {
                    callee.rows.iter().any(|row| {
                        (row.result_case.is_none() || row.result_case == case)
                            && row.label == label
                            && (row.supplies_evidence || !needs_evidence)
                    })
                };
            for output in outputs {
                if assigned.contains_key(&output.arena_index()) {
                    continue;
                }
                let term = proof.evidence_terms.get(*output);
                let facts = program
                    .machine_contracts(machine)
                    .iter()
                    .filter(|contract| {
                        matches!(
                            contract.kind,
                            typed_trees::signature::SignatureContractKind::Ensures
                        ) && contract
                            .binding
                            .as_ref()
                            .is_some_and(|binding| binding.as_str() == term.name)
                    })
                    .flat_map(|contract| fact_handles(contract.facts))
                    .collect::<Vec<_>>();
                let supplied = !facts.is_empty()
                    && facts.iter().all(|fact| {
                        fact_label(program, *fact, &[])
                            .is_some_and(|label| discharged(None, &label, true))
                    });
                if !supplied {
                    messages.insert(format!(
                        "named ensures evidence `{}` is not definitely assigned on the ordinary exit through {}::{}; the terminal call to `{}` does not guarantee it",
                        term.name, machine.name, state.name, callee.callee_name
                    ));
                }
            }
            for (_, row) in proof
                .outcome_specific_guarantees
                .iter()
                .filter(|(_, row)| row.machine_symbol == machine.symbol)
            {
                let case_is_unreachable = callee
                    .result_cases
                    .as_ref()
                    .is_some_and(|cases| !cases.contains(&(row.result_data, row.result_case)));
                if case_is_unreachable {
                    continue;
                }
                if let Some(output) = row.evidence_term
                    && assigned.contains_key(&output.arena_index())
                {
                    messages.insert(format!(
                        "outcome-specific evidence `{}` is assigned before the terminal call to `{}` on exit through {}::{}; the callee's result case decides it",
                        proof.evidence_terms.get(output).name,
                        callee.callee_name,
                        machine.name,
                        state.name
                    ));
                    continue;
                }
                let supplied = row.fact_is_truth && row.evidence_term.is_none()
                    || fact_label(program, row.fact, &[]).is_some_and(|label| {
                        discharged(
                            Some((row.result_data, row.result_case)),
                            &label,
                            row.evidence_term.is_some(),
                        )
                    });
                if !supplied {
                    messages.insert(format!(
                        "cannot discharge outcome-specific guarantee on exit through {}::{} from the terminal call to `{}`; the callee must guarantee the same fact on that result case",
                        machine.name, state.name, callee.callee_name
                    ));
                }
            }
        }

        fn evidence_term_named(
            terms: &arena::Arena<CheckedEvidenceTerm>,
            machine_symbol: SymbolHandle,
            name: &str,
            kind: ContractProofFactKind,
        ) -> Option<arena::Handle<CheckedEvidenceTerm>> {
            terms.iter().find_map(|(handle, term)| {
                (term.owner == ContractProofFactOwner::Machine { machine_symbol }
                    && term.kind == kind
                    && term.name == name)
                    .then_some(handle)
            })
        }
    }
    mod float_meaning {
        /*
            Erase validated source float-projection invocations into checked proof
            rows, and bind the sealed `FloatSemantics` applications those rows
            reference so a contract obligation cites its exact checked kernel binding.

            Semantic applications ride the same projection table: each application
            owns one canonical proof value whose projection source stays transitional
            until lowering rejoins it to the Terminal `SemanticApplication` carrier.
            Validation and checked binding share exact sealed contract and format
            recognition. Binding records identity; it does not itself prove equality.
        */
        use crate::flow::expressions;
        use checked_trees::{
            CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
            CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
            CheckedDirectOperationFloatResult, CheckedDirectStructuralFloatLeaf,
            CheckedFloatMeaningEqualityProposition as EqualityProposition,
            CheckedFloatMeaningProjection as MeaningProjection,
            CheckedFloatMeaningProjectionOccurrence, CheckedFloatMeaningProjectionOccurrenceId,
            CheckedFloatProjectionInput, CheckedFloatProjectionInputId,
            CheckedFloatProjectionSource as ProjectionSource, CheckedFloatSemanticApplication,
            CheckedFloatSemanticApplicationOperand as ApplicationOperand, CheckedFloatUseSite,
            CheckedProofOnlyValueType, CheckedProofPropositionId as PropositionId,
            CheckedProofValueDeclaration, CheckedProofValueId, ContractProofFactKind,
            ContractProofFactRef, ProofFacts,
        };
        use diagnostics::Diagnostic;
        use numerics::float_projection::FloatProjectionOperation;
        use numerics::float_semantics_catalog::{
            FloatSemanticContractIdentity, FloatSemanticValueKind as ValueKind,
        };
        use semantic_vocabulary::IeeeFloatFormat;
        use std::collections::{HashMap, HashSet};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::operator::resolve_named_expression_call;
        use typed_trees::types::PrimitiveType;
        use validation::{
            ValidatedFloatMeaningEqualityProposition,
            ValidatedFloatMeaningProjectionInvocation as ProjectionInvocation,
            exact_toolchain_float_format_const, exact_toolchain_float_semantic_contract,
        };

        macro_rules! err {
            ($message:literal) => {
                vec![Diagnostic::error($message)]
            };
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        enum CheckedFloatProjectionSourceKey {
            DirectMachineParameter {
                owner_machine: symbols::SymbolHandle,
                parameter: symbols::SymbolHandle,
            },
            DirectMachineResult {
                owner_machine: symbols::SymbolHandle,
            },
            DirectBlockParameter {
                owner_machine: symbols::SymbolHandle,
                owner_state: symbols::SymbolHandle,
                parameter: symbols::SymbolHandle,
            },
            DirectStructuralLeaf {
                owner_machine: symbols::SymbolHandle,
                field: checked_trees::CheckedStructuralParameterField,
            },
            /*
                The scalar result produced at one exact checked call use. A
                transported `ensures` clause names the importing use's produced
                result, never a shared declaration-level value.
            */
            DirectCallResult {
                use_site: CheckedFloatUseSite,
            },
            /*
                The scalar result produced at one exact checked non-call operation
                use. `use_expression` keeps two uses sharing a statement distinct
                before the lowered occurrence join runs.
            */
            DirectOperationResult {
                use_site: CheckedFloatUseSite,
                use_expression: typed_trees::expression::ExpressionHandle,
            },
            ResolvedSymbol(symbols::SymbolHandle),
            Binary32Literal(u32),
            Binary64Literal(u64),
            /*
                Transitional exact typed-expression custody for source forms whose
                artifact-reconstructible Terminal coordinate has not landed yet.
            */
            TypedExpression(typed_trees::expression::ExpressionHandle),
            /*
                One application of a sealed `FloatSemantics` catalog row. The key
                carries the exact contract, declared result format, and bound operand
                values, so the result dedupes only when the whole semantic
                application repeats; distinct transport sites and operand bindings
                keep distinct transitional identities.
            */
            SemanticApplication {
                contract: FloatSemanticContractIdentity,
                format: IeeeFloatFormat,
                operands: Vec<ApplicationOperand>,
            },
        }

        fn projection_source_key(
            program: &dyn ::validation::CheckingInput,
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> CheckedFloatProjectionSourceKey {
            match program.expression_table.expression(fact.source) {
                ExpressionNode::Name(path) => {
                    direct_machine_parameter_source(program, carriers, fact)
                        .map(|(owner_machine, parameter)| {
                            CheckedFloatProjectionSourceKey::DirectMachineParameter {
                                owner_machine,
                                parameter,
                            }
                        })
                        .or_else(|| {
                            direct_machine_result_source(program, carriers, fact).map(
                                |owner_machine| {
                                    CheckedFloatProjectionSourceKey::DirectMachineResult {
                                        owner_machine,
                                    }
                                },
                            )
                        })
                        .or_else(|| {
                            direct_block_parameter_source(program, carriers, fact).map(
                                |(owner_machine, owner_state, parameter)| {
                                    CheckedFloatProjectionSourceKey::DirectBlockParameter {
                                        owner_machine,
                                        owner_state,
                                        parameter,
                                    }
                                },
                            )
                        })
                        .or_else(|| {
                            direct_structural_float_leaf_source(program, carriers, fact).map(
                                |(owner_machine, field)| {
                                    CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                                        owner_machine,
                                        field,
                                    }
                                },
                            )
                        })
                        .unwrap_or_else(|| {
                            if path.symbol.is_valid() {
                                CheckedFloatProjectionSourceKey::ResolvedSymbol(path.symbol)
                            } else {
                                CheckedFloatProjectionSourceKey::TypedExpression(fact.source)
                            }
                        })
                }
                ExpressionNode::Float(literal) => match fact.source_primitive {
                    PrimitiveType::F32 => {
                        CheckedFloatProjectionSourceKey::Binary32Literal(literal.f32_bits())
                    }
                    PrimitiveType::F64 => CheckedFloatProjectionSourceKey::Binary64Literal(
                        literal.landed_f64().to_bits(),
                    ),
                    _ => CheckedFloatProjectionSourceKey::TypedExpression(fact.source),
                },
                _ => direct_structural_float_leaf_source(program, carriers, fact)
                    .map(|(owner_machine, field)| {
                        CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                            owner_machine,
                            field,
                        }
                    })
                    .unwrap_or(CheckedFloatProjectionSourceKey::TypedExpression(
                        fact.source,
                    )),
            }
        }

        /*
            One machine or state contract fact carrying a validated projection
            invocation somewhere in its expression tree.
        */
        #[derive(Clone, Copy)]
        struct ContractCarrier {
            machine: symbols::SymbolHandle,
            state: Option<symbols::SymbolHandle>,
            kind: ContractProofFactKind,
        }

        /*
            Contract facts carrying each validated projection invocation, gathered in
            one walk over every machine and state contract expression rather than a
            rescan per invocation and source class.
        */
        struct ContractCarriers {
            by_invocation: HashMap<ExpressionHandle, Vec<ContractCarrier>>,
        }

        impl ContractCarriers {
            fn new(
                program: &dyn ::validation::CheckingInput,
                proof: &ProofFacts,
                facts: &[ProjectionInvocation],
            ) -> Self {
                let mut by_invocation = facts
                    .iter()
                    .map(|fact| (fact.invocation, Vec::new()))
                    .collect::<HashMap<_, Vec<ContractCarrier>>>();
                if by_invocation.is_empty() {
                    return Self { by_invocation };
                }
                let mut visited = HashSet::new();
                let mut pending = Vec::new();
                for (_, contract) in proof.contract_facts.iter() {
                    let (machine, state) = match contract.owner {
                        checked_trees::ContractProofFactOwner::Machine { machine_symbol } => {
                            (machine_symbol, None)
                        }
                        checked_trees::ContractProofFactOwner::MachineState {
                            machine_symbol,
                            state_symbol,
                        } => (machine_symbol, Some(state_symbol)),
                        _ => continue,
                    };
                    let carrier = ContractCarrier {
                        machine,
                        state,
                        kind: contract.kind,
                    };
                    visited.clear();
                    pending.clear();
                    pending.extend_from_slice(proof_fact_roots(program, contract.fact));
                    while let Some(expression) = pending.pop() {
                        if let Some(carried) = by_invocation.get_mut(&expression)
                            && !visited.contains(&expression)
                        {
                            carried.push(carrier);
                        }
                        if !expression.is_valid() || !visited.insert(expression) {
                            continue;
                        }
                        pending.extend(expression_children(program, expression));
                    }
                }
                Self { by_invocation }
            }

            fn of(&self, invocation: ExpressionHandle) -> &[ContractCarrier] {
                self.by_invocation
                    .get(&invocation)
                    .map_or(&[], Vec::as_slice)
            }
        }

        /*
            Unique machine contract owner carrying the validated invocation, retaining
            the owning nested state when the fact belongs to a state-owned arrival
            contract. Nested states admit `requires` only; their direct scalar
            parameters are Terminal block parameters, a distinct source class from
            machine parameters.
        */
        fn direct_machine_contract_owner(
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> Option<(symbols::SymbolHandle, Option<symbols::SymbolHandle>)> {
            let [carrier] = carriers.of(fact.invocation) else {
                return None;
            };
            Some((carrier.machine, carrier.state))
        }

        fn direct_structural_float_leaf_source(
            program: &dyn ::validation::CheckingInput,
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> Option<(
            symbols::SymbolHandle,
            checked_trees::CheckedStructuralParameterField,
        )> {
            let (owner_machine, _) = direct_machine_contract_owner(carriers, fact)?;
            let machine = program.machine_by_symbol(owner_machine)?;
            let entry = program.machine_states(machine).first()?;
            let parameters = program.state_parameters(entry);
            let place = validation::canonical_place_from_expression(program, fact.source)?;
            let root = place.root.as_symbol()?;
            let parameter_position = parameters
                .iter()
                .position(|parameter| {
                    parameter.symbol == root
                        && !parameter.is_const
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .and_then(|position| u32::try_from(position).ok())?;
            let path = place
                .segments
                .iter()
                .map(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        validation::structural_member_identity(program, *symbol)
                            .map(checked_trees::CheckedStructuralPredicatePathSegment::Field)
                    }
                    facts::PlaceSegment::Case { variant } => {
                        validation::structural_member_identity(program, *variant)
                            .map(checked_trees::CheckedStructuralPredicatePathSegment::Case)
                    }
                    facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::OpaqueRange
                    | facts::PlaceSegment::Index { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if path.is_empty() {
                return None;
            }
            let root_type = crate::values::scalar::structural_fields::predicate_parameter_type(
                program,
                parameters[parameter_position as usize].type_reference,
            )?;
            let (path, _) = crate::values::scalar::structural_fields::explicit_observation_path(
                program, root_type, &path,
            )?;
            Some((
                owner_machine,
                checked_trees::CheckedStructuralParameterField {
                    parameter_position,
                    path,
                },
            ))
        }

        fn direct_machine_result_source(
            program: &dyn ::validation::CheckingInput,
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> Option<symbols::SymbolHandle> {
            let path = program.expression_table.name_path_node(fact.source)?;
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if name.as_str() != "result" {
                return None;
            }
            let (owner_machine, _) = direct_machine_contract_owner(carriers, fact)?;
            let owning_contract = carriers.of(fact.invocation).iter().any(|carrier| {
                carrier.machine == owner_machine
                    && carrier.state.is_none()
                    && carrier.kind == ContractProofFactKind::Ensures
            });
            if !owning_contract {
                return None;
            }
            let machine = program.machine_by_symbol(owner_machine)?;
            let entry = program.machine_states(machine).first()?;
            if !language_core::signature_reserves_result(
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| program.symbols.name(parameter.symbol)),
            ) {
                return None;
            }
            let primitive = program.primitive_type_reference(entry.return_type)?;
            if primitive != fact.source_primitive
                || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            Some(owner_machine)
        }

        fn direct_machine_parameter_source(
            program: &dyn ::validation::CheckingInput,
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
            let path = program.expression_table.name_path_node(fact.source)?;
            if expressions(program).name_path_members(path.members).len() != 1 {
                return None;
            }
            let (owner_machine, _) = direct_machine_contract_owner(carriers, fact)?;
            let machine = program.machine_by_symbol(owner_machine)?;
            let entry = program.machine_states(machine).first()?;
            let parameter = program.state_parameter_by_symbol(entry, path.symbol)?;
            if parameter.is_const {
                return None;
            }
            let primitive = program.primitive_type_reference(parameter.type_reference)?;
            if primitive != fact.source_primitive
                || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            Some((owner_machine, parameter.symbol))
        }

        /*
            A scalar parameter of the owning nested state is a Terminal block
            parameter. The entry state is excluded: its parameters already carry the
            machine-parameter class. States admit arrival `requires` only, so no
            result carrier exists here.
        */
        fn direct_block_parameter_source(
            program: &dyn ::validation::CheckingInput,
            carriers: &ContractCarriers,
            fact: ProjectionInvocation,
        ) -> Option<(
            symbols::SymbolHandle,
            symbols::SymbolHandle,
            symbols::SymbolHandle,
        )> {
            let path = program.expression_table.name_path_node(fact.source)?;
            if expressions(program).name_path_members(path.members).len() != 1 {
                return None;
            }
            let (owner_machine, owner_state) = direct_machine_contract_owner(carriers, fact)?;
            let owner_state = owner_state?;
            let machine = program.machine_by_symbol(owner_machine)?;
            let entry = program.machine_states(machine).first()?;
            if entry.symbol == owner_state {
                return None;
            }
            let state = program.state_in_machine(machine, owner_state)?;
            let parameter = program.state_parameter_by_symbol(state, path.symbol)?;
            if parameter.is_const {
                return None;
            }
            let primitive = program.primitive_type_reference(parameter.type_reference)?;
            if primitive != fact.source_primitive
                || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            Some((owner_machine, owner_state, parameter.symbol))
        }

        fn proof_fact_roots(
            program: &dyn ::validation::CheckingInput,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
        ) -> &[ExpressionHandle] {
            match program.proof_facts.get(fact) {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    std::slice::from_ref(expression)
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    std::slice::from_ref(&membership.value)
                }
                typed_trees::domain::ProofFact::Proposition(application) => {
                    expressions(program).expression_handles(application.arguments)
                }
            }
        }

        fn expression_children(
            program: &dyn ::validation::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Vec<typed_trees::expression::ExpressionHandle> {
            let mut children = Vec::new();
            program.subtree_children(
                program.expression_table.expression(expression),
                typed_trees::SubtreeEdges::STRUCTURAL,
                |child| {
                    children.push(child);
                    false
                },
            );
            children
        }

        fn replay_invocation(
            program: &dyn ::validation::CheckingInput,
            fact: ProjectionInvocation,
        ) -> Result<numerics::float_projection::FloatProjectionContractIdentity, Diagnostic>
        {
            let Some(call) = program.expression_table.call_node(fact.invocation) else {
                return Err(Diagnostic::error(
                    "validated float-meaning projection invocation is no longer a call",
                ));
            };
            let operator = resolve_named_expression_call(program, call)
                .or_else(|| ::validation::float_static_call_resolution(program, call))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "validated float-meaning projection operator no longer resolves exactly",
                    )
                })?;
            if operator.symbol != fact.selected_operator_symbol {
                return Err(Diagnostic::error(
                    "validated float-meaning projection operator identity drifted before checked binding",
                ));
            }
            let [namespace, name] = program.operator_path_members(operator.name) else {
                return Err(Diagnostic::error(
                    "validated float-meaning projection operator path is no longer exact",
                ));
            };
            let operation =
                FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
                    .ok_or_else(|| {
                    Diagnostic::error(
                        "validated float-meaning projection operator identity is not canonical",
                    )
                })?;
            let Some((replayed_primitive, contract)) =
                validation::exact_toolchain_float_projection_contract(program, operator, operation)
            else {
                return Err(Diagnostic::error(
                    "validated float-meaning projection lost its sealed toolchain declaration before checked binding",
                ));
            };
            if operation != fact.operation {
                return Err(Diagnostic::error(
                    "validated float-meaning projection operation drifted before checked binding",
                ));
            }
            if contract != fact.contract {
                return Err(Diagnostic::error(
                    "validated float-meaning projection contract/catalog identity drifted before checked binding",
                ));
            }
            let [parameter] = program.operator_parameters(operator) else {
                return Err(Diagnostic::error(
                    "validated float-meaning projection signature no longer has one source parameter",
                ));
            };
            if !operator.symbol.is_valid()
                || operator.is_boundary
                || operator.spelling.is_some()
                || !operator.lifetime_parameters.is_empty()
                || !program.operator_type_parameters(operator).is_empty()
                || parameter.is_const
                || parameter.is_mutable
            {
                return Err(Diagnostic::error(
                    "validated float-meaning projection declaration shape drifted before checked binding",
                ));
            }
            let [source] = program.expression_table.expression_handles(call.arguments) else {
                return Err(Diagnostic::error(
                    "validated float-meaning projection call no longer has one source operand",
                ));
            };
            if *source != fact.source || replayed_primitive != fact.source_primitive {
                return Err(Diagnostic::error(
                    "validated float-meaning projection source/result shape drifted before checked binding",
                ));
            }
            Ok(contract)
        }

        type FloatMeaningProjectionKey = (
            ProjectionSource,
            FloatProjectionOperation,
            numerics::float_projection::FloatProjectionContractIdentity,
        );

        /*
            Canonicalize one checked float-meaning projection row: allocate the
            transitional input identity once per exact source key, then dedupe on
            (source, operation, contract) so a repeated invocation names the same
            canonical proof value.
        */
        fn push_float_meaning_projection(
            projections: &mut Vec<MeaningProjection>,
            projection_keys: &mut Vec<FloatMeaningProjectionKey>,
            transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
            source_key: CheckedFloatProjectionSourceKey,
            operation: FloatProjectionOperation,
            contract: numerics::float_projection::FloatProjectionContractIdentity,
            source_primitive: PrimitiveType,
        ) -> Result<CheckedProofValueId, Vec<Diagnostic>> {
            let source = match source_key {
                CheckedFloatProjectionSourceKey::Binary32Literal(bits) => {
                    ProjectionSource::ExactBinary32Literal(bits)
                }
                CheckedFloatProjectionSourceKey::Binary64Literal(bits) => {
                    ProjectionSource::ExactBinary64Literal(bits)
                }
                transitional => {
                    let source_index = match transitional_source_keys
                        .iter()
                        .position(|key| *key == transitional)
                    {
                        Some(index) => index,
                        None => {
                            transitional_source_keys.push(transitional.clone());
                            transitional_source_keys.len() - 1
                        }
                    };
                    let source_id = CheckedFloatProjectionInputId(
                        u32::try_from(source_index).map_err(|_| {
                            err!(
                                "float-meaning projection sources exceed their dense identity space"
                            )
                        })?,
                    );
                    let fallback = CheckedFloatProjectionInput {
                        id: source_id,
                        primitive: source_primitive,
                    };
                    match transitional {
                        CheckedFloatProjectionSourceKey::DirectMachineParameter {
                            owner_machine,
                            parameter,
                        } => ProjectionSource::DirectMachineParameter(
                            CheckedDirectMachineFloatParameter {
                                owner_machine,
                                parameter,
                                fallback,
                            },
                        ),
                        CheckedFloatProjectionSourceKey::DirectMachineResult { owner_machine } => {
                            ProjectionSource::DirectMachineResult(CheckedDirectMachineFloatResult {
                                owner_machine,
                                fallback,
                            })
                        }
                        CheckedFloatProjectionSourceKey::DirectBlockParameter {
                            owner_machine,
                            owner_state,
                            parameter,
                        } => ProjectionSource::DirectBlockParameter(
                            CheckedDirectBlockFloatParameter {
                                owner_machine,
                                owner_state,
                                parameter,
                                fallback,
                            },
                        ),
                        CheckedFloatProjectionSourceKey::DirectStructuralLeaf {
                            owner_machine,
                            field,
                        } => ProjectionSource::DirectStructuralLeaf(
                            CheckedDirectStructuralFloatLeaf {
                                owner_machine,
                                field,
                                fallback,
                            },
                        ),
                        CheckedFloatProjectionSourceKey::DirectCallResult { use_site } => {
                            ProjectionSource::DirectCallResult(CheckedDirectCallFloatResult {
                                use_site,
                                fallback,
                            })
                        }
                        CheckedFloatProjectionSourceKey::DirectOperationResult {
                            use_site,
                            use_expression,
                        } => ProjectionSource::DirectOperationResult(
                            CheckedDirectOperationFloatResult {
                                use_site,
                                use_expression,
                                fallback,
                            },
                        ),
                        CheckedFloatProjectionSourceKey::ResolvedSymbol(_)
                        | CheckedFloatProjectionSourceKey::TypedExpression(_)
                        | CheckedFloatProjectionSourceKey::SemanticApplication { .. } => {
                            ProjectionSource::TransitionalInput(fallback)
                        }
                        CheckedFloatProjectionSourceKey::Binary32Literal(_)
                        | CheckedFloatProjectionSourceKey::Binary64Literal(_) => {
                            unreachable!(
                                "exact literals were handled before transitional allocation"
                            )
                        }
                    }
                }
            };
            let projection_key = (source.clone(), operation, contract);
            let value_index = match projection_keys
                .iter()
                .position(|key| *key == projection_key)
            {
                Some(index) => index,
                None => {
                    projection_keys.push(projection_key);
                    let id = u32::try_from(projections.len()).map_err(|_| {
                        err!("float-meaning projection plan exceeds its dense identity space")
                    })?;
                    let projection = MeaningProjection {
                        result: CheckedProofValueDeclaration {
                            id: CheckedProofValueId(id),
                            value_type: CheckedProofOnlyValueType::FloatMeaning,
                        },
                        source,
                        operation,
                        contract,
                    };
                    projection.validate().map_err(|_| {
                        err!("checked float-meaning projection failed exact format replay")
                    })?;
                    projections.push(projection);
                    projections.len() - 1
                }
            };
            Ok(CheckedProofValueId(u32::try_from(value_index).map_err(
                |_| err!("float-meaning projection plan exceeds its dense identity space"),
            )?))
        }

        /*
            The source key one imported `ensures` operand takes at a call use site:
            each callee parameter names the authored argument expression supplied at
            the site, and the `result` operand names the scalar the call itself
            produces there.
        */
        fn imported_call_operand_key(
            program: &dyn ::validation::CheckingInput,
            invocation: &ProjectionInvocation,
            use_site: CheckedFloatUseSite,
            target_parameters: Option<&[typed_trees::signature::StateParameter]>,
            argument_expressions: Option<&[typed_trees::expression::ExpressionHandle]>,
        ) -> CheckedFloatProjectionSourceKey {
            let Some(path) = program.expression_table.name_path_node(invocation.source) else {
                return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
            };
            let [name] = program.expression_table.name_path_members(path.members) else {
                return CheckedFloatProjectionSourceKey::TypedExpression(invocation.source);
            };
            if path.symbol.is_valid()
                && let Some(parameters) = target_parameters
                && let Some(position) =
                    typed_trees::signature::state_parameter_position(parameters, path.symbol)
                && let Some(arguments) = argument_expressions
                && let Some(argument) = arguments.get(position)
            {
                return CheckedFloatProjectionSourceKey::TypedExpression(*argument);
            }
            let parameter_named_result = target_parameters.is_some_and(|parameters| {
                !language_core::signature_reserves_result(
                    parameters
                        .iter()
                        .map(|parameter| program.symbols.name(parameter.symbol)),
                )
            });
            if name.is_reserved_result() && !parameter_named_result {
                return CheckedFloatProjectionSourceKey::DirectCallResult { use_site };
            }
            CheckedFloatProjectionSourceKey::TypedExpression(invocation.source)
        }

        /*
            The source key one imported `ensures` operand takes at a boundary
            operation use: the `result` operand names the scalar the selected
            operation produces there.
        */
        fn imported_operation_operand_key(
            program: &dyn ::validation::CheckingInput,
            invocation: &ProjectionInvocation,
            use_site: CheckedFloatUseSite,
            use_expression: typed_trees::expression::ExpressionHandle,
        ) -> CheckedFloatProjectionSourceKey {
            if expressions(program).names_reserved_result(invocation.source) {
                return CheckedFloatProjectionSourceKey::DirectOperationResult {
                    use_site,
                    use_expression,
                };
            }
            CheckedFloatProjectionSourceKey::TypedExpression(invocation.source)
        }

        /*
            Instantiate one transported `ensures` expression at one use site: for
            every validated float-meaning equality the expression authored, emit the
            per-site equality whose operands re-bind through `operand_key`. Imported
            rows keep the declaration's `source_expression`; the use-site coordinate
            keeps the reflexivity rejoin's (owner, expression, use site) key disjoint
            from declaration rows.
        */
        fn instantiate_equalities_at_use(
            facts: &[ProjectionInvocation],
            equality_facts: &[ValidatedFloatMeaningEqualityProposition],
            expression: typed_trees::expression::ExpressionHandle,
            use_site: CheckedFloatUseSite,
            operand_key: &dyn Fn(&ProjectionInvocation) -> CheckedFloatProjectionSourceKey,
            projections: &mut Vec<MeaningProjection>,
            projection_keys: &mut Vec<FloatMeaningProjectionKey>,
            transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
            invocation_contracts: &[(
                typed_trees::expression::ExpressionHandle,
                numerics::float_projection::FloatProjectionContractIdentity,
            )],
            equalities: &mut Vec<EqualityProposition>,
        ) -> Result<(), Vec<Diagnostic>> {
            for equality_fact in equality_facts
                .iter()
                .copied()
                .filter(|fact| fact.expression == expression)
            {
                let mut operands = Vec::with_capacity(2);
                for invocation_handle in [equality_fact.left, equality_fact.right] {
                    let invocation = facts
                        .iter()
                        .find(|invocation| invocation.invocation == invocation_handle)
                        .ok_or_else(|| {
                            err!("validated float-meaning equality lost its projection contract")
                        })?;
                    let contract = invocation_contracts
                        .iter()
                        .find(|(invocation, _)| *invocation == invocation_handle)
                        .map(|(_, contract)| *contract)
                        .ok_or_else(|| {
                            err!("validated float-meaning equality lost its projection contract")
                        })?;
                    let key = operand_key(invocation);
                    operands.push(push_float_meaning_projection(
                        projections,
                        projection_keys,
                        transitional_source_keys,
                        key,
                        invocation.operation,
                        contract,
                        invocation.source_primitive,
                    )?);
                }
                let [left, right] = operands.as_slice() else {
                    unreachable!("float-meaning equality has exactly two operands")
                };
                let id = u32::try_from(equalities.len()).map_err(|_| {
                    err!("float-meaning equality plan exceeds its dense identity space")
                })?;
                equalities.push(EqualityProposition {
                    id: PropositionId(id),
                    left: CheckedProofValueId(left.0.min(right.0)),
                    right: CheckedProofValueId(left.0.max(right.0)),
                    source_expression: equality_fact.expression,
                    use_site: Some(use_site),
                });
            }
            Ok(())
        }

        /*
            A transported `ensures` clause instantiates per use site: every imported
            equality re-binds `result` to the scalar produced by that exact call or
            boundary operation use and each callee parameter to the authored argument
            expression supplied there. Imported instances share the declaration's
            checked contract expression; the use-site coordinate joins each
            instantiation to the exact producer the verifier rejoins.
        */
        fn instantiate_transported_ensures(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            facts: &[ProjectionInvocation],
            equality_facts: &[ValidatedFloatMeaningEqualityProposition],
            projections: &mut Vec<MeaningProjection>,
            projection_keys: &mut Vec<FloatMeaningProjectionKey>,
            transitional_source_keys: &mut Vec<CheckedFloatProjectionSourceKey>,
            invocation_contracts: &[(
                typed_trees::expression::ExpressionHandle,
                numerics::float_projection::FloatProjectionContractIdentity,
            )],
            equalities: &mut Vec<EqualityProposition>,
            applications: &mut Vec<CheckedFloatSemanticApplication>,
        ) -> Result<(), Vec<Diagnostic>> {
            let mut instantiate_ensures_at_use =
                |ensures: arena::HandleSpan<ContractProofFactRef>,
                 use_site: CheckedFloatUseSite,
                 operand_key: &dyn Fn(&ProjectionInvocation) -> CheckedFloatProjectionSourceKey|
                 -> Result<(), Vec<Diagnostic>> {
                    for fact_ref in proof.contract_fact_refs.span_or_empty(ensures) {
                        let contract_fact = proof.contract_facts.get(fact_ref.fact);
                        if contract_fact.kind != ContractProofFactKind::Ensures {
                            continue;
                        }
                        let Some(expression) =
                            program.proof_facts.get(contract_fact.fact).as_expression()
                        else {
                            continue;
                        };
                        let expression = *expression;
                        instantiate_equalities_at_use(
                            facts,
                            equality_facts,
                            expression,
                            use_site,
                            operand_key,
                            projections,
                            projection_keys,
                            transitional_source_keys,
                            invocation_contracts,
                            equalities,
                        )?;
                        let mut application_values = Vec::new();
                        let mut application_tables = SemanticApplicationTables {
                            projections,
                            projection_keys,
                            transitional_source_keys,
                            applications,
                            application_values: &mut application_values,
                            equalities,
                        };
                        bind_semantic_applications_in_expression(
                            program,
                            expression,
                            &SemanticOperandSite::UseSite {
                                facts,
                                invocation_contracts,
                                operand_key,
                            },
                            Some(use_site),
                            &mut application_tables,
                            &mut Vec::new(),
                        )?;
                    }
                    Ok(())
                };
            for (_, call) in proof.contract_calls.iter() {
                let use_site = CheckedFloatUseSite {
                    owner_machine: call.caller_machine_symbol,
                    owner_state: call.caller_state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                };
                let argument_expressions = validation::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .map(|site| validation::call_site_argument_expressions(program, &site));
                let target_parameters =
                    validation::call_target_parameters(program, call.target_state_symbol);
                let operand_key = |invocation: &ProjectionInvocation| {
                    imported_call_operand_key(
                        program,
                        invocation,
                        use_site,
                        target_parameters,
                        argument_expressions,
                    )
                };
                instantiate_ensures_at_use(call.ensures, use_site, &operand_key)?;
            }
            for (_, operator_use) in proof.contract_operator_uses.iter() {
                let checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: owner_machine,
                    state_symbol: owner_state,
                    statement_index,
                    ..
                } = operator_use.origin
                else {
                    continue;
                };
                let use_site = CheckedFloatUseSite {
                    owner_machine,
                    owner_state,
                    statement_index,
                    call_ordinal: 0,
                };
                let use_expression = operator_use.expression;
                let operand_key = |invocation: &ProjectionInvocation| {
                    imported_operation_operand_key(program, invocation, use_site, use_expression)
                };
                instantiate_ensures_at_use(operator_use.ensures, use_site, &operand_key)?;
            }
            Ok(())
        }

        fn ieee_format_primitive(format: IeeeFloatFormat) -> PrimitiveType {
            match format {
                IeeeFloatFormat::Binary32 => PrimitiveType::F32,
                IeeeFloatFormat::Binary64 => PrimitiveType::F64,
            }
        }

        fn ieee_format_operation(format: IeeeFloatFormat) -> FloatProjectionOperation {
            match format {
                IeeeFloatFormat::Binary32 => FloatProjectionOperation::Meaning32,
                IeeeFloatFormat::Binary64 => FloatProjectionOperation::Meaning64,
            }
        }

        /*
            How one `Meaning`-typed application argument reaches its checked proof
            value. A declaration position names the authored invocation's canonical
            value; a transported-ensures use site re-binds each operand invocation
            through the site's operand key so the per-site application cites the
            exact argument the use produced.
        */
        enum SemanticOperandSite<'a> {
            Declaration(
                &'a [(
                    typed_trees::expression::ExpressionHandle,
                    CheckedProofValueId,
                )],
            ),
            UseSite {
                facts: &'a [ProjectionInvocation],
                invocation_contracts: &'a [(
                    typed_trees::expression::ExpressionHandle,
                    numerics::float_projection::FloatProjectionContractIdentity,
                )],
                operand_key: &'a dyn Fn(&ProjectionInvocation) -> CheckedFloatProjectionSourceKey,
            },
        }

        /*
            The tables semantic-application binding writes through, shared by the
            declaration pass and every transported-ensures instantiation so each
            application result joins the one canonical projection space.
            `application_values` maps an authored application expression to its
            bound value within one binding context — reset per use site because the
            shared declaration text produces site-specific operand values there.
        */
        struct SemanticApplicationTables<'a> {
            projections: &'a mut Vec<MeaningProjection>,
            projection_keys: &'a mut Vec<FloatMeaningProjectionKey>,
            transitional_source_keys: &'a mut Vec<CheckedFloatProjectionSourceKey>,
            applications: &'a mut Vec<CheckedFloatSemanticApplication>,
            application_values: &'a mut Vec<(
                typed_trees::expression::ExpressionHandle,
                CheckedProofValueId,
            )>,
            equalities: &'a mut Vec<EqualityProposition>,
        }

        /*
            Resolve one `Meaning`-typed application argument to its bound proof value
            and format. Nested applications already visited post-order rejoin through
            `application_values`; an argument that is neither a bound invocation nor
            a bound application binds nothing.
        */
        fn semantic_meaning_operand(
            argument: ExpressionHandle,
            site: &SemanticOperandSite<'_>,
            tables: &mut SemanticApplicationTables<'_>,
        ) -> Result<Option<(CheckedProofValueId, IeeeFloatFormat)>, Vec<Diagnostic>> {
            let operand_format = |value: CheckedProofValueId,
                                  tables: &SemanticApplicationTables<'_>|
             -> IeeeFloatFormat {
                let projection =
                    &tables.projections[usize::try_from(value.0).unwrap_or(usize::MAX)];
                match projection.operation {
                    FloatProjectionOperation::Meaning32 => IeeeFloatFormat::Binary32,
                    FloatProjectionOperation::Meaning64 => IeeeFloatFormat::Binary64,
                }
            };
            if let Some((_, value)) = tables
                .application_values
                .iter()
                .find(|(expression, _)| *expression == argument)
            {
                return Ok(Some((*value, operand_format(*value, tables))));
            }
            match site {
                SemanticOperandSite::Declaration(invocation_values) => Ok(invocation_values
                    .iter()
                    .find(|(invocation, _)| *invocation == argument)
                    .map(|(_, value)| (*value, operand_format(*value, tables)))),
                SemanticOperandSite::UseSite {
                    facts,
                    invocation_contracts,
                    operand_key,
                } => {
                    let Some(invocation) = facts
                        .iter()
                        .find(|invocation| invocation.invocation == argument)
                    else {
                        return Ok(None);
                    };
                    let Some(contract) = invocation_contracts
                        .iter()
                        .find(|(handle, _)| *handle == argument)
                        .map(|(_, contract)| *contract)
                    else {
                        return Ok(None);
                    };
                    let value = push_float_meaning_projection(
                        tables.projections,
                        tables.projection_keys,
                        tables.transitional_source_keys,
                        operand_key(invocation),
                        invocation.operation,
                        contract,
                        invocation.source_primitive,
                    )?;
                    Ok(Some((value, operand_format(value, tables))))
                }
            }
        }

        /*
            Bind one expression-position `FloatSemantics::<name>(...)` call to its
            checked semantic-application row. Only the sealed toolchain declaration
            binds, only `Meaning` results occupy the proof-value carrier, and every
            operand must spell its catalog kind — a `Format` parameter takes the
            sealed `FloatFormat::BINARY*` const, a `Meaning` parameter an already
            bound invocation or nested application; unrepresentable operands bind no
            row rather than approximating.
        */
        fn bind_semantic_application_call(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            site: &SemanticOperandSite<'_>,
            tables: &mut SemanticApplicationTables<'_>,
        ) -> Result<Option<CheckedProofValueId>, Vec<Diagnostic>> {
            let Some(operator) = resolve_named_expression_call(program, call) else {
                return Ok(None);
            };
            let Some((row, contract)) = exact_toolchain_float_semantic_contract(program, operator)
            else {
                return Ok(None);
            };
            if row.result != ValueKind::Meaning {
                return Ok(None);
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != row.parameters.len() {
                return Err(vec![Diagnostic::error(
                    "validated float-semantic application arity drifted before checked binding",
                )]);
            }
            let mut operands = Vec::with_capacity(arguments.len());
            let mut declared_format = None;
            let mut operand_format = None;
            for (kind, argument) in row.parameters.iter().zip(arguments.iter()) {
                match kind {
                    ValueKind::Format => {
                        let Some(format) = exact_toolchain_float_format_const(program, *argument)
                        else {
                            return Ok(None);
                        };
                        declared_format = Some(format);
                        operands.push(ApplicationOperand::Format(format));
                    }
                    ValueKind::Meaning => {
                        let Some((value, format)) =
                            semantic_meaning_operand(*argument, site, tables)?
                        else {
                            return Ok(None);
                        };
                        operand_format = match operand_format {
                            None => Some(format),
                            Some(existing) if existing == format => Some(existing),
                            Some(existing) => {
                                return Err(vec![Diagnostic::error(format!(
                                    "float-semantic application meaning operands do not share one exact format ({existing:?} vs {format:?})"
                                ))]);
                            }
                        };
                        operands.push(ApplicationOperand::Meaning(value));
                    }
                    ValueKind::Bool | ValueKind::Class | ValueKind::Integer(_) => return Ok(None),
                }
            }
            let Some(format) = declared_format.or(operand_format) else {
                return Ok(None);
            };
            let operation = ieee_format_operation(format);
            let value = push_float_meaning_projection(
                tables.projections,
                tables.projection_keys,
                tables.transitional_source_keys,
                CheckedFloatProjectionSourceKey::SemanticApplication {
                    contract,
                    format,
                    operands: operands.clone(),
                },
                operation,
                operation.contract_identity(),
                ieee_format_primitive(format),
            )?;
            if !tables
                .applications
                .iter()
                .any(|application| application.result == value)
            {
                let application = CheckedFloatSemanticApplication {
                    result: value,
                    contract,
                    format,
                    operands,
                };
                application.validate().map_err(|_| {
                    err!("checked float-semantic application failed exact catalog replay")
                })?;
                tables.applications.push(application);
            }
            tables.application_values.push((expression, value));
            Ok(Some(value))
        }

        /*
            Emit the proof-only equality a `==` proposition between bound meaning
            values authors. This covers only pairs the validated equality facts do
            not already carry — a side bound by a semantic application — so a
            two-invocation equality never duplicates here.
        */
        fn bind_semantic_application_equality(
            expression: ExpressionHandle,
            binary: &typed_trees::expression::TableBinaryExpression,
            site: &SemanticOperandSite<'_>,
            use_site: Option<CheckedFloatUseSite>,
            tables: &mut SemanticApplicationTables<'_>,
        ) -> Result<(), Vec<Diagnostic>> {
            let is_application =
                |expression: ExpressionHandle, tables: &SemanticApplicationTables<'_>| {
                    tables
                        .application_values
                        .iter()
                        .any(|(bound, _)| *bound == expression)
                };
            if !is_application(binary.left, tables) && !is_application(binary.right, tables) {
                return Ok(());
            }
            let mut operands = Vec::with_capacity(2);
            for operand in [binary.left, binary.right] {
                let Some((value, _)) = semantic_meaning_operand(operand, site, tables)? else {
                    return Ok(());
                };
                operands.push(value);
            }
            let [left, right] = operands.as_slice() else {
                unreachable!("float-semantic equality has exactly two operands")
            };
            let left_row = &tables.projections[usize::try_from(left.0).unwrap_or(usize::MAX)];
            let right_row = &tables.projections[usize::try_from(right.0).unwrap_or(usize::MAX)];
            if left_row.operation != right_row.operation || left_row.contract != right_row.contract
            {
                return Err(vec![Diagnostic::error(
                    "checked float-semantic equality operands do not share one exact format and projection contract",
                )]);
            }
            let id = u32::try_from(tables.equalities.len()).map_err(|_| {
                err!("float-meaning equality plan exceeds its dense identity space")
            })?;
            tables.equalities.push(EqualityProposition {
                id: PropositionId(id),
                left: CheckedProofValueId(left.0.min(right.0)),
                right: CheckedProofValueId(left.0.max(right.0)),
                source_expression: expression,
                use_site,
            });
            Ok(())
        }

        /*
            Post-order walk binding the semantic applications inside one contract
            expression: inner applications bind before the outer rows referencing
            them, each `FloatSemantics::*` call gains a checked application row, and
            each `==` proposition with an application operand emits its equality at
            the given use-site coordinate (`None` at the declaration).
        */
        fn bind_semantic_applications_in_expression(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            site: &SemanticOperandSite<'_>,
            use_site: Option<CheckedFloatUseSite>,
            tables: &mut SemanticApplicationTables<'_>,
            visited: &mut Vec<ExpressionHandle>,
        ) -> Result<Option<CheckedProofValueId>, Vec<Diagnostic>> {
            if !expression.is_valid() {
                return Ok(None);
            }
            if let Some((_, value)) = tables
                .application_values
                .iter()
                .find(|(bound, _)| *bound == expression)
            {
                return Ok(Some(*value));
            }
            if visited.contains(&expression) {
                return Ok(None);
            }
            visited.push(expression);
            for child in expression_children(program, expression) {
                bind_semantic_applications_in_expression(
                    program, child, site, use_site, tables, visited,
                )?;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    bind_semantic_application_call(program, expression, call, site, tables)
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                    bind_semantic_application_equality(expression, binary, site, use_site, tables)?;
                    Ok(None)
                }
                _ => Ok(None),
            }
        }

        /*
            Bind every semantic application authored in the program's checked contract
            expressions and each `==` equality that references one — the
            declaration-level rows a transported `ensures` shares before it
            instantiates at a use site.
        */
        fn bind_declaration_semantic_applications(
            program: &dyn ::validation::CheckingInput,
            invocation_values: &[(
                typed_trees::expression::ExpressionHandle,
                CheckedProofValueId,
            )],
            tables: &mut SemanticApplicationTables<'_>,
        ) -> Result<(), Vec<Diagnostic>> {
            let site = SemanticOperandSite::Declaration(invocation_values);
            let mut visited = Vec::new();
            for (_, fact) in typed_trees::references::program_proof_facts(program) {
                let roots: &[ExpressionHandle] = match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        std::slice::from_ref(expression)
                    }
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        std::slice::from_ref(&membership.value)
                    }
                    typed_trees::domain::ProofFact::Proposition(application) => {
                        expressions(program).expression_handles(application.arguments)
                    }
                };
                for root in roots {
                    bind_semantic_applications_in_expression(
                        program,
                        *root,
                        &site,
                        None,
                        tables,
                        &mut visited,
                    )?;
                }
            }
            Ok(())
        }

        pub(crate) fn bind_float_meaning_projection_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &mut ProofFacts,
            facts: &[ProjectionInvocation],
            equality_facts: &[ValidatedFloatMeaningEqualityProposition],
        ) -> Result<(), Vec<Diagnostic>> {
            let mut projections = Vec::with_capacity(facts.len());
            let mut transitional_source_keys = Vec::<CheckedFloatProjectionSourceKey>::new();
            let mut projection_keys = Vec::<FloatMeaningProjectionKey>::new();
            let mut invocation_values = Vec::with_capacity(facts.len());
            let mut invocation_contracts = Vec::with_capacity(facts.len());
            let mut occurrences = Vec::with_capacity(facts.len());
            let carriers = ContractCarriers::new(program, proof, facts);
            for (index, fact) in facts.iter().copied().enumerate() {
                let contract =
                    replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
                let source_key = projection_source_key(program, &carriers, fact);
                let value = push_float_meaning_projection(
                    &mut projections,
                    &mut projection_keys,
                    &mut transitional_source_keys,
                    source_key,
                    fact.operation,
                    contract,
                    fact.source_primitive,
                )?;
                invocation_values.push((fact.invocation, value));
                invocation_contracts.push((fact.invocation, contract));
                occurrences.push(CheckedFloatMeaningProjectionOccurrence {
                    id: CheckedFloatMeaningProjectionOccurrenceId(u32::try_from(index).map_err(|_| {
                        err!("float-meaning projection occurrences exceed their dense identity space")
                    })?),
                    value,
                    source_span: program.expression_table.source_span(fact.invocation),
                });
            }
            if projection_keys.len() != projections.len() {
                return Err(err!(
                    "checked float-meaning projection canonicalization lost a semantic key"
                ));
            }
            let mut equalities = Vec::with_capacity(equality_facts.len());
            for (index, fact) in equality_facts.iter().copied().enumerate() {
                let Some(expression) = program.expression_table.binary_node(fact.expression) else {
                    return Err(err!(
                        "validated float-meaning equality is no longer a binary proposition"
                    ));
                };
                if expression.operator != BinaryOperator::Equal
                    || expression.left != fact.left
                    || expression.right != fact.right
                {
                    return Err(vec![Diagnostic::error(
                        "validated float-meaning equality identity drifted before checked binding",
                    )]);
                }
                let left_projection = facts
                    .iter()
                    .find(|projection| projection.invocation == fact.left)
                    .ok_or_else(|| {
                        err!("validated float-meaning equality lost its left projection contract")
                    })?;
                let right_projection = facts
                    .iter()
                    .find(|projection| projection.invocation == fact.right)
                    .ok_or_else(|| {
                        err!("validated float-meaning equality lost its right projection contract")
                    })?;
                if left_projection.source_primitive != right_projection.source_primitive
                    || left_projection.operation != right_projection.operation
                    || left_projection.contract != right_projection.contract
                {
                    return Err(vec![Diagnostic::error(
                        "validated FloatMeaningEqual operands do not share one exact format and projection contract",
                    )]);
                }
                let left = invocation_values
                    .iter()
                    .find(|(invocation, _)| *invocation == fact.left)
                    .map(|(_, value)| value.0)
                    .ok_or_else(|| {
                        err!("validated float-meaning equality lost its left projection")
                    })?;
                let right = invocation_values
                    .iter()
                    .find(|(invocation, _)| *invocation == fact.right)
                    .map(|(_, value)| value.0)
                    .ok_or_else(|| {
                        err!("validated float-meaning equality lost its right projection")
                    })?;
                let id = u32::try_from(index).map_err(|_| {
                    err!("float-meaning equality plan exceeds its dense identity space")
                })?;
                equalities.push(EqualityProposition {
                    id: PropositionId(id),
                    left: CheckedProofValueId(left.min(right)),
                    right: CheckedProofValueId(left.max(right)),
                    source_expression: fact.expression,
                    use_site: None,
                });
            }
            let mut applications = Vec::new();
            {
                let mut application_values = Vec::new();
                let mut application_tables = SemanticApplicationTables {
                    projections: &mut projections,
                    projection_keys: &mut projection_keys,
                    transitional_source_keys: &mut transitional_source_keys,
                    applications: &mut applications,
                    application_values: &mut application_values,
                    equalities: &mut equalities,
                };
                bind_declaration_semantic_applications(
                    program,
                    &invocation_values,
                    &mut application_tables,
                )?;
            }
            instantiate_transported_ensures(
                program,
                proof,
                facts,
                equality_facts,
                &mut projections,
                &mut projection_keys,
                &mut transitional_source_keys,
                &invocation_contracts,
                &mut equalities,
                &mut applications,
            )?;
            proof.float_meaning_projections = projections;
            proof.float_meaning_projection_occurrences = occurrences;
            proof.float_meaning_equalities = equalities;
            proof.float_semantic_applications = applications;
            Ok(())
        }
    }
    mod obligations {
        use checked_trees::{ProofFactKind, ProofObligationFact, ProofObligationOwner};
        use symbols::SymbolHandle;

        pub(crate) fn lower_proof_obligation(
            obligation: &proof::obligations::ProofObligation,
        ) -> ProofObligationFact {
            match obligation {
                proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedAssignment,
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                    }
                }
                proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedCallArgument,
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        owner: ProofObligationOwner::CallParameter {
                            machine_symbol: obligation.machine_symbol,
                            state_symbol: obligation.state_symbol,
                            target_symbol: obligation.target_symbol,
                            parameter_symbol: obligation.parameter_symbol,
                        },
                    }
                }
                proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
                    let (machine_symbol, state_symbol) = match &obligation.owner {
                        proof::obligations::ProofObligationOwner::StateLocal {
                            machine_symbol,
                            state_symbol,
                            ..
                        }
                        | proof::obligations::ProofObligationOwner::ConstructorField {
                            machine_symbol,
                            state_symbol,
                            ..
                        }
                        | proof::obligations::ProofObligationOwner::ArrayElement {
                            machine_symbol,
                            state_symbol,
                            ..
                        } => (*machine_symbol, *state_symbol),
                        _ => (
                            symbols::SymbolHandle::invalid(),
                            symbols::SymbolHandle::invalid(),
                        ),
                    };
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedInitializer,
                        machine_symbol,
                        state_symbol,
                        owner: proof_obligation_owner(&obligation.owner),
                    }
                }
                proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedStateReturn,
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        owner: ProofObligationOwner::StateReturn {
                            machine_symbol: obligation.machine_symbol,
                            state_symbol: obligation.state_symbol,
                        },
                    }
                }
                proof::obligations::ProofObligation::BoundedValue(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedValue,
                        machine_symbol: symbols::SymbolHandle::invalid(),
                        state_symbol: symbols::SymbolHandle::invalid(),
                        owner: proof_obligation_owner(&obligation.owner),
                    }
                }
                proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::BoundedTransitionArgument,
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        owner: ProofObligationOwner::TransitionParameter {
                            machine_symbol: obligation.machine_symbol,
                            state_symbol: obligation.state_symbol,
                            parameter_symbol: obligation.parameter_symbol,
                        },
                    }
                }
                proof::obligations::ProofObligation::GuardedTransition(obligation) => {
                    ProofObligationFact {
                        kind: ProofFactKind::GuardedTransition,
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                    }
                }
            }
        }

        pub(crate) fn state_owner(
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> ProofObligationOwner {
            ProofObligationOwner::MachineState {
                machine_symbol,
                state_symbol,
            }
        }

        pub(crate) fn proof_obligation_owner(
            owner: &proof::obligations::ProofObligationOwner,
        ) -> ProofObligationOwner {
            match owner {
                proof::obligations::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
                proof::obligations::ProofObligationOwner::MachineOwnedData {
                    machine_symbol,
                    machine: _,
                    data_symbol,
                    data: _,
                } => ProofObligationOwner::MachineOwnedData {
                    machine_symbol: *machine_symbol,
                    data_symbol: *data_symbol,
                },
                proof::obligations::ProofObligationOwner::StateParameter {
                    machine_symbol,
                    machine: _,
                    state_symbol,
                    state: _,
                    parameter_symbol,
                    parameter: _,
                } => ProofObligationOwner::StateParameter {
                    machine_symbol: *machine_symbol,
                    state_symbol: *state_symbol,
                    parameter_symbol: *parameter_symbol,
                },
                proof::obligations::ProofObligationOwner::StateReturn {
                    machine_symbol,
                    machine: _,
                    state_symbol,
                    state: _,
                } => ProofObligationOwner::StateReturn {
                    machine_symbol: *machine_symbol,
                    state_symbol: *state_symbol,
                },
                proof::obligations::ProofObligationOwner::StateLocal {
                    machine_symbol,
                    state_symbol,
                    ..
                }
                | proof::obligations::ProofObligationOwner::ConstructorField {
                    machine_symbol,
                    state_symbol,
                    ..
                }
                | proof::obligations::ProofObligationOwner::ArrayElement {
                    machine_symbol,
                    state_symbol,
                    ..
                } => state_owner(*machine_symbol, *state_symbol),
            }
        }
    }
    mod outcome_arms {
        /*
            Outcome-specific arm facts and the exact outcome case tests.
        */
        use crate::flow::expressions;
        use crate::flow::state_statements;
        use crate::published::facts::contract_occurrences::fact_referenced_occurrences;
        use crate::published::proof::proposition_vocabulary::{
            lower_checked_proposition_application, proposition_application_label,
            proposition_label_parts,
        };
        use checked_trees::{CheckedEvidenceTerm, ContractProofFactOwner, ProofFacts};
        use symbols::SymbolHandle;
        use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

        /*
            Bind outcome-specific producer guarantees to the one transition arm that
            tests the saved result of a direct call, including immutable binding copies.
            Mutable bindings and non-binding value transformations remain fail-closed.
        */
        pub(crate) fn bind_outcome_specific_arm_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &mut ProofFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            use typed_trees::statement::TransitionGuardNode;

            let mut diagnostics = Vec::new();
            let mut arms = arena::Arena::default();

            for caller_machine in program.machines() {
                for caller_state in program.machine_states(caller_machine) {
                    let statements = state_statements(program, caller_state.statement_nodes);
                    for (statement_index, statement) in statements.iter().enumerate() {
                        let Some(transition) = statement.as_transition() else {
                            continue;
                        };
                        let selectors = program
                            .statement_table
                            .outcome_proof_selectors(transition.proof_selectors);
                        let arm_span = transition.source_span;
                        let located = |diagnostic: diagnostics::Diagnostic| {
                            if arm_span == source::SourceSpan::default() {
                                diagnostic
                            } else {
                                diagnostic.with_source_span(arm_span)
                            }
                        };
                        let TransitionGuardNode::When(guard) = transition.guard else {
                            if !selectors.is_empty() {
                                diagnostics.push(located(diagnostics::Diagnostic::error(
                                    "outcome proof selectors require an exact guarded result-case arm",
                                )));
                            }
                            continue;
                        };
                        let Some((result_expression, result_case)) =
                            exact_outcome_case_test(program, guard)
                        else {
                            if !selectors.is_empty() {
                                diagnostics.push(located(diagnostics::Diagnostic::error(
                                    "outcome proof selectors require one exact nominal result-case test",
                                )));
                            }
                            continue;
                        };
                        let Some((
                            result_call_statement_index,
                            result_call_expression,
                            result_call,
                        )) = saved_result_call(
                            program,
                            caller_state,
                            statement_index,
                            result_expression,
                        )
                        else {
                            if !selectors.is_empty() {
                                diagnostics.push(located(diagnostics::Diagnostic::error(
                                    "outcome proof selectors require one immutable saved direct-call result",
                                )));
                            }
                            continue;
                        };
                        let Some((target_machine, target_state)) =
                            validation::find_state(program, result_call.target_symbol)
                        else {
                            if !selectors.is_empty() {
                                diagnostics.push(located(diagnostics::Diagnostic::error(format!(
                                    "outcome proof selector source call `{}` must target a concrete machine state",
                                    result_call.target
                                ))));
                            }
                            continue;
                        };
                        let Some(result_data) = program
                            .variant_owner(result_case)
                            .map(|(definition, _)| definition.symbol)
                        else {
                            if !selectors.is_empty() {
                                diagnostics.push(located(diagnostics::Diagnostic::error(
                                    "outcome proof selector case does not resolve to a declared nominal sum case",
                                )));
                            }
                            continue;
                        };

                        let matching_rows = proof
                            .outcome_specific_guarantees
                            .iter()
                            .filter_map(|(handle, row)| {
                                (row.machine_symbol == target_machine.symbol
                                    && row.result_data == result_data
                                    && row.result_case == result_case)
                                    .then_some((handle, row.clone()))
                            })
                            .collect::<Vec<_>>();
                        if matching_rows.is_empty() && selectors.is_empty() {
                            continue;
                        }
                        let mut selected_names = std::collections::BTreeSet::new();
                        let mut invalid = false;
                        for selector in selectors {
                            if !selected_names.insert(selector.binding.as_str()) {
                                diagnostics.push(located(diagnostics::Diagnostic::error(format!(
                                    "outcome evidence term `{}` is bound more than once in this arm",
                                    selector.binding
                                ))));
                                invalid = true;
                            }
                            if !matching_rows.iter().any(|(_, row)| {
                                row.public_selector.as_deref()
                                    == Some(selector.output_field.as_str())
                            }) {
                                diagnostics.push(located(diagnostics::Diagnostic::error(format!(
                                    "outcome proof selector `{}` is not a named guarantee of the matching result case",
                                    selector.output_field
                                ))));
                                invalid = true;
                            }
                        }
                        if invalid {
                            continue;
                        }

                        let substitutions = outcome_call_substitutions(
                            program,
                            caller_state.symbol,
                            result_expression,
                            result_call,
                            target_state,
                        );
                        let mut arm_rows = Vec::with_capacity(matching_rows.len());
                        for (guarantee, row) in matching_rows {
                            let (
                                instantiated_proposition,
                                instantiated_label,
                                captured_argument_identities,
                            ) = instantiate_outcome_arm_fact(
                                program,
                                row.fact,
                                &substitutions,
                                caller_state.symbol,
                                result_call_statement_index,
                                validation::CallSite::Expression {
                                    expression: result_call_expression,
                                    call: result_call,
                                },
                                target_state,
                            );
                            let referenced_occurrences =
                                fact_referenced_occurrences(program, row.fact)
                                    .into_iter()
                                    .map(|occurrence| checked_trees::OutcomeReferencedOccurrence {
                                        occurrence,
                                        root: occurrence_root(program, target_state, occurrence),
                                    })
                                    .collect::<Vec<_>>();
                            let argument_roots = row.argument_roots.clone();
                            let validity = checked_trees::OutcomeSpecificValidityFact {
                                result_occurrence: result_expression,
                                evidence_interface_scope: instantiated_proposition
                                    .as_ref()
                                    .and_then(|proposition| {
                                        outcome_evidence_interface_scope(
                                            program,
                                            proposition,
                                            &referenced_occurrences,
                                        )
                                    }),
                                referenced_occurrences,
                            };
                            let selected = row.public_selector.as_deref().and_then(|public| {
                                selectors
                                    .iter()
                                    .find(|selector| selector.output_field.as_str() == public)
                            });
                            let selected_term = selected.and_then(|selector| {
                                let producer = row.evidence_term?;
                                let mut term = proof.evidence_terms.get(producer).clone();
                                let proposition = instantiated_proposition.clone()?;
                                term.name = selector.binding.as_str().to_owned();
                                term.owner = ContractProofFactOwner::MachineState {
                                    machine_symbol: caller_machine.symbol,
                                    state_symbol: caller_state.symbol,
                                };
                                term.lane_position = arm_rows.len();
                                term.proposition = proposition;
                                Some(proof.evidence_terms.append(term))
                            });
                            arm_rows.push(checked_trees::OutcomeSpecificArmRowFact {
                                guarantee,
                                instantiated_proposition,
                                instantiated_label,
                                captured_argument_identities,
                                argument_roots,
                                validity,
                                selected_term,
                            });
                        }
                        arms.append(checked_trees::OutcomeSpecificArmFact {
                            caller_machine_symbol: caller_machine.symbol,
                            caller_state_symbol: caller_state.symbol,
                            statement_index,
                            result_call_statement_index,
                            result_call_expression,
                            result_call_target: target_state.symbol,
                            result_data,
                            result_case,
                            result_expression,
                            rows: arm_rows,
                        });
                    }
                }
            }

            if diagnostics.is_empty() {
                proof.outcome_specific_arms = arms;
                Ok(())
            } else {
                Err(diagnostics)
            }
        }

        /*
            Follow exact immutable binding copies back to the completed call. Dispatch
            subject capture introduces the same kind of copy as an authored local; it
            does not change the event that established the outcome. Each step must be
            earlier in admitted execution order, so malformed cycles and later bindings
            fail without replaying an initializer or admitting a mutable snapshot.
        */
        fn saved_result_call<'program>(
            program: &'program dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            mut expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<(
            usize,
            typed_trees::expression::ExpressionHandle,
            &'program typed_trees::expression::TableCallExpression,
        )> {
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let mut before = order.position(statement_index)?;
            loop {
                let path = program.expression_table.name_path_node(expression)?;
                if !path.symbol.is_valid() || path.head_symbol != path.symbol {
                    return None;
                }
                let (ordinal, local) = program.state_local_declaration(state, path.symbol)?;
                let position = order.position(ordinal)?;
                if local.is_mutable || !local.initial_value.is_valid() || position >= before {
                    return None;
                }
                expression = local.initial_value;
                if let Some(call) = program.expression_table.call_node(expression) {
                    return Some((ordinal, expression, call));
                }
                before = position;
            }
        }

        fn outcome_evidence_interface_scope(
            program: &dyn ::validation::CheckingInput,
            proposition: &checked_trees::CheckedPropositionApplication,
            retained_occurrences: &[checked_trees::OutcomeReferencedOccurrence],
        ) -> Option<checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact> {
            let interface = proposition.evidence_interface.clone()?;
            let definition = program.proposition_definition_by_symbol(proposition.declaration)?;
            let typed_trees::proposition::PropositionBody::Witness { evidence } = definition.body
            else {
                return None;
            };
            let mut reference_regions = Vec::new();
            append_evidence_interface_reference_regions(program, evidence, &mut reference_regions);
            reference_regions.sort_by_key(|reference| reference.arena_index());
            reference_regions.dedup();
            Some(checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact {
                interface,
                evidence_type: evidence,
                reference_regions,
                retained_occurrences: retained_occurrences.to_vec(),
            })
        }

        fn append_evidence_interface_reference_regions(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
            regions: &mut Vec<typed_trees::types::TypeReferenceHandle>,
        ) {
            if !type_reference.is_valid() {
                return;
            }
            use typed_trees::types::TypeReferenceNode;
            match program.type_reference_table.type_reference(type_reference) {
                // A closed code descriptor holds no loans of its future call formals.
                TypeReferenceNode::Callable { .. } => {}
                TypeReferenceNode::Reference { referee, .. } => {
                    regions.push(type_reference);
                    append_evidence_interface_reference_regions(program, *referee, regions);
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    append_evidence_interface_reference_regions(program, *base_type, regions);
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    append_evidence_interface_reference_regions(program, *element_type, regions);
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    for argument in program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                    {
                        append_evidence_interface_reference_regions(program, *argument, regions);
                    }
                }
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Unit => {}
            }
        }

        pub(crate) fn exact_outcome_case_test(
            program: &dyn ::validation::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)> {
            use typed_trees::expression::{BinaryOperator, ExpressionNode};
            let binary = program.expression_table.binary_node(expression)?;
            if binary.operator == BinaryOperator::CaseMembership {
                let case = program.expression_table.name_path_node(binary.right)?;
                return Some((binary.left, case.symbol));
            }
            if binary.operator == BinaryOperator::Equal {
                let is_true = |candidate| {
                    matches!(
                        program.expression_table.expression(candidate),
                        ExpressionNode::Boolean(true)
                    )
                };
                if is_true(binary.left) {
                    return exact_outcome_case_test(program, binary.right);
                }
                if is_true(binary.right) {
                    return exact_outcome_case_test(program, binary.left);
                }
                let case = |candidate| {
                    match program.expression_table.expression(candidate) {
                    ExpressionNode::Name(path) if program.data_definitions().iter().any(|definition| {
                        program.data_members(definition).iter().any(|member| {
                            matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == path.symbol)
                        })
                    }) => Some(path.symbol),
                    ExpressionNode::StructLiteral(literal)
                        if literal.case_symbol.is_some()
                            && expressions(program).struct_fields(literal.fields)
                                .is_empty() =>
                    {
                        literal.case_symbol
                    }
                    _ => None,
                }
                };
                if let Some(case) = case(binary.right) {
                    return Some((binary.left, case));
                }
                if let Some(case) = case(binary.left) {
                    return Some((binary.right, case));
                }
            }
            None
        }

        pub(super) use validation::contract_argument_root as occurrence_root;

        fn outcome_call_substitutions(
            program: &dyn ::validation::CheckingInput,
            _caller_state_symbol: SymbolHandle,
            result_expression: typed_trees::expression::ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            target_state: &typed_trees::state::State,
        ) -> Vec<(SymbolHandle, String, String)> {
            let mut substitutions = Vec::new();
            let arguments = program.expression_table.expression_handles(call.arguments);
            for (argument_index, parameter) in
                program.state_parameters(target_state).iter().enumerate()
            {
                if let Some(value) = arguments.get(argument_index).copied() {
                    substitutions.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        program.render_proof_expression(value, ProofSubstitutions::None),
                    ));
                }
            }
            substitutions.push((
                SymbolHandle::invalid(),
                "result".to_owned(),
                program.render_proof_expression(result_expression, ProofSubstitutions::None),
            ));
            substitutions
        }

        fn instantiate_outcome_arm_fact(
            program: &dyn ::validation::CheckingInput,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
            substitutions: &[(SymbolHandle, String, String)],
            caller_state: SymbolHandle,
            statement_index: usize,
            call: validation::CallSite<'_>,
            target_state: &typed_trees::state::State,
        ) -> (
            Option<checked_trees::CheckedPropositionApplication>,
            Option<String>,
            Option<Vec<Option<String>>>,
        ) {
            let Some(application) = program.proof_facts.get(fact).as_proposition() else {
                let identity = match program.proof_facts.get(fact) {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        Some(program.render_proof_expression(
                            *expression,
                            ProofSubstitutions::ByParameter(substitutions),
                        ))
                    }
                    typed_trees::domain::ProofFact::Membership(_) => None,
                    typed_trees::domain::ProofFact::Proposition(_) => unreachable!(),
                };
                return (None, identity, Some(Vec::new()));
            };
            let parameters = program.state_parameters(target_state);
            let (binder_labels, argument_labels) =
                proposition_label_parts(program, application, substitutions);
            let captured_argument_identities = expressions(program)
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| {
                    if validation::owned_contract_subject(program, parameters, *argument).is_some()
                    {
                        validation::captured_call_subject_identity(
                            program,
                            caller_state,
                            statement_index,
                            call,
                            parameters,
                            *argument,
                        )
                        .map(Some)
                    } else if matches!(
                        occurrence_root(program, target_state, *argument),
                        checked_trees::OutcomeOccurrenceRoot::CalleeResult
                            | checked_trees::OutcomeOccurrenceRoot::ClosedValue
                    ) {
                        Some(None)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>();
            let proposition = program
                .normalize_nominal_proposition_application(
                    application,
                    Some(PropositionLabels {
                        binder_labels: &binder_labels,
                        argument_labels: &argument_labels,
                    }),
                )
                .map(lower_checked_proposition_application);
            let identity = captured_argument_identities.as_ref().and_then(|captures| {
                let labels = argument_labels
                    .iter()
                    .zip(captures)
                    .map(|(label, capture)| capture.clone().unwrap_or_else(|| label.clone()))
                    .collect::<Vec<_>>();
                program.normalized_identity_label(application, &binder_labels, &labels)
            });
            (proposition, identity, captured_argument_identities)
        }

        pub(crate) fn exact_result_case(
            program: &dyn ::validation::CheckingInput,
            result: typed_trees::expression::ExpressionHandle,
        ) -> Option<(SymbolHandle, SymbolHandle)> {
            use typed_trees::expression::ExpressionNode;
            let (data_symbol, case_symbol) = match program.expression_table.expression(result) {
                ExpressionNode::Name(path)
                    if path.head_symbol.is_valid() && path.symbol.is_valid() =>
                {
                    (path.head_symbol, path.symbol)
                }
                ExpressionNode::StructLiteral(literal) => {
                    (literal.type_symbol, literal.case_symbol?)
                }
                _ => return None,
            };
            let data = program.data_definition_by_symbol(data_symbol)?;
            program
                .data_members(data)
                .iter()
                .any(|member| {
                    matches!(
                        member,
                        typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == case_symbol
                    )
                })
                .then_some((data_symbol, case_symbol))
        }

        /// The normalized label of a proof fact with `substitutions` applied,
        /// in the vocabulary `known` uses: `boolean:` plus the rendered
        /// expression, or the normalized proposition identity. Membership
        /// facts have no label.
        pub(crate) fn fact_label(
            program: &dyn ::validation::CheckingInput,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
            substitutions: &[(SymbolHandle, String, String)],
        ) -> Option<String> {
            use typed_trees::domain::ProofFact;
            match program.proof_facts.get(fact) {
                ProofFact::Expression(expression) => Some(format!(
                    "boolean:{}",
                    program.render_proof_expression(
                        *expression,
                        ProofSubstitutions::ByParameter(substitutions)
                    )
                )),
                ProofFact::Proposition(application) => {
                    proposition_application_label(program, application, substitutions)
                }
                ProofFact::Membership(_) => None,
            }
        }

        pub(crate) fn outcome_specific_fact_is_proved(
            program: &dyn ::validation::CheckingInput,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
            result: Option<typed_trees::expression::ExpressionHandle>,
            known: &std::collections::BTreeSet<String>,
        ) -> bool {
            use typed_trees::domain::ProofFact;
            use typed_trees::expression::ExpressionNode;

            let Some(result) = result else {
                return false;
            };
            let result_label = program.render_proof_expression(result, ProofSubstitutions::None);
            let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
            match program.proof_facts.get(fact) {
                ProofFact::Expression(expression) => {
                    matches!(
                        program.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    ) || known.contains(&format!(
                        "boolean:{}",
                        program.render_proof_expression(
                            *expression,
                            ProofSubstitutions::ByParameter(&substitutions)
                        )
                    ))
                }
                ProofFact::Proposition(application) => {
                    proposition_application_label(program, application, &substitutions)
                        .is_some_and(|goal| known.contains(&goal))
                }
                ProofFact::Membership(_) => false,
            }
        }

        pub(crate) fn outcome_specific_assignment_matches_result(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            fact: arena::Handle<typed_trees::domain::ProofFact>,
            result: Option<typed_trees::expression::ExpressionHandle>,
            source: arena::Handle<CheckedEvidenceTerm>,
        ) -> bool {
            let Some(result) = result else {
                return false;
            };
            let Some(application) = program.proof_facts.get(fact).as_proposition() else {
                return false;
            };
            let result_label = program.render_proof_expression(result, ProofSubstitutions::None);
            let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
            let (binder_labels, argument_labels) =
                proposition_label_parts(program, application, &substitutions);
            program
                .normalize_nominal_proposition_application(
                    application,
                    Some(PropositionLabels {
                        binder_labels: &binder_labels,
                        argument_labels: &argument_labels,
                    }),
                )
                .map(lower_checked_proposition_application)
                .is_some_and(|expected| proof.evidence_terms.get(source).proposition == expected)
        }
    }
    mod proof_output_calls {
        /*
            Proof-output call facts, open requirement checks and named witness lanes.
        */
        use crate::flow::expressions;
        use crate::published::proof::proposition_vocabulary::{
            contract_proposition_labels, lower_checked_proposition_application,
        };
        use checked_trees::{
            CheckedEvidenceTerm, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
        };
        use symbols::SymbolHandle;
        use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

        pub(crate) fn bind_proof_output_call_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &mut ProofFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            use typed_trees::expression::ExpressionNode;

            let mut diagnostics = Vec::new();
            let mut invocations = arena::Arena::default();

            for package in &program.proof_output_calls {
                let Some(call) = program.expression_table.call_node(package.call) else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "proof-output binding requires a direct call",
                    ));
                    continue;
                };
                let source_site = validation::CallSite::Expression {
                    expression: package.call,
                    call,
                };
                let runtime_source = if call.runtime_call.is_some() {
                    match validation::call_contract_source(
                        program,
                        package.machine_symbol,
                        &source_site,
                    ) {
                        Ok(Some(source @ validation::CallContractSource::Runtime { .. })) => {
                            Some(source)
                        }
                        Ok(_) => {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "runtime evidence invocation has no exact callable contract",
                            ));
                            continue;
                        }
                        Err(reason) => {
                            diagnostics.push(diagnostics::Diagnostic::error(reason));
                            continue;
                        }
                    }
                } else {
                    None
                };
                let machine_target = validation::find_state(program, call.target_symbol);
                let open_requirement = program
                    .machine_by_symbol(package.machine_symbol)
                    .filter(|_| runtime_source.is_none())
                    .map(|machine| {
                        validation::named_conformance_target_requirement(
                            program,
                            machine,
                            call.target_symbol,
                        )
                    })
                    .transpose();
                let open_requirement = match open_requirement {
                    Ok(requirement) => requirement.flatten(),
                    Err(error) => {
                        diagnostics.push(diagnostics::Diagnostic::error(error));
                        continue;
                    }
                };
                let static_requirement = if let Some((machine, state)) = machine_target {
                    match checked_static_requirement_dispatch(
                        program,
                        package.machine_symbol,
                        call,
                        machine,
                        state,
                    ) {
                        Ok(dispatch) => dispatch,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            continue;
                        }
                    }
                } else if runtime_source.is_some()
                    || (open_requirement.is_some() && call.static_requirement_dispatch.is_none())
                {
                    None
                } else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!("proof-output call `{}` must target a checked machine state or exact generic requirement", call.target)));
                    continue;
                };
                if let Some(requirement) = open_requirement.as_ref()
                    && let Err(diagnostic) = check_open_proof_output_requirement(
                        program,
                        package.machine_symbol,
                        call,
                        requirement.application.declaration.symbol,
                        requirement.signature,
                    )
                {
                    diagnostics.push(diagnostic);
                    continue;
                }
                let caller_is_generic = program
                    .machine_by_symbol(package.machine_symbol)
                    .is_some_and(|machine| {
                        !machine.lifetime_parameters.is_empty()
                            || !machine.type_parameters.is_empty()
                            || !machine.conformance_bounds.is_empty()
                    });
                if !caller_is_generic
                    && machine_target.is_some_and(|(machine, _)| {
                        !machine.lifetime_parameters.is_empty()
                            || !machine.type_parameters.is_empty()
                            || !machine.conformance_bounds.is_empty()
                            || !call.machine_arguments.is_empty()
                    })
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "proof-output call `{}` from a concrete caller requires one complete concrete application", call.target,
                    )));
                    continue;
                }
                let signature_application =
                    if let Some(statement_index) = package.runtime_call_statement_index {
                        let mut calls = proof.contract_calls.iter().filter_map(|(_, call)| {
                            (call.caller_machine_symbol == package.machine_symbol
                                && call.caller_state_symbol == package.state_symbol
                                && call.statement_index == statement_index
                                && call.call_ordinal == 0)
                                .then_some(call.signature)
                        });
                        let application = calls.next();
                        if application.is_none() || calls.next().is_some() {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "proof-output package has no unique exact ordinary invocation",
                            ));
                            continue;
                        }
                        application.unwrap_or_default()
                    } else {
                        match super::contracts::call_signatures::retain(
                            program,
                            &mut proof.call_contract_signatures,
                            &mut proof.contract_facts,
                            &mut proof.evidence_terms,
                            package.machine_symbol,
                            package.state_symbol,
                            package.statement_index,
                            0,
                            &validation::CallSite::Expression {
                                expression: package.call,
                                call,
                            },
                        ) {
                            Ok(application) => application,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                continue;
                            }
                        }
                    };
                let public_owner = super::contracts::call_signatures::owner(
                    &proof.call_contract_signatures,
                    signature_application,
                );
                if (runtime_source.is_some()
                    || open_requirement.is_some()
                    || static_requirement.is_some())
                    && public_owner.is_none()
                {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "proof-output requirement lost its invocation-owned logical signature",
                    ));
                    continue;
                }
                let logical_signature = public_owner.map(|_| {
                    proof
                        .call_contract_signatures
                        .get(signature_application)
                        .signature
                        .clone()
                });
                if let Some(signature) = logical_signature.as_ref() {
                    let source_site = validation::CallSite::Expression {
                        expression: package.call,
                        call,
                    };
                    let matches = validation::call_contract_source(program, package.machine_symbol, &source_site)
                        .ok().flatten().is_some_and(|source| {
                            source.signature() == signature && public_owner.is_some_and(|owner| {
                                matches!(owner, ContractProofFactOwner::CallSignature { owner_symbol, .. } if owner_symbol == source.lexical_owner())
                            })
                        });
                    if !matches {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output package does not select its ordinary invocation's complete logical contract",
                        ));
                        continue;
                    }
                }
                let (target_machine_symbol, target_state_symbol, mut return_type, immediate) =
                    if let Some((machine, state)) = machine_target {
                        (
                            machine.symbol,
                            state.symbol,
                            state.return_type,
                            program.machine_states(machine).len() == 1
                                && machine.supply_mode
                                    == language_semantics::MachineSupplyMode::CheckedBody,
                        )
                    } else if let Some(source) = runtime_source.as_ref() {
                        (
                            SymbolHandle::invalid(),
                            SymbolHandle::invalid(),
                            source.signature().return_type,
                            true,
                        )
                    } else if let Some(requirement) = open_requirement.as_ref() {
                        (
                            requirement.application.declaration.symbol,
                            requirement.signature.symbol,
                            requirement.signature.return_type,
                            true,
                        )
                    } else {
                        continue;
                    };
                if let Some(signature) = logical_signature.as_ref() {
                    return_type = signature.return_type;
                }
                let runtime_value_type = return_type
                    .is_valid()
                    .then(|| program.primitive_type_reference(return_type))
                    .flatten();
                let proof_only = !return_type.is_valid();
                if !immediate || (!proof_only && runtime_value_type.is_none()) {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "proof-output call `{}` is currently limited to a one-state Unit- or scalar-result checked machine or exact generic requirement",
                        call.target
                    )));
                    continue;
                }

                let target_parameters = logical_signature
                    .as_ref()
                    .map(|signature| program.state_signature_parameters(signature))
                    .or_else(|| machine_target.map(|(_, state)| program.state_parameters(state)))
                    .unwrap_or_default();
                let mut callee_inputs = proof
                    .evidence_terms
                    .iter()
                    .filter_map(|(handle, term)| {
                        let owner_matches = public_owner.map_or_else(
                            || {
                                term.owner
                                    == ContractProofFactOwner::Machine {
                                        machine_symbol: target_machine_symbol,
                                    }
                                    || term.owner
                                        == (ContractProofFactOwner::MachineState {
                                            machine_symbol: target_machine_symbol,
                                            state_symbol: target_state_symbol,
                                        })
                            },
                            |owner| term.owner == owner,
                        );
                        (term.kind == ContractProofFactKind::Requires && owner_matches)
                            .then_some(handle)
                    })
                    .collect::<Vec<_>>();
                callee_inputs.sort_by_key(|handle| proof.evidence_terms.get(*handle).lane_position);
                let mut callee_outputs = proof
                    .evidence_terms
                    .iter()
                    .filter_map(|(handle, term)| {
                        let owner_matches = public_owner.map_or(
                            term.owner
                                == (ContractProofFactOwner::Machine {
                                    machine_symbol: target_machine_symbol,
                                }),
                            |owner| term.owner == owner,
                        );
                        (term.kind == ContractProofFactKind::Ensures
                            && owner_matches
                            && !proof
                                .outcome_specific_guarantees
                                .iter()
                                .any(|(_, row)| row.evidence_term == Some(handle)))
                        .then_some(handle)
                    })
                    .collect::<Vec<_>>();
                callee_outputs
                    .sort_by_key(|handle| proof.evidence_terms.get(*handle).lane_position);
                if callee_outputs.is_empty() {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "proof-output call `{}` requires at least one unconditional named ensures output",
                        call.target
                    )));
                    continue;
                }
                if call.evidence_arguments.len() != callee_inputs.len() {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "proof-output call `{}` supplies {} erased evidence argument{} but its named requires lane has {}",
                        call.target,
                        call.evidence_arguments.len(),
                        if call.evidence_arguments.len() == 1 { "" } else { "s" },
                        callee_inputs.len(),
                    )));
                    continue;
                }
                let mut fields = std::collections::BTreeMap::new();
                let mut local_names = std::collections::BTreeSet::new();
                let mut invalid = false;
                for binding in &package.bindings {
                    if binding.output_field.as_str() == "value" && binding.binding.as_str() == "_" {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output binding cannot discard its runtime Type result",
                        ));
                        invalid = true;
                        continue;
                    }
                    if fields
                        .insert(binding.output_field.as_str(), binding)
                        .is_some()
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-output selector `{}` is bound more than once",
                            binding.output_field
                        )));
                        invalid = true;
                    }
                    if binding.binding.as_str() != "_"
                        && !local_names.insert(binding.binding.as_str())
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "evidence term `{}` is bound more than once in this package pattern",
                            binding.binding
                        )));
                        invalid = true;
                    }
                }
                let value_binding = fields.get("value").copied();
                match (runtime_value_type, value_binding) {
                    (Some(_), None) => {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-output binding from `{}` is missing its runtime Type result",
                            call.target
                        )));
                        invalid = true;
                    }
                    (None, Some(_)) => {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-only call `{}` has no runtime Type result to bind",
                            call.target
                        )));
                        invalid = true;
                    }
                    _ => {}
                }
                for field in fields.keys() {
                    if (*field != "value" || runtime_value_type.is_none())
                        && !callee_outputs
                            .iter()
                            .any(|output| proof.evidence_terms.get(*output).name == *field)
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "call `{}` publishes no proof-output selector `{field}`",
                            call.target
                        )));
                        invalid = true;
                    }
                }

                let runtime_call = if let Some(statement_index) =
                    package.runtime_call_statement_index
                {
                    let Some(caller_state) = program.state_by_symbol(package.state_symbol) else {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output binding has no caller state",
                        ));
                        continue;
                    };
                    let runtime_statement = program
                        .statement_table
                        .statement_at(caller_state, statement_index);
                    let exact_runtime_statement =
                        match (runtime_value_type, value_binding, runtime_statement) {
                            (
                                Some(runtime_value_type),
                                Some(value_binding),
                                Some(typed_trees::statement::StatementNode::LocalData(local)),
                            ) => {
                                let local_call = local.initial_value.is_valid().then(|| {
                                    program.expression_table.expression(local.initial_value)
                                });
                                local.name == value_binding.binding
                                    && program.primitive_type_reference(local.type_reference)
                                        == Some(runtime_value_type)
                                    && matches!(local_call, Some(ExpressionNode::Call(local_call))
                                        if local_call.target_symbol == call.target_symbol)
                            }
                            (
                                None,
                                None,
                                Some(typed_trees::statement::StatementNode::Call(unit_call)),
                            ) => unit_call.target_symbol == call.target_symbol,
                            _ => false,
                        };
                    if !exact_runtime_statement
                        || package.statement_index != statement_index.saturating_add(1)
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output binding does not match its exact ordinary call",
                        ));
                        continue;
                    }
                    let mut matching_calls = proof.contract_calls.iter().filter_map(|(_, fact)| {
                        (fact.caller_machine_symbol == package.machine_symbol
                            && fact.caller_state_symbol == package.state_symbol
                            && fact.statement_index == statement_index
                            && fact.call_ordinal == 0)
                            .then_some(fact)
                    });
                    let Some(contract_call) = matching_calls.next() else {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output call has no exact checked contract-call row",
                        ));
                        continue;
                    };
                    if matching_calls.next().is_some()
                        || contract_call.target_machine_symbol != target_machine_symbol
                        || contract_call.target_state_symbol != target_state_symbol
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "proof-output call disagrees with its checked contract-call row",
                        ));
                        continue;
                    }
                    Some(checked_trees::ProofOutputRuntimeCallFact {
                        statement_index,
                        call_ordinal: 0,
                    })
                } else {
                    if runtime_value_type.is_some() {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-output call `{}` is missing its runtime Type result",
                            call.target
                        )));
                        continue;
                    }
                    None
                };
                for binding in &package.bindings {
                    if binding.binding.as_str() == "_" {
                        continue;
                    }
                    let duplicate = proof.evidence_terms.iter().any(|(_, term)| {
                        term.name == binding.binding.as_str()
                            && (term.owner
                                == (ContractProofFactOwner::Machine {
                                    machine_symbol: package.machine_symbol,
                                })
                                || term.owner
                                    == (ContractProofFactOwner::MachineState {
                                        machine_symbol: package.machine_symbol,
                                        state_symbol: package.state_symbol,
                                    }))
                    }) || invocations.iter().any(
                        |(_, invocation): (_, &checked_trees::ProofOutputCallFact)| {
                            invocation.caller_machine_symbol == package.machine_symbol
                                && invocation.caller_state_symbol == package.state_symbol
                                && invocation.outputs.iter().any(|output| {
                                    output.output.is_some_and(|output| {
                                        proof.evidence_terms.get(output).name
                                            == binding.binding.as_str()
                                    })
                                })
                        },
                    );
                    if duplicate {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "evidence term `{}` is already bound in this machine state",
                            binding.binding
                        )));
                        invalid = true;
                    }
                }
                if invalid {
                    continue;
                }

                let mut evidence_arguments = Vec::with_capacity(callee_inputs.len());
                for (input_position, (authored, callee_input)) in call
                    .evidence_arguments
                    .iter()
                    .zip(callee_inputs)
                    .enumerate()
                {
                    let Some(source) = proof_output_source_term_by_name(
                        proof,
                        &invocations,
                        package,
                        authored.as_str(),
                    ) else {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "unknown incoming evidence term `{}` in proof-output call `{}`",
                            authored, call.target,
                        )));
                        invalid = true;
                        continue;
                    };
                    let Some((instantiated_proposition, instantiated_label)) =
                        instantiate_proof_output_proposition(
                            program,
                            proof,
                            package,
                            call,
                            target_parameters,
                            callee_input,
                        )
                    else {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-output call `{}` cannot instantiate erased requires position {}",
                            call.target, input_position,
                        )));
                        invalid = true;
                        continue;
                    };
                    if proof.evidence_terms.get(source).proposition != instantiated_proposition {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "evidence term `{}` does not inhabit erased requires position {} of proof-output call `{}`",
                            authored, input_position, call.target,
                        )));
                        invalid = true;
                        continue;
                    }
                    append_proposition_application_if_missing(proof, &instantiated_proposition);
                    evidence_arguments.push(checked_trees::ProofOutputEvidenceArgumentFact {
                        input_position,
                        callee_input,
                        source,
                        instantiated_proposition,
                        instantiated_label,
                    });
                }
                if invalid {
                    continue;
                }

                let mut outputs = Vec::with_capacity(callee_outputs.len());
                for callee_output in callee_outputs {
                    let declaration = proof.evidence_terms.get(callee_output).clone();
                    let Some((instantiated_proposition, instantiated_label)) =
                        instantiate_proof_output_proposition(
                            program,
                            proof,
                            package,
                            call,
                            target_parameters,
                            callee_output,
                        )
                    else {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "proof-output call `{}` cannot instantiate output `{}`",
                            call.target, declaration.name,
                        )));
                        invalid = true;
                        continue;
                    };
                    append_proposition_application_if_missing(proof, &instantiated_proposition);
                    let binding = fields.get(declaration.name.as_str());
                    let output = binding
                        .filter(|binding| binding.binding.as_str() != "_")
                        .map(|binding| {
                            proof.evidence_terms.append(CheckedEvidenceTerm {
                                name: binding.binding.as_str().to_owned(),
                                owner: ContractProofFactOwner::MachineState {
                                    machine_symbol: package.machine_symbol,
                                    state_symbol: package.state_symbol,
                                },
                                kind: ContractProofFactKind::Ensures,
                                lane_position: declaration.lane_position,
                                proposition: instantiated_proposition.clone(),
                                evidence_type: declaration.evidence_type,
                                evidence_interface: declaration.evidence_interface,
                            })
                        });
                    outputs.push(checked_trees::ProofOutputFact {
                        output_position: declaration.lane_position,
                        callee_output,
                        instantiated_proposition,
                        instantiated_label,
                        output,
                    });
                }
                if invalid {
                    continue;
                }
                invocations.append(checked_trees::ProofOutputCallFact {
                    caller_machine_symbol: package.machine_symbol,
                    caller_state_symbol: package.state_symbol,
                    statement_index: package.statement_index,
                    source_statement_index: package.source_statement_index,
                    runtime_call,
                    target_machine_symbol,
                    target_state_symbol,
                    static_requirement_dispatch: static_requirement.map(|(fact, _)| fact),
                    evidence_arguments,
                    outputs,
                });
            }

            if diagnostics.is_empty() {
                proof.proof_output_calls = invocations;
                Ok(())
            } else {
                Err(diagnostics)
            }
        }

        fn check_open_proof_output_requirement(
            program: &dyn ::validation::CheckingInput,
            caller: SymbolHandle,
            call: &typed_trees::expression::TableCallExpression,
            owner: SymbolHandle,
            requirement: &typed_trees::signature::StateSignature,
        ) -> Result<(), diagnostics::Diagnostic> {
            let rejected = |reason: &str| {
                diagnostics::Diagnostic::error(format!(
                    "static named-witness requirement call `{}` cannot use its generic public contract: {reason}",
                    call.target,
                ))
            };
            let Some(definition) = program.trait_definition_by_symbol(owner) else {
                return Err(rejected("its declaring trait is absent"));
            };
            if !definition.lifetime_parameters.is_empty()
                || !trait_parameters_are_data_types(program, definition)
                || !requirement.lifetime_parameters.is_empty()
                || !program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
            {
                return Err(rejected(
                    "the public trait and requirement must be concrete and non-generic",
                ));
            }
            let unit = !requirement.return_type.is_valid();
            let scalar = matches!(
                program.primitive_type_reference(requirement.return_type),
                Some(
                    typed_trees::types::PrimitiveType::I32
                        | typed_trees::types::PrimitiveType::Bool
                )
            ) && program.state_signature_parameters(requirement).is_empty()
                && expressions(program)
                    .expression_handles(call.arguments)
                    .is_empty()
                && program
                    .machine_by_symbol(caller)
                    .is_some_and(|machine| machine.attached_data.is_none());
            if !unit && !scalar {
                return Err(rejected(
                    "the public requirement must be Unit, or exact i32 or bool with a free caller and zero ordinary arguments",
                ));
            }
            check_public_named_witness_lanes(program, requirement).map_err(rejected)
        }

        /*
            A trait's data types are its ordinary type parameters; a closed application
            supplies them concretely. Const, machine and proposition parameters would
            put generic values into the witness lanes, which this rung does not
            instantiate.
        */
        fn trait_parameters_are_data_types(
            program: &dyn ::validation::CheckingInput,
            definition: &typed_trees::trait_definition::TraitDefinition,
        ) -> bool {
            program
                .trait_type_parameters(definition)
                .iter()
                .all(|parameter| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
        }

        fn checked_static_requirement_dispatch<'program>(
            program: &'program dyn ::validation::CheckingInput,
            caller_machine: symbols::SymbolHandle,
            call: &typed_trees::expression::TableCallExpression,
            realization_machine: &'program typed_trees::machine::Machine,
            realization_state: &'program typed_trees::state::State,
        ) -> Result<
            Option<(
                checked_trees::StaticRequirementDispatchFact,
                &'program typed_trees::signature::StateSignature,
            )>,
            diagnostics::Diagnostic,
        > {
            let Some(dispatch) = call.static_requirement_dispatch.as_ref() else {
                return Ok(None);
            };
            let rejected = |reason: &str| {
                diagnostics::Diagnostic::error(format!(
                    "static named-witness requirement call `{}` is outside the first closed dispatch rung: {reason}",
                    call.target,
                ))
            };

            if dispatch.application_report_fingerprint == 0
                || dispatch.application_commitment.is_zero()
                || dispatch.realization_machine != realization_machine.symbol
                || dispatch.realization_state != realization_state.symbol
                || call.target_symbol != realization_state.symbol
            {
                return Err(rejected(
                    "its retained public/private dispatch identities do not match the executable target",
                ));
            }

            let applications = program.selected_conformance_applications(
                caller_machine,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                return Err(rejected(
                    "its exact owner-scoped closed conformance application is absent or ambiguous",
                ));
            };
            if application.trait_definition != dispatch.declaring_trait {
                let Some(application_trait) =
                    program.trait_definition_by_symbol(application.trait_definition)
                else {
                    return Err(rejected("its application trait is absent"));
                };
                if !trait_requires_transitively(
                    program,
                    application_trait,
                    dispatch.declaring_trait,
                ) {
                    return Err(rejected(
                        "its declaring trait is absent from the application trait's requirement closure",
                    ));
                }
            }
            let rows = application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait == dispatch.declaring_trait
                        && row.requirement == dispatch.requirement
                        && row.realization_machine == dispatch.realization_machine
                        && row.realization_state == dispatch.realization_state
                })
                .collect::<Vec<_>>();
            if rows.len() != 1 {
                return Err(rejected(
                    "its closed conformance application does not contain one exact requirement-to-realization row",
                ));
            }

            let Some(trait_definition) =
                program.trait_definition_by_symbol(dispatch.declaring_trait)
            else {
                return Err(rejected("its declaring trait is absent"));
            };
            let requirements = program
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|requirement| requirement.symbol == dispatch.requirement)
                .collect::<Vec<_>>();
            let [requirement] = requirements.as_slice() else {
                return Err(rejected("its public requirement is absent or ambiguous"));
            };
            let Some(selected) = program.conformance_by_symbol(application.declaration) else {
                return Err(rejected("its selected conformance declaration is absent"));
            };
            let selected_rows = selected.rows.as_slice();
            let exact_rows = selected_rows
                .iter()
                .filter(|row| {
                    row.declaring_trait == dispatch.declaring_trait
                        && row.requirement == dispatch.requirement
                        && row.realization_machine == dispatch.realization_machine
                        && row.realization_state == dispatch.realization_state
                })
                .collect::<Vec<_>>();
            let [_selected_row] = exact_rows.as_slice() else {
                return Err(rejected(
                    "its selected conformance no longer owns one exact realization row",
                ));
            };
            let concrete = application.lifetime_arguments.is_empty()
                && application.type_arguments.is_empty()
                && application.const_arguments.is_empty()
                && application.machine_arguments.is_empty()
                && trait_definition.lifetime_parameters.is_empty()
                && trait_parameters_are_data_types(program, trait_definition)
                && requirement.lifetime_parameters.is_empty()
                && program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
                && selected.lifetime_parameters.is_empty()
                && program.conformance_type_parameters(selected).is_empty()
                && realization_machine.lifetime_parameters.is_empty()
                && program
                    .machine_type_parameters(realization_machine)
                    .is_empty();
            if !concrete {
                return Err(rejected(
                    "the trait, requirement, selected conformance, and realization must be concrete and non-generic",
                ));
            }
            let unit_result =
                !requirement.return_type.is_valid() && !realization_state.return_type.is_valid();
            let requirement_result = program.primitive_type_reference(requirement.return_type);
            let realization_result =
                program.primitive_type_reference(realization_state.return_type);
            let bounded_scalar_result = requirement_result == realization_result
                && matches!(
                    requirement_result,
                    Some(
                        typed_trees::types::PrimitiveType::I32
                            | typed_trees::types::PrimitiveType::Bool
                    )
                )
                && program.state_signature_parameters(requirement).is_empty()
                && program.state_parameters(realization_state).is_empty()
                && expressions(program)
                    .expression_handles(call.arguments)
                    .is_empty()
                && !call.receiver.is_valid()
                && program
                    .machine_by_symbol(caller_machine)
                    .is_some_and(|machine| machine.attached_data.is_none());
            if program.machine_states(realization_machine).len() != 1
                || realization_machine.supply_mode
                    != language_semantics::MachineSupplyMode::CheckedBody
                || !(unit_result || bounded_scalar_result)
            {
                return Err(rejected(
                    "the requirement and realization must be one-state Unit callables, or the scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
                ));
            }

            check_public_named_witness_lanes(program, requirement).map_err(rejected)?;

            Ok(Some((
                checked_trees::StaticRequirementDispatchFact {
                    selection_application: dispatch.selection_application,
                    evidence_parameter: dispatch.evidence_parameter,
                    application_report_fingerprint: dispatch.application_report_fingerprint,
                    application_commitment: dispatch.application_commitment,
                    declaring_trait: dispatch.declaring_trait,
                    requirement: dispatch.requirement,
                    contract_signature: dispatch.contract_signature,
                    realization_machine: dispatch.realization_machine,
                    realization_state: dispatch.realization_state,
                },
                requirement,
            )))
        }

        /*
            Whether `required_trait` appears in the transitive `requires` closure of
            `trait_definition`. Inherited rows keep the parent's declaring-trait
            identity, so a dispatch declared on a parent is admitted through a
            conformance to its descendant.
        */
        fn trait_requires_transitively(
            program: &dyn ::validation::CheckingInput,
            trait_definition: &typed_trees::trait_definition::TraitDefinition,
            required_trait: SymbolHandle,
        ) -> bool {
            let mut pending = vec![trait_definition];
            let mut visited = Vec::new();
            while let Some(definition) = pending.pop() {
                if visited.contains(&definition.symbol) {
                    continue;
                }
                visited.push(definition.symbol);
                for requirement in program.trait_requirements(definition) {
                    if requirement.symbol == required_trait {
                        return true;
                    }
                    if let Some(parent) = program.trait_definition_by_symbol(requirement.symbol) {
                        pending.push(parent);
                    }
                }
            }
            false
        }

        fn check_public_named_witness_lanes(
            program: &dyn ::validation::CheckingInput,
            requirement: &typed_trees::signature::StateSignature,
        ) -> Result<(), &'static str> {
            use typed_trees::domain::ProofFact;
            use typed_trees::signature::SignatureContractKind;
            let contracts = program.state_signature_contracts(requirement);
            if contracts.iter().any(|contract| {
                !matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                )
            }) {
                return Err("outcome-guarded and crash contract rows remain unsupported");
            }
            let named_requires = contracts
                .iter()
                .filter(|contract| {
                    contract.binding.is_some() && contract.kind == SignatureContractKind::Requires
                })
                .collect::<Vec<_>>();
            let named_ensures = contracts
                .iter()
                .filter(|contract| {
                    contract.binding.is_some() && contract.kind == SignatureContractKind::Ensures
                })
                .collect::<Vec<_>>();
            if named_ensures.is_empty() {
                return Err(
                    "the public requirement must own at least one unconditional named ensures output",
                );
            }
            if contracts.len() != named_requires.len() + named_ensures.len() {
                return Err("every public requires and ensures row must be named");
            }
            for contract in named_requires.into_iter().chain(named_ensures) {
                let [ProofFact::Proposition(proposition)] =
                    program.proof_facts.span_or_empty(contract.facts)
                else {
                    return Err(
                        "each public named lane must contain one witness-bearing proposition",
                    );
                };
                if !proposition.binder_arguments.is_empty()
                    || !expressions(program)
                        .expression_handles(proposition.arguments)
                        .is_empty()
                {
                    return Err(
                        "the public witness proposition must be subjectless and non-generic",
                    );
                }
            }

            Ok(())
        }

        fn proof_output_source_term_by_name(
            proof: &ProofFacts,
            invocations: &arena::Arena<checked_trees::ProofOutputCallFact>,
            package: &typed_trees::typed_trees::ProofOutputCall,
            name: &str,
        ) -> Option<arena::Handle<CheckedEvidenceTerm>> {
            proof
                .evidence_terms
                .iter()
                .find_map(|(handle, term)| {
                    let owner_matches = term.owner
                        == (ContractProofFactOwner::Machine {
                            machine_symbol: package.machine_symbol,
                        })
                        || term.owner
                            == (ContractProofFactOwner::MachineState {
                                machine_symbol: package.machine_symbol,
                                state_symbol: package.state_symbol,
                            });
                    (owner_matches
                        && term.kind == ContractProofFactKind::Requires
                        && term.name == name)
                        .then_some(handle)
                })
                .or_else(|| {
                    invocations
                        .iter()
                        .filter_map(|(_, invocation)| {
                            (invocation.caller_machine_symbol == package.machine_symbol
                                && invocation.caller_state_symbol == package.state_symbol
                                && invocation.source_statement_index
                                    < package.source_statement_index)
                                .then_some(
                                    invocation.outputs.iter().filter_map(|output| output.output),
                                )
                        })
                        .flatten()
                        .find(|term| proof.evidence_terms.get(*term).name == name)
                })
        }

        fn instantiate_proof_output_proposition(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            package: &typed_trees::typed_trees::ProofOutputCall,
            call: &typed_trees::expression::TableCallExpression,
            target_parameters: &[typed_trees::signature::StateParameter],
            term: arena::Handle<CheckedEvidenceTerm>,
        ) -> Option<(checked_trees::CheckedPropositionApplication, String)> {
            let contract = proof
                .contract_facts
                .iter()
                .map(|(_, contract)| contract)
                .find(|contract| contract.evidence_term == Some(term))?;
            let application = program.proof_facts.get(contract.fact).as_proposition()?;
            let call_site = validation::CallSite::Expression {
                expression: package.call,
                call,
            };
            let (binder_labels, argument_handles) =
                crate::published::labels::proposition_label_sources(application, program);
            let argument_labels = argument_handles
                .iter()
                .map(|argument| {
                    crate::checks::contracts::labels::instantiate_call_contract_expression_label(
                        program,
                        package.state_symbol,
                        package.statement_index,
                        &call_site,
                        target_parameters,
                        *argument,
                    )
                })
                .collect::<Vec<_>>();
            let normalized = program.normalize_nominal_proposition_application(
                application,
                Some(PropositionLabels {
                    binder_labels: &binder_labels,
                    argument_labels: &argument_labels,
                }),
            )?;
            let identity =
                program.normalized_identity_label(application, &binder_labels, &argument_labels)?;
            Some((lower_checked_proposition_application(normalized), identity))
        }

        fn append_proposition_application_if_missing(
            proof: &mut ProofFacts,
            application: &checked_trees::CheckedPropositionApplication,
        ) {
            if !proof
                .proposition_vocabulary
                .applications
                .contains(application)
            {
                proof
                    .proposition_vocabulary
                    .applications
                    .push(application.clone());
            }
        }

        pub(crate) fn intake_checked_proof_output_propositions(
            proof: &ProofFacts,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            known: &mut std::collections::BTreeSet<String>,
        ) {
            for (_, invocation) in proof.proof_output_calls.iter() {
                if invocation.caller_machine_symbol == machine_symbol
                    && invocation.caller_state_symbol == state_symbol
                    && invocation.statement_index == statement_index
                {
                    known.extend(
                        invocation
                            .outputs
                            .iter()
                            .map(|output| output.instantiated_label.clone()),
                    );
                }
            }
        }

        /// Substitute each of a callee entry state's parameters by the rendered
        /// argument a call passes for it, so the callee's contracts read in the
        /// caller's terms. A parameter without an argument keeps its name.
        pub(crate) fn call_parameter_substitutions(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            arguments: &[typed_trees::expression::ExpressionHandle],
        ) -> Vec<(SymbolHandle, String, String)> {
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let replacement = arguments
                        .get(index)
                        .map(|argument| {
                            program.render_proof_expression(*argument, ProofSubstitutions::None)
                        })
                        .unwrap_or_else(|| parameter.name.as_str().to_owned());
                    (
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        replacement,
                    )
                })
                .collect()
        }

        pub(crate) fn intake_call_ensures_propositions(
            program: &dyn ::validation::CheckingInput,
            call: &typed_trees::statement::TableCall,
            known: &mut std::collections::BTreeSet<String>,
        ) {
            if let Some(binding) = call.runtime_call {
                let Ok(signature) = binding.signature(program.typed_program()) else {
                    return;
                };
                let parameters = program.state_signature_parameters(signature);
                let arguments = program.statement_table.expression_handles(call.arguments);
                if parameters.len() != arguments.len() {
                    return;
                }
                let substitutions = parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| {
                        (
                            parameter.symbol,
                            parameter.name.to_string(),
                            program.render_proof_expression(*argument, ProofSubstitutions::None),
                        )
                    })
                    .collect::<Vec<_>>();
                known.extend(contract_proposition_labels(
                    program,
                    program.state_signature_contracts(signature),
                    typed_trees::signature::SignatureContractKind::Ensures,
                    &substitutions,
                ));
                return;
            }
            let Some((callee, state)) = program
                .machine_by_symbol(call.target_symbol)
                .and_then(|machine| {
                    program
                        .machine_states(machine)
                        .first()
                        .map(|state| (machine, state))
                })
                .or_else(|| validation::find_state(program, call.target_symbol))
            else {
                return;
            };
            let substitutions = call_parameter_substitutions(
                program,
                state,
                program.statement_table.expression_handles(call.arguments),
            );
            known.extend(contract_proposition_labels(
                program,
                program.machine_contracts(callee),
                typed_trees::signature::SignatureContractKind::Ensures,
                &substitutions,
            ));
            known.extend(contract_proposition_labels(
                program,
                program.state_contracts(state),
                typed_trees::signature::SignatureContractKind::Ensures,
                &substitutions,
            ));
        }
    }
    mod proposition_vocabulary {
        /*
            The checked proposition vocabulary and lowered proposition applications.
        */
        use crate::flow::expressions;
        use arena::{Handle, HandleSpan};
        use checked_trees::{
            CheckedEvidenceInterfaceIdentity, CheckedPropositionApplication,
            CheckedPropositionBinder, CheckedPropositionBinderKind as BinderKind,
            CheckedPropositionDeclaration, CheckedPropositionEvidence,
            CheckedPropositionVocabulary, ContractProofFactKind,
        };
        use symbols::SymbolHandle;
        use typed_trees::proposition::{
            NormalizedEvidenceInterfaceIdentity, NormalizedPropositionApplicationIdentity,
            ProofSubstitutions, PropositionApplication, PropositionBinderKind, PropositionBody,
        };
        use typed_trees::references::program_proof_facts;
        use typed_trees::signature::SignatureContract;
        use typed_trees::signature::SignatureContractKind;

        pub(crate) fn contract_proposition_labels(
            program: &dyn ::validation::CheckingInput,
            contracts: &[SignatureContract],
            kind: SignatureContractKind,
            substitutions: &[(SymbolHandle, String, String)],
        ) -> std::collections::BTreeSet<String> {
            use typed_trees::domain::ProofFact;

            contracts
                .iter()
                .filter(|contract| contract.kind == kind)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .filter_map(|fact| match fact {
                    ProofFact::Expression(expression) => Some(format!(
                        "boolean:{}",
                        program.render_proof_expression(
                            *expression,
                            ProofSubstitutions::ByParameter(substitutions)
                        )
                    )),
                    ProofFact::Proposition(application) => {
                        proposition_application_label(program, application, substitutions)
                    }
                    ProofFact::Membership(_) => None,
                })
                .collect()
        }

        /*
            Binder and argument labels for one proposition application under the
            caller's parameter substitutions.
        */
        pub(crate) fn proposition_label_parts(
            program: &dyn ::validation::CheckingInput,
            application: &PropositionApplication,
            substitutions: &[(SymbolHandle, String, String)],
        ) -> (Vec<String>, Vec<String>) {
            let binder_labels = application
                .binder_arguments
                .iter()
                .map(|argument| {
                    substitutions
                        .iter()
                        .find(|(symbol, _, _)| *symbol == argument.symbol)
                        .map(|(_, _, replacement)| replacement.clone())
                        .unwrap_or_else(|| argument.display_name())
                })
                .collect::<Vec<_>>();
            let argument_labels = expressions(program)
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| {
                    program.render_proof_expression(
                        *argument,
                        ProofSubstitutions::ByParameter(substitutions),
                    )
                })
                .collect::<Vec<_>>();
            (binder_labels, argument_labels)
        }

        pub(crate) fn proposition_application_label(
            program: &dyn ::validation::CheckingInput,
            application: &PropositionApplication,
            substitutions: &[(SymbolHandle, String, String)],
        ) -> Option<String> {
            let (binder_labels, argument_labels) =
                proposition_label_parts(program, application, substitutions);
            program.normalized_identity_label(application, &binder_labels, &argument_labels)
        }

        pub(crate) fn build_checked_proposition_vocabulary(
            program: &dyn ::validation::CheckingInput,
        ) -> CheckedPropositionVocabulary {
            let declarations = program
                .propositions()
                .iter()
                .filter_map(|declaration| {
                    let evidence = match declaration.body {
                        PropositionBody::Primitive => CheckedPropositionEvidence::FactOnly,
                        PropositionBody::Witness { evidence } => {
                            CheckedPropositionEvidence::Witness {
                                evidence_type: program.display_type_reference(evidence),
                            }
                        }
                        PropositionBody::Transparent { .. } => return None,
                    };
                    let binders = program
                        .proposition_binders(declaration)
                        .iter()
                        .map(|binder| CheckedPropositionBinder {
                            name: binder.name.as_str().to_owned(),
                            kind: match binder.kind {
                                PropositionBinderKind::Type => BinderKind::Type,
                                PropositionBinderKind::Const { type_reference } => {
                                    BinderKind::Const {
                                        type_identity: program
                                            .normalized_type_identity(type_reference),
                                    }
                                }
                                PropositionBinderKind::Machine => BinderKind::Machine,
                            },
                        })
                        .collect();
                    let parameter_types = program
                        .proposition_parameters(declaration)
                        .iter()
                        .map(|parameter| program.display_type_reference(parameter.type_reference))
                        .collect();
                    Some(CheckedPropositionDeclaration {
                        symbol: declaration.symbol,
                        name: declaration.name.as_str().to_owned(),
                        is_public: declaration.is_public,
                        binders,
                        parameter_types,
                        evidence,
                    })
                })
                .collect();
            let applications = program_proof_facts(program)
                .filter_map(|(_, fact)| {
                    let application = fact.as_proposition()?;
                    let normalized =
                        program.normalize_nominal_proposition_application(application, None)?;
                    Some(lower_checked_proposition_application(normalized))
                })
                .collect();
            CheckedPropositionVocabulary {
                declarations,
                applications,
            }
        }

        pub(crate) fn lower_checked_proposition_application(
            normalized: NormalizedPropositionApplicationIdentity,
        ) -> CheckedPropositionApplication {
            CheckedPropositionApplication::from_normalized(normalized)
        }

        pub(crate) fn lower_checked_evidence_interface(
            interface: &NormalizedEvidenceInterfaceIdentity,
        ) -> CheckedEvidenceInterfaceIdentity {
            CheckedEvidenceInterfaceIdentity::from_normalized(interface)
        }

        pub(crate) fn fact_handles(
            facts: HandleSpan<typed_trees::domain::ProofFact>,
        ) -> impl Iterator<Item = Handle<typed_trees::domain::ProofFact>> {
            facts.handles()
        }

        pub(crate) fn contract_fact_kind(
            kind: &SignatureContractKind,
        ) -> Option<ContractProofFactKind> {
            match kind {
                SignatureContractKind::Requires => Some(ContractProofFactKind::Requires),
                SignatureContractKind::Ensures => Some(ContractProofFactKind::Ensures),
                SignatureContractKind::EnsuresForResultCase { .. } => None,
                SignatureContractKind::Crashes { .. } => None,
            }
        }
    }

    pub(crate) use contracts::contract_target_from_state_symbol;
    pub(crate) use contracts::machine_parameter_evidence_signatures;
    pub(crate) use evidence_forwarding::{
        bind_evidence_forwarding_facts, bind_evidence_projection_facts,
    };
    pub(crate) use float_meaning::bind_float_meaning_projection_facts;
    pub(crate) use outcome_arms::{bind_outcome_specific_arm_facts, exact_outcome_case_test};
    pub(crate) use proof_output_calls::bind_proof_output_call_facts;
    pub(crate) use proposition_vocabulary::lower_checked_proposition_application;

    use checked_trees::{
        BorrowFacts, CheckedOperatorFacts, ContractProofFactKind, ContractProofFactOwner,
        ProofFacts,
    };

    use crate::flow::expressions;
    use crate::published::proof::proposition_vocabulary::{
        build_checked_proposition_vocabulary, fact_handles,
    };
    use contracts::{
        ContractEvidenceLanes, append_inherited_trait_contract_facts,
        append_machine_contract_facts, append_named_contract_evidence_term,
        append_operator_declaration_contract_facts, append_parameter_evidence_contract_facts,
        append_state_contract_facts, append_state_signature_contract_facts,
        build_contract_call_facts, build_contract_exit_facts, build_contract_operator_use_facts,
        estimated_contract_fact_capacity,
    };
    use obligations::lower_proof_obligation;

    pub(crate) fn build_proof_facts(
        program: &dyn ::validation::CheckingInput,
        proof_plan: &proof::obligations::ProofPlan,
        borrow: &BorrowFacts,
        operators: &CheckedOperatorFacts,
    ) -> Result<ProofFacts, Vec<diagnostics::Diagnostic>> {
        let mut obligations = arena::Arena::with_capacity(proof_plan.obligations.len());
        let mut contract_facts =
            arena::Arena::with_capacity(estimated_contract_fact_capacity(program));
        let mut inherited_contract_scopes = arena::Arena::default();
        let mut outcome_specific_guarantees = arena::Arena::default();
        let mut evidence_terms = arena::Arena::default();

        for (_, obligation) in proof_plan.obligations.iter() {
            obligations.append(lower_proof_obligation(obligation));
        }

        for machine in program.machines() {
            append_machine_contract_facts(
                program,
                machine,
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
            let mut guarded_lanes = ContractEvidenceLanes::default();
            for contract in program.machine_contracts(machine) {
                let typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } = &contract.kind
                else {
                    continue;
                };
                for fact in fact_handles(contract.facts) {
                    let evidence_term = match contract.binding.as_ref() {
                        Some(binding) => Some(
                            append_named_contract_evidence_term(
                                program,
                                ContractProofFactOwner::Machine {
                                    machine_symbol: machine.symbol,
                                },
                                ContractProofFactKind::Ensures,
                                guarded_lanes.next(ContractProofFactKind::Ensures),
                                fact,
                                binding.as_str(),
                                &mut evidence_terms,
                            )
                            .map_err(|error| vec![error])?,
                        ),
                        None => None,
                    };
                    outcome_specific_guarantees.append(
                        checked_trees::OutcomeSpecificGuaranteeFact {
                            machine_symbol: machine.symbol,
                            result_data: *result_data,
                            result_case: *result_case,
                            public_selector: contract
                                .binding
                                .as_ref()
                                .map(|binding| binding.as_str().to_owned()),
                            fact,
                            fact_is_truth: matches!(
                                program.proof_facts.get(fact),
                                typed_trees::domain::ProofFact::Expression(expression)
                                    if matches!(
                                        program.expression_table.expression(*expression),
                                        typed_trees::expression::ExpressionNode::Boolean(true)
                                    )
                            ),
                            argument_roots: program
                                .proof_facts
                                .get(fact)
                                .as_proposition()
                                .and_then(|application| {
                                    program
                                        .machine_states(machine)
                                        .first()
                                        .map(|state| (application, state))
                                })
                                .map(|(application, state)| {
                                    expressions(program)
                                        .expression_handles(application.arguments)
                                        .iter()
                                        .map(|argument| {
                                            outcome_arms::occurrence_root(program, state, *argument)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            evidence_term,
                        },
                    );
                }
            }
            for state in program.machine_states(machine) {
                append_state_contract_facts(
                    program,
                    machine,
                    state,
                    &mut contract_facts,
                    &mut evidence_terms,
                )
                .map_err(|error| vec![error])?;
            }
            append_inherited_trait_contract_facts(
                program,
                machine,
                &mut contract_facts,
                &mut inherited_contract_scopes,
                &evidence_terms,
            );
            append_parameter_evidence_contract_facts(
                program,
                program.machine_type_parameters(machine),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        for definition in program.data_definitions() {
            append_parameter_evidence_contract_facts(
                program,
                program.data_type_parameters(definition),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        for definition in program.domain_definitions() {
            append_parameter_evidence_contract_facts(
                program,
                program.domain_type_parameters(definition),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        for trait_definition in program.traits() {
            append_parameter_evidence_contract_facts(
                program,
                program.trait_type_parameters(trait_definition),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
            for requirement in program.trait_machine_signatures(trait_definition) {
                append_parameter_evidence_contract_facts(
                    program,
                    program.state_signature_type_parameters(requirement),
                    &mut contract_facts,
                    &mut evidence_terms,
                )
                .map_err(|error| vec![error])?;
            }
            append_state_signature_contract_facts(
                program,
                trait_definition.symbol,
                program.trait_machine_signatures(trait_definition),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        for application in program.closed_requirement_applications() {
            append_state_signature_contract_facts(
                program,
                application.declaring_trait,
                std::slice::from_ref(application.signature),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        contracts::append_runtime_callable_contract_facts(
            program,
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
        for operator in program.all_operators() {
            append_operator_declaration_contract_facts(program, operator, &mut contract_facts);
        }
        let (mut contract_fact_refs, contract_calls, call_contract_signatures) =
            build_contract_call_facts(program, borrow, &mut contract_facts, &mut evidence_terms)?;
        let contract_operator_uses = build_contract_operator_use_facts(
            program,
            operators,
            &mut contract_facts,
            &mut contract_fact_refs,
        );
        let contract_exits =
            build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);
        let proposition_vocabulary = build_checked_proposition_vocabulary(program);

        Ok(ProofFacts {
            obligations,
            contract_facts,
            inherited_contract_scopes,
            outcome_specific_guarantees,
            evidence_terms,
            contract_fact_refs,
            contract_calls,
            call_contract_signatures,
            contract_exits,
            contract_operator_uses,
            proposition_vocabulary,
            ..ProofFacts::default()
        })
    }
}
pub(crate) mod semantic {
    /*
        Semantic facts: what the checked program knows about its own values.

        This area answers one question in three parts, so its children are the
        parts rather than three unrelated roots:

        - [`facts`] states the knowledge itself — the contract, field-domain and
          point facts a checked machine publishes for later stages to replay.
        - [`calls`] names the coordinates a fact is stated about: which call site,
          which target parameters, which arguments. Fact construction and every
          later consumer ask here instead of re-deriving a call's shape, which is
          why this is the area's shared vocabulary rather than a private helper of
          `facts`.
        - [`places`] renders the storage a fact is stated about, built from a call
          coordinate and a contract term.

        The dependency direction inside the area is `places` -> {`calls`,
        `facts`} and `facts` -> `calls`; nothing here depends on the checking
        rules that consume it.
    */
    pub(crate) mod calls {
        use checked_trees::expression::ExpressionHandle;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        pub(crate) mod dispatch_subjects {
            /*
                The call a closed-case dispatch evaluates once.

                The parser spells `transition f(x) { Case::A { v } -> .. Case::B -> .. }`
                as one guarded arm per case. Each guard compares a copy of `f(x)` with its
                variant, and each payload read is a case-qualified member of another copy.
                The copies are separate expression nodes that repeat the same call,
                operands and authored selection occurrence. The language evaluates the
                call once, at the first arm (wiki/spec/language/expressions.md), so each
                copy names that one result rather than a call site of its own.
            */
            use typed_trees::expression::{ExpressionHandle, ExpressionNode};
            use typed_trees::statement::{StatementNode, TransitionGuardNode};

            /*
                The authored call whose saved result a case guard tests. Ordinary call
                operand preparation owns its nested invocations and evaluation order.
            */
            pub(crate) fn computed_case_subject(
                program: &dyn ::validation::CheckingInput,
                guard: ExpressionHandle,
            ) -> Option<ExpressionHandle> {
                let (subject, _) =
                    crate::published::proof::exact_outcome_case_test(program, guard)?;
                repeats_subject(program, subject, subject).then_some(subject)
            }

            /*
                Whether `copy` repeats the computed `subject`: the same call and operands
                carrying exactly the subject's nonempty authored selection occurrence.
                Source spans are diagnostic only, so a separately authored call of the
                same shape keeps its own occurrence and stays a separate evaluation.
            */
            pub(crate) fn repeats_subject(
                program: &dyn ::validation::CheckingInput,
                subject: ExpressionHandle,
                copy: ExpressionHandle,
            ) -> bool {
                let table = &program.expression_table;
                if !table.expression_is_valid(subject)
                    || !table.expression_is_valid(copy)
                    || !matches!(table.expression(subject), ExpressionNode::Call(_))
                    || !matches!(table.expression(copy), ExpressionNode::Call(_))
                    || !table.expressions_structurally_equal(subject, copy)
                {
                    return false;
                }
                let mut occurrences = table.authored_selection_occurrences(subject).peekable();
                occurrences.peek().is_some()
                    && occurrences.eq(table.authored_selection_occurrences(copy))
            }

            /*
                The computed subject statement `index` evaluates: the subject of its
                guard when no earlier transition of `statements` tests a copy of it.
            */
            pub(crate) fn evaluated_case_subject(
                program: &dyn ::validation::CheckingInput,
                statements: &[StatementNode],
                index: usize,
            ) -> Option<ExpressionHandle> {
                let tested = |statement: &StatementNode| {
                    let transition = statement.as_transition()?;
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        return None;
                    };
                    computed_case_subject(program, guard)
                };
                let subject = tested(statements.get(index)?)?;
                (!statements[..index].iter().any(|statement| {
                    tested(statement)
                        .is_some_and(|earlier| repeats_subject(program, subject, earlier))
                }))
                .then_some(subject)
            }

            /*
                The dispatch roles of the calls one transition statement spells: the
                computed subject it evaluates when it is the dispatch's first arm, and
                every call that reads the saved result instead of running, which is each
                copy of the subject in its guard or arm and each call inside such a copy.
            */
            #[derive(Default)]
            pub(crate) struct StatementDispatchCalls {
                pub(crate) evaluation: ExpressionHandle,
                pub(crate) saved_reads: Vec<ExpressionHandle>,
            }

            impl StatementDispatchCalls {
                /* The role of the call expression `expression` in this statement. */
                pub(crate) fn role(
                    &self,
                    expression: ExpressionHandle,
                ) -> checked_trees::FlowCallDispatchRole {
                    if !expression.is_valid() {
                        checked_trees::FlowCallDispatchRole::Ordinary
                    } else if expression == self.evaluation {
                        checked_trees::FlowCallDispatchRole::SubjectEvaluation
                    } else if self.saved_reads.contains(&expression) {
                        checked_trees::FlowCallDispatchRole::SavedSubjectRead
                    } else {
                        checked_trees::FlowCallDispatchRole::Ordinary
                    }
                }
            }

            /*
                Classify the calls statement `index` spells against the computed case
                subject its guard tests. A statement whose guard tests no computed subject
                spells only ordinary calls.
            */
            pub(crate) fn statement_dispatch_calls(
                program: &dyn ::validation::CheckingInput,
                statements: &[StatementNode],
                index: usize,
            ) -> StatementDispatchCalls {
                let mut calls = StatementDispatchCalls::default();
                let Some(statement @ StatementNode::Transition(transition)) = statements.get(index)
                else {
                    return calls;
                };
                let TransitionGuardNode::When(guard) = transition.guard else {
                    return calls;
                };
                let Some(subject) = computed_case_subject(program, guard) else {
                    return calls;
                };
                if evaluated_case_subject(program, statements, index) == Some(subject) {
                    calls.evaluation = subject;
                }
                let table = &program.expression_table;
                let mut pending = program
                    .tables
                    .expression_roots(statement)
                    .into_iter()
                    .map(|(_, root)| (root, false))
                    .collect::<Vec<_>>();
                let mut visited = Vec::new();
                while let Some((expression, inside_copy)) = pending.pop() {
                    if !table.expression_is_valid(expression)
                        || visited.contains(&(expression, inside_copy))
                    {
                        continue;
                    }
                    visited.push((expression, inside_copy));
                    let node = table.expression(expression);
                    let copy = inside_copy
                        || (expression != calls.evaluation
                            && repeats_subject(program, subject, expression));
                    if copy
                        && matches!(node, ExpressionNode::Call(_))
                        && !calls.saved_reads.contains(&expression)
                    {
                        calls.saved_reads.push(expression);
                    }
                    validation::expression_traversal::children(program, node, |child| {
                        pending.push((child, copy));
                    });
                }
                calls
            }
        }
        mod parameter_domains {
            /*
                Parameter qualifications are requirements in the selected call's type scope.

                The declaration contract already owns authored qualifications. Selecting a
                qualified type for a formal adds obligations on the actual argument; it does
                not add assumptions to the caller or mutate the generic declaration.
            */
            use facts::FactPayload;
            use symbols::SymbolHandle;
            use typed_trees::expression::ExpressionHandle;
            use validation::{
                CallSite, call_site_argument_expressions, call_type_arguments,
                parameter_domain_constraints,
            };

            pub(crate) fn call_parameter_domains(
                program: &dyn ::validation::CheckingInput,
                caller_machine: SymbolHandle,
                caller_state: SymbolHandle,
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
                site: &CallSite<'_>,
            ) -> Vec<(ExpressionHandle, FactPayload)> {
                let Some(caller) = program.machine_by_symbol(caller_machine) else {
                    return Vec::new();
                };
                let Some(callee) = program.machine_by_symbol(target_machine) else {
                    return Vec::new();
                };
                let Some(state) = program.state_in_machine(caller, caller_state) else {
                    return Vec::new();
                };
                let Some(entry) = program.state_in_machine(callee, target_state) else {
                    return Vec::new();
                };
                let selected = match site {
                    CallSite::Statement(call) => &call.machine_arguments[..],
                    CallSite::Expression { call, .. } => &call.machine_arguments[..],
                    CallSite::TransitionNamed { .. } => &[],
                };
                let arguments = call_site_argument_expressions(program, site);
                let parameters = program.state_parameters(entry);
                let Some(bindings) = call_type_arguments(
                    program,
                    caller,
                    Some(state),
                    callee,
                    entry,
                    selected,
                    arguments,
                ) else {
                    return Vec::new();
                };
                let mut requirements = Vec::new();
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let declared =
                        parameter_domain_constraints(program, parameter.type_reference, &[]);
                    for domain in
                        parameter_domain_constraints(program, parameter.type_reference, &bindings)
                    {
                        if declared.contains(&domain) {
                            continue;
                        }
                        let (domain_symbol, name, _, semantic_domain) = domain;
                        let payload = if domain_symbol.is_valid() {
                            FactPayload::DomainMembership {
                                value: *argument,
                                domain: Default::default(),
                                domain_symbol,
                                semantic_domain,
                            }
                        } else if let Some(permission) =
                            language_semantics::CarryPermission::from_name(&name)
                        {
                            FactPayload::CarryPermission {
                                value: *argument,
                                permission,
                            }
                        } else {
                            continue;
                        };
                        requirements.push((*argument, payload));
                    }
                }
                requirements
            }
        }
        pub(crate) use parameter_domains::call_parameter_domains;

        /*
            Identify the value a shared formal lends, whether the source explicitly
            lends shared or mutable access, or the shared loan was implicit. This only classifies the operand: callers still establish its
            type, storage, loan validity and any temporary-owner cleanup. In particular,
            a reference formal must never turn a value constructor into a reference
            carrier, and this rule grants no implicit mutable or write-only lending.
            A computed slice view lends a new descriptor, not an owned slice referent:
            preserve its complete expression and type so preparation and consumption
            name the same evaluated descriptor even when an explicit borrow encloses a
            range. A view of existing storage peels like any shared borrow.
        */
        pub(crate) fn shared_argument_referent(
            program: &dyn ::validation::CheckingInput,
            argument: ExpressionHandle,
            formal: TypeReferenceHandle,
        ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
            let TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                referee,
                ..
            } = program.type_reference_table.type_reference(formal)
            else {
                return None;
            };
            if validation::slice_view_shell(program, formal, &[]).is_some()
                && !view_argument_names_existing_place(program, argument)
            {
                return Some((argument, formal));
            }
            let value = match program.expression_table.expression(argument) {
                typed_trees::expression::ExpressionNode::Borrow(borrow)
                    if matches!(
                        borrow.access,
                        language_semantics::ReferenceAccess::Shared
                            | language_semantics::ReferenceAccess::Mutable
                    ) =>
                {
                    borrow.target
                }
                typed_trees::expression::ExpressionNode::Borrow(_) => return None,
                _ => argument,
            };
            Some((value, *referee))
        }

        /*
            Whether a view argument names existing storage, a stored view or a whole
            collection lent as one, rather than computing a new descriptor such as a
            range window or a call's view result. Existing storage lends through its
            place's loan route; only a computed formation is an evaluated operand with
            its own result. The place builder roots exactly the computed forms at their
            expression.
        */
        pub(crate) fn view_argument_names_existing_place(
            program: &dyn ::validation::CheckingInput,
            argument: ExpressionHandle,
        ) -> bool {
            if validation::slice_recast_source(program, argument).is_some() {
                return false;
            }
            validation::canonical_place_from_expression(program, argument)
                .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Symbol(_)))
        }
    }
    pub(crate) mod facts {
        use crate::published::labels::semantic_proof_obligation_kind;
        use checked_trees::ProofFacts;
        use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, QualificationEvidence};
        mod contracts {
            use super::points;
            use crate::flow::expressions;
            use crate::published::semantic::places::instantiate_call_contract_place;
            use arena::HandleSpan;
            use checked_trees::{
                ContractCallFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
                ContractProofFactRef, ProofFacts,
            };
            use facts::{
                Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FactRef, ProgramPoint,
                QualificationEvidence,
            };
            use typed_trees::proposition::PropositionLabels;
            mod case_observations {
                /*
                    Case contracts publish discriminator observations with their own lifetime.

                    The retained Boolean contract still supplies its proof obligation and full
                    identity. Its selected case is also a live value fact on the caller result,
                    so case consumers never interpret a callee's reserved `result` as caller
                    syntax or parse a rendered contract label. Caller writes invalidate this
                    fact through the ordinary place/context lifetime rules. Independent tag
                    contexts keep payload promises and whole-value snapshots from extending or
                    shortening the discriminator lifetime; dynamic selectors remain dependencies.
                */
                use super::places::contract_expression_place;
                use crate::published::facts::contract_occurrences::append_expression_occurrences;
                use crate::published::semantic::facts::append_observation_context;
                use crate::published::semantic::places::instantiate_call_contract_expression_place;
                use checked_trees::{
                    ContractCallFact, ContractProofFact, ContractProofFactKind,
                    ContractProofFactOwner,
                };
                use facts::{
                    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint,
                    QualificationEvidence,
                };

                pub(super) fn append_result(
                    program: &dyn validation::CheckingInput,
                    proof: &checked_trees::ProofFacts,
                    facts: &mut FactPlan,
                    call: &ContractCallFact,
                    contract: &ContractProofFact,
                    point: ProgramPoint,
                    evidence: QualificationEvidence,
                ) {
                    if contract.kind != ContractProofFactKind::Ensures {
                        return;
                    }
                    let (cases, callable, runtime) =
                        if let ContractProofFactOwner::CallSignature { application, .. } =
                            contract.owner
                        {
                            let Some(signature) =
                                crate::published::proof::contracts::call_signatures::signature(
                                    program,
                                    proof,
                                    contract.owner,
                                )
                            else {
                                return;
                            };
                            let retained = proof.call_contract_signatures.get(application);
                            let runtime = match retained.source {
                                checked_trees::CallContractSignatureSource::Runtime {
                                    contract,
                                    ..
                                } => Some(contract),
                                _ => None,
                            };
                            (
                                selected_signature_cases(program, signature, contract),
                                signature.symbol,
                                runtime,
                            )
                        } else {
                            let owns = match contract.owner {
                                ContractProofFactOwner::Machine { machine_symbol } => {
                                    machine_symbol == call.target_machine_symbol
                                }
                                ContractProofFactOwner::MachineState {
                                    machine_symbol,
                                    state_symbol,
                                } => {
                                    machine_symbol == call.target_machine_symbol
                                        && state_symbol == call.target_state_symbol
                                }
                                _ => false,
                            };
                            if !owns {
                                return;
                            }
                            let Some(machine) =
                                program.machine_by_symbol(call.target_machine_symbol)
                            else {
                                return;
                            };
                            let Some(state) =
                                program.state_in_machine(machine, call.target_state_symbol)
                            else {
                                return;
                            };
                            (
                                selected_cases(program, machine, state, contract),
                                machine.symbol,
                                None,
                            )
                        };
                    for (subject_expression, variant) in cases {
                        let subject = if let Some(contract) = runtime {
                            validation::runtime_reserved_result_place(
                                program,
                                contract,
                                subject_expression,
                            )
                        } else {
                            validation::reserved_result_place(program, subject_expression)
                        };
                        let Some(subject) = subject else {
                            continue;
                        };
                        if subject.machine_symbol != callable
                            || !detached_result_projection(program, &subject)
                        {
                            continue;
                        }
                        let Some(place) = instantiate_call_contract_expression_place(
                            program,
                            facts,
                            call,
                            subject_expression,
                        ) else {
                            continue;
                        };
                        let dependencies: Vec<_> =
                            selector_occurrences(program, subject_expression)
                                .into_iter()
                                .map(|expression| {
                                    instantiate_call_contract_expression_place(
                                        program, facts, call, expression,
                                    )
                                    .map(FactPlace::Place)
                                    .unwrap_or(FactPlace::Unknown)
                                })
                                .collect();
                        append_observation_context(
                            facts,
                            Fact {
                                place: FactPlace::Place(place),
                                point,
                                origin: FactOrigin::CallEnsures,
                                evidence,
                                payload: FactPayload::CaseValue {
                                    variant,
                                    value: true,
                                },
                            },
                            &dependencies,
                        );
                    }
                }

                /*
                    A completed result occurrence owns its by-value projections. A projection
                    through a reference still denotes mutable external storage and must retain
                    the original contract's referent dependencies rather than a detached tag.
                */
                fn detached_result_projection(
                    program: &dyn validation::CheckingInput,
                    subject: &validation::ReservedResultPlace,
                ) -> bool {
                    let root_type = if let Some(contract) = subject.runtime_contract {
                        program
                            .runtime_callable_contracts
                            .get(contract)
                            .signature
                            .return_type
                    } else {
                        let Some((_, root_type)) =
                            validation::reserved_result_owner(program, subject.root)
                        else {
                            return false;
                        };
                        root_type
                    };
                    (0..=subject.segments.len()).all(|length| {
                        validation::project_type_reference_from_segments(
                            program,
                            root_type,
                            &subject.segments[..length],
                        )
                        .is_some_and(|reference| {
                            validation::reference_shell(program, reference).is_none()
                        })
                    })
                }

                /*
                    Requirements are live assumptions only at their exact entry. Extract the
                    same positive case atoms as result contracts; never treat an ensures
                    declaration or an inherited foreign parameter as an entry observation.
                */
                pub(super) fn append_requirement(
                    program: &dyn validation::CheckingInput,
                    proof: &checked_trees::ProofFacts,
                    facts: &mut FactPlan,
                    contract: &ContractProofFact,
                    declaration: &Fact,
                ) {
                    if contract.kind != ContractProofFactKind::Requires
                        || contract.inherited_scope.is_some()
                    {
                        return;
                    }
                    let ProgramPoint::State {
                        machine_symbol,
                        state_symbol,
                    } = declaration.point
                    else {
                        return;
                    };
                    let Some((machine, state)) = validation::find_state(program, state_symbol)
                        .filter(|(machine, _)| machine.symbol == machine_symbol)
                    else {
                        return;
                    };
                    for (expression, variant) in selected_cases(program, machine, state, contract) {
                        let Some(place) =
                            contract_expression_place(program, proof, facts, contract, expression)
                        else {
                            continue;
                        };
                        let dependencies: Vec<_> = selector_occurrences(program, expression)
                            .into_iter()
                            .map(|expression| {
                                contract_expression_place(
                                    program, proof, facts, contract, expression,
                                )
                                .map(FactPlace::Place)
                                .unwrap_or(FactPlace::Unknown)
                            })
                            .collect();
                        append_observation_context(
                            facts,
                            Fact {
                                place: FactPlace::Place(place),
                                payload: FactPayload::CaseValue {
                                    variant,
                                    value: true,
                                },
                                ..*declaration
                            },
                            &dependencies,
                        );
                    }
                }

                /*
                    Only resolved builtin conjunction and exact case membership justify
                    decomposition. Authored operators and alternatives grant no case atom.
                */
                fn selected_cases(
                    program: &dyn validation::CheckingInput,
                    machine: &typed_trees::machine::Machine,
                    state: &typed_trees::state::State,
                    contract: &ContractProofFact,
                ) -> Vec<(
                    typed_trees::expression::ExpressionHandle,
                    symbols::SymbolHandle,
                )> {
                    let typed_trees::domain::ProofFact::Expression(expression) =
                        program.proof_facts.get(contract.fact)
                    else {
                        return Vec::new();
                    };
                    let mut cases = Vec::new();
                    let mut pending = vec![*expression];
                    while let Some(expression) = pending.pop() {
                        let Some(binary) = program.expression_table.binary_node(expression) else {
                            continue;
                        };
                        if binary.operator == typed_trees::expression::BinaryOperator::And
                            && validation::has_builtin_decomposed_guard_meaning(
                                program,
                                machine,
                                Some(state),
                                expression,
                            )
                        {
                            pending.extend([binary.left, binary.right]);
                            continue;
                        }
                        if let Some(case) = validation::applied_case_membership_variant(
                            program,
                            machine,
                            Some(state),
                            expression,
                            binary,
                        ) {
                            cases.push((binary.left, case));
                        }
                    }
                    cases
                }

                fn selected_signature_cases(
                    program: &dyn validation::CheckingInput,
                    signature: &typed_trees::signature::StateSignature,
                    contract: &ContractProofFact,
                ) -> Vec<(
                    typed_trees::expression::ExpressionHandle,
                    symbols::SymbolHandle,
                )> {
                    let typed_trees::domain::ProofFact::Expression(expression) =
                        program.proof_facts.get(contract.fact)
                    else {
                        return Vec::new();
                    };
                    let parameters = program.state_signature_parameters(signature);
                    let mut cases = Vec::new();
                    let mut pending = vec![*expression];
                    while let Some(expression) = pending.pop() {
                        let Some(binary) = program.expression_table.binary_node(expression) else {
                            continue;
                        };
                        if binary.operator == typed_trees::expression::BinaryOperator::And
                            && validation::has_builtin_parameter_bound_expression_meaning(
                                program,
                                signature.symbol,
                                parameters,
                                expression,
                            )
                        {
                            pending.extend([binary.left, binary.right]);
                            continue;
                        }
                        if let Some(case) = validation::applied_declaration_case_membership_variant(
                            program,
                            signature.symbol,
                            parameters,
                            expression,
                            binary,
                        ) {
                            cases.push((binary.left, case));
                        }
                    }
                    cases
                }

                /*
                    Index operands choose which discriminator the observation names. They keep
                    whole-value dependencies even though writes below the selected sum do not.
                */
                fn selector_occurrences(
                    program: &dyn validation::CheckingInput,
                    subject: typed_trees::expression::ExpressionHandle,
                ) -> Vec<typed_trees::expression::ExpressionHandle> {
                    let mut occurrences = Vec::new();
                    append_expression_occurrences(program, subject, &mut occurrences);
                    occurrences.retain(|occurrence| *occurrence != subject);
                    occurrences
                }
            }
            mod parameters {
                /*
                    Bind authored contract formals through their retained conformance edge.
                    The authored row keeps declaration identities; the call supplies the
                    satisfying state's arguments. Both place substitution and operand-epoch
                    selection must use this same correspondence, including adapter offsets.
                */
                use crate::published::places::place_at;
                use checked_trees::{ContractProofFact, ProofFacts};
                use symbols::SymbolHandle;

                /*
                    Resolve the captured source of a formal projection. Named arguments keep
                    their field path; constructed arguments select the corresponding authored
                    child. Publication and checking share this exact place correspondence.
                */
                pub(crate) fn projected_argument_place(
                    program: &dyn ::validation::CheckingInput,
                    state: SymbolHandle,
                    statement: usize,
                    expression: typed_trees::expression::ExpressionHandle,
                    reference: typed_trees::types::TypeReferenceHandle,
                    path: &[facts::PlaceSegment],
                ) -> Option<validation::CanonicalPlace> {
                    let projections = crate::flow::literal_expression_projections(
                        program, expression, reference, path, false,
                    )?;
                    let [projection] = projections.as_slice() else {
                        return None;
                    };
                    let mut place = place_at(program, state, statement, projection.expression)?;
                    place.extend_segments(&projection.remaining);
                    Some(place)
                }

                pub(crate) fn contract_parameter_position(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    contract: &ContractProofFact,
                    target: SymbolHandle,
                    declared: SymbolHandle,
                ) -> Result<Option<usize>, ()> {
                    if let checked_trees::ContractProofFactOwner::RuntimeCallableContract {
                        contract: handle,
                    } = contract.owner
                    {
                        if target.is_valid()
                            || contract.inherited_scope.is_some()
                            || !program.runtime_callable_contracts.is_valid(handle)
                        {
                            return Err(());
                        }
                        let row = program.runtime_callable_contracts.get(handle);
                        return Ok(typed_trees::signature::state_parameter_position(
                            program.state_signature_parameters(&row.signature),
                            declared,
                        ));
                    }
                    if let checked_trees::ContractProofFactOwner::CallSignature {
                        application,
                        ..
                    } = contract.owner
                    {
                        let signature =
                            crate::published::proof::contracts::call_signatures::signature(
                                program,
                                proof,
                                contract.owner,
                            )
                            .ok_or(())?;
                        let required = program.state_signature_parameters(signature);
                        let Some(position) =
                            typed_trees::signature::state_parameter_position(required, declared)
                        else {
                            return Ok(None);
                        };
                        let retained = proof.call_contract_signatures.get(application);
                        return match retained.source {
                            checked_trees::CallContractSignatureSource::Runtime { .. }
                                if !target.is_valid() =>
                            {
                                Ok(Some(position))
                            }
                            checked_trees::CallContractSignatureSource::Requirement { .. } => {
                                let parameters =
                                    validation::call_target_parameters(program, target).ok_or(())?;
                                (required.len() == parameters.len() && position < parameters.len())
                                    .then_some(Some(position))
                                    .ok_or(())
                            }
                            _ => Err(()),
                        };
                    }
                    let parameters =
                        validation::call_target_parameters(program, target).ok_or(())?;
                    if let Some(position) =
                        typed_trees::signature::state_parameter_position(parameters, declared)
                    {
                        return Ok(Some(position));
                    }
                    if contract.inherited_scope.is_none() {
                        return Ok(None);
                    }
                    let (trait_definition, requirement, satisfier_state, _) =
                        super::inherited_contract_scope_parts(program, proof, contract).ok_or(())?;
                    let required = program.state_signature_parameters(requirement);
                    let actual = validation::inherited_satisfier_parameters(
                        program,
                        trait_definition,
                        requirement,
                        satisfier_state,
                    )
                    .ok_or(())?;
                    let Some(position) =
                        typed_trees::signature::state_parameter_position(required, declared)
                    else {
                        return Ok(None);
                    };
                    let actual = actual.get(position).ok_or(())?.symbol;
                    typed_trees::signature::state_parameter_position(parameters, actual)
                        .map(Some)
                        .ok_or(())
                }
            }
            mod payload {
                use crate::published::labels::semantic_contract_fact_kind;
                use arena::Handle;
                use checked_trees::ContractProofFact;
                use facts::FactPayload;
                use language_semantics::CarryPermission;
                use symbols::JoinedPath;
                use typed_trees::domain::ProofFact;

                pub(super) fn semantic_contract_payload(
                    program: &dyn ::validation::CheckingInput,
                    contract: &ContractProofFact,
                ) -> FactPayload {
                    let kind = semantic_contract_fact_kind(contract.kind);
                    match program.proof_facts.get(contract.fact) {
                        ProofFact::Expression(expression) => {
                            FactPayload::ContractBooleanExpression {
                                kind,
                                fact: contract.fact,
                                expression: *expression,
                                instantiated: Handle::invalid(),
                            }
                        }
                        ProofFact::Membership(membership) => {
                            let carry_permission =
                                program.domain_path_members(membership.domain).joined_path();
                            if let Some(permission) = CarryPermission::from_name(&carry_permission)
                            {
                                return FactPayload::ContractCarryPermission {
                                    kind,
                                    fact: contract.fact,
                                    value: membership.value,
                                    permission,
                                };
                            }
                            FactPayload::ContractDomainMembership {
                                kind,
                                fact: contract.fact,
                                value: membership.value,
                                domain: membership.domain,
                                domain_symbol: membership.domain_symbol,
                                semantic_domain: membership.semantic_domain,
                            }
                        }
                        ProofFact::Proposition(application) => {
                            FactPayload::ContractPropositionApplication {
                                kind,
                                fact: contract.fact,
                                proposition: application.proposition,
                                instantiated: Handle::invalid(),
                            }
                        }
                    }
                }
            }
            mod places {
                use crate::published::facts::contract_occurrences::fact_referenced_occurrences_in_state;
                use crate::published::semantic::places::canonical_place_to_fact_place_in_state;
                use crate::published::semantic::places::instantiate_outcome_contract_expression_place;
                use validation::call_target_parameters;
                use validation::effective_member_symbol;

                use crate::flow::expressions;
                use crate::published::semantic::places::{
                    append_place_segment, resolve_place_member_symbol,
                };
                use checked_trees::expression::{ExpressionHandle, ExpressionNode};
                use checked_trees::{
                    ContractProofFact, ContractProofFactKind, ContractProofFactOwner as FactOwner,
                    ProofFacts,
                };
                use facts::PlaceHandle;
                use facts::{FactPlace, FactPlan};
                use language_core::is_self_receiver;
                use symbols::SymbolHandle;

                pub(crate) fn contract_fact_place(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    facts: &mut FactPlan,
                    contract: &ContractProofFact,
                ) -> FactPlace {
                    match program.proof_facts.get(contract.fact) {
                        typed_trees::domain::ProofFact::Expression(expression) => FactPlace::Place(
                            contract_expression_place(program, proof, facts, contract, *expression)
                                .unwrap_or_else(|| {
                                    facts.append_place_from_expression(program, *expression)
                                }),
                        ),
                        typed_trees::domain::ProofFact::Membership(membership) => FactPlace::Place(
                            contract_expression_place(
                                program,
                                proof,
                                facts,
                                contract,
                                membership.value,
                            )
                            .unwrap_or_else(|| {
                                facts.append_place_from_expression(program, membership.value)
                            }),
                        ),
                        typed_trees::domain::ProofFact::Proposition(application) => {
                            FactPlace::Place(facts.append_symbol_place(application.proposition))
                        }
                    }
                }

                /*
                    Returns every place read by a contract fact. Requires retain their actual
                    subject and separate storage-dependency rows in one context, so mutating any
                    operand invalidates the whole assumption without turning each dependency
                    into a new membership subject.
                */
                pub(crate) fn contract_fact_dependency_places(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    facts: &mut FactPlan,
                    contract: &ContractProofFact,
                ) -> Vec<PlaceHandle> {
                    let mut places = Vec::new();
                    let state = match (contract.kind, contract.owner) {
                        (
                            ContractProofFactKind::Requires,
                            FactOwner::MachineState { state_symbol, .. },
                        ) => Some(state_symbol),
                        (
                            ContractProofFactKind::Requires,
                            FactOwner::Machine { machine_symbol },
                        ) => program
                            .machine(machine_symbol)
                            .and_then(|machine| program.machine_states(machine).first())
                            .map(|state| state.symbol),
                        _ => None,
                    };
                    for expression in
                        fact_referenced_occurrences_in_state(program, contract.fact, state)
                    {
                        if let Some(place) =
                            contract_expression_place(program, proof, facts, contract, expression)
                        {
                            push_unique_place(facts, &mut places, place);
                        }
                    }
                    places
                }

                /*
                    Resolve the structured validity inputs retained for an outcome-specific
                    caller row. Contract occurrences remain typed expression handles until this
                    point; parameter and `result` paths are instantiated through the exact
                    source call rather than reconstructed from normalized display labels.
                */
                pub(crate) fn outcome_specific_fact_dependency_places(
                    program: &dyn ::validation::CheckingInput,
                    facts: &mut FactPlan,
                    proof: &ProofFacts,
                    arm: &checked_trees::OutcomeSpecificArmFact,
                    row: &checked_trees::OutcomeSpecificArmRowFact,
                ) -> Vec<PlaceHandle> {
                    let mut places = Vec::new();
                    if let Some(result) = canonical_place_to_fact_place_in_state(
                        program,
                        facts,
                        arm.caller_state_symbol,
                        arm.statement_index,
                        row.validity.result_occurrence,
                    ) {
                        push_unique_place(facts, &mut places, result);
                    }

                    let mut calls = proof.contract_calls.iter().filter_map(|(_, call)| {
                        (call.caller_machine_symbol == arm.caller_machine_symbol
                            && call.caller_state_symbol == arm.caller_state_symbol
                            && call.statement_index == arm.result_call_statement_index
                            && call.call_ordinal == 0)
                            .then_some(call)
                    });
                    let Some(call) = calls.next() else {
                        return places;
                    };
                    if calls.next().is_some() {
                        return places;
                    }

                    let interface_occurrences = row
                        .validity
                        .evidence_interface_scope
                        .as_ref()
                        .map(|scope| scope.retained_occurrences.as_slice())
                        .unwrap_or(&[]);
                    for expression in row
                        .validity
                        .referenced_occurrences
                        .iter()
                        .chain(interface_occurrences)
                        .map(|occurrence| &occurrence.occurrence)
                    {
                        if let Some(place) = instantiate_outcome_contract_expression_place(
                            program,
                            facts,
                            call,
                            arm.statement_index,
                            row.validity.result_occurrence,
                            *expression,
                        ) {
                            push_unique_place(facts, &mut places, place);
                        }
                    }
                    places
                }

                fn push_unique_place(
                    facts: &FactPlan,
                    places: &mut Vec<PlaceHandle>,
                    candidate: PlaceHandle,
                ) {
                    let candidate_place = facts.places.get(candidate);
                    let candidate_segments =
                        facts.place_segments.span_or_empty(candidate_place.segments);
                    let duplicate = places.iter().any(|place| {
                        let place = facts.places.get(*place);
                        place.root == candidate_place.root
                            && facts.place_segments.span_or_empty(place.segments)
                                == candidate_segments
                    });
                    if !duplicate {
                        places.push(candidate);
                    }
                }

                pub(super) fn contract_expression_place(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    facts: &mut FactPlan,
                    contract: &ContractProofFact,
                    expression: ExpressionHandle,
                ) -> Option<PlaceHandle> {
                    if !expression.is_valid() {
                        return None;
                    }

                    let runtime_contract = match contract.owner {
                        FactOwner::RuntimeCallableContract { contract } => Some(contract),
                        FactOwner::CallSignature { application, .. }
                            if proof.call_contract_signatures.is_valid(application) =>
                        {
                            match proof.call_contract_signatures.get(application).source {
                                checked_trees::CallContractSignatureSource::Runtime {
                                    contract,
                                    ..
                                } => Some(contract),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    let result = if let Some(contract) = runtime_contract {
                        validation::runtime_reserved_result_place(program, contract, expression)
                    } else {
                        validation::reserved_result_place(program, expression)
                    };
                    if let Some(result) = result {
                        let owner_matches = match contract.owner {
                            FactOwner::RuntimeCallableContract { contract } => {
                                result.runtime_contract == Some(contract)
                            }
                            FactOwner::Machine { machine_symbol }
                            | FactOwner::MachineState { machine_symbol, .. } => {
                                machine_symbol == result.machine_symbol
                            }
                            FactOwner::StateSignature { state_symbol, .. }
                            | FactOwner::CallSignature { state_symbol, .. } => {
                                state_symbol == result.machine_symbol
                            }
                            _ => false,
                        };
                        if contract.kind != ContractProofFactKind::Ensures || !owner_matches {
                            return None;
                        }
                        let origin = super::super::points::contract_fact_origin(contract)?;
                        for member in &result.members {
                            facts.contract_member_projections.insert(
                                facts::ContractMemberProjection {
                                    origin,
                                    fact: contract.fact,
                                    expression: member.expression,
                                    receiver_type: member.receiver_type,
                                    authored_field: member.authored_field,
                                    applied_field: member.applied_field,
                                    applied_case: member.applied_case,
                                    authored_case: member.authored_case,
                                },
                            );
                        }
                        let mut place = facts.append_expression_place(result.root);
                        for segment in result.segments {
                            place = append_place_segment(facts, place, segment);
                        }
                        return Some(place);
                    }

                    match program.expression_table.expression(expression) {
                        ExpressionNode::Borrow(inner) => {
                            contract_expression_place(program, proof, facts, contract, inner.target)
                        }
                        ExpressionNode::Name(path) => {
                            contract_name_path_place(program, proof, facts, contract, path)
                        }
                        ExpressionNode::Member(member) => {
                            let receiver = contract_expression_place(
                                program,
                                proof,
                                facts,
                                contract,
                                member.receiver,
                            )
                            .unwrap_or_else(|| {
                                facts.append_place_from_expression(program, member.receiver)
                            });
                            let symbol = {
                                let symbol =
                                    effective_member_symbol(program, member.receiver, member);
                                if symbol.is_valid() {
                                    symbol
                                } else {
                                    resolve_place_member_symbol(
                                        program,
                                        facts,
                                        receiver,
                                        member.member.as_str(),
                                    )
                                    .unwrap_or_else(SymbolHandle::invalid)
                                }
                            };
                            let receiver = if let Some(variant) =
                                facts::payload_variant_for_field(program, symbol)
                            {
                                append_place_segment(
                                    facts,
                                    receiver,
                                    facts::PlaceSegment::Case { variant },
                                )
                            } else {
                                receiver
                            };
                            Some(append_place_segment(
                                facts,
                                receiver,
                                facts::PlaceSegment::Field { symbol },
                            ))
                        }
                        ExpressionNode::Indexed(indexed) => {
                            let collection = contract_expression_place(
                                program,
                                proof,
                                facts,
                                contract,
                                indexed.collection,
                            )
                            .unwrap_or_else(|| {
                                facts.append_place_from_expression(program, indexed.collection)
                            });
                            Some(append_place_segment(
                                facts,
                                collection,
                                validation::index_place_segment(program, indexed.index),
                            ))
                        }
                        _ => None,
                    }
                }

                fn contract_name_path_place(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    facts: &mut FactPlan,
                    contract: &ContractProofFact,
                    path: &typed_trees::expression::TableNamePath,
                ) -> Option<PlaceHandle> {
                    let members = program.expression_table.name_path_members(path.members);
                    let member_symbols =
                        expressions(program).name_path_member_symbols(path.member_symbols);

                    let (mut place, start_index) = if members
                        .first()
                        .is_some_and(|member| member.is_self_receiver())
                    {
                        return None;
                    } else if path.head_symbol.is_valid() {
                        (facts.append_symbol_place(path.head_symbol), 1usize)
                    } else {
                        let parameter_symbol = contract_owner_parameter_symbol(
                            program,
                            proof,
                            contract.owner,
                            path,
                            members.first(),
                        )?;
                        (facts.append_symbol_place(parameter_symbol), 1usize)
                    };

                    for (offset, member_name) in members.iter().skip(start_index).enumerate() {
                        let member_symbol = member_symbols
                            .get(offset + start_index)
                            .copied()
                            .filter(|symbol| symbol.is_valid())
                            .or_else(|| {
                                resolve_place_member_symbol(
                                    program,
                                    facts,
                                    place,
                                    member_name.as_str(),
                                )
                            })
                            .unwrap_or_else(SymbolHandle::invalid);
                        if let Some(variant) =
                            facts::payload_variant_for_field(program, member_symbol)
                        {
                            place = append_place_segment(
                                facts,
                                place,
                                facts::PlaceSegment::Case { variant },
                            );
                        }
                        place = append_place_segment(
                            facts,
                            place,
                            facts::PlaceSegment::Field {
                                symbol: member_symbol,
                            },
                        );
                    }

                    Some(place)
                }

                /*
                    Resolves a contract name-path head that names one of the owner's
                    parameters (for example `item` in `requires item in Item::Ready`). Returns
                    the parameter symbol so the fact place is rooted at the parameter, which
                    is how call-site obligations instantiate the same contract.
                */
                fn contract_owner_parameter_symbol(
                    program: &dyn ::validation::CheckingInput,
                    proof: &ProofFacts,
                    owner: FactOwner,
                    path: &typed_trees::expression::TableNamePath,
                    head_name: Option<&typed_trees::name::Identifier>,
                ) -> Option<SymbolHandle> {
                    let head_name = head_name?.as_str();
                    if is_self_receiver(head_name) {
                        return None;
                    }

                    contract_owner_parameter(program, proof, owner, |parameter| {
                        parameter.name.as_str() == head_name
                            || (path.symbol.is_valid() && parameter.symbol == path.symbol)
                    })
                    .map(|parameter| parameter.symbol)
                }

                fn contract_owner_parameter<'program>(
                    program: &'program dyn ::validation::CheckingInput,
                    proof: &'program ProofFacts,
                    owner: FactOwner,
                    mut matches: impl FnMut(&typed_trees::signature::StateParameter) -> bool,
                ) -> Option<&'program typed_trees::signature::StateParameter> {
                    if matches!(
                        owner,
                        FactOwner::CallSignature { .. } | FactOwner::RuntimeCallableContract { .. }
                    ) {
                        let signature =
                            crate::published::proof::contracts::call_signatures::signature(
                                program, proof, owner,
                            )?;
                        return program
                            .state_signature_parameters(signature)
                            .iter()
                            .find(|parameter| matches(parameter));
                    }
                    let state_symbol = match owner {
                        FactOwner::MachineState { state_symbol, .. }
                        | FactOwner::StateSignature { state_symbol, .. } => state_symbol,
                        FactOwner::Machine { machine_symbol } => {
                            return program.machine(machine_symbol).and_then(|machine| {
                                program.machine_states(machine).first().and_then(|state| {
                                    program
                                        .state_parameters(state)
                                        .iter()
                                        .find(|parameter| matches(parameter))
                                })
                            });
                        }
                        FactOwner::OperatorDeclaration { operator_symbol } => {
                            return typed_trees::operator::declaration_by_symbol(
                                program,
                                operator_symbol,
                            )
                            .and_then(|operator| {
                                program
                                    .operator_parameters(operator)
                                    .iter()
                                    .find(|parameter| matches(parameter))
                            });
                        }
                        FactOwner::Unknown
                        | FactOwner::OperatorUse { .. }
                        | FactOwner::RuntimeCallableContract { .. }
                        | FactOwner::CallSignature { .. } => {
                            return None;
                        }
                    };

                    call_target_parameters(program, state_symbol)?
                        .iter()
                        .find(|parameter| matches(parameter))
                }
            }
            mod scopes {
                /*
                    Resolve inherited obligations through their retained declaration and application.

                    A public parameter or result keeps its authored identity when an ordinary
                    implementation owes its contract. Consumers share this exact application
                    lookup; declaration names and equal carrier shapes cannot select an edge.
                */
                use checked_trees::{ContractProofFact, ContractProofFactOwner, ProofFacts};
                use typed_trees::signature::StateSignature;
                use typed_trees::trait_definition::AppliedTraitScope;
                use typed_trees::type_identity::TypeIdentityRequest;
                use typed_trees::types::TypeReferenceHandle;

                /*
                    Resolve one inherited contract fact's migrated proposition endpoint: the
                    declaring trait, its requirement signature, and the satisfying state the
                    fact is owned by. `None` fails closed -- the fact keeps its authored schema
                    identity rather than publishing a partial substitution.
                */
                pub(crate) fn inherited_contract_scope_parts<'program>(
                    program: &'program dyn ::validation::CheckingInput,
                    proof: &'program ProofFacts,
                    contract: &ContractProofFact,
                ) -> Option<(
                    &'program typed_trees::trait_definition::TraitDefinition,
                    &'program typed_trees::signature::StateSignature,
                    &'program typed_trees::state::State,
                    &'program checked_trees::InheritedContractScope,
                )> {
                    let scope = proof
                        .inherited_contract_scopes
                        .get(contract.inherited_scope?);
                    let trait_definition =
                        program.trait_definition_by_symbol(scope.declaring_trait)?;
                    let requirement = program
                        .trait_machine_signature_by_symbol(trait_definition, scope.requirement)?;
                    let ContractProofFactOwner::MachineState { state_symbol, .. } = contract.owner
                    else {
                        return None;
                    };
                    let satisfier_state = program
                        .machines()
                        .iter()
                        .find_map(|machine| program.state_in_machine(machine, state_symbol))?;
                    Some((trait_definition, requirement, satisfier_state, scope))
                }

                /*
                    A declaration and its complete applied telescope remain distinct. Select
                    the exact retained conformance, rejecting ambiguous inherited applications.
                */
                pub(crate) fn applied_inherited_requirement<'program>(
                    program: &'program dyn validation::CheckingInput,
                    requirement: &'program StateSignature,
                    scope: &checked_trees::InheritedContractScope,
                ) -> Option<&'program StateSignature> {
                    let owner = program.machine_by_symbol(scope.conformance_machine)?;
                    let mut application = None;
                    for conformance in program.machine_trait_conformances(owner) {
                        let root = AppliedTraitScope::from_machine_conformance(
                            program.typed_program(),
                            owner,
                            conformance,
                        )?;
                        let Some(candidate) =
                            root.declaring_scope(program.typed_program(), scope.declaring_trait)
                        else {
                            continue;
                        };
                        if let Some(known) = &application
                            && !candidate.same_application(program.typed_program(), known)
                        {
                            return None;
                        }
                        application = Some(candidate);
                    }
                    let application = application?;
                    let binders = program.trait_type_parameters(application.declaration);
                    if binders.len() != scope.trait_arguments.len()
                        || !binders
                            .iter()
                            .zip(&scope.trait_arguments)
                            .all(|(binder, actual)| {
                                same_type(
                                    program,
                                    application
                                        .parameter_argument(program.typed_program(), binder.symbol),
                                    *actual,
                                )
                            })
                    {
                        return None;
                    }
                    program
                        .applied_callable_signature(&application, requirement.symbol)
                        .or_else(|| {
                            (requirement.type_parameters.is_empty()
                                && application.substitutions.is_empty()
                                && application.declaration.lifetime_parameters.is_empty())
                            .then_some(requirement)
                        })
                }

                fn same_type(
                    program: &dyn validation::CheckingInput,
                    left: TypeReferenceHandle,
                    right: TypeReferenceHandle,
                ) -> bool {
                    left.is_valid()
                        && right.is_valid()
                        && program.type_identity(TypeIdentityRequest::ordinary(left))
                            == program.type_identity(TypeIdentityRequest::ordinary(right))
                }
            }
            pub(crate) use parameters::{contract_parameter_position, projected_argument_place};
            pub(crate) use places::contract_fact_place;
            pub(crate) use scopes::{
                applied_inherited_requirement, inherited_contract_scope_parts,
            };

            use payload::semantic_contract_payload;

            /* `extension` names storage strictly inside `prefix`. */
            fn fact_place_strictly_extends(
                facts: &FactPlan,
                extension: FactPlace,
                prefix: facts::PlaceHandle,
            ) -> bool {
                let FactPlace::Place(extension) = extension else {
                    return false;
                };
                let extension = facts.places.get(extension);
                let prefix = facts.places.get(prefix);
                if extension.root != prefix.root {
                    return false;
                }
                let extension_segments = facts.place_segments.span_or_empty(extension.segments);
                let prefix_segments = facts.place_segments.span_or_empty(prefix.segments);
                extension_segments.len() > prefix_segments.len()
                    && extension_segments.starts_with(prefix_segments)
            }

            use points::{contract_fact_origin, contract_fact_point};

            pub(super) fn append_contract_semantic_facts(
                program: &dyn ::validation::CheckingInput,
                proof: &ProofFacts,
                operators: &checked_trees::CheckedOperatorFacts,
                facts: &mut FactPlan,
            ) {
                let mut semantic_handles = Vec::with_capacity(proof.contract_facts.len());

                for (contract_handle, contract) in proof.contract_facts.iter() {
                    let Some(point) = contract_fact_point(program, contract) else {
                        continue;
                    };
                    let Some(origin) = contract_fact_origin(contract) else {
                        continue;
                    };
                    let place = contract_fact_place(program, proof, facts, contract);
                    let mut payload = semantic_contract_payload(program, contract);
                    instantiate_inherited_contract_payload(
                        program,
                        proof,
                        facts,
                        contract,
                        &mut payload,
                    );
                    let declaration_fact = Fact {
                        place,
                        point,
                        origin,
                        evidence: QualificationEvidence::default(),
                        payload,
                    };
                    seed_current_requirement(program, operators, facts, &declaration_fact);
                    case_observations::append_requirement(
                        program,
                        proof,
                        facts,
                        contract,
                        &declaration_fact,
                    );
                    let fact = match contract.kind {
                        ContractProofFactKind::Requires => {
                            let dependency_places = places::contract_fact_dependency_places(
                                program, proof, facts, contract,
                            );
                            let fact = facts.append_fact(declaration_fact);
                            let mut refs = HandleSpan::empty();
                            facts.append_ref(&mut refs, fact);
                            for place in dependency_places {
                                if facts.fact_place_equals(declaration_fact.place, place)
                                    || fact_place_strictly_extends(
                                        facts,
                                        declaration_fact.place,
                                        place,
                                    )
                                {
                                    continue;
                                }
                                let dependency = facts.append_fact(Fact {
                                    place: FactPlace::Place(place),
                                    payload: FactPayload::StorageDependency { dependent: fact },
                                    ..declaration_fact
                                });
                                facts.append_ref(&mut refs, dependency);
                            }
                            facts.append_context(point, refs);
                            fact
                        }
                        ContractProofFactKind::Ensures => {
                            let fact = facts.append_fact(declaration_fact);
                            for place in places::contract_fact_dependency_places(
                                program, proof, facts, contract,
                            ) {
                                if !facts.fact_place_equals(declaration_fact.place, place) {
                                    facts.append_fact(Fact {
                                        place: FactPlace::Place(place),
                                        ..declaration_fact
                                    });
                                }
                            }
                            fact
                        }
                    };
                    let contract_index = usize::try_from(contract_handle.arena_index())
                        .expect("contract fact handle index overflow");
                    while semantic_handles.len() <= contract_index {
                        semantic_handles.push(None);
                    }
                    semantic_handles[contract_index] = Some(fact);
                }

                for (_, call) in proof.contract_calls.iter() {
                    let mut combined_ref_values = Vec::new();
                    let mut requires = HandleSpan::empty();
                    append_call_semantic_contract_refs(
                        program,
                        proof,
                        facts,
                        call,
                        proof.contract_fact_refs.span_or_empty(call.requires),
                        FactOrigin::CallRequires,
                        ProgramPoint::CallRequires {
                            machine_symbol: call.caller_machine_symbol,
                            state_symbol: call.caller_state_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        },
                        &mut requires,
                    );
                    append_parameter_domain_requirements(program, facts, call, &mut requires);
                    combined_ref_values.extend(facts.refs.span_or_empty(requires).iter().copied());
                    if !requires.is_empty() {
                        facts.append_context(
                            ProgramPoint::CallRequires {
                                machine_symbol: call.caller_machine_symbol,
                                state_symbol: call.caller_state_symbol,
                                statement_index: call.statement_index,
                                call_ordinal: call.call_ordinal,
                            },
                            requires,
                        );
                    }

                    let point = ProgramPoint::CallEnsures {
                        machine_symbol: call.caller_machine_symbol,
                        state_symbol: call.caller_state_symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                    };
                    for source_ref in proof.contract_fact_refs.span_or_empty(call.ensures) {
                        let mut ensures = HandleSpan::empty();
                        append_call_semantic_contract_refs(
                            program,
                            proof,
                            facts,
                            call,
                            std::slice::from_ref(source_ref),
                            FactOrigin::CallEnsures,
                            point,
                            &mut ensures,
                        );
                        combined_ref_values
                            .extend(facts.refs.span_or_empty(ensures).iter().copied());
                        if !ensures.is_empty() {
                            facts.append_context(point, ensures);
                        }
                    }
                    let mut refs = HandleSpan::empty();
                    for fact_ref in combined_ref_values {
                        facts.refs.append_to_span(&mut refs, fact_ref);
                    }
                    facts.append_symbol_set(call.target_machine_symbol, refs);
                }

                for (_, invocation) in proof.proof_output_calls.iter() {
                    if invocation.runtime_call.is_some() {
                        continue;
                    }
                    let point = ProgramPoint::CallEnsures {
                        machine_symbol: invocation.caller_machine_symbol,
                        state_symbol: invocation.caller_state_symbol,
                        statement_index: invocation.statement_index,
                        call_ordinal: 0,
                    };
                    let mut refs = HandleSpan::empty();
                    for output in &invocation.outputs {
                        let Some((contract_handle, contract)) =
                            proof.contract_facts.iter().find(|(_, contract)| {
                                contract.evidence_term == Some(output.callee_output)
                            })
                        else {
                            continue;
                        };
                        let contract_index = usize::try_from(contract_handle.arena_index())
                            .expect("contract fact handle index overflow");
                        let Some(Some(source_fact)) = semantic_handles.get(contract_index) else {
                            continue;
                        };
                        let mut source = *facts.facts.get(*source_fact);
                        if let FactPayload::ContractPropositionApplication {
                            ref mut instantiated,
                            ..
                        } = source.payload
                        {
                            *instantiated = facts
                                .append_instantiated_expression(output.instantiated_label.clone());
                        }
                        let Some(evidence) =
                            crate::published::facts::qualification_evidence::call_contract_evidence(
                                program,
                                facts,
                                invocation.target_machine_symbol,
                                invocation.target_state_symbol,
                                contract,
                                source.payload,
                                true,
                            )
                        else {
                            continue;
                        };
                        let fact = facts.append_fact(Fact {
                            point,
                            origin: FactOrigin::CallEnsures,
                            evidence,
                            ..source
                        });
                        facts.append_ref(&mut refs, fact);
                    }
                    if !refs.is_empty() {
                        facts.append_context(point, refs);
                    }
                }

                for (_, arm) in proof.outcome_specific_arms.iter() {
                    let point = ProgramPoint::Statement {
                        machine_symbol: arm.caller_machine_symbol,
                        state_symbol: arm.caller_state_symbol,
                        statement_index: arm.statement_index,
                    };
                    for row in &arm.rows {
                        if row.instantiated_proposition.is_some()
                            && row.captured_argument_identities.is_none()
                        {
                            continue;
                        }
                        let mut refs = HandleSpan::empty();
                        let guarantee = proof.outcome_specific_guarantees.get(row.guarantee);
                        let contract = ContractProofFact {
                            kind: ContractProofFactKind::Ensures,
                            owner: ContractProofFactOwner::MachineState {
                                machine_symbol: arm.caller_machine_symbol,
                                state_symbol: arm.caller_state_symbol,
                            },
                            fact: guarantee.fact,
                            evidence_term: row.selected_term,
                            nominal_argument_roots: None,
                            qualification_authorization: None,
                            inherited_scope: None,
                        };
                        let mut payload = semantic_contract_payload(program, &contract);
                        if let Some(identity) = &row.instantiated_label {
                            let instantiated =
                                facts.append_instantiated_expression(identity.clone());
                            match &mut payload {
                                FactPayload::ContractBooleanExpression {
                                    instantiated: slot,
                                    ..
                                }
                                | FactPayload::ContractPropositionApplication {
                                    instantiated: slot,
                                    ..
                                } => *slot = instantiated,
                                _ => {}
                            }
                        }
                        let dependency_places = places::outcome_specific_fact_dependency_places(
                            program, facts, proof, arm, row,
                        );
                        if dependency_places.is_empty() {
                            let fact = facts.append_fact(Fact {
                                place: FactPlace::Unknown,
                                point,
                                origin: FactOrigin::CallEnsures,
                                evidence: QualificationEvidence::default(),
                                payload,
                            });
                            facts.append_ref(&mut refs, fact);
                        } else {
                            for place in dependency_places {
                                let fact = facts.append_fact(Fact {
                                    place: FactPlace::Place(place),
                                    point,
                                    origin: FactOrigin::CallEnsures,
                                    evidence: QualificationEvidence::default(),
                                    payload,
                                });
                                facts.append_ref(&mut refs, fact);
                            }
                        }
                        if !refs.is_empty() {
                            facts.append_context(point, refs);
                        }
                    }
                }

                for (_, exit) in proof.contract_exits.iter() {
                    let mut refs = HandleSpan::empty();
                    append_semantic_contract_refs(
                        proof,
                        facts,
                        &semantic_handles,
                        exit.ensures,
                        &mut refs,
                    );
                    facts.append_context(
                        ProgramPoint::Exit {
                            machine_symbol: exit.machine_symbol,
                            state_symbol: exit.state_symbol,
                            statement_index: exit.statement_index,
                            transition_target: exit.transition_target,
                        },
                        refs,
                    );
                    facts.append_symbol_set(exit.machine_symbol, refs);
                }
            }

            /*
                An authored precondition is an assumption only at its exact arrival, with
                the same storage-dependent lifetime as every other current predicate.
                Its declaration row remains for contract identity and admission; this
                checked formula lets ordinary value consumers use the whole proposition.
                Ensures declarations are obligations and never enter this premise path.
            */
            fn seed_current_requirement(
                program: &dyn ::validation::CheckingInput,
                operators: &checked_trees::CheckedOperatorFacts,
                facts: &mut FactPlan,
                fact: &Fact,
            ) {
                let FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Requires,
                    expression,
                    instantiated,
                    ..
                } = fact.payload
                else {
                    return;
                };
                if instantiated.is_valid() {
                    return;
                }
                let ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                } = fact.point
                else {
                    return;
                };
                let Some((machine, state)) = validation::find_state(program, state_symbol)
                    .filter(|(machine, _)| machine.symbol == machine_symbol)
                else {
                    return;
                };
                let Some(expression) = crate::values::lower_state_scalar_contract_predicate(
                    program, operators, machine, state, expression, &mut 4096,
                ) else {
                    return;
                };
                let Some(predicate) = crate::published::facts::boolean_predicates::bind_parameters(
                    program,
                    machine_symbol,
                    state_symbol,
                    program.state_parameters(state),
                    None,
                    expression,
                ) else {
                    return;
                };
                crate::published::facts::boolean_predicates::seed(
                    facts,
                    &predicate,
                    fact.point,
                    fact.origin,
                );
            }

            /*
                Instantiate one requirement-authored proposition fact through the exact
                conformance edge recorded on it. The migrated obligation keeps its
                obligation kind; only the endpoint family and the argument labels move from
                the requirement's schema onto the satisfying state's own terms.
            */
            fn instantiate_inherited_contract_payload(
                program: &dyn ::validation::CheckingInput,
                proof: &ProofFacts,
                facts: &mut FactPlan,
                contract: &ContractProofFact,
                payload: &mut FactPayload,
            ) {
                let FactPayload::ContractPropositionApplication { instantiated, .. } = payload
                else {
                    return;
                };
                let Some((trait_definition, requirement, satisfier_state, scope)) =
                    inherited_contract_scope_parts(program, proof, contract)
                else {
                    return;
                };
                let Some(application) = program.proof_facts.get(contract.fact).as_proposition()
                else {
                    return;
                };
                let Some(label) = validation::inherited_requirement_proposition_label(
                    program,
                    trait_definition,
                    requirement,
                    satisfier_state,
                    &scope.trait_arguments,
                    application,
                ) else {
                    return;
                };
                *instantiated = facts.append_instantiated_expression(label);
            }

            /*
                A callee's declared domain membership keeps its template index binders —
                `v in Coordinate<I>` — but this call bound `I` to a caller-side subject or
                literal, and the caller's own membership facts intern the bound instance's
                identity, not the template's. Ask the domain identity owner to substitute the
                complete tuple, then adopt only an already-interned identity. No matching
                syntax node needs to exist for a compound index, and identity alone never
                establishes membership. Missing evidence still fails ordinary entailment.
            */
            fn instantiate_call_domain_membership_instance(
                program: &dyn ::validation::CheckingInput,
                call: &ContractCallFact,
                contract: &ContractProofFact,
                payload: &mut FactPayload,
            ) {
                let FactPayload::ContractDomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                } = payload
                else {
                    return;
                };
                let Some(mut membership) = program.proof_facts.get(contract.fact).as_membership()
                else {
                    return;
                };
                let Some(domain) = program.domain_definition(*domain_symbol) else {
                    return;
                };
                let index_parameters = typed_trees::domain::index_parameters(program, domain);
                let Some(site) = validation::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    return;
                };
                if let validation::CallSite::Expression {
                    call: expression, ..
                } = &site
                    && (expression.requirement_application.is_valid()
                        || program
                            .closed_requirement_application(expression.target_symbol)
                            .is_some())
                    && let ContractProofFactOwner::StateSignature { state_symbol, .. } =
                        contract.owner
                {
                    let Some((_, declared)) = program.trait_machine_signature(state_symbol) else {
                        return;
                    };
                    let Some(selected) =
                        program.requirement_call_signature(call.caller_machine_symbol, expression)
                    else {
                        return;
                    };
                    let Some(selected_membership) = program
                        .applied_signature_contract_fact(declared, selected, contract.fact)
                        .and_then(typed_trees::domain::ProofFact::as_membership)
                    else {
                        return;
                    };
                    if selected_membership.domain_symbol != membership.domain_symbol {
                        return;
                    }
                    membership = selected_membership;
                }
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments);
                if arguments.is_empty() || arguments.len() != index_parameters.len() {
                    return;
                }
                let bindings =
                    crate::published::facts::index_compatibility::call_index_substitutions(
                        program,
                        call.caller_machine_symbol,
                        &site,
                    );
                let substitutions =
                    crate::published::facts::index_compatibility::index_identity_bindings(
                        &bindings,
                    );
                let Ok(identity) =
                    typed_trees::domain::indexed_domain_instance_name_with_substitutions(
                        program,
                        domain,
                        index_parameters,
                        arguments,
                        &substitutions,
                    )
                else {
                    return;
                };
                if let Some(id) = program.semantic_domains.lookup(&identity) {
                    *semantic_domain = id;
                }
            }

            fn instantiate_call_contract_payload(
                program: &dyn ::validation::CheckingInput,
                proof: &ProofFacts,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                contract: &ContractProofFact,
                payload: &mut FactPayload,
            ) {
                if matches!(payload, FactPayload::ContractDomainMembership { .. }) {
                    instantiate_call_domain_membership_instance(program, call, contract, payload);
                    return;
                }
                if !matches!(
                    payload,
                    FactPayload::ContractPropositionApplication { .. }
                        | FactPayload::ContractBooleanExpression { .. }
                ) {
                    return;
                }
                let Some(call_site) = validation::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    return;
                };
                let target_parameters =
                    if let Some(dispatch) = call_site.static_requirement_dispatch() {
                        let Some(requirement) = program
                            .trait_definition_by_symbol(dispatch.declaring_trait)
                            .and_then(|definition| {
                                program.trait_machine_signature_by_symbol(
                                    definition,
                                    dispatch.contract_symbol(),
                                )
                            })
                        else {
                            return;
                        };
                        program.state_signature_parameters(requirement)
                    } else {
                        let Some(parameters) =
                            validation::call_target_parameters(program, call.target_state_symbol)
                        else {
                            return;
                        };
                        parameters
                    };
                let mut alias_parameters = Vec::new();
                let mut inherited_endpoint = None;
                if let Some((trait_definition, requirement, satisfier_state, scope)) =
                    inherited_contract_scope_parts(program, proof, contract)
                {
                    let required_parameters = program.state_signature_parameters(requirement);
                    let Some(satisfier_parameters) = validation::inherited_satisfier_parameters(
                        program,
                        trait_definition,
                        requirement,
                        satisfier_state,
                    ) else {
                        return;
                    };
                    alias_parameters = required_parameters
                        .iter()
                        .zip(satisfier_parameters.iter())
                        .map(
                            |(required, actual)| typed_trees::signature::StateParameter {
                                symbol: required.symbol,
                                name: required.name.clone(),
                                type_reference: actual.type_reference,
                                is_const: actual.is_const,
                                is_mutable: actual.is_mutable,
                                relevance: actual.relevance,
                            },
                        )
                        .collect();
                    if matches!(payload, FactPayload::ContractPropositionApplication { .. }) {
                        let Some(application) =
                            program.proof_facts.get(contract.fact).as_proposition()
                        else {
                            return;
                        };
                        let Some(endpoint) =
                            validation::inherited_requirement_proposition_application(
                                program,
                                trait_definition,
                                requirement,
                                satisfier_state,
                                &scope.trait_arguments,
                                application,
                            )
                        else {
                            return;
                        };
                        inherited_endpoint = Some(endpoint);
                    }
                }
                let label_parameters: &[typed_trees::signature::StateParameter] =
                    if inherited_endpoint.is_some() || !alias_parameters.is_empty() {
                        &alias_parameters
                    } else {
                        target_parameters
                    };
                if let FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } = payload
                {
                    let label = crate::checks::contracts::labels::instantiate_call_contract_expression_label(
                        program,
                        call.caller_state_symbol,
                        call.statement_index,
                        &call_site,
                        label_parameters,
                        *expression,
                    );
                    *instantiated = facts.append_instantiated_expression(label);
                    return;
                }
                let FactPayload::ContractPropositionApplication { instantiated, .. } = payload
                else {
                    return;
                };
                let (application, binder_labels) = match inherited_endpoint {
                    Some(endpoint) => (endpoint.application, endpoint.binder_labels),
                    None => {
                        let Some(application) =
                            program.proof_facts.get(contract.fact).as_proposition()
                        else {
                            return;
                        };
                        let (binder_labels, _) =
                            crate::published::labels::proposition_label_sources(
                                application,
                                program,
                            );
                        (application.clone(), binder_labels)
                    }
                };
                let argument_labels = expressions(program)
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|argument| {
                        crate::checks::contracts::labels::instantiate_call_contract_expression_label(
                            program,
                            call.caller_state_symbol,
                            call.statement_index,
                            &call_site,
                            label_parameters,
                            *argument,
                        )
                    })
                    .collect::<Vec<_>>();
                if let Some(formula) = program.normalize_proposition_application(
                    &application,
                    Some(PropositionLabels {
                        binder_labels: &binder_labels,
                        argument_labels: &argument_labels,
                    }),
                ) {
                    *instantiated = facts.append_instantiated_expression(formula.identity_label());
                }
            }

            fn append_call_semantic_contract_refs(
                program: &dyn ::validation::CheckingInput,
                proof: &ProofFacts,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                source_refs: &[ContractProofFactRef],
                origin: FactOrigin,
                point: ProgramPoint,
                refs: &mut HandleSpan<FactRef>,
            ) {
                for source_ref in source_refs {
                    let contract = proof.contract_facts.get(source_ref.fact);
                    let owns = crate::published::proof::contracts::call_signatures::owns_contract(
                        program, proof, call, contract,
                    );
                    if !owns {
                        continue;
                    }
                    let place =
                        instantiate_call_contract_place(program, proof, facts, call, contract);
                    let mut payload = semantic_contract_payload(program, contract);
                    instantiate_call_contract_payload(
                        program,
                        proof,
                        facts,
                        call,
                        contract,
                        &mut payload,
                    );
                    if matches!(payload, FactPayload::ContractBooleanExpression { instantiated, .. } if !instantiated.is_valid())
                    {
                        continue;
                    }
                    let evidence =
                        crate::published::facts::qualification_evidence::call_contract_evidence(
                            program,
                            facts,
                            call.target_machine_symbol,
                            call.target_state_symbol,
                            contract,
                            payload,
                            matches!(origin, FactOrigin::CallEnsures),
                        );
                    let Some(evidence) = evidence else {
                        continue;
                    };
                    let carry_origin = admitted_resource_carry_origin(program, payload, evidence);
                    let fact = facts.append_fact(Fact {
                        place,
                        point,
                        origin,
                        evidence,
                        payload,
                    });
                    facts.append_ref(refs, fact);
                    if origin == FactOrigin::CallEnsures {
                        case_observations::append_result(
                            program, proof, facts, call, contract, point, evidence,
                        );
                    }
                    if origin == FactOrigin::CallEnsures
                        && matches!(
                            payload,
                            FactPayload::ContractBooleanExpression { .. }
                                | FactPayload::ContractPropositionApplication { .. }
                        )
                    {
                        for occurrence in
                            crate::published::facts::contract_occurrences::fact_referenced_occurrences(
                                program,
                                contract.fact,
                            )
                        {
                            let dependency =
                                crate::published::semantic::places::instantiate_call_contract_expression_place(
                                    program, facts, call, occurrence,
                                )
                                .map(FactPlace::Place)
                                .unwrap_or(FactPlace::Unknown);
                            if dependency == place {
                                continue;
                            }
                            let dependency_fact = facts.append_fact(Fact {
                                place: dependency,
                                point,
                                origin,
                                evidence,
                                payload: FactPayload::StorageDependency { dependent: fact },
                            });
                            facts.append_ref(refs, dependency_fact);
                        }
                    }
                    if let Some(value) = carry_origin {
                        let fact = facts.append_fact(Fact {
                            place,
                            point,
                            origin,
                            evidence,
                            payload: FactPayload::CarryOrigin { value },
                        });
                        facts.append_ref(refs, fact);
                    }
                }
            }

            fn admitted_resource_carry_origin(
                program: &dyn ::validation::CheckingInput,
                payload: FactPayload,
                evidence: QualificationEvidence,
            ) -> Option<typed_trees::expression::ExpressionHandle> {
                if evidence.origin
                    != language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
                {
                    return None;
                }
                let FactPayload::ContractDomainMembership {
                    value,
                    domain_symbol,
                    ..
                } = payload
                else {
                    return None;
                };
                let domain = program.domain_definition(domain_symbol)?;
                (domain.predicate_presence == language_semantics::DomainPredicatePresence::Absent
                    && crate::checks::type_multiplicity(program, domain.target_type)
                        == language_semantics::Multiplicity::Linear)
                    .then_some(value)
            }

            fn append_semantic_contract_refs(
                proof: &ProofFacts,
                facts: &mut FactPlan,
                semantic_handles: &[Option<facts::FactHandle>],
                source_refs: HandleSpan<ContractProofFactRef>,
                refs: &mut HandleSpan<FactRef>,
            ) {
                for source_ref in proof.contract_fact_refs.span_or_empty(source_refs) {
                    let source_index = usize::try_from(source_ref.fact.arena_index())
                        .expect("contract fact ref handle index overflow");
                    let Some(Some(fact)) = semantic_handles.get(source_index) else {
                        continue;
                    };
                    facts.append_ref(refs, *fact);
                }
            }

            /*
                Instantiated parameter qualifications join the same requires context as
                authored contracts. They carry no proof evidence and never become entry
                premises merely because a caller selected a qualified type.
            */
            fn append_parameter_domain_requirements(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                refs: &mut HandleSpan<FactRef>,
            ) {
                let Some(site) = validation::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    return;
                };
                for (argument, payload) in crate::published::semantic::calls::call_parameter_domains(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.target_machine_symbol,
                    call.target_state_symbol,
                    &site,
                ) {
                    let place =
                        crate::published::semantic::places::canonical_place_to_fact_place_in_state(
                            program,
                            facts,
                            call.caller_state_symbol,
                            call.statement_index,
                            argument,
                        )
                        .unwrap_or_else(|| facts.append_place_from_expression(program, argument));
                    let duplicate = facts.refs.span_or_empty(*refs).iter().any(|reference| {
                        let fact = facts.facts.get(reference.fact);
                        facts.fact_place_equals(fact.place, place)
                            && match (fact.payload, payload) {
                                (
                                    FactPayload::DomainMembership {
                                        domain_symbol: left,
                                        semantic_domain: left_instance,
                                        ..
                                    }
                                    | FactPayload::ContractDomainMembership {
                                        domain_symbol: left,
                                        semantic_domain: left_instance,
                                        ..
                                    },
                                    FactPayload::DomainMembership {
                                        domain_symbol: right,
                                        semantic_domain: right_instance,
                                        ..
                                    },
                                ) => left == right && left_instance == right_instance,
                                (
                                    facts::carry_permission_pair_pattern!(permission:left, ..),
                                    FactPayload::CarryPermission {
                                        permission: right, ..
                                    },
                                ) => left == right,
                                _ => false,
                            }
                    });
                    if duplicate {
                        continue;
                    }
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point: ProgramPoint::CallRequires {
                            machine_symbol: call.caller_machine_symbol,
                            state_symbol: call.caller_state_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        },
                        origin: FactOrigin::CallRequires,
                        evidence: QualificationEvidence::default(),
                        payload,
                    });
                    facts.append_ref(refs, fact);
                }
            }
        }
        mod field_domains {
            /*
                Declared default-domain fields become entry facts at exact storage places.
                Ordinary mutations invalidate these facts; a nominal annotation alone does
                not restore them. Call and state-arrival checks require the same field
                obligations over the current incoming values before reusing an entry fact.

                Zero-initialized storage requires its ZII value to satisfy the domain.
                Nominal input storage relies on the checked incoming argument, independently
                of whether its machine declaration has a qualified name.
                Write-only input views supply no facts read from incoming contents, but
                they do carry the caller's declared field invariants as entry premises —
                the same rows a readable borrow gets, since every admitted write through
                the view re-establishes them. Independent fields
                use separate contexts so invalidation does not erase unrelated evidence.

                Collection elements carry their declared fields at TWO coordinates: every
                literal element at its exact `FixedIndex`, and one whole-extent row at the
                `0..usize::MAX` window that any element index narrows from. The window row
                is the only elementwise spelling a slice can have (no static extent exists
                to enumerate); it is still exact evidence -- a write to any element
                overlaps the containing window and retires it, so an unresolved index is
                never itself a fact place and never universal coverage. Where per-element
                rows exist (fixed arrays, machine storage) the window row is seeded under
                `StatementTransfer` as evidence only -- the element rows already spell the
                complete call-boundary obligation. For a slice the window row is the ONLY
                elementwise claim, so it rides the obligation origin and every caller owes
                the whole extent.
                Element qualifications use the same declaration roster as record fields:
                the whole-extent fact for `[i32 in D]` denotes every live scalar element,
                rather than treating its static element type as proof at a later read.

                Immutable scalar parameters also carry their enforced float ranges. The
                range-obligation owner checks incoming values; these source-free predicates
                transport that promise into ordinary operand captures. They are not new
                domain-membership obligations and cannot restore a fact after a mutation.
            */
            use crate::checks::type_multiplicity;
            use facts::{
                Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
                QualificationEvidence,
            };
            use symbols::SymbolHandle;
            use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

            use super::points::state_point;
            use crate::published::facts::{checked_validation_evidence, propagated_evidence};

            use validation::field_domains::{
                FieldDomainRow, WHOLE_ELEMENT_EXTENT, readable_fixed_array_elements,
                readable_nominal_definition, readable_slice_element,
            };

            mod zero_cases {
                /*
                    Discriminator observations from actual owned zero formation.

                    A zero-valid first case can leave every other payload unestablished. Calls
                    still owe the containing value's conditional field contracts, so retain the
                    tag that makes those inactive obligations vacuous. Only a local without an
                    initializer enters here; a type annotation or a borrowed zero pointer is
                    never a source of value observations. Normal mutation invalidation owns the
                    lifetime of each independent tag, just as it does constructor-written tags.

                    A fixed array forms every element alike, so its tags are one
                    `ZeroCaseElements` fact per case position inside the element rather than
                    one `CaseValue` per element: a 131,072-element table costs the same as a
                    single value. Only an array nested inside such an element still spells
                    its own indices, because the element path of the outer fact is one
                    static suffix.
                */
                use facts::{
                    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
                    QualificationEvidence,
                };
                use symbols::SymbolHandle;
                use typed_trees::data::DataMember;
                use typed_trees::types::{
                    FixedArrayLength, TypeReferenceHandle, TypeReferenceNode,
                };

                struct ZeroCase {
                    path: Vec<PlaceSegment>,
                    variant: SymbolHandle,
                    /* Element count and in-element path when `path` names a whole array. */
                    elements: Option<(usize, Vec<PlaceSegment>)>,
                }

                pub(super) fn append(
                    program: &dyn validation::CheckingInput,
                    facts: &mut FactPlan,
                    root: SymbolHandle,
                    reference: TypeReferenceHandle,
                    point: ProgramPoint,
                    evidence: QualificationEvidence,
                ) {
                    if validation::proof_only_classification(program)
                        .proof_only_mention(program, reference)
                        .is_some()
                    {
                        return;
                    }
                    let mut rows = Vec::new();
                    collect(
                        program,
                        reference,
                        &mut Vec::new(),
                        &mut Vec::new(),
                        false,
                        &mut rows,
                    );
                    for row in rows {
                        let place = facts.append_symbol_place(root);
                        for segment in row.path {
                            facts.push_place_segment(place, segment);
                        }
                        let payload = match row.elements {
                            None => FactPayload::CaseValue {
                                variant: row.variant,
                                value: true,
                            },
                            Some((length, element_path)) => {
                                let mut element = arena::HandleSpan::empty();
                                for segment in element_path {
                                    facts.place_segments.append_to_span(&mut element, segment);
                                }
                                FactPayload::ZeroCaseElements {
                                    first: 0,
                                    end: length,
                                    element,
                                    variant: row.variant,
                                }
                            }
                        };
                        facts.append_fact_context(Fact {
                            place: FactPlace::Place(place),
                            point,
                            origin: FactOrigin::StatementTransfer,
                            evidence,
                            payload,
                        });
                    }
                }

                fn collect(
                    program: &dyn validation::CheckingInput,
                    reference: TypeReferenceHandle,
                    path: &mut Vec<PlaceSegment>,
                    visited: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
                    inside_array_element: bool,
                    rows: &mut Vec<ZeroCase>,
                ) {
                    let carrier = validation::peel_declared_constraints(program, reference);
                    let node = program.type_reference_table.type_reference(carrier);
                    if let TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    } = node
                    {
                        if *length == 0 {
                            return;
                        }
                        let mut element_rows = Vec::new();
                        collect(
                            program,
                            *element_type,
                            &mut Vec::new(),
                            visited,
                            true,
                            &mut element_rows,
                        );
                        if !inside_array_element {
                            rows.extend(element_rows.into_iter().map(|row| ZeroCase {
                                path: path.clone(),
                                variant: row.variant,
                                elements: Some((*length, row.path)),
                            }));
                            return;
                        }
                        for index in 0..*length {
                            for row in &element_rows {
                                let mut placed =
                                    Vec::with_capacity(path.len() + 1 + row.path.len());
                                placed.extend_from_slice(path);
                                placed.push(PlaceSegment::FixedIndex { index });
                                placed.extend_from_slice(&row.path);
                                rows.push(ZeroCase {
                                    path: placed,
                                    variant: row.variant,
                                    elements: None,
                                });
                            }
                        }
                        return;
                    }
                    if !matches!(
                        node,
                        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                    ) {
                        return;
                    }
                    let Some(data) = program
                        .data_application_definition(carrier)
                        .or_else(|| program.closed_data_definition(carrier))
                        .or_else(|| {
                            validation::field_domains::owned_nominal_data_definition(
                                program, carrier,
                            )
                        })
                    else {
                        return;
                    };
                    let identity = (data.symbol, data.generic_instance.unwrap_or_default());
                    if visited.contains(&identity) {
                        return;
                    }
                    visited.push(identity);
                    let members = program.data_members(data);
                    for member in members {
                        if let DataMember::Field(field) = member
                            && !field.relevance.is_erased()
                        {
                            let prefix = path.len();
                            validation::push_field_place_segments(program, path, field.symbol);
                            collect(
                                program,
                                field.type_reference,
                                path,
                                visited,
                                inside_array_element,
                                rows,
                            );
                            path.truncate(prefix);
                        }
                    }
                    if let Some(variant) = members.iter().find_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant),
                        DataMember::Field(_) => None,
                    }) && !validation::type_requires_establishment(program, reference)
                    {
                        rows.push(ZeroCase {
                            path: path.clone(),
                            variant: variant.symbol,
                            elements: None,
                        });
                        for field in program.data_payload_fields(variant) {
                            if field.relevance.is_erased() {
                                continue;
                            }
                            let prefix = path.len();
                            validation::push_field_place_segments(program, path, field.symbol);
                            collect(
                                program,
                                field.type_reference,
                                path,
                                visited,
                                inside_array_element,
                                rows,
                            );
                            path.truncate(prefix);
                        }
                    }
                    visited.pop();
                }
            }

            /*
                The declared surface shared by entry field premises and exit field duties:
                `&`, `&mut`, and `&write` shells all peel, along with `Constrained` domain
                wrappers. `readable_type_reference` refuses write-only references because
                they expose no readable contents, but these seeded rows are the caller's
                declared field obligations — premises a `requires <place> in D` discharge
                consumes — never reads of the incoming value.
            */
            pub(crate) fn declared_field_domain_carrier(
                program: &dyn ::validation::CheckingInput,
                mut reference: TypeReferenceHandle,
            ) -> Option<TypeReferenceHandle> {
                while reference.is_valid() {
                    match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                        TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                        _ => return Some(reference),
                    }
                }
                None
            }

            #[derive(Clone)]
            struct ZeroFieldDomain {
                path: Vec<PlaceSegment>,
                domain_symbol: SymbolHandle,
                semantic_domain: language_semantics::SemanticDomainId,
                origin: FactOrigin,
            }

            #[derive(Default)]
            struct ZeroFieldDomains {
                exact: Vec<ZeroFieldDomain>,
                elementwise: Vec<ZeroFieldDomain>,
            }

            /*
                Seed only domains established by a local's zero initialization. Incoming
                nominal values use the declared parameter obligations instead: zero validity
                says nothing about an established argument or its machine's qualification.

                Fixed-array fields additionally enumerate every element at its exact
                `FixedIndex` place: `value.rows[i].label` is live evidence for the single
                element `i`, invalidated only by writes overlapping that index. Collection
                fields (fixed array or slice) also carry the whole-extent `0..usize::MAX`
                row `self.rows[*].label`, the only elementwise spelling that survives a
                runtime selector or a slice view. Coverage is never encoded at an
                unresolved `Index` -- a runtime selector cannot be a fact place.

                Zero-initialized locals use this walker under
                `ProgramPoint::State`/`StatementTransfer` (their binding statement's
                zeroed storage is the evidence -- see `append_local_zii_field_domain_facts`).
            */
            #[allow(clippy::too_many_arguments)]
            fn append_zero_initialized_field_domain_facts(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                root_symbol: SymbolHandle,
                point: ProgramPoint,
                origin: FactOrigin,
                evidence: QualificationEvidence,
                reference: TypeReferenceHandle,
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                let mut rows = ZeroFieldDomains::default();
                collect_zero_initialized_field_domains(
                    program,
                    reference,
                    &[],
                    &[],
                    origin,
                    &mut rows,
                );
                for row in rows.exact.into_iter().chain(
                    rows.elementwise
                        .into_iter()
                        .filter(|row| row.path.contains(&WHOLE_ELEMENT_EXTENT)),
                ) {
                    let place = facts.append_symbol_place(root_symbol);
                    for segment in row.path {
                        facts.push_place_segment(place, segment);
                    }
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: row.origin,
                        evidence,
                        payload: domain_membership_payload(row.domain_symbol, row.semantic_domain),
                    });
                    facts.append_ref(refs, fact);
                }
            }

            fn collect_zero_initialized_field_domains(
                program: &dyn ::validation::CheckingInput,
                reference: TypeReferenceHandle,
                prefix: &[PlaceSegment],
                visited: &[&str],
                origin: FactOrigin,
                rows: &mut ZeroFieldDomains,
            ) {
                let carrier = validation::peel_declared_constraints(program, reference);
                if matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::Reference { .. }
                ) {
                    return;
                }
                let collection = readable_fixed_array_elements(program, carrier)
                    .map(|(element, length)| (element, Some(length)))
                    .or_else(|| {
                        readable_slice_element(program, carrier).map(|element| (element, None))
                    });
                if let Some((element, length)) = collection {
                    let mut element_rows = ZeroFieldDomains::default();
                    collect_zero_initialized_field_domains(
                        program,
                        element,
                        &[],
                        visited,
                        origin,
                        &mut element_rows,
                    );
                    let place = |row: &ZeroFieldDomain, segment| {
                        let mut path = Vec::with_capacity(prefix.len() + 1 + row.path.len());
                        path.extend_from_slice(prefix);
                        path.push(segment);
                        path.extend_from_slice(&row.path);
                        ZeroFieldDomain {
                            path,
                            domain_symbol: row.domain_symbol,
                            semantic_domain: row.semantic_domain,
                            origin: row.origin,
                        }
                    };
                    if let Some(length) = length
                        && !element_rows.exact.is_empty()
                    {
                        for index in 0..length {
                            for row in &element_rows.exact {
                                rows.exact
                                    .push(place(row, PlaceSegment::FixedIndex { index }));
                            }
                        }
                    }
                    for row in &element_rows.elementwise {
                        rows.elementwise.push(place(row, WHOLE_ELEMENT_EXTENT));
                    }
                    return;
                }
                let Some(data) = readable_nominal_definition(program, carrier) else {
                    return;
                };
                if visited.contains(&data.name.as_str()) {
                    return;
                }
                let mut next_visited = visited.to_vec();
                next_visited.push(data.name.as_str());
                let members = program.data_members(data);
                let zero_payload = members
                    .iter()
                    .find_map(|member| member.as_variant())
                    .filter(|_| !validation::data_requires_establishment(program, data))
                    .map_or(&[][..], |variant| program.data_payload_fields(variant));
                for field in members
                    .iter()
                    .filter_map(|member| member.as_field())
                    .chain(zero_payload.iter())
                {
                    let mut field_path = prefix.to_vec();
                    validation::push_field_place_segments(program, &mut field_path, field.symbol);
                    for (domain_symbol, semantic_domain) in
                        validation::field_domains::predicate_domain_constraint_identities(
                            program,
                            field.type_reference,
                        )
                        .into_iter()
                        .filter(|_| {
                            !validation::type_requires_establishment(program, field.type_reference)
                        })
                    {
                        let row = ZeroFieldDomain {
                            path: field_path.clone(),
                            domain_symbol,
                            semantic_domain,
                            origin,
                        };
                        rows.exact.push(row.clone());
                        rows.elementwise.push(ZeroFieldDomain {
                            origin: FactOrigin::StatementTransfer,
                            ..row
                        });
                    }
                    collect_zero_initialized_field_domains(
                        program,
                        field.type_reference,
                        &field_path,
                        &next_visited,
                        origin,
                        rows,
                    );
                }
            }

            /*
                #66/P1a param entry-assumption: an IMMUTABLE state parameter declared with a
                domain qualification carries that domain throughout the state body. The
                param's implicit `requires param in Domain` makes every caller establish
                membership (by proof for a bodyful predicate, by retained evidence for a
                bodyless qualification), and immutability means it can never be reassigned
                out of the domain. Surfaced at `ProgramPoint::State` (the param belongs to the state),
                which `build_state_flow_fact` folds into the state entry; the flow's context
                threading then carries it to every call, including guarded-transition
                fallthrough arms (now that a transition no longer leaks its branch-taken exit
                context onto its sibling fallthrough -- see flow/statements.rs).

                Nominal parameters additionally carry their declared field predicates on
                entry, including mutable and write-only references: calls and transitions
                are default-domain consumption points. These are live entry facts, not
                facts restored after arbitrary writes. Ordinary storage invalidation
                retires them. A write-only view still cannot inspect the incoming value —
                the seeded rows are the caller's declared invariants, and the lent place's
                `requires <place> in D` obligation at a nested `&write` subloan discharges
                against them.
            */
            pub(super) fn append_state_parameter_domain_facts(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
            ) {
                let mut field_domain_walks: Vec<(TypeReferenceHandle, Vec<FieldDomainRow>)> =
                    Vec::new();
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        let mut refs = arena::HandleSpan::empty();
                        for parameter in program.state_parameters(state) {
                            let obligation_origin = FactOrigin::StateParameterDomain {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            };
                            let lent =
                                declared_field_domain_carrier(program, parameter.type_reference);
                            if let Some(lent) = lent {
                                let row_index = field_domain_walks
                                    .iter()
                                    .position(|(reference, _)| *reference == lent)
                                    .unwrap_or_else(|| {
                                        let index = field_domain_walks.len();
                                        field_domain_walks.push((
                                            lent,
                                            validation::field_domains::declared_type_domain_rows(
                                                program, lent,
                                            ),
                                        ));
                                        index
                                    });
                                let rows = &field_domain_walks[row_index].1;
                                append_state_parameter_data_field_domain_facts(
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    parameter.symbol,
                                    obligation_origin,
                                    &[],
                                    rows,
                                    &mut refs,
                                );
                            }
                            for bound in validation::declared_float_field_bounds(
                                program,
                                parameter.type_reference,
                            ) {
                                let Some(predicate) = bound.predicate() else {
                                    continue;
                                };
                                let place = facts.append_symbol_place(parameter.symbol);
                                for symbol in bound.path {
                                    facts.push_place_segment(place, PlaceSegment::Field { symbol });
                                }
                                let fact = facts.append_fact(Fact {
                                    place: FactPlace::Place(place),
                                    point: state_point(machine.symbol, state.symbol),
                                    origin: obligation_origin,
                                    evidence: propagated_evidence(state.symbol),
                                    payload: FactPayload::FloatPredicate { predicate },
                                });
                                facts.append_ref(&mut refs, fact);
                            }
                            if parameter.is_mutable {
                                if validation::reference_shell(program, parameter.type_reference)
                                    .is_some_and(|shell| shell.access.is_exclusive())
                                {
                                    for (domain_symbol, semantic_domain) in
                                        validation::field_domains::domain_constraint_identities(
                                            program,
                                            parameter.type_reference,
                                        )
                                    {
                                        if validation::field_domains::domain_requires_provenance(
                                            program,
                                            domain_symbol,
                                        ) {
                                            append_state_parameter_domain_fact(
                                                facts,
                                                machine.symbol,
                                                state.symbol,
                                                parameter.symbol,
                                                obligation_origin,
                                                &[],
                                                domain_symbol,
                                                semantic_domain,
                                                &mut refs,
                                            );
                                        }
                                    }
                                }
                                continue;
                            }
                            let mut has_resource_claim = false;
                            for (domain_symbol, semantic_domain) in
                                validation::field_domains::domain_constraint_identities(
                                    program,
                                    parameter.type_reference,
                                )
                            {
                                has_resource_claim |= state_parameter_domain_is_resource_claim(
                                    program,
                                    parameter.type_reference,
                                    domain_symbol,
                                );
                                append_state_parameter_domain_fact(
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    parameter.symbol,
                                    obligation_origin,
                                    &[],
                                    domain_symbol,
                                    semantic_domain,
                                    &mut refs,
                                );
                            }
                            if has_resource_claim {
                                append_state_parameter_carry_origin(
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    parameter.symbol,
                                    &mut refs,
                                );
                            }
                            for permission in
                                carry_constraint_permissions(program, parameter.type_reference)
                            {
                                append_state_parameter_carry_fact(
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    parameter.symbol,
                                    permission,
                                    &mut refs,
                                );
                            }
                        }

                        if refs.is_empty() {
                            continue;
                        }
                        append_independent_place_contexts(
                            facts,
                            state_point(machine.symbol, state.symbol),
                            refs,
                        );
                    }
                }
            }

            fn append_independent_place_contexts(
                facts: &mut FactPlan,
                point: ProgramPoint,
                refs: arena::HandleSpan<facts::FactRef>,
            ) {
                for group in facts.group_fact_refs_by_place(refs) {
                    let mut refs = arena::HandleSpan::empty();
                    for reference in group {
                        facts.refs.append_to_span(&mut refs, reference);
                    }
                    facts.append_context(point, refs);
                }
            }

            fn state_parameter_domain_is_resource_claim(
                program: &dyn ::validation::CheckingInput,
                type_reference: typed_trees::types::TypeReferenceHandle,
                domain_symbol: SymbolHandle,
            ) -> bool {
                type_multiplicity(program, type_reference)
                    == language_semantics::Multiplicity::Linear
                    && program
                        .domain_definition(domain_symbol)
                        .is_some_and(|domain| {
                            domain.predicate_presence
                                == language_semantics::DomainPredicatePresence::Absent
                        })
            }

            /*
                A domain-membership row keyed at a symbol place. No authored expression
                produced it and no expression span narrows its domain: the row claims the
                place's declared domain, so both expression fields stay empty.
            */
            fn domain_membership_payload(
                domain_symbol: SymbolHandle,
                semantic_domain: language_semantics::SemanticDomainId,
            ) -> FactPayload {
                FactPayload::DomainMembership {
                    value: typed_trees::expression::ExpressionHandle::invalid(),
                    domain: arena::HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                }
            }

            /*
                The `Place(parameter_symbol)` + `State` point + `StateParameterDomain`
                origin + `Propagated` evidence scaffolding both carry facts share.
            */
            fn append_state_parameter_carry(
                facts: &mut FactPlan,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
                payload: FactPayload,
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                let place = facts.append_symbol_place(parameter_symbol);
                let fact = facts.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: state_point(machine_symbol, state_symbol),
                    origin: FactOrigin::StateParameterDomain {
                        machine_symbol,
                        state_symbol,
                    },
                    evidence: propagated_evidence(state_symbol),
                    payload,
                });
                facts.append_ref(refs, fact);
            }

            fn append_state_parameter_carry_origin(
                facts: &mut FactPlan,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                append_state_parameter_carry(
                    facts,
                    machine_symbol,
                    state_symbol,
                    parameter_symbol,
                    FactPayload::CarryOrigin {
                        value: typed_trees::expression::ExpressionHandle::invalid(),
                    },
                    refs,
                );
            }

            fn carry_constraint_permissions(
                program: &dyn ::validation::CheckingInput,
                type_reference: typed_trees::types::TypeReferenceHandle,
            ) -> Vec<language_semantics::CarryPermission> {
                match program.type_reference_table.type_reference(type_reference) {
                    typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                        carry_constraint_permissions(program, *referee)
                    }
                    typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } => {
                        program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .filter_map(|constraint| match constraint {
                                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                                    language_semantics::CarryPermission::from_name(
                                        domain.name.as_str(),
                                    )
                                }
                                _ => None,
                            })
                            .collect()
                    }
                    _ => Vec::new(),
                }
            }

            fn append_state_parameter_carry_fact(
                facts: &mut FactPlan,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
                permission: language_semantics::CarryPermission,
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                append_state_parameter_carry(
                    facts,
                    machine_symbol,
                    state_symbol,
                    parameter_symbol,
                    FactPayload::CarryPermission {
                        value: typed_trees::expression::ExpressionHandle::invalid(),
                        permission,
                    },
                    refs,
                );
            }

            #[allow(clippy::too_many_arguments)]
            fn append_state_parameter_data_field_domain_facts(
                facts: &mut FactPlan,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
                origin: FactOrigin,
                prefix: &[PlaceSegment],
                rows: &[FieldDomainRow],
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                let mut path = Vec::new();
                for row in rows {
                    path.clear();
                    path.extend_from_slice(prefix);
                    path.extend_from_slice(&row.path);
                    append_state_parameter_domain_fact(
                        facts,
                        machine_symbol,
                        state_symbol,
                        parameter_symbol,
                        if row.transfer {
                            FactOrigin::StatementTransfer
                        } else {
                            origin
                        },
                        &path,
                        row.domain_symbol,
                        row.semantic_domain,
                        refs,
                    );
                }
            }

            #[allow(clippy::too_many_arguments)]
            fn append_state_parameter_domain_fact(
                facts: &mut FactPlan,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
                origin: FactOrigin,
                path: &[PlaceSegment],
                domain_symbol: SymbolHandle,
                semantic_domain: language_semantics::SemanticDomainId,
                refs: &mut arena::HandleSpan<facts::FactRef>,
            ) {
                let place = facts.append_symbol_place(parameter_symbol);
                for segment in path {
                    facts.push_place_segment(place, *segment);
                }
                let fact = facts.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: state_point(machine_symbol, state_symbol),
                    origin,
                    evidence: propagated_evidence(state_symbol),
                    payload: domain_membership_payload(domain_symbol, semantic_domain),
                });
                facts.append_ref(refs, fact);
            }

            /*
                `let value: T;` binds zero-initialized storage (dependent_values.md:
                zero-initializability is a storage representation guarantee). When `T`'s
                declared field domains admit the zero/empty byte sequence, that zero value
                already satisfies each such field, so `value.<field> in D` is live evidence
                from the binding onward. Seeded at `ProgramPoint::State` with the
                `StatementTransfer` origin: the binding statement is the formation point,
                and the symbol cannot resolve before its declaration, so folding the rows
                into state entry is exact. Ordinary mutation invalidation retires every
                row, and domains the zero value violates stay gated -- a read of a gated
                field still requires an establishing write first.

                An initializer (`let value: T = expr;`) gets its domain evidence from the
                assignment transfer instead; seeding the zero here too would be wrong
                because the initializer replaces the zero before any observation.
            */
            pub(super) fn append_local_zii_field_domain_facts(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
            ) {
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        let mut refs = arena::HandleSpan::empty();
                        for statement in program.statement_table.statements(state.statement_nodes) {
                            let Some(local_data) = statement.as_local_data() else {
                                continue;
                            };
                            if local_data.initial_value.is_valid() {
                                continue;
                            }
                            let point = state_point(machine.symbol, state.symbol);
                            let origin = FactOrigin::StatementTransfer;
                            let evidence = checked_validation_evidence(state.symbol);

                            if !local_data.relevance.is_erased() {
                                zero_cases::append(
                                    program,
                                    facts,
                                    local_data.symbol,
                                    local_data.type_reference,
                                    point,
                                    evidence,
                                );
                            }

                            for (domain_symbol, semantic_domain) in
                                validation::field_domains::domain_constraint_identities(
                                    program,
                                    local_data.type_reference,
                                )
                                .into_iter()
                                .filter(|(domain_symbol, _)| {
                                    validation::field_domains::domain_admits_zero_byte_carrier(
                                        program,
                                        *domain_symbol,
                                        local_data.type_reference,
                                    )
                                })
                            {
                                let place = facts.append_symbol_place(local_data.symbol);
                                let fact = facts.append_fact(Fact {
                                    place: FactPlace::Place(place),
                                    point,
                                    origin,
                                    evidence,
                                    payload: domain_membership_payload(
                                        domain_symbol,
                                        semantic_domain,
                                    ),
                                });
                                facts.append_ref(&mut refs, fact);
                            }

                            if !local_data.relevance.is_erased() {
                                append_zero_initialized_field_domain_facts(
                                    program,
                                    facts,
                                    local_data.symbol,
                                    point,
                                    origin,
                                    evidence,
                                    local_data.type_reference,
                                    &mut refs,
                                );
                            }
                        }

                        if refs.is_empty() {
                            continue;
                        }
                        append_independent_place_contexts(
                            facts,
                            state_point(machine.symbol, state.symbol),
                            refs,
                        );
                    }
                }
            }
        }
        mod points {
            use checked_trees::{
                ContractProofFact, ContractProofFactKind, ContractProofFactOwner as FactOwner,
                ProofObligationFact, ProofObligationOwner,
            };
            use facts::{FactOrigin, ProgramPoint};
            use symbols::SymbolHandle;

            /*
                The `State` point of one machine state. Every entry row in this crate is
                attached to a state rather than to a statement offset, so the point is
                spelled once here instead of at each seeding site.
            */
            pub(super) fn state_point(
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            ) -> ProgramPoint {
                ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                }
            }

            pub(super) fn proof_obligation_point(obligation: &ProofObligationFact) -> ProgramPoint {
                match obligation.owner {
                    ProofObligationOwner::MachineState {
                        machine_symbol,
                        state_symbol,
                    }
                    | ProofObligationOwner::StateReturn {
                        machine_symbol,
                        state_symbol,
                    } => state_point(machine_symbol, state_symbol),
                    ProofObligationOwner::MachineOwnedData {
                        machine_symbol,
                        data_symbol: _,
                    } => ProgramPoint::Machine { machine_symbol },
                    ProofObligationOwner::StateParameter {
                        machine_symbol,
                        state_symbol,
                        parameter_symbol: _,
                    }
                    | ProofObligationOwner::CallParameter {
                        machine_symbol,
                        state_symbol,
                        target_symbol: _,
                        parameter_symbol: _,
                    }
                    | ProofObligationOwner::TransitionParameter {
                        machine_symbol,
                        state_symbol,
                        parameter_symbol: _,
                    } => state_point(machine_symbol, state_symbol),
                    ProofObligationOwner::Unknown => ProgramPoint::Global,
                }
            }

            pub(super) fn contract_fact_point(
                program: &dyn ::validation::CheckingInput,
                contract: &ContractProofFact,
            ) -> Option<ProgramPoint> {
                Some(match contract.owner {
                    FactOwner::Machine { machine_symbol } => {
                        if contract.kind == ContractProofFactKind::Requires
                            && let Some(entry) = program
                                .machine(machine_symbol)
                                .and_then(|machine| program.machine_states(machine).first())
                        {
                            return Some(state_point(machine_symbol, entry.symbol));
                        }
                        ProgramPoint::Machine { machine_symbol }
                    }
                    FactOwner::MachineState {
                        machine_symbol,
                        state_symbol,
                    } => state_point(machine_symbol, state_symbol),
                    FactOwner::StateSignature {
                        owner_symbol,
                        state_symbol,
                    } => state_point(owner_symbol, state_symbol),
                    FactOwner::OperatorDeclaration { operator_symbol } => {
                        ProgramPoint::Definition {
                            symbol: operator_symbol,
                        }
                    }
                    FactOwner::OperatorUse {
                        origin:
                            checked_trees::CheckedValueOrigin::StateStatement {
                                machine_symbol,
                                state_symbol,
                                statement_index,
                                ..
                            },
                        ..
                    } => ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    FactOwner::OperatorUse { .. } => ProgramPoint::Global,
                    FactOwner::RuntimeCallableContract { .. } | FactOwner::CallSignature { .. } => {
                        return None;
                    }
                    FactOwner::Unknown => ProgramPoint::Global,
                })
            }

            pub(super) fn contract_fact_origin(contract: &ContractProofFact) -> Option<FactOrigin> {
                Some(match contract.owner {
                    FactOwner::Machine { machine_symbol } => {
                        FactOrigin::MachineContract { machine_symbol }
                    }
                    FactOwner::MachineState {
                        machine_symbol,
                        state_symbol,
                    } => FactOrigin::StateContract {
                        machine_symbol,
                        state_symbol,
                    },
                    FactOwner::StateSignature {
                        owner_symbol,
                        state_symbol,
                    } => FactOrigin::StateSignatureContract {
                        owner_symbol,
                        state_symbol,
                    },
                    FactOwner::OperatorDeclaration { operator_symbol } => match contract.kind {
                        ContractProofFactKind::Requires => {
                            FactOrigin::OperatorRequires { operator_symbol }
                        }
                        ContractProofFactKind::Ensures => {
                            FactOrigin::OperatorEnsures { operator_symbol }
                        }
                    },
                    FactOwner::OperatorUse {
                        operator_symbol, ..
                    } => match contract.kind {
                        ContractProofFactKind::Requires => {
                            FactOrigin::OperatorRequires { operator_symbol }
                        }
                        ContractProofFactKind::Ensures => {
                            FactOrigin::OperatorEnsures { operator_symbol }
                        }
                    },
                    FactOwner::RuntimeCallableContract { .. } | FactOwner::CallSignature { .. } => {
                        return None;
                    }
                    FactOwner::Unknown => FactOrigin::Unknown,
                })
            }
        }
        use contracts::append_contract_semantic_facts;
        pub(crate) use contracts::{
            applied_inherited_requirement, contract_fact_place, inherited_contract_scope_parts,
        };
        pub(crate) use contracts::{contract_parameter_position, projected_argument_place};
        pub(crate) use field_domains::declared_field_domain_carrier;
        use field_domains::{
            append_local_zii_field_domain_facts, append_state_parameter_domain_facts,
        };
        use points::proof_obligation_point;

        /*
            One observed property and its storage dependencies share one lifetime.
            A dependency asserts no property of its own; an overlapping write retires
            the context through ordinary invalidation instead of leaving a stale row.
        */
        pub(crate) fn append_observation_context(
            facts: &mut FactPlan,
            observation: Fact,
            dependencies: &[FactPlace],
        ) -> facts::FactContextHandle {
            let dependent = facts.append_fact(observation);
            let mut references = arena::HandleSpan::empty();
            facts.append_ref(&mut references, dependent);
            for &place in dependencies {
                let dependency = facts.append_fact(Fact {
                    place,
                    payload: FactPayload::StorageDependency { dependent },
                    ..observation
                });
                facts.append_ref(&mut references, dependency);
            }
            facts.append_context(observation.point, references)
        }

        pub(crate) fn build_semantic_facts(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            operators: &checked_trees::CheckedOperatorFacts,
        ) -> Result<FactPlan, diagnostics::Diagnostic> {
            let mut facts = validation::build_definition_fact_plan(program);
            crate::published::facts::qualification_evidence::prepare_issuers(
                program, proof, &mut facts,
            )?;
            append_proof_obligation_semantic_facts(proof, &mut facts);
            append_contract_semantic_facts(program, proof, operators, &mut facts);
            append_state_parameter_domain_facts(program, &mut facts);
            append_state_parameter_predicates(program, operators, &mut facts);
            append_local_zii_field_domain_facts(program, &mut facts);

            Ok(facts)
        }

        /*
            Call admission checks these same declaration rows. Bind the complete rows
            at entry, rather than rediscovering individual fields at a later consumer.
            The roster excludes write-only contents; the normal predicate dependencies
            retire a row when any storage it observes is changed.

            Spelling and binding a state's rows reads only declarations and the
            operator roster; only seeding writes the plan. Every state's rows are
            bound on the checking pool, then seeded in roster order so each fact,
            place and context keeps the handle a one-state-at-a-time walk gives it.
        */
        fn append_state_parameter_predicates(
            program: &dyn validation::CheckingInput,
            operators: &checked_trees::CheckedOperatorFacts,
            facts: &mut FactPlan,
        ) {
            use rayon::prelude::*;
            let states = program
                .machines()
                .iter()
                .flat_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .map(move |state| (machine, state))
                })
                .collect::<Vec<_>>();
            let bound = validation::checking_thread_pool().install(|| {
                states
                    .par_iter()
                    .map(|(machine, state)| {
                        validation::state_declaration_predicates(program, operators, state)
                            .into_iter()
                            .filter_map(|row| {
                                crate::published::facts::boolean_predicates::bind_parameters(
                                    program,
                                    machine.symbol,
                                    state.symbol,
                                    program.state_parameters(state),
                                    None,
                                    row.predicate,
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            });
            for ((machine, state), predicates) in states.iter().zip(bound) {
                for predicate in predicates {
                    crate::published::facts::boolean_predicates::seed(
                        facts,
                        &predicate,
                        facts::ProgramPoint::State {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                        },
                        FactOrigin::StatementTransfer,
                    );
                }
            }
        }

        fn append_proof_obligation_semantic_facts(proof: &ProofFacts, facts: &mut FactPlan) {
            for (_, obligation) in proof.obligations.iter() {
                let point = proof_obligation_point(obligation);
                facts.append_fact_context(Fact {
                    place: FactPlace::Unknown,
                    point,
                    origin: FactOrigin::ProofObligation,
                    evidence: QualificationEvidence::default(),
                    payload: FactPayload::ProofObligation {
                        kind: semantic_proof_obligation_kind(obligation.kind.clone()),
                    },
                });
            }
        }
    }
    pub(crate) mod places {
        /*
            Contract fact places: the storage a callee's contract fact names,
            rewritten into the caller's terms at one call, and the place builders that
            fact construction and flow facts share.

            `instantiate_call_contract_place` is the entry;
            `semantic::facts::contracts` calls it for each contract fact of a call.
            For an expression or membership fact it first tries `expression`, which
            builds the place from the contract expression with the call's argument,
            receiver or result in place of each callee name. Otherwise it takes the
            fact's declared place (`semantic::facts::contract_fact_place`) and, when
            that place is rooted at a callee parameter, replaces the root with the
            place of the matching argument or receiver (`substitution`), keeping the
            remaining segments.

            `receiver` finds the place of a call's receiver for `expression` and
            `substitution`. `place_builders` appends places and segments to a
            `facts::FactPlan` and converts canonical flow places into fact places;
            flow facts and contract facts call it directly. `expression` also exports
            `instantiate_outcome_contract_expression_place`, which binds `result` to a
            given result expression, and `call_contract_argument_projection`, which
            the contract prover uses.
        */
        use validation::CallSite;
        use validation::call_site_argument_expressions;
        use validation::call_target_parameters;
        use validation::find_call_site;
        use validation::find_state_in_machine;

        use crate::published::semantic::facts::contract_fact_place;
        use checked_trees::{ContractCallFact, ContractProofFact, ProofFacts};
        use facts::{FactPlace, FactPlan};
        fn call_contract_parameters<'program>(
            program: &'program dyn validation::CheckingInput,
            call: &ContractCallFact,
        ) -> Option<&'program [typed_trees::signature::StateParameter]> {
            let site = validation::find_call_site(
                program,
                call.caller_machine_symbol,
                call.caller_state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            if site.runtime_call().is_some() {
                let validation::CallContractSource::Runtime { contract, .. } =
                    validation::call_contract_source(program, call.caller_machine_symbol, &site)
                        .ok()??
                else {
                    return None;
                };
                return Some(program.state_signature_parameters(&contract.signature));
            }
            validation::call_target_parameters(program, call.target_state_symbol)
        }

        mod expression {
            use crate::flow::expressions;
            use crate::published::places::place_at;
            use validation::effective_member_symbol;

            use checked_trees::ContractCallFact;
            use checked_trees::expression::{ExpressionHandle, ExpressionNode};
            use facts::FactPlan;
            use language_core::is_self_receiver;
            use symbols::SymbolHandle;

            pub(crate) fn instantiate_call_contract_expression_place(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                expression: ExpressionHandle,
            ) -> Option<facts::PlaceHandle> {
                instantiate_contract_expression_place(program, facts, call, None, expression)
            }

            pub(crate) fn instantiate_outcome_contract_expression_place(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                result_statement_index: usize,
                result_expression: ExpressionHandle,
                expression: ExpressionHandle,
            ) -> Option<facts::PlaceHandle> {
                instantiate_contract_expression_place(
                    program,
                    facts,
                    call,
                    Some((result_statement_index, result_expression)),
                    expression,
                )
            }

            fn instantiate_contract_expression_place(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                result: Option<(usize, ExpressionHandle)>,
                expression: ExpressionHandle,
            ) -> Option<facts::PlaceHandle> {
                if !expression.is_valid() {
                    return None;
                }

                if result.is_none()
                    && let Some(site) = super::find_call_site(
                        program,
                        call.caller_machine_symbol,
                        call.caller_state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    )
                    && let Some(parameters) = super::call_contract_parameters(program, call)
                    && let Some((value, remaining)) = call_contract_argument_projection(
                        program,
                        parameters,
                        super::call_site_argument_expressions(program, &site),
                        expression,
                    )
                {
                    let mut place = place_at(
                        program,
                        call.caller_state_symbol,
                        call.statement_index,
                        value,
                    )?;
                    place.segments.extend(remaining);
                    return Some(super::append_place(facts, place.root, &place.segments));
                }

                match program.expression_table.expression(expression) {
                    ExpressionNode::Borrow(inner) => instantiate_contract_expression_place(
                        program,
                        facts,
                        call,
                        result,
                        inner.target,
                    ),
                    ExpressionNode::Name(path) => instantiate_call_contract_name_path_place(
                        program, facts, call, result, path,
                    ),
                    ExpressionNode::Member(member) => {
                        let receiver = instantiate_contract_expression_place(
                            program,
                            facts,
                            call,
                            result,
                            member.receiver,
                        )?;
                        let symbol = {
                            let symbol = effective_member_symbol(program, member.receiver, member);
                            if symbol.is_valid() {
                                symbol
                            } else {
                                super::resolve_place_member_symbol(
                                    program,
                                    facts,
                                    receiver,
                                    member.member.as_str(),
                                )
                                .unwrap_or_else(SymbolHandle::invalid)
                            }
                        };
                        let receiver = if let Some(variant) =
                            facts::payload_variant_for_field(program, symbol)
                        {
                            super::append_place_segment(
                                facts,
                                receiver,
                                facts::PlaceSegment::Case { variant },
                            )
                        } else {
                            receiver
                        };
                        Some(super::append_place_segment(
                            facts,
                            receiver,
                            facts::PlaceSegment::Field { symbol },
                        ))
                    }
                    ExpressionNode::Indexed(indexed) => {
                        let receiver = instantiate_contract_expression_place(
                            program,
                            facts,
                            call,
                            result,
                            indexed.collection,
                        )?;
                        let segment = validation::index_place_segment(program, indexed.index);
                        Some(super::append_place_segment(facts, receiver, segment))
                    }
                    _ => None,
                }
            }

            /*
                Select the captured value of an explicit formal projection using exact
                nominal fields. Non-constructed actuals retain their remaining field path.
            */
            pub(crate) fn call_contract_argument_projection(
                program: &dyn ::validation::CheckingInput,
                parameters: &[typed_trees::signature::StateParameter],
                arguments: &[ExpressionHandle],
                expression: ExpressionHandle,
            ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
                let formal = validation::canonical_place_from_expression(program, expression)?;
                let symbol = formal.root.as_symbol()?;
                let (position, parameter) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == symbol)?;
                if formal
                    .segments
                    .iter()
                    .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
                {
                    return None;
                }
                let mut projections = crate::flow::literal_expression_projections(
                    program,
                    *arguments.get(position)?,
                    parameter.type_reference,
                    &formal.segments,
                    false,
                )?;
                let projection = projections.pop()?;
                if !projections.is_empty() {
                    return None;
                }
                Some((projection.expression, projection.remaining))
            }

            fn instantiate_call_contract_name_path_place(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                result: Option<(usize, ExpressionHandle)>,
                path: &typed_trees::expression::TableNamePath,
            ) -> Option<facts::PlaceHandle> {
                let members = program.expression_table.name_path_members(path.members);
                let call_site = super::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                let target_parameters = super::call_contract_parameters(program, call)?;
                let first_member = members.first().map(|member| member.as_str());

                let mut place = if first_member.is_some_and(language_core::is_reserved_result) {
                    if let Some((statement_index, expression)) = result {
                        super::canonical_place_to_fact_place_in_state(
                            program,
                            facts,
                            call.caller_state_symbol,
                            statement_index,
                            expression,
                        )?
                    } else {
                        let super::CallSite::Expression { expression, .. } = call_site else {
                            return None;
                        };
                        facts.append_place_from_expression(program, expression)
                    }
                } else if first_member.is_some_and(is_self_receiver) {
                    super::receiver_place_for_call(program, facts, call, &call_site)?
                } else {
                    let mut argument_index = 0usize;
                    let mut matched = None;
                    for parameter in target_parameters {
                        let argument = super::call_site_argument_expressions(program, &call_site)
                            .get(argument_index)
                            .copied();
                        argument_index = argument_index.saturating_add(1);

                        if path.head_symbol == parameter.symbol || path.symbol == parameter.symbol {
                            matched = argument.and_then(|expression| {
                                call_argument_place(program, facts, call, expression)
                            });
                            break;
                        }
                    }
                    matched?
                };

                let tail_count = members.len().saturating_sub(1);
                if tail_count == 0 {
                    return Some(place);
                }

                let member_symbols =
                    expressions(program).name_path_member_symbols(path.member_symbols);
                for (offset, member_name) in members.iter().skip(1).enumerate() {
                    let symbol = member_symbols
                        .get(offset + 1)
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .or_else(|| {
                            super::resolve_place_member_symbol(
                                program,
                                facts,
                                place,
                                member_name.as_str(),
                            )
                        })
                        .unwrap_or_else(SymbolHandle::invalid);
                    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                        place = super::append_place_segment(
                            facts,
                            place,
                            facts::PlaceSegment::Case { variant },
                        );
                    }
                    place = super::append_place_segment(
                        facts,
                        place,
                        facts::PlaceSegment::Field { symbol },
                    );
                }

                Some(place)
            }

            fn call_argument_place(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                expression: ExpressionHandle,
            ) -> Option<facts::PlaceHandle> {
                super::canonical_place_to_fact_place_in_state(
                    program,
                    facts,
                    call.caller_state_symbol,
                    call.statement_index,
                    expression,
                )
            }
        }
        mod place_builders {
            use crate::published::places::place_at;
            use validation::{
                data_member_symbol_by_name, expression_type_symbol, symbol_type_symbol,
            };

            use arena::HandleSpan;
            use checked_trees::expression::{ExpressionHandle, NamePath};
            use facts::{FactPlan, Place, PlaceHandle, PlaceRoot, PlaceSegment};
            use symbols::SymbolHandle;

            pub(crate) fn canonical_place_to_fact_place_in_state(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                state_symbol: SymbolHandle,
                statement_index: usize,
                expression: ExpressionHandle,
            ) -> Option<PlaceHandle> {
                let canonical = place_at(program, state_symbol, statement_index, expression)?;
                Some(append_place(facts, canonical.root, &canonical.segments))
            }

            pub(crate) fn append_place_segment(
                facts: &mut FactPlan,
                base_place: PlaceHandle,
                segment: PlaceSegment,
            ) -> PlaceHandle {
                let place = *facts.places.get(base_place);
                let mut segments: Vec<_> =
                    facts.place_segments.span_or_empty(place.segments).to_vec();
                segments.push(segment);
                append_place(facts, place.root, &segments)
            }

            pub(crate) fn resolve_place_member_symbol(
                program: &dyn ::validation::CheckingInput,
                facts: &FactPlan,
                place: PlaceHandle,
                member_name: &str,
            ) -> Option<SymbolHandle> {
                let place = facts.places.get(place);
                let base_symbol = fact_place_type_symbol(program, facts, place)?;

                if let Some(machine) = program.machine_by_symbol(base_symbol)
                    && let Some(data) = program.attached_data_definition(machine)
                    && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
                {
                    return Some(symbol);
                }

                if let Some(data) = program.data_definition_by_symbol(base_symbol)
                    && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
                {
                    return Some(symbol);
                }

                None
            }

            fn fact_place_type_symbol(
                program: &dyn ::validation::CheckingInput,
                facts: &FactPlan,
                place: &Place,
            ) -> Option<SymbolHandle> {
                let mut current = match place.root {
                    PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
                    PlaceRoot::Expression(expression) => {
                        expression_type_symbol(program, expression)?
                    }
                    _ => return None,
                };

                for segment in facts.place_segments.span_or_empty(place.segments) {
                    match segment {
                        PlaceSegment::Case { .. } => {}
                        PlaceSegment::Field { symbol } => {
                            current = symbol_type_symbol(program, *symbol)?;
                        }
                        PlaceSegment::FixedIndex { .. }
                        | PlaceSegment::FixedRange { .. }
                        | PlaceSegment::OpaqueRange
                        | PlaceSegment::Index { .. } => {
                            return None;
                        }
                    }
                }

                Some(current)
            }

            pub(crate) fn append_place(
                facts: &mut FactPlan,
                root: PlaceRoot,
                segments: &[PlaceSegment],
            ) -> PlaceHandle {
                let place = facts.append_place(Place {
                    root,
                    segments: HandleSpan::empty(),
                });
                for segment in segments {
                    facts.push_place_segment(place, *segment);
                }
                place
            }

            pub(crate) fn append_place_from_name_path(
                facts: &mut FactPlan,
                path: &NamePath,
            ) -> PlaceHandle {
                let place = facts.append_symbol_place(path.head_symbol());
                for symbol in path.member_symbols().iter().skip(1) {
                    if symbol.is_valid() {
                        facts.push_place_segment(place, PlaceSegment::Field { symbol: *symbol });
                    }
                }
                place
            }
        }
        mod receiver {
            use validation::{statement_call_receiver_members, statement_call_receiver_path};

            use checked_trees::ContractCallFact;
            use facts::FactPlan;

            /*
                The place a call reads its subject from, or `None` when the call names no
                subject operand.

                Every call supplies its subject explicitly: a statement call names a
                receiver path, an expression call names a receiver expression, and a
                transition supplies the record as the argument at the callee's data-binding
                position, which `instantiate_call_relative_places` resolves positionally.
                Nothing arrives implicitly -- a carrier qualification names the semantic
                owner and contributes no argument or storage -- so a call with no receiver
                operand has no subject place to report here.

                The three arms below each ended in a `StateParameter::is_self` lookup for
                the retired implicit receiver, and the statement arm recognized a receiver
                path headed by the `self` keyword. Psi 01 rejects both spellings, so each
                was an unconditional refusal standing in front of the explicit operand.
            */
            pub(crate) fn receiver_place_for_call(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                call_site: &super::CallSite<'_>,
            ) -> Option<facts::PlaceHandle> {
                match call_site {
                    super::CallSite::Statement(statement) => {
                        if statement_call_receiver_members(program, statement).is_some() {
                            let state = super::find_state_in_machine(
                                program,
                                call.caller_machine_symbol,
                                call.caller_state_symbol,
                            )?;
                            if let Some(path) = statement_call_receiver_path(
                                program,
                                state,
                                call.statement_index,
                                statement,
                            ) {
                                return Some(super::append_place_from_name_path(facts, &path));
                            }
                        }
                        statement
                            .receiver_symbol
                            .is_valid()
                            .then(|| facts.append_symbol_place(statement.receiver_symbol))
                    }
                    super::CallSite::Expression {
                        call: expression, ..
                    } => expression
                        .receiver
                        .is_valid()
                        .then(|| facts.append_place_from_expression(program, expression.receiver)),
                    super::CallSite::TransitionNamed { .. } => None,
                }
            }
        }
        mod substitution {
            use crate::published::labels::symbol_name;
            use checked_trees::ContractCallFact;
            use facts::FactPlan;
            use language_core::is_self_receiver;

            /* The canonical fact place — retained verbatim. */
            pub(crate) type ContractPlaceSubstitution = validation::CanonicalPlace;

            pub(crate) fn call_contract_place_substitution(
                program: &dyn ::validation::CheckingInput,
                facts: &mut FactPlan,
                call: &ContractCallFact,
                original_place_handle: facts::PlaceHandle,
            ) -> Option<ContractPlaceSubstitution> {
                let original_place = *facts.places.get(original_place_handle);
                let parameter_symbol = original_place.root.as_symbol()?;
                let call_site = super::find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                let target_parameters =
                    super::call_target_parameters(program, call.target_state_symbol)?;
                let mut argument_index = 0usize;

                for parameter in target_parameters {
                    let parameter_matches = parameter.symbol == parameter_symbol
                        || symbol_name(program, parameter_symbol) == parameter.name.as_str();
                    let substitution_place = {
                        let argument = super::call_site_argument_expressions(program, &call_site)
                            .get(argument_index)
                            .copied();
                        argument_index = argument_index.saturating_add(1);
                        if !parameter_matches {
                            continue;
                        }
                        argument.map(|expression| {
                            facts.append_place_from_expression(program, expression)
                        })
                    }?;

                    let place = *facts.places.get(substitution_place);
                    let segments = facts.place_segments.span_or_empty(place.segments).to_vec();
                    return Some(ContractPlaceSubstitution {
                        root: place.root,
                        segments,
                    });
                }

                if is_self_receiver(&symbol_name(program, parameter_symbol)) {
                    let substitution_place =
                        super::receiver_place_for_call(program, facts, call, &call_site)?;
                    let place = *facts.places.get(substitution_place);
                    let segments = facts.place_segments.span_or_empty(place.segments).to_vec();
                    return Some(ContractPlaceSubstitution {
                        root: place.root,
                        segments,
                    });
                }

                None
            }
        }

        pub(crate) use expression::{
            call_contract_argument_projection, instantiate_call_contract_expression_place,
            instantiate_outcome_contract_expression_place,
        };

        pub(crate) fn instantiate_call_contract_place(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            facts: &mut FactPlan,
            call: &ContractCallFact,
            contract: &ContractProofFact,
        ) -> FactPlace {
            match program.proof_facts.get(contract.fact) {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    if let Some(place) = expression::instantiate_call_contract_expression_place(
                        program,
                        facts,
                        call,
                        *expression,
                    ) {
                        return FactPlace::Place(place);
                    }
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    if let Some(place) = membership_argument_place(
                        program,
                        proof,
                        facts,
                        call,
                        contract,
                        membership.value,
                    ) {
                        return place;
                    }
                    if let Some(place) = expression::instantiate_call_contract_expression_place(
                        program,
                        facts,
                        call,
                        membership.value,
                    ) {
                        return FactPlace::Place(place);
                    }
                }
                typed_trees::domain::ProofFact::Proposition(_) => {}
            }

            let original_place = contract_fact_place(program, proof, facts, contract);
            let Some(original_place_handle) = original_place.as_place() else {
                return original_place;
            };

            let Some(substitution) = substitution::call_contract_place_substitution(
                program,
                facts,
                call,
                original_place_handle,
            ) else {
                return original_place;
            };

            let original_place = *facts.places.get(original_place_handle);
            let original_segments: Vec<_> = facts
                .place_segments
                .span_or_empty(original_place.segments)
                .to_vec();

            let mut segments = substitution.segments;
            segments.extend(original_segments);
            FactPlace::Place(place_builders::append_place(
                facts,
                substitution.root,
                &segments,
            ))
        }

        /*
            Membership subjects retain their declaration's formal symbols even when
            inherited by a differently named satisfier. Bind through that exact edge
            before projecting an actual; display-name substitution cannot select it.
        */
        fn membership_argument_place(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            facts: &mut FactPlan,
            call: &ContractCallFact,
            contract: &ContractProofFact,
            expression: checked_trees::expression::ExpressionHandle,
        ) -> Option<FactPlace> {
            let declared = validation::canonical_place_from_expression(program, expression)?;
            let symbol = declared.root.as_symbol()?;
            let position = match crate::published::semantic::facts::contract_parameter_position(
                program,
                proof,
                contract,
                call.target_state_symbol,
                symbol,
            ) {
                Ok(Some(position)) => position,
                Ok(None) => return None,
                Err(()) => return Some(FactPlace::Unknown),
            };
            let bound = || {
                let site = find_call_site(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                let argument = *call_site_argument_expressions(program, &site).get(position)?;
                let parameter = call_contract_parameters(program, call)?.get(position)?;
                crate::published::semantic::facts::projected_argument_place(
                    program,
                    call.caller_state_symbol,
                    call.statement_index,
                    argument,
                    parameter.type_reference,
                    &declared.segments,
                )
            };
            Some(bound().map_or(FactPlace::Unknown, |actual| {
                FactPlace::Place(place_builders::append_place(
                    facts,
                    actual.root,
                    &actual.segments,
                ))
            }))
        }
        pub(crate) use place_builders::{
            append_place, append_place_from_name_path, append_place_segment,
            canonical_place_to_fact_place_in_state, resolve_place_member_symbol,
        };
        pub(crate) use receiver::receiver_place_for_call;
    }
}
