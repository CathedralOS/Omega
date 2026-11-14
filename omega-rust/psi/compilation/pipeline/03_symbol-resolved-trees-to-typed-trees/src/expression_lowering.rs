/*
Expression-table lowering (`expressions::expression`), member and operator
binding, structural equality expansion, match dispatch, domain membership, and
native code layout requests.
*/

pub(crate) mod expression {
    /*
    Entry points for typing one expression from a resolved expression table.

    Each entry builds an `ExpressionTableLowerer` (in `table`) and lowers one
    expression handle and its operands into the typed trees. They differ in
    their inputs:

    - `lower_expression_handle` reads the package's body expression table
      through the `Lowerer` and carries its current equality scope, so `==`
      can find an operand's data type. Declaration and statement lowering use
      it.
    - `lower_expression_handle_from_table` and
      `lower_expression_handle_from_table_with_symbol_substitution` take any
      expression table, with or without the resolved program; the second can
      substitute one resolved binder with a typed expression. Type references and
      constraints use them.
    - `lower_expression_handle_from_table_in_fact_position` lowers a proof
      fact with its proposition-placement diagnostics. Comparisons keep their
      selected executable meaning in this context too.

    `lower_static_machine_argument` lowers a static machine argument with its
    type reference and nested application arguments; machine, trait and call
    lowering use it, and so does `table`.

    Children: `table` holds the expression-table lowerer; `domain_membership`,
    `name_paths` and `operators` lower membership tests, name paths and binary
    operators for it.
    */

    use crate::lowerer::Lowerer;
    use crate::tables::resolved_expressions;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub(crate) mod domain_membership {
        use super::lower_expression_handle_from_table_with_symbol_substitution;
        use crate::expressions::exhaustiveness::data_definition_index;
        use crate::lowerer::name::lower_name;
        use crate::tables::{expression_table, expression_table_mut};
        use crate::type_reference::domain_aliases::expand_domain_reference;
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbols::SymbolHandle;
        use typed_trees as typed;

        pub(crate) fn lower_case_membership_expression(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            value: typed::expression::ExpressionHandle,
            domain: HandleSpan<resolved::name::DiagnosticName>,
            type_symbol: SymbolHandle,
            case_symbol: SymbolHandle,
            is_pattern: bool,
        ) -> Option<typed::expression::ExpressionHandle> {
            case_membership_test(
                program,
                target,
                value,
                source.name_path_members(domain),
                type_symbol,
                case_symbol,
                if is_pattern {
                    typed::expression::BinaryOperator::CaseMembership
                } else {
                    typed::expression::BinaryOperator::Equal
                },
            )
        }

        fn replayed_case_membership_test(
            program: &resolved::SymbolResolvedTrees,
            target: &mut typed::TypedTrees,
            value: typed::expression::ExpressionHandle,
            domain_members: &[resolved::name::DiagnosticName],
            type_symbol: SymbolHandle,
            case_symbol: SymbolHandle,
        ) -> Option<typed::expression::ExpressionHandle> {
            case_membership_test(
                program,
                target,
                value,
                domain_members,
                type_symbol,
                case_symbol,
                typed::expression::BinaryOperator::CaseMembership,
            )
        }

        fn case_membership_test(
            program: &resolved::SymbolResolvedTrees,
            target: &mut typed::TypedTrees,
            value: typed::expression::ExpressionHandle,
            domain_members: &[resolved::name::DiagnosticName],
            type_symbol: SymbolHandle,
            case_symbol: SymbolHandle,
            operator: typed::expression::BinaryOperator,
        ) -> Option<typed::expression::ExpressionHandle> {
            let [.., type_name, case_name] = domain_members else {
                return None;
            };
            if !type_symbol.is_valid() || !case_symbol.is_valid() {
                return None;
            }

            let data_definition = program.data_definition_by_symbol(type_symbol)?;
            let exact_case_belongs_to_type = program
        .data_members(data_definition.members)
        .iter()
        .any(|member| matches!(member, resolved::data::DataMember::Variant(variant) if variant.symbol == case_symbol));
            if !exact_case_belongs_to_type {
                return None;
            }

            let mut members = HandleSpan::empty();
            for member in [type_name, case_name] {
                expression_table_mut(target)
                    .push_name_path_member(&mut members, lower_name(member));
            }
            let mut member_symbols = HandleSpan::empty();
            expression_table_mut(target)
                .push_name_path_member_symbol(&mut member_symbols, type_symbol);
            expression_table_mut(target)
                .push_name_path_member_symbol(&mut member_symbols, case_symbol);
            let case_reference = expression_table_mut(target).insert(
                typed::expression::ExpressionNode::Name(typed::expression::TableNamePath::new(
                    members,
                    member_symbols,
                    type_symbol,
                    case_symbol,
                )),
            );

            Some(
                expression_table_mut(target).insert(typed::expression::ExpressionNode::Binary(
                    typed::expression::TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: value,
                        operator,
                        right: case_reference,
                    },
                )),
            )
        }

        fn domain_definition(
            program: &resolved::SymbolResolvedTrees,
            domain_symbol: SymbolHandle,
        ) -> Result<&resolved::domain::DomainDefinition, Diagnostic> {
            program
                .domain_definition_by_symbol(domain_symbol)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "cannot lower executable membership for unknown domain symbol {}",
                        domain_symbol.arena_index()
                    ))
                })
        }

        fn is_pending_data_subject(
            program: &resolved::SymbolResolvedTrees,
            target: &typed::TypedTrees,
            mut value: typed::expression::ExpressionHandle,
        ) -> bool {
            let mut visited = Vec::new();
            while value.is_valid() && !visited.contains(&value) {
                visited.push(value);
                match target.expression_table.expression(value) {
                    typed::expression::ExpressionNode::Name(path) => {
                        return path.symbol.is_valid()
                            && program
                                .data_definition_by_symbol(program.symbols.get(path.symbol).parent)
                                .is_some_and(|definition| {
                                    definition.predicate_subject_symbol == path.symbol
                                        && !definition.requires_facts.is_empty()
                                });
                    }
                    typed::expression::ExpressionNode::SavedSubject(saved) => {
                        if !expression_table(target).contains_value_capture(saved.capture) {
                            return true;
                        }
                        value = target.expression_table.value_capture(saved.capture).subject;
                    }
                    typed::expression::ExpressionNode::Borrow(borrow) => value = borrow.target,
                    typed::expression::ExpressionNode::Cast(cast) => value = cast.value,
                    _ => return false,
                }
            }
            value.is_valid()
        }

        pub(crate) fn lower_domain_membership_expression(
            program: &resolved::SymbolResolvedTrees,
            target: &mut typed::TypedTrees,
            value: typed::expression::ExpressionHandle,
            domain_symbol: SymbolHandle,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            let domain_definition = domain_definition(program, domain_symbol)?;
            if is_pending_data_subject(program, target, value) {
                return Err(Diagnostic::error(
            "cannot form named-domain membership: the logical data subject's own default domain is still pending admission",
        ).with_source_span(target.expression_table.source_span(value)));
            }
            let expanded = expand_domain_reference(
                program,
                domain_symbol,
                vec![domain_definition.name.clone()],
            )?;
            let mut lowered = Vec::with_capacity(expanded.len());
            for atom in expanded {
                lowered.push(lower_atomic_domain_membership_expression(
                    program,
                    target,
                    value,
                    atom.symbol,
                )?);
            }
            combine_conjunction(target, lowered)
        }

        fn lower_atomic_domain_membership_expression(
            program: &resolved::SymbolResolvedTrees,
            target: &mut typed::TypedTrees,
            value: typed::expression::ExpressionHandle,
            domain_symbol: SymbolHandle,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            let domain_definition = domain_definition(program, domain_symbol)?;
            let source = &program.tables.bodies.expressions;
            let mut lowered_facts = Vec::new();

            for fact in program.proof_facts(domain_definition.facts) {
                let lowered = match fact {
                    resolved::domain::ProofFact::Expression(expression) => {
                        match source.expression(*expression) {
                            resolved::expression::ExpressionNode::Membership(membership)
                                if membership.case_type_symbol.is_valid()
                                    && membership.case_symbol.is_valid() =>
                            {
                                let nested_value =
                                    lower_expression_handle_from_table_with_symbol_substitution(
                                        Some(program),
                                        source,
                                        target,
                                        membership.value,
                                        Some((domain_definition.subject_symbol, value)),
                                    )?;
                                let generated = replayed_case_membership_test(
                            program,
                            target,
                            nested_value,
                            source.name_path_members(membership.domain),
                            membership.case_type_symbol,
                            membership.case_symbol,
                        )
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "replayed domain case membership lost its selected owner or case",
                            )
                        })?;
                                expression_table_mut(target)
                                    .set_source_span(generated, source.source_span(*expression));
                                expression_table_mut(target).attach_authored_selection_occurrences(
                                    generated,
                                    source
                                        .authored_selection_occurrences(*expression)
                                        .collect::<Vec<_>>(),
                                );
                                generated
                            }
                            _ => lower_expression_handle_from_table_with_symbol_substitution(
                                Some(program),
                                source,
                                target,
                                *expression,
                                Some((domain_definition.subject_symbol, value)),
                            )?,
                        }
                    }
                    resolved::domain::ProofFact::Membership(membership) => {
                        let nested_value =
                            lower_expression_handle_from_table_with_symbol_substitution(
                                Some(program),
                                source,
                                target,
                                membership.value,
                                Some((domain_definition.subject_symbol, value)),
                            )?;
                        let (case_type_symbol, case_symbol) =
                            case_symbols_for_domain_fact(program, membership.domain);
                        if let Some(case_membership) = replayed_case_membership_test(
                            program,
                            target,
                            nested_value,
                            program.domain_path_members(membership.domain),
                            case_type_symbol,
                            case_symbol,
                        ) {
                            case_membership
                        } else {
                            lower_domain_membership_expression(
                                program,
                                target,
                                nested_value,
                                membership.domain_symbol,
                            )?
                        }
                    }
                };
                lowered_facts.push(lowered);
            }

            let predicate = combine_conjunction(target, lowered_facts)?;
            target.expression_table.retain_executable_domain_test(
                predicate,
                typed::expression::ExecutableDomainTest {
                    subject: value,
                    domain: domain_symbol,
                },
            );
            Ok(predicate)
        }

        fn case_symbols_for_domain_fact(
            program: &resolved::SymbolResolvedTrees,
            domain: HandleSpan<resolved::name::DiagnosticName>,
        ) -> (SymbolHandle, SymbolHandle) {
            let [type_name, case_name] = program.domain_path_members(domain) else {
                return (SymbolHandle::invalid(), SymbolHandle::invalid());
            };
            let Some(data_index) = data_definition_index(program, type_name) else {
                return (SymbolHandle::invalid(), SymbolHandle::invalid());
            };
            let data = &program.data_definitions[data_index];
            let case_symbol = program
                .data_members(data.members)
                .iter()
                .find_map(|member| match member {
                    resolved::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case_name.as_str() =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .unwrap_or_else(SymbolHandle::invalid);
            (data.symbol, case_symbol)
        }

        fn combine_conjunction(
            target: &mut typed::TypedTrees,
            lowered_facts: Vec<typed::expression::ExpressionHandle>,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            let mut lowered_facts = lowered_facts.into_iter();
            let Some(mut combined) = lowered_facts.next() else {
                return Ok(expression_table_mut(target)
                    .insert(typed::expression::ExpressionNode::Boolean(true)));
            };

            for fact in lowered_facts {
                combined =
                    expression_table_mut(target).insert(typed::expression::ExpressionNode::Binary(
                        typed::expression::TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: combined,
                            operator: typed::expression::BinaryOperator::And,
                            right: fact,
                        },
                    ));
            }

            Ok(combined)
        }
    }
    pub(crate) mod name_paths {
        use crate::lowerer::name::lower_name;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_name_path_members_into_table(
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::expression::ExpressionTable,
            members: arena::HandleSpan<resolved::name::DiagnosticName>,
        ) -> arena::HandleSpan<typed::name::Identifier> {
            let mut lowered = arena::HandleSpan::empty();

            for member in source.name_path_members(members) {
                target.push_name_path_member(&mut lowered, lower_name(member));
            }

            lowered
        }

        pub(crate) fn lower_table_name_path_node_into_table(
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::expression::ExpressionTable,
            path: &resolved::expression::TableNamePath,
        ) -> typed::expression::TableNamePath {
            let members = lower_name_path_members_into_table(source, target, path.members);
            let member_symbols = lower_name_path_member_symbols_into_table(source, target, path);

            typed::expression::TableNamePath {
                reserved_result_of: path.reserved_result_of,
                ..typed::expression::TableNamePath::new(
                    members,
                    member_symbols,
                    path.head_symbol,
                    path.symbol,
                )
            }
        }

        fn lower_name_path_member_symbols_into_table(
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::expression::ExpressionTable,
            path: &resolved::expression::TableNamePath,
        ) -> arena::HandleSpan<symbols::SymbolHandle> {
            let mut lowered = arena::HandleSpan::empty();

            for member_symbol in source.name_path_member_symbols(path.member_symbols) {
                let member_symbol = *member_symbol;
                target.push_name_path_member_symbol(&mut lowered, member_symbol);
            }

            lowered
        }
    }
    pub(crate) mod native_code_layout {
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
        use symbol_resolved_trees as resolved;
        use symbols::SymbolKind;
        use typed_trees as typed;

        pub(crate) fn lower_request(
            program: Option<&resolved::SymbolResolvedTrees>,
            target: &mut typed::TypedTrees,
            exposure: Option<SelectionExposure>,
            operands: &[resolved::expression::StaticMachineArgument],
            value_count: usize,
            evidence_count: usize,
        ) -> Result<typed::expression::NativeCodeLayoutRequest, Diagnostic> {
            if value_count != 0 || evidence_count != 0 || operands.len() != 1 {
                return Err(Diagnostic::error(
                    "`code_layout` requires exactly one selected machine application and no value or evidence arguments",
                ));
            }
            let operand =
                super::lower_static_machine_argument(program, target, exposure, &operands[0])?;
            let valid = program.is_some_and(|program| {
                operand.symbol.is_valid()
                    && matches!(
                        program.symbols.get(operand.symbol).kind,
                        SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
                    )
            });
            if !valid || operand.const_literal.is_some() || operand.evidence_projection.is_some() {
                return Err(Diagnostic::error(
                    "`code_layout` requires one exact executable machine application; data and proof-only declarations have no native realization",
                ));
            }
            Ok(typed::expression::NativeCodeLayoutRequest {
                operand,
                discards_result: false,
            })
        }

        pub(crate) fn result_type(
            program: Option<&resolved::SymbolResolvedTrees>,
            target: &mut typed::TypedTrees,
        ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
            let program = program.ok_or_else(|| {
                Diagnostic::error("native layout query requires its resolved core declarations")
            })?;
            let symbol = program
                .data_definitions
                .iter()
                .find(|data| {
                    data.name.as_str() == "CodeLayout"
                        && program.symbols.core_source(data.symbol)
                            == Some(language_core::core_vocabulary::CoreSource::CodeLayout)
                })
                .map(|data| data.symbol)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "native layout query lost its canonical core result declaration",
                    )
                })?;
            Ok(target
                .type_reference_table
                .find_named_type_reference(symbol)
                .unwrap_or_else(|| {
                    target
                        .type_reference_table
                        .insert(typed::types::TypeReferenceNode::Named {
                            symbol,
                            name: typed::name::Identifier::generated_static("CodeLayout"),
                        })
                }))
        }
    }
    pub(crate) mod operators {
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_binary_operator(
            operator: resolved::expression::BinaryOperator,
        ) -> typed::expression::BinaryOperator {
            match operator {
                resolved::expression::BinaryOperator::Add => typed::expression::BinaryOperator::Add,
                resolved::expression::BinaryOperator::And => typed::expression::BinaryOperator::And,
                resolved::expression::BinaryOperator::BitwiseAnd => {
                    typed::expression::BinaryOperator::BitwiseAnd
                }
                resolved::expression::BinaryOperator::BitwiseOr => {
                    typed::expression::BinaryOperator::BitwiseOr
                }
                resolved::expression::BinaryOperator::BitwiseXor => {
                    typed::expression::BinaryOperator::BitwiseXor
                }
                resolved::expression::BinaryOperator::Divide => {
                    typed::expression::BinaryOperator::Divide
                }
                resolved::expression::BinaryOperator::Equal => {
                    typed::expression::BinaryOperator::Equal
                }
                resolved::expression::BinaryOperator::Greater => {
                    typed::expression::BinaryOperator::Greater
                }
                resolved::expression::BinaryOperator::GreaterOrEqual => {
                    typed::expression::BinaryOperator::GreaterOrEqual
                }
                resolved::expression::BinaryOperator::Less => {
                    typed::expression::BinaryOperator::Less
                }
                resolved::expression::BinaryOperator::LessOrEqual => {
                    typed::expression::BinaryOperator::LessOrEqual
                }
                resolved::expression::BinaryOperator::Modulo => {
                    typed::expression::BinaryOperator::Modulo
                }
                resolved::expression::BinaryOperator::Multiply => {
                    typed::expression::BinaryOperator::Multiply
                }
                resolved::expression::BinaryOperator::NotEqual => {
                    typed::expression::BinaryOperator::NotEqual
                }
                resolved::expression::BinaryOperator::Or => typed::expression::BinaryOperator::Or,
                resolved::expression::BinaryOperator::ShiftLeft => {
                    typed::expression::BinaryOperator::ShiftLeft
                }
                resolved::expression::BinaryOperator::ShiftRight => {
                    typed::expression::BinaryOperator::ShiftRight
                }
                resolved::expression::BinaryOperator::Subtract => {
                    typed::expression::BinaryOperator::Subtract
                }
            }
        }
    }
    pub(crate) mod table {
        pub(crate) mod lowerer {

            use crate::expressions::equatable::EqualityScope;
            use crate::expressions::expression::domain_membership::{
                lower_case_membership_expression, lower_domain_membership_expression,
            };
            use crate::expressions::expression::lower_static_machine_argument;
            use crate::expressions::expression::name_paths::{
                lower_name_path_members_into_table, lower_table_name_path_node_into_table,
            };
            use crate::expressions::expression::native_code_layout::lower_request;
            use crate::expressions::expression::operators::lower_binary_operator;
            use crate::expressions::proposition::is_proposition_call_target;
            use crate::lowerer::name::lower_name;
            use crate::type_reference::lower_type_reference_into_trees_with_exposure;
            use diagnostics::Diagnostic;
            use language_semantics::declaration_selection::CollectionMeasure;
            use resolved::expression::display_name_path;
            use resolved::types::TypeReference;
            use symbol_resolved_trees as resolved;
            use typed_trees as typed;

            pub(crate) mod match_dispatch {

                use super::{Diagnostic, ExpressionTableLowerer, lower_name, resolved, typed};
                use crate::tables::expression_table;
                use arena::Handle;

                impl ExpressionTableLowerer<'_, '_, '_> {
                    pub(crate) fn lower_match(
                        &mut self,
                        dispatch: resolved::expression::TableMatchExpression,
                    ) -> Result<typed::expression::ExpressionHandle, Diagnostic>
                    {
                        let subject = self.lower_value(dispatch.subject)?;
                        // Even a fieldless case match owns one completed
                        // subject for dispatch and cleanup. Resolution needs a
                        // saved-subject name only when an arm refers to it.
                        let capture = self.target().insert_match_subject(subject);
                        let outer_captures = self.match_captures.len();
                        if dispatch.capture.is_valid() {
                            self.match_captures.push((dispatch.capture, capture));
                        }
                        let lowered = (|| {
                            let mut arms = Vec::new();
                            for arm in self.source.match_arms(dispatch.arms) {
                                let pattern = match arm.pattern {
                                    resolved::expression::MatchPattern::Value(value) => {
                                        typed::expression::MatchPattern::Value(
                                            self.lower_value(value)?,
                                        )
                                    }
                                    resolved::expression::MatchPattern::Wildcard => {
                                        typed::expression::MatchPattern::Wildcard
                                    }
                                    resolved::expression::MatchPattern::Destructure(pattern) => {
                                        typed::expression::MatchPattern::Destructure(
                                            self.lower_destructure_pattern(pattern, capture, None)?,
                                        )
                                    }
                                };
                                let value = self.lower(arm.value)?;
                                arms.push(typed::expression::TableMatchArm {
                                    pattern,
                                    value,
                                    source_span: arm.source_span,
                                });
                            }
                            let arms = self.target().insert_match_arms(arms);
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Match(
                                    typed::expression::TableMatchExpression {
                                        subject,
                                        capture,
                                        arms,
                                    },
                                )))
                        })();
                        self.match_captures.truncate(outer_captures);
                        lowered
                    }

                    fn lower_destructure_pattern(
                        &mut self,
                        pattern: resolved::expression::TableDestructurePattern,
                        capture: Handle<typed::expression::CompletedValueCapture>,
                        completed_subject: Option<typed::expression::ExpressionHandle>,
                    ) -> Result<typed::expression::TableDestructurePattern, Diagnostic>
                    {
                        let program = self.program.ok_or_else(|| {
                Diagnostic::error(
                    "structural match predicates require their resolved declaration context",
                )
            })?;
                        if !pattern.condition.is_valid() {
                            return Err(Diagnostic::error(
                                "structural match has no resolved executable predicate",
                            ));
                        }
                        let application_type =
                            self.lower_application_type(pattern.application_type, pattern.head)?;
                        let head = self.lower_value(pattern.head)?;
                        let mut fields = Vec::new();
                        for field in self.source.match_fields(pattern.fields) {
                            use typed::expression::MatchFieldAction;
                            let action = match &field.action {
                                resolved::expression::MatchFieldAction::Bind(binding) => {
                                    MatchFieldAction::Bind(lower_name(binding))
                                }
                                resolved::expression::MatchFieldAction::Equal(value) => {
                                    MatchFieldAction::Equal(self.lower_value(*value)?)
                                }
                                resolved::expression::MatchFieldAction::Waive => {
                                    MatchFieldAction::Waive
                                }
                            };
                            fields.push(typed::expression::TableMatchField {
                                component: field.component,
                                member: lower_name(&field.member),
                                field_symbol: field.field_symbol,
                                action,
                            });
                        }
                        let guard = self.lower_value(pattern.guard)?;
                        let predicate = self.lower_value(pattern.condition)?;
                        use typed::expression::{
                            BinaryOperator, ExpressionNode, TableBinaryExpression,
                            TableSavedSubject,
                        };
                        let saved = completed_subject.unwrap_or_else(|| {
                            self.target()
                                .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                                    component: Default::default(),
                                    capture,
                                    pattern_head: head,
                                }))
                        });
                        let case = expression_table(self.target_trees)
                            .name_path_node(head)
                            .map(|name| name.symbol)
                            .filter(|symbol| {
                                symbol.is_valid()
                                    && program.symbols.get(*symbol).kind
                                        == symbols::SymbolKind::Variant
                            });
                        let condition = if case.is_some() {
                            let tag = self.target().insert(ExpressionNode::Binary(
                                TableBinaryExpression {
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                    left: saved,
                                    operator: BinaryOperator::CaseMembership,
                                    right: head,
                                },
                            ));
                            self.target()
                                .insert(ExpressionNode::Binary(TableBinaryExpression {
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                    left: tag,
                                    operator: BinaryOperator::And,
                                    right: predicate,
                                }))
                        } else {
                            predicate
                        };
                        let fields = self.target().insert_match_fields(fields);
                        let proof_selectors = self
                            .source
                            .match_proof_selectors(pattern.proof_selectors)
                            .iter()
                            .map(|selector| typed::statement::OutcomeProofSelector {
                                output_field: lower_name(&selector.output_field),
                                binding: lower_name(&selector.binding),
                            })
                            .collect::<Vec<_>>();
                        let proof_selectors =
                            self.target().insert_match_proof_selectors(proof_selectors);
                        Ok(typed::expression::TableDestructurePattern {
                            application_type,
                            head,
                            fields,
                            has_rest: pattern.has_rest,
                            guard,
                            condition,
                            proof_selectors,
                        })
                    }

                    pub(crate) fn lower_transition_pattern(
                        &mut self,
                        retained: resolved::statement::TransitionPattern,
                    ) -> Result<typed::statement::TransitionPattern, Diagnostic>
                    {
                        let subject = self.lower_value(retained.subject)?;
                        let pattern = self.lower_destructure_pattern(
                            retained.pattern,
                            Handle::invalid(),
                            Some(subject),
                        )?;
                        Ok(typed::statement::TransitionPattern {
                            is_structural: retained.is_structural,
                            subject,
                            pattern,
                        })
                    }

                    pub(crate) fn lower_saved_subject(
                        &mut self,
                        saved: resolved::expression::TableSavedSubject,
                    ) -> Result<typed::expression::ExpressionHandle, Diagnostic>
                    {
                        let definition = self.source.match_subject(saved.capture);
                        if definition.purpose
                            == resolved::expression::SubjectCapturePurpose::Transition
                        {
                            // This is an alias of the dispatch's already completed local, not
                            // another expression-match capture or an additional evaluation.
                            let subject = self.lower_value(definition.subject)?;
                            if saved.component == language_core::PatternComponent::Fields {
                                return Ok(subject);
                            }
                            if !matches!(
                                self.target().expression(subject),
                                typed::expression::ExpressionNode::Name(_)
                            ) {
                                return Err(Diagnostic::error(
                                    "transition case component lost its completed subject local",
                                ));
                            }
                            let copy = self.target().insert_copy(subject);
                            self.target().set_pattern_component(copy, saved.component);
                            return Ok(copy);
                        }
                        let capture = self
                            .match_captures
                            .iter()
                            .rev()
                            .find_map(|(source, target)| {
                                (*source == saved.capture).then_some(*target)
                            })
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "saved subject has no enclosing typed match capture",
                                )
                            })?;
                        let pattern_head = self.lower_value(saved.pattern_head)?;
                        Ok(self
                            .target()
                            .insert(typed::expression::ExpressionNode::SavedSubject(
                                typed::expression::TableSavedSubject {
                                    component: saved.component,
                                    capture,
                                    pattern_head,
                                },
                            )))
                    }
                }
            }

            #[derive(Clone)]
            struct NullaryErasedInitializer {
                field_name: resolved::name::DiagnosticName,
                field_symbol: symbols::SymbolHandle,
                type_name: resolved::name::DiagnosticName,
                type_symbol: symbols::SymbolHandle,
                variant_name: resolved::name::DiagnosticName,
                variant_symbol: symbols::SymbolHandle,
            }

            pub(crate) struct ExpressionTableLowerer<'program, 'target, 'scope> {
                pub(crate) program: Option<&'program resolved::SymbolResolvedTrees>,
                pub(crate) source: &'program resolved::expression::ExpressionTable,
                pub(crate) target_trees: &'target mut typed::TypedTrees,
                symbol_substitution:
                    Option<(symbols::SymbolHandle, typed::expression::ExpressionHandle)>,
                pub(crate) equality_scope: Option<&'scope EqualityScope>,
                /// Tracks proposition-only placement diagnostics. Calls and value
                /// children leave this context explicitly; comparison meaning is unchanged.
                pub(crate) fact_position: bool,
                /// Whether a suffixed policy operator may appear here.
                /// Preparation types one against its operands' carrier, and it
                /// reads executable statements and the contracts of machines
                /// and their states. Every other position rejects the suffix
                /// here instead of letting it be checked as the bare Exact
                /// operator.
                admits_policy_operators: bool,
                lowered_expressions: Vec<(
                    resolved::expression::ExpressionHandle,
                    bool,
                    typed::expression::ExpressionHandle,
                )>,
                match_captures: Vec<(
                    arena::Handle<resolved::expression::MatchSubjectCapture>,
                    arena::Handle<typed::expression::CompletedValueCapture>,
                )>,
            }

            impl<'program, 'target, 'scope> ExpressionTableLowerer<'program, 'target, 'scope> {
                pub(crate) fn new(
                    program: Option<&'program resolved::SymbolResolvedTrees>,
                    source: &'program resolved::expression::ExpressionTable,
                    target_trees: &'target mut typed::TypedTrees,
                    symbol_substitution: Option<(
                        symbols::SymbolHandle,
                        typed::expression::ExpressionHandle,
                    )>,
                    equality_scope: Option<&'scope EqualityScope>,
                ) -> Self {
                    Self {
                        program,
                        source,
                        target_trees,
                        symbol_substitution,
                        equality_scope,
                        fact_position: false,
                        admits_policy_operators: false,
                        lowered_expressions: Vec::new(),
                        match_captures: Vec::new(),
                    }
                }

                pub(crate) fn target(&mut self) -> &mut typed::expression::ExpressionTable {
                    &mut self.target_trees.expression_table
                }

                fn retain_builtin_type_reference(
                    &mut self,
                    atom: symbols::BuiltinTypeAtom,
                ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
                    let Some(program) = self.program else {
                        return Ok(typed::types::TypeReferenceHandle::invalid());
                    };
                    let symbol = program.symbols.builtin_atom_symbol(atom).ok_or_else(|| {
                        Diagnostic::error(
                            "typed expression lost its compiler-installed builtin type",
                        )
                    })?;
                    Ok(self
                        .target_trees
                        .type_reference_table
                        .find_named_type_reference(symbol)
                        .unwrap_or_else(|| {
                            self.target_trees.type_reference_table.insert(
                                typed::types::TypeReferenceNode::Named {
                                    symbol,
                                    name: typed::name::Identifier::generated(
                                        atom.symbol_name().as_ref(),
                                    ),
                                },
                            )
                        }))
                }

                pub(crate) fn in_fact_position(mut self) -> Self {
                    self.fact_position = true;
                    self
                }

                pub(crate) fn admitting_policy_operators(mut self, admitted: bool) -> Self {
                    self.admits_policy_operators = admitted;
                    self
                }

                fn retain_data_type_reference(&mut self, symbol: symbols::SymbolHandle) {
                    let Some(program) = self.program else {
                        return;
                    };
                    let Some(owner) = program.data_definition_by_symbol(symbol) else {
                        return;
                    };
                    if !owner.type_parameters.is_empty()
                        || self
                            .target_trees
                            .type_reference_table
                            .find_named_type_reference(symbol)
                            .is_some()
                    {
                        return;
                    }
                    self.target_trees.type_reference_table.insert(
                        typed::types::TypeReferenceNode::Named {
                            symbol,
                            name: lower_name(&owner.name),
                        },
                    );
                }

                pub(super) fn finalize_generated_call_selection(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                    selected_target: symbols::SymbolHandle,
                ) -> Result<(), Diagnostic> {
                    let occurrences = self
            .source
            .authored_selection_occurrences(expression)
            .filter(|occurrence| {
                self.target_trees
                    .authored_declaration_selections()
                    .get(*occurrence)
                    .is_some_and(|selection| {
                        selection.kind()
                            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                    })
            })
            .collect::<Vec<_>>();
                    let [occurrence] = occurrences.as_slice() else {
                        return Err(Diagnostic::error(format!(
                            "rewritten call retains {} call selections; expected one",
                            occurrences.len()
                        )));
                    };

                    crate::call_and_product_selections::bind_generated_call_selection(
                        self.target_trees,
                        *occurrence,
                        selected_target,
                    )?;
                    Ok(())
                }

                pub(crate) fn lower(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    if !expression.is_valid() {
                        return Ok(typed::expression::ExpressionHandle::invalid());
                    }
                    if let Some((_, _, lowered)) =
                        self.lowered_expressions
                            .iter()
                            .find(|(source, fact_position, _)| {
                                *source == expression && *fact_position == self.fact_position
                            })
                    {
                        return Ok(*lowered);
                    }
                    let source_span = self.source.source_span(expression);
                    let authored_selection_occurrences = self
                        .source
                        .authored_selection_occurrences(expression)
                        .collect::<Vec<_>>();
                    let lowered = self.lower_node(expression)?;
                    if let resolved::expression::ExpressionNode::Call(source_call) =
                        self.source.expression(expression)
                    {
                        let mut own = None;
                        for &occurrence in &authored_selection_occurrences {
                            let Some(program) = self.program else {
                                continue;
                            };
                            let selection = program
                                .authored_declaration_selections()
                                .get(occurrence)
                                .ok_or_else(|| {
                                    Diagnostic::error(
                                        "typing call lost its source selection occurrence",
                                    )
                                })?;
                            if selection.kind() != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                    || selection.source_span() != source_call.target.source_span() { continue; }
                            if own.is_some_and(|previous| previous != occurrence) {
                                return Err(Diagnostic::error(
                                    "typing call has conflicting source-owned Call occurrences",
                                )
                                .with_source_span(source_call.target.source_span()));
                            }
                            own = Some(occurrence);
                        }
                        if let Some(occurrence) = own
                            && let typed::expression::ExpressionNode::Call(call) =
                                self.target().expression(lowered)
                        {
                            let selected = call.target_symbol;
                            // A generated call owns the occurrence only when
                            // the answer the author got is that call's target.
                            let owned = if call.build_selection.is_valid() {
                                crate::call_and_product_selections::bind_generated_call_selection(
                                    self.target_trees,
                                    occurrence,
                                    selected,
                                )?
                            } else {
                                !crate::call_and_product_selections::names_replaced_prelude_operation(
                                    self.target_trees,
                                    occurrence,
                                    selected,
                                )
                            };
                            if !owned {
                                own = None;
                            }
                        }
                        if let typed::expression::ExpressionNode::Call(call) =
                            self.target().expression_mut(lowered)
                        {
                            call.authored_call_selection = own;
                        }
                    }
                    if matches!(self.target().expression(lowered), typed::expression::ExpressionNode::Call(call) if call.layout_query.is_some())
                    {
                        let result =
                            self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::U64)?;
                        self.target()
                            .set_constructed_value_type_reference(lowered, result);
                    }
                    if matches!(self.target().expression(lowered), typed::expression::ExpressionNode::Call(call) if call.native_code_layout.is_some())
                    {
                        let result =
                            crate::expressions::expression::native_code_layout::result_type(
                                self.program,
                                self.target_trees,
                            )?;
                        self.target()
                            .set_constructed_value_type_reference(lowered, result);
                    }
                    if matches!(self.target().expression(lowered), typed::expression::ExpressionNode::Call(call)
            if !call.target_symbol.is_valid() && !call.receiver.is_valid()
                && call.target.as_str() == "address_of"
                && call.machine_arguments.is_empty() && call.arguments.len() == 1)
                    {
                        let result =
                            self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::Address)?;
                        self.target()
                            .set_constructed_value_type_reference(lowered, result);
                    }
                    self.target().set_source_span(lowered, source_span);
                    self.target().attach_authored_selection_occurrences(
                        lowered,
                        authored_selection_occurrences,
                    );
                    self.lowered_expressions
                        .push((expression, self.fact_position, lowered));
                    let authored = self.source.materialization_source(expression);
                    if authored.is_valid() {
                        let authored = self.lower_value(authored)?;
                        self.target()
                            .retain_materialization_source(lowered, authored);
                    }
                    Ok(lowered)
                }

                fn lower_value(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let fact_position = self.fact_position;
                    self.fact_position = false;
                    let lowered = self.lower(expression);
                    self.fact_position = fact_position;
                    lowered
                }

                fn lower_node(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let node = self.source.expression(expression);
                    let resolved::expression::ExpressionNode::Binary(binary) = node else {
                        return self.lower_nonbinary_node(expression, node);
                    };
                    if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
                        && !self.admits_policy_operators
                    {
                        return Err(Diagnostic::error(format!(
                            "the policy operator `{}{}` is supported only in executable machine code and in the contracts of a machine; this position cannot use a suffixed operator yet",
                            binary.operator.display_name(),
                            binary.policy.operator_suffix(),
                        ))
                        .with_source_span(self.source.source_span(expression)));
                    }
                    self.lower_binary_node(binary)
                }

                fn lower_binary_node(
                    &mut self,
                    binary: &resolved::expression::TableBinaryExpression,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    if matches!(
                        binary.operator,
                        resolved::expression::BinaryOperator::Equal
                            | resolved::expression::BinaryOperator::NotEqual
                    ) && let Some(lowered) = self.try_lower_structural_equality(binary)?
                    {
                        return Ok(lowered);
                    }
                    let logical_children = matches!(
                        binary.operator,
                        resolved::expression::BinaryOperator::And
                            | resolved::expression::BinaryOperator::Or
                            | resolved::expression::BinaryOperator::Equal
                            | resolved::expression::BinaryOperator::NotEqual
                    );
                    let left = if logical_children {
                        self.lower(binary.left)?
                    } else {
                        self.lower_value(binary.left)?
                    };
                    let right = if logical_children {
                        self.lower(binary.right)?
                    } else {
                        self.lower_value(binary.right)?
                    };
                    if matches!(
                        self.target_trees.expression_table.expression(left),
                        typed::expression::ExpressionNode::TypeExpression(_)
                    ) && matches!(
                        self.target_trees.expression_table.expression(right),
                        typed::expression::ExpressionNode::TypeExpression(_)
                    ) {
                        self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::Bool)?;
                    }
                    // A suffixed operator keeps its policy on the node: the
                    // operand carrier is not known yet, so preparation types
                    // it once the whole program is lowered.
                    Ok(self
                        .target()
                        .insert(typed::expression::ExpressionNode::Binary(
                            typed::expression::TableBinaryExpression {
                                left,
                                operator: lower_binary_operator(binary.operator),
                                right,
                                policy: binary.policy,
                            },
                        )))
                }

                fn lower_nonbinary_node(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                    node: &resolved::expression::ExpressionNode,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    match node {
                        resolved::expression::ExpressionNode::Match(dispatch) => {
                            self.lower_match(*dispatch)
                        }
                        resolved::expression::ExpressionNode::SavedSubject(saved) => {
                            self.lower_saved_subject(*saved)
                        }
                        resolved::expression::ExpressionNode::ArrayLiteral(values) => {
                            let values = self.lower_expression_handle_span(*values)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::ArrayLiteral(values)))
                        }
                        resolved::expression::ExpressionNode::Atomic(atomic) => {
                            if !atomic.result_custody.is_valid_for(atomic.ordering)
                                || (atomic.result_custody.has_structural_result()
                                    && atomic.result.is_valid())
                            {
                                return Err(Diagnostic::error(
                                    "atomic expression result custody does not match its operation axis",
                                ));
                            }
                            let value = self.lower_value(atomic.value)?;
                            let result = if atomic.result.is_valid() {
                                self.lower_value(atomic.result)?
                            } else {
                                typed::expression::ExpressionHandle::invalid()
                            };
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Atomic(
                                    typed::expression::TableAtomicExpression {
                                        value,
                                        result,
                                        ordering: atomic.ordering,
                                        result_custody: atomic.result_custody,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::Binary(_) => {
                            unreachable!("binary expressions use the stack-bounded lowering path")
                        }
                        resolved::expression::ExpressionNode::Boolean(value) => Ok(self
                            .target()
                            .insert(typed::expression::ExpressionNode::Boolean(*value))),
                        resolved::expression::ExpressionNode::Cast(cast) => {
                            let value = self.lower_value(cast.value)?;
                            let program = self.program.ok_or_else(|| {
                        Diagnostic::error(
                            "cast target type lowering requires the enclosing resolved program",
                        )
                    })?;
                            let exposure = self.source.authored_expression_exposure(expression).unwrap_or(
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                );
                            let target_type = lower_type_reference_into_trees_with_exposure(
                                program,
                                self.target_trees,
                                program.child_type_reference(cast.target_type),
                                exposure,
                            )?;
                            let target_label = lower_name_path_members_into_table(
                                self.source,
                                self.target(),
                                cast.target_label,
                            );
                            let semantic_domain = lower_name_path_members_into_table(
                                self.source,
                                self.target(),
                                cast.semantic_domain,
                            );
                            let semantic_domain_arguments = program
                                .child_type_references(cast.semantic_domain_arguments)
                                .iter()
                                .map(|argument| {
                                    lower_type_reference_into_trees_with_exposure(
                                        program,
                                        self.target_trees,
                                        argument,
                                        exposure,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let semantic_domain_arguments = self
                                .target_trees
                                .type_reference_table
                                .insert_type_reference_handles(semantic_domain_arguments);
                            // A domain entry has no carrier until its run is accounted for.
                            if semantic_domain.is_empty()
                                && !cast.form.is_recast()
                                && cast.form.entry().is_none()
                            {
                                crate::type_reference::retain_arithmetic_result_type(
                                    &program.symbols,
                                    self.target_trees,
                                    target_type,
                                    Some(cast.domain),
                                );
                            }
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Cast(
                                    typed::expression::TableCastExpression {
                                        value,
                                        target_type,
                                        result_type: typed::types::TypeReferenceHandle::invalid(),
                                        target_label,
                                        domain: cast.domain,
                                        semantic_domain,
                                        semantic_domain_arguments,
                                        semantic_domain_symbol: cast.semantic_domain_symbol,
                                        semantic_domain_id:
                                            language_semantics::SemanticDomainId::NULL,
                                        form: cast.form,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::Call(call) => {
                            if !call.receiver.is_valid()
                                && call.target.as_str()
                                    == symbols::BuiltinFunction::IntegerEmbed.name()
                                && self.program.is_some_and(|program| {
                                    program
                                        .symbols
                                        .builtin_function_for_symbol(call.target_symbol)
                                        == Some(symbols::BuiltinFunction::IntegerEmbed)
                                })
                            {
                                self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::Int)?;
                            }
                            if let Some(program) = self.program
                                && is_proposition_call_target(&program.symbols, call.target_symbol)
                            {
                                return Err(Diagnostic::error(if self.fact_position {
                                    "a proposition application must be the complete fact in this implementation slice"
                                } else {
                                    "a proposition application is proof-only and cannot appear in a runtime value expression"
                                }));
                            }
                            let fact_position = self.fact_position;
                            self.fact_position = false;
                            let synthesized =
                                self.try_lower_synthesized_equatable_call(expression, call);
                            self.fact_position = fact_position;
                            if let Some(lowered) = synthesized? {
                                return Ok(lowered);
                            }
                            if let Some(program) = self.program {
                                let facet =
                                    crate::expressions::product_selections::test_group_facet(
                                        program,
                                        self.source,
                                        call.receiver,
                                        call.target_symbol,
                                        call.target.as_str(),
                                        call.arguments.len(),
                                    );
                                let namespace = if let Some(build) = facet {
                                    build
                                } else if call.receiver.is_valid() {
                                    match self.source.expression(call.receiver) {
                                        resolved::expression::ExpressionNode::Name(path) => {
                                            path.symbol
                                        }
                                        _ => symbols::SymbolHandle::invalid(),
                                    }
                                } else {
                                    symbols::SymbolHandle::invalid()
                                };
                                if let Some(operation) =
                                    crate::expressions::product_queries::operation(
                                        program,
                                        namespace,
                                        call.target_symbol,
                                        call.target.as_str(),
                                    )
                                {
                                    if !call.machine_arguments.is_empty()
                                        || !call.evidence_arguments.is_empty()
                                    {
                                        return Err(Diagnostic::error(
                                            "Product queries accept only their ordinary value arguments",
                                        ));
                                    }
                                    let arguments = self
                                        .source
                                        .expression_handles(call.arguments)
                                        .to_vec()
                                        .into_iter()
                                        .map(|argument| self.lower_value(argument))
                                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                                    let selected = operation.target;
                                    let query = crate::expressions::product_queries::lower(
                                        program,
                                        self.target_trees,
                                        operation,
                                        arguments,
                                        self.source.source_span(expression),
                                        call.operational_acknowledgement,
                                    )?;
                                    self.finalize_generated_call_selection(expression, selected)?;
                                    return Ok(query);
                                }
                                if let Some(request) =
                                    crate::expressions::product_selections::request(
                                        program,
                                        self.target_trees,
                                        self.source.authored_expression_exposure(expression),
                                        &self
                                            .source
                                            .authored_selection_occurrences(expression)
                                            .collect::<Vec<_>>(),
                                        namespace,
                                        call.target_symbol,
                                        call.target.as_str(),
                                        &call.machine_arguments,
                                        call.arguments.len() + usize::from(facet.is_some()),
                                        call.evidence_arguments.len(),
                                        self.source.source_span(expression),
                                    )?
                                {
                                    let arguments = match facet {
                                        Some(_) => vec![call.receiver],
                                        None => {
                                            self.source.expression_handles(call.arguments).to_vec()
                                        }
                                    }
                                    .into_iter()
                                    .map(|argument| self.lower_value(argument))
                                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                                    return crate::expressions::build_selections::lower(
                                        program,
                                        self.target_trees,
                                        request,
                                        arguments,
                                        call.operational_acknowledgement,
                                    );
                                }
                            }
                            let quotient_operation = self.lower_quotient_operation_request(call)?;
                            let private_layout_operation =
                                self.lower_private_layout_operation_request(call)?;
                            let installed_interrupt_operation =
                                crate::expressions::installed_interrupts::lower_request(
                                    self.program,
                                    self.target_trees,
                                    self.source.authored_expression_exposure(expression),
                                    &self
                                        .source
                                        .authored_selection_occurrences(expression)
                                        .collect::<Vec<_>>(),
                                    call.target_symbol,
                                    &call.machine_arguments,
                                )?;
                            let layout_query = self.lower_data_layout_query(expression, call)?;
                            let native_code_layout = if call.target.as_str() == "code_layout"
                                && !call.target_symbol.is_valid()
                                && !call.receiver.is_valid()
                            {
                                Some(lower_request(
                                    self.program,
                                    self.target_trees,
                                    self.source.authored_expression_exposure(expression),
                                    &call.machine_arguments,
                                    call.arguments.len(),
                                    call.evidence_arguments.len(),
                                )?)
                            } else {
                                None
                            };
                            let receiver = if quotient_operation.is_some() {
                                typed::expression::ExpressionHandle::invalid()
                            } else {
                                self.lower_optional(call.receiver)?
                            };
                            let arguments = self.lower_expression_handle_span(call.arguments)?;
                            let target_symbol = if !call.target_symbol.is_valid() {
                                self.program
                            .map(|program| {
                                let declared = super::declared_receiver_method_target(
                                    program,
                                    self.source,
                                    call.receiver,
                                    &call.target,
                                );
                                if declared.is_valid() {
                                    return declared;
                                }
                                crate::expressions::call_results::computed_receiver_method_target(
                                    program,
                                    &self.target_trees.expression_table,
                                    receiver,
                                    &call.target,
                                )
                            })
                            .unwrap_or_default()
                            } else {
                                call.target_symbol
                            };
                            if !call.target_symbol.is_valid()
                                && let Some(program) = self.program
                                && program.machines.iter().any(|machine| {
                                    machine.has_structural_type_equations
                                        && machine.symbol
                                            == program.symbols.get(target_symbol).parent
                                })
                            {
                                return Err(Diagnostic::error(
                        "late-selected receiver method retains an undischarged structural type equation; use a closed explicitly selected receiver",
                    ).with_source_span(call.target.source_span()));
                            }
                            let machine_arguments = if private_layout_operation.is_some()
                                || quotient_operation.is_some()
                                || layout_query.is_some()
                                || native_code_layout.is_some()
                            {
                                Box::default()
                            } else {
                                crate::signatures::lexical_context::call_arguments(
                                    self.program,
                                    self.target_trees,
                                    self.source.authored_expression_exposure(expression),
                                    target_symbol,
                                    if installed_interrupt_operation.is_some() {
                                        &call.machine_arguments[2..]
                                    } else {
                                        &call.machine_arguments
                                    },
                                )?
                            };
                            let requirement_application = if call.requirement_application.is_valid()
                            {
                                let program = self.program.ok_or_else(|| {
                                    Diagnostic::error(
                                        "an applied requirement call requires its resolved program",
                                    )
                                })?;
                                lower_type_reference_into_trees_with_exposure(
                                    program,
                                    self.target_trees,
                                    program.child_type_reference(call.requirement_application),
                                    self.source.authored_expression_exposure(expression),
                                )?
                            } else {
                                typed::types::TypeReferenceHandle::invalid()
                            };
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Call(
                                    typed::expression::TableCallExpression {
                                        authored_call_selection: None,
                                        authored_call_binding: call.authored_call_binding,
                                        authored_machine_application: Default::default(),
                                        exact_coercion_target:
                                            typed::types::TypeReferenceHandle::invalid(),
                                        requirement_application,
                                        receiver,
                                        target_symbol,
                                        target: lower_name(&call.target),
                                        static_machine_parameter: symbols::SymbolHandle::invalid(),
                                        static_requirement_dispatch: None,
                                        machine_arguments,
                                        quotient_operation,
                                        private_layout_operation,
                                        installed_interrupt_operation,
                                        layout_query,
                                        native_code_layout,
                                        build_selection: Default::default(),
                                        arguments,
                                        evidence_arguments: call
                                            .evidence_arguments
                                            .iter()
                                            .map(lower_name)
                                            .collect::<Vec<_>>()
                                            .into_boxed_slice(),
                                        operational_acknowledgement: call
                                            .operational_acknowledgement,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::UndeterminedNaN(format) => Ok(self
                            .target()
                            .insert(typed::expression::ExpressionNode::UndeterminedNaN(*format))),
                        resolved::expression::ExpressionNode::Float(value) => Ok(self
                            .target()
                            .insert(typed::expression::ExpressionNode::Float(value.clone()))),
                        resolved::expression::ExpressionNode::Indexed(indexed) => {
                            let collection = self.lower_value(indexed.collection)?;
                            let index = self.lower_value(indexed.index)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Indexed(
                                    typed::expression::TableIndexedExpression { collection, index },
                                )))
                        }
                        resolved::expression::ExpressionNode::Integer(value) => {
                            if let Some(landing) = value.landing() {
                                self.retain_builtin_type_reference(
                                    typed::types::builtin_type_atom(landing.landed_type),
                                )?;
                            }
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Integer(value.clone())))
                        }
                        resolved::expression::ExpressionNode::Membership(membership) => {
                            self.lower_membership_expression(membership)
                        }
                        resolved::expression::ExpressionNode::Member(member) => {
                            let (selection, case_variant) = self.selected_member(member);
                            if matches!(
                                selection,
                                typed::expression::MemberSelection::CollectionMeasure(
                                    CollectionMeasure::Length
                                )
                            ) {
                                self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::U64)?;
                            }
                            let receiver = self.lower_value(member.receiver)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Member(
                                    typed::expression::TableMemberExpression {
                                        receiver,
                                        selection,
                                        member: lower_name(&member.member),
                                        case_variant: case_variant.as_ref().map(lower_name),
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::Borrow(expression) => {
                            let target = self.lower_value(expression.target)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Borrow(
                                    typed::expression::TableBorrowExpression {
                                        target,
                                        access: expression.access,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::Name(path) => {
                            if self.program.is_some_and(|program| {
                                program.symbols.builtin_type_member(path.symbol)
                                    == Some(symbols::BuiltinTypeMember::AddressBound)
                            }) {
                                self.retain_builtin_type_reference(symbols::BuiltinTypeAtom::Int)?;
                            }
                            if let Some(program) = self.program
                                && program.symbols.get(path.symbol).kind
                                    == symbols::SymbolKind::Variant
                            {
                                self.retain_data_type_reference(
                                    program.symbols.get(path.symbol).parent,
                                );
                            }
                            if let Some((symbol, substitution)) = self.symbol_substitution
                                && symbol.is_valid()
                                && path.symbol == symbol
                                && path.head_symbol == symbol
                                && path.members.count() == 1
                                && !path.static_application.is_valid()
                                && self.source.name_path_member_symbols(path.member_symbols)
                                    == [symbol]
                            {
                                return Ok(substitution);
                            }
                            let mut lowered = lower_table_name_path_node_into_table(
                                self.source,
                                self.target(),
                                path,
                            );
                            if path.static_application.is_valid() {
                                let selected = self
                                    .source
                                    .static_member(path.static_application)
                                    .ok_or_else(|| {
                                    Diagnostic::error(
                                        "declaration path lost its owned static application",
                                    )
                                })?;
                                let selected = lower_static_machine_argument(
                                    self.program,
                                    self.target_trees,
                                    self.source.authored_expression_exposure(expression),
                                    selected,
                                )?;
                                lowered.static_application =
                                    self.target().insert_static_member(selected);
                            }
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Name(lowered)))
                        }
                        resolved::expression::ExpressionNode::Range(range) => {
                            let start = self.lower_optional(range.start)?;
                            let end = self.lower_optional(range.end)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Range(
                                    typed::expression::TableRangeExpression {
                                        start,
                                        end,
                                        end_inclusive: range.end_inclusive,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::StructLiteral(struct_literal) => {
                            let application_type = self.lower_application_type(
                                struct_literal.application_type,
                                expression,
                            )?;

                            self.retain_data_type_reference(struct_literal.type_symbol);
                            let omitted = self.nullary_erased_initializers(struct_literal);
                            let mut fields =
                                self.lower_struct_literal_field_span(struct_literal.fields)?;
                            for initializer in omitted {
                                let value = self.synthesize_nullary_initializer(&initializer);
                                self.target().push_struct_field(
                                    &mut fields,
                                    typed::expression::TableStructLiteralField {
                                        name: lower_name(&initializer.field_name),
                                        field_symbol: initializer.field_symbol,
                                        value,
                                    },
                                );
                            }
                            Ok(self.target().insert(
                                typed::expression::ExpressionNode::StructLiteral(
                                    typed::expression::TableStructLiteral {
                                        application_type,
                                        type_name: lower_name(&struct_literal.type_name),
                                        type_symbol: struct_literal.type_symbol,
                                        case_name: struct_literal
                                            .case_name
                                            .as_ref()
                                            .map(lower_name),
                                        case_symbol: struct_literal.case_symbol,
                                        fields,
                                    },
                                ),
                            ))
                        }
                        resolved::expression::ExpressionNode::String(value) => Ok(self
                            .target()
                            .insert(typed::expression::ExpressionNode::String(value.clone()))),
                        resolved::expression::ExpressionNode::Unary(unary) => {
                            let operand = if unary.operator
                                == resolved::expression::UnaryOperator::LogicalNot
                            {
                                self.lower(unary.operand)?
                            } else {
                                self.lower_value(unary.operand)?
                            };
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Unary(
                                    typed::expression::TableUnaryExpression {
                                        operator: lower_unary_operator(unary.operator),
                                        operand,
                                    },
                                )))
                        }
                        resolved::expression::ExpressionNode::TypeExpression(type_reference) => {
                            let program = self.program.ok_or_else(|| {
                                Diagnostic::error(
                                    "static type operand requires its resolved program",
                                )
                            })?;
                            let exposure = self.source.authored_expression_exposure(expression).unwrap_or(
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                );
                            let reference = lower_type_reference_into_trees_with_exposure(
                                program,
                                self.target_trees,
                                program.child_type_reference(*type_reference),
                                exposure,
                            )?;
                            Ok(self.target().insert(
                                typed::expression::ExpressionNode::TypeExpression(reference),
                            ))
                        }
                        resolved::expression::ExpressionNode::ZeroValue(type_reference) => {
                            let program = self.program.ok_or_else(|| {
                        Diagnostic::error(
                            "zero-value type lowering requires the enclosing resolved program",
                        )
                    })?;
                            if let Some(quotient) = quotient_zero_value_target(
                                program,
                                program.child_type_reference(*type_reference),
                            ) {
                                return Err(Diagnostic::error(format!(
                                    "`zero_value<{quotient}>()` cannot observe or choose a retained quotient representative: quotient values are opaque and have no compiler-verified canonical representative; use a named lifted operation with its role-correctness contract"
                                )));
                            }
                            let exposure = self.source.authored_expression_exposure(expression).unwrap_or(
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                );
                            let type_reference = lower_type_reference_into_trees_with_exposure(
                                program,
                                self.target_trees,
                                program.child_type_reference(*type_reference),
                                exposure,
                            )?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::ZeroValue(
                                    type_reference,
                                )))
                        }
                    }
                }

                fn lower_quotient_operation_request(
                    &mut self,
                    call: &resolved::expression::TableCallExpression,
                ) -> Result<Option<typed::expression::QuotientOperationRequest>, Diagnostic>
                {
                    let kind = match call.target.as_str() {
                        "lift" => typed::expression::QuotientOperationKind::Lift,
                        "define" => typed::expression::QuotientOperationKind::Define,
                        _ => return Ok(None),
                    };
                    if !call.receiver.is_valid() {
                        return Ok(None);
                    }
                    let resolved::expression::ExpressionNode::Name(receiver) =
                        self.source.expression(call.receiver)
                    else {
                        return Ok(None);
                    };
                    let [namespace] = self.source.name_path_members(receiver.members) else {
                        return Ok(None);
                    };
                    if namespace.as_str() != "Quotient" {
                        return Ok(None);
                    }

                    let Some(program) = self.program else {
                        return Err(Diagnostic::error(
                            "a sealed Quotient operation requires a complete resolved program",
                        ));
                    };
                    if receiver.head_symbol.is_valid()
                        || receiver.symbol.is_valid()
                        || call.target_symbol.is_valid()
                    {
                        return Err(Diagnostic::error(
                            "the sealed `Quotient` operation namespace cannot be shadowed by an authored declaration",
                        ));
                    }
                    let (representative_operation, selected_theorems) = call
            .machine_arguments
            .split_first()
            .ok_or_else(|| Diagnostic::error(format!(
                "`Quotient::{}` requires a representative operation and its canonical theorem evidence",
                call.target,
            )))?;
                    let expected_theorem_count = match kind {
                        typed::expression::QuotientOperationKind::Define => 1,
                        typed::expression::QuotientOperationKind::Lift => {
                            if selected_theorems.len() == 1 || selected_theorems.len() == 2 {
                                selected_theorems.len()
                            } else {
                                return Err(Diagnostic::error(
                                    "`Quotient::lift` requires `F, Congruence` or `F, Congruence, Transport` in canonical role order",
                                ));
                            }
                        }
                    };
                    if selected_theorems.len() != expected_theorem_count {
                        return Err(Diagnostic::error(
                            "`Quotient::define` requires exactly `F, Congruence`; forward transport is not a `define` role",
                        ));
                    }
                    if representative_operation.const_literal.is_some()
                        || representative_operation.evidence_projection.is_some()
                        || !representative_operation.symbol.is_valid()
                        || program.symbols.get(representative_operation.symbol).kind
                            != symbols::SymbolKind::State
                    {
                        return Err(Diagnostic::error(format!(
                            "the first static argument to `Quotient::{}` must resolve exactly to a representative machine entry",
                            call.target,
                        )));
                    }
                    for (position, selected_theorem) in selected_theorems.iter().enumerate() {
                        if selected_theorem.const_literal.is_some()
                            || selected_theorem.evidence_projection.is_some()
                            || !selected_theorem.symbol.is_valid()
                            || program.symbols.get(selected_theorem.symbol).kind
                                != symbols::SymbolKind::State
                        {
                            return Err(Diagnostic::error(format!(
                                "static theorem argument {} to `Quotient::{}` must resolve exactly to one resultless theorem machine entry; conformance or structural proof discovery is not permitted",
                                position + 2,
                                call.target,
                            )));
                        }
                    }

                    let theorem_evidence = selected_theorems
                .iter()
                .enumerate()
                .map(|(position, selected)| {
                    Ok(typed::expression::QuotientTheoremSelection {
                        role: if position == 0 {
                            typed::expression::QuotientTheoremRole::Congruence
                        } else {
                            typed::expression::QuotientTheoremRole::ForwardPreconditionTransport
                        },
                        application: lower_static_machine_argument(
                            self.program,
                            self.target_trees,
                            None,
                            selected,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .into_boxed_slice();

                    Ok(Some(typed::expression::QuotientOperationRequest {
                        kind,
                        representative_operation: lower_static_machine_argument(
                            self.program,
                            self.target_trees,
                            None,
                            representative_operation,
                        )?,
                        theorem_evidence,
                    }))
                }

                fn lower_private_layout_operation_request(
                    &mut self,
                    call: &resolved::expression::TableCallExpression,
                ) -> Result<Option<typed::expression::PrivateLayoutOperationRequest>, Diagnostic>
                {
                    if call.target.as_str() != "place_private" || !call.receiver.is_valid() {
                        return Ok(None);
                    }
                    let resolved::expression::ExpressionNode::Name(receiver) =
                        self.source.expression(call.receiver)
                    else {
                        return Ok(None);
                    };
                    let [namespace] = self.source.name_path_members(receiver.members) else {
                        return Ok(None);
                    };
                    if namespace.as_str() != "LayoutPlan" {
                        return Ok(None);
                    }
                    let Some(program) = self.program else {
                        return Err(Diagnostic::error(
                            "a private layout placement requires a complete resolved program",
                        ));
                    };
                    let [selected_slot] = call.machine_arguments.as_ref() else {
                        return Err(Diagnostic::error(
                            "`LayoutPlan::place_private` requires exactly one static argument naming one exact conformance",
                        ));
                    };
                    if selected_slot.const_literal.is_some()
                        || selected_slot.evidence_projection.is_some()
                        || !selected_slot.symbol.is_valid()
                        || program.symbols.get(selected_slot.symbol).kind
                            != symbols::SymbolKind::Conformance
                    {
                        return Err(Diagnostic::error(
                            "the static argument to `LayoutPlan::place_private` must resolve exactly to one named conformance; slot discovery is not permitted",
                        ));
                    }
                    if !call.target_symbol.is_valid()
                        || program.symbols.get(call.target_symbol).kind
                            != symbols::SymbolKind::State
                    {
                        return Err(Diagnostic::error(
                            "`LayoutPlan::place_private` must resolve to its exact compiler-known LayoutPlan operation",
                        ));
                    }
                    let exact_owner = program.machines.iter().find(|machine| {
                        machine.attached_data_symbol.is_valid()
                            && program.symbols.name(machine.attached_data_symbol) == "LayoutPlan"
                            && program
                                .machine_state_handles(machine.states)
                                .iter()
                                .any(|state| {
                                    program.machine_state(*state).symbol == call.target_symbol
                                })
                    });
                    let Some(exact_owner) = exact_owner else {
                        return Err(Diagnostic::error(
                            "`LayoutPlan::place_private` must be the LayoutPlan-owned compiler-known operation",
                        ));
                    };
                    if receiver.symbol != exact_owner.attached_data_symbol
                        || program.symbols.symbol_source_origin(call.target_symbol)
                            != Some(source::SourceOrigin::Toolchain)
                        || program
                            .symbols
                            .symbol_source_origin(exact_owner.attached_data_symbol)
                            != Some(source::SourceOrigin::Toolchain)
                    {
                        return Err(Diagnostic::error(
                            "the private-layout operation is sealed to core `LayoutPlan::place_private`; an authored lookalike cannot mint a placement receipt",
                        ));
                    }

                    Ok(Some(typed::expression::PrivateLayoutOperationRequest {
                        selected_slot: lower_static_machine_argument(
                            self.program,
                            self.target_trees,
                            None,
                            selected_slot,
                        )?,
                    }))
                }

                fn lower_data_layout_query(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                    call: &resolved::expression::TableCallExpression,
                ) -> Result<Option<typed::expression::DataLayoutQueryRequest>, Diagnostic>
                {
                    let Some(kind) =
                        language_core::data_layout_queries::DataLayoutQueryKind::from_target_name(
                            call.target.as_str(),
                        )
                    else {
                        return Ok(None);
                    };
                    if call.target_symbol.is_valid() || call.receiver.is_valid() {
                        return Ok(None);
                    }
                    let spelling = kind.target_name();
                    if kind == language_core::data_layout_queries::DataLayoutQueryKind::Alignment
                        && call.arguments.len() == 1
                        && call.machine_arguments.is_empty()
                    {
                        return Ok(Some(typed::expression::DataLayoutQueryRequest {
                            kind,
                            operand: typed::expression::StaticMachineArgument::default(),
                            nonempty_view: typed::expression::ExpressionHandle::invalid(),
                        }));
                    }
                    if !call.arguments.is_empty() {
                        return Err(Diagnostic::error(format!(
                "`{spelling}` takes no value arguments; the queried type is its single `<T>` operand"
            ))
            .with_source_span(call.target.source_span()));
                    }
                    let [operand] = call.machine_arguments.as_ref() else {
                        return Err(Diagnostic::error(format!(
                    "`{spelling}` requires exactly one static type argument, `{spelling}<T>()`"
                ))
                .with_source_span(call.target.source_span()));
                    };
                    if operand.const_literal.is_some() || operand.evidence_projection.is_some() {
                        return Err(Diagnostic::error(format!(
                "the static argument to `{spelling}` must be a type, not a value or evidence term"
            ))
            .with_source_span(call.target.source_span()));
                    }
                    let mut lowered = lower_static_machine_argument(
                        self.program,
                        self.target_trees,
                        self.source.authored_expression_exposure(expression),
                        operand,
                    )?;
                    if !lowered.type_reference.is_valid() {
                        let operand_kind = self
                            .program
                            .filter(|_| operand.symbol.is_valid())
                            .map(|program| program.symbols.get(operand.symbol).kind);
                        match self.layout_query_operand_type(operand)? {
                            Some(type_reference) => lowered.type_reference = type_reference,
                            None => {
                                return Err(Diagnostic::error(match operand_kind {
                        Some(symbols::SymbolKind::Machine | symbols::SymbolKind::State) => {
                            format!(
                                "`{spelling}` takes a data type; a machine declaration is not a data carrier"
                            )
                        }
                        Some(
                            symbols::SymbolKind::Proposition
                            | symbols::SymbolKind::PropositionParameter
                            | symbols::SymbolKind::MathematicalDefinition
                            | symbols::SymbolKind::Trait
                            | symbols::SymbolKind::Conformance,
                        ) => {
                            format!(
                                "`{spelling}` takes a data carrier; a proof-only declaration owns no layout"
                            )
                        }
                        _ => format!("`{spelling}` requires a resolved data type operand"),
                    })
                    .with_source_span(call.target.source_span()));
                            }
                        }
                    }
                    Ok(Some(typed::expression::DataLayoutQueryRequest {
                        kind,
                        operand: lowered,
                        nonempty_view: typed::expression::ExpressionHandle::invalid(),
                    }))
                }

                fn layout_query_operand_type(
                    &mut self,
                    operand: &resolved::expression::StaticMachineArgument,
                ) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
                    if operand.const_literal.is_some() || operand.evidence_projection.is_some() {
                        return Ok(None);
                    }
                    let program = self.program.ok_or_else(|| {
                        Diagnostic::error("layout query requires its resolved program")
                    })?;
                    if operand.type_reference.is_valid() {
                        return lower_type_reference_into_trees_with_exposure(
                            program,
                            self.target_trees,
                            program.child_type_reference(operand.type_reference),
                            None,
                        )
                        .map(Some);
                    }
                    if !operand.symbol.is_valid() {
                        return Ok(None);
                    }
                    if !matches!(
                        program.symbols.get(operand.symbol).kind,
                        symbols::SymbolKind::Data
                            | symbols::SymbolKind::BuiltinType
                            | symbols::SymbolKind::TypeParameter
                    ) {
                        return Ok(None);
                    }
                    let Some(base_name) = operand.path.last() else {
                        return Ok(None);
                    };
                    let base_name = lower_name(base_name);
                    Ok(Some(match &operand.application {
                        None => self.target_trees.type_reference_table.insert(
                            typed::types::TypeReferenceNode::Named {
                                symbol: operand.symbol,
                                name: base_name,
                            },
                        ),
                        Some(application) => {
                            let mut arguments = Vec::with_capacity(application.arguments.len());
                            for argument in application.arguments.iter() {
                                let Some(argument) = self.layout_query_operand_type(argument)?
                                else {
                                    return Ok(None);
                                };
                                arguments.push(argument);
                            }
                            let arguments = self
                                .target_trees
                                .type_reference_table
                                .insert_type_reference_handles(arguments);
                            self.target_trees.type_reference_table.insert(
                                typed::types::TypeReferenceNode::Generic {
                                    base_symbol: operand.symbol,
                                    base_name,
                                    lifetime_arguments: application
                                        .lifetime_arguments
                                        .iter()
                                        .map(lower_name)
                                        .collect(),
                                    arguments,
                                },
                            )
                        }
                    }))
                }

                fn lower_optional(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    if !expression.is_valid() {
                        return Ok(typed::expression::ExpressionHandle::invalid());
                    }

                    self.lower_value(expression)
                }

                fn lower_expression_handle_span(
                    &mut self,
                    expressions: arena::HandleSpan<resolved::expression::ExpressionHandle>,
                ) -> Result<arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic>
                {
                    let lowered = self
                        .source
                        .expression_handles(expressions)
                        .iter()
                        .copied()
                        .map(|expression| self.lower_value(expression))
                        .collect::<Result<Vec<_>, _>>()?;

                    Ok(self.target().insert_expression_handles(lowered))
                }

                fn lower_struct_literal_field_span(
                    &mut self,
                    fields: arena::HandleSpan<resolved::expression::TableStructLiteralField>,
                ) -> Result<arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic>
                {
                    let lowered = self
                        .source
                        .struct_fields(fields)
                        .iter()
                        .map(|field| {
                            let value = self.lower_value(field.value)?;
                            Ok(typed::expression::TableStructLiteralField {
                                name: lower_name(&field.name),
                                field_symbol: field.field_symbol,
                                value,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;

                    Ok(self.target().insert_struct_fields(lowered))
                }

                fn nullary_erased_initializers(
                    &self,
                    literal: &resolved::expression::TableStructLiteral,
                ) -> Vec<NullaryErasedInitializer> {
                    let Some(program) = self.program else {
                        return Vec::new();
                    };
                    let Some(holder) = program.data_definitions.iter().find(|definition| {
                        definition.symbol == literal.type_symbol
                            && definition.supply_mode
                                == language_semantics::DataSupplyMode::CheckedShape
                    }) else {
                        return Vec::new();
                    };
                    let authored = self.source.struct_fields(literal.fields);
                    let mut selected_fields = Vec::new();
                    for member in program.data_members(holder.members) {
                        match member {
                            resolved::data::DataMember::Field(field) => selected_fields.push(field),
                            resolved::data::DataMember::Variant(variant)
                                if literal.case_symbol == Some(variant.symbol) =>
                            {
                                selected_fields
                                    .extend(program.data_payload_fields(variant.payload));
                            }
                            resolved::data::DataMember::Variant(_) => {}
                        }
                    }

                    // Holder genericity does not make an independently concrete evidence
                    // field ambiguous. Each erased field must still name the nongeneric,
                    // accessible unique nullary declaration accepted below. Open or
                    // applied evidence types still supply no implicit inhabitance.
                    selected_fields
                        .into_iter()
                        .filter(|field| field.relevance.is_erased())
                        .filter(|field| {
                            !authored.iter().any(|authored| authored.name == field.name)
                        })
                        .filter_map(|field| {
                            let evidence =
                                resolved_data_definition_for_type(program, &field.type_reference)?;
                            if evidence.supply_mode
                                != language_semantics::DataSupplyMode::CheckedShape
                                || !evidence.symbol.is_valid()
                                || !evidence.type_parameters.is_empty()
                                || evidence.name.as_str().contains('<')
                                || program.data_members(evidence.members).iter().any(|member| {
                                    matches!(member, resolved::data::DataMember::Field(_))
                                })
                            {
                                return None;
                            }
                            let mut nullary = program
                                .data_members(evidence.members)
                                .iter()
                                .filter_map(|member| match member {
                                    resolved::data::DataMember::Variant(variant)
                                        if variant.payload.is_empty()
                                            && variant.symbol.is_valid() =>
                                    {
                                        Some(variant)
                                    }
                                    _ => None,
                                });
                            let variant = nullary.next()?;
                            if nullary.next().is_some() {
                                return None;
                            }
                            Some(NullaryErasedInitializer {
                                field_name: field.name.clone(),
                                field_symbol: field.symbol,
                                type_name: evidence.name.clone(),
                                type_symbol: evidence.symbol,
                                variant_name: variant.name.clone(),
                                variant_symbol: variant.symbol,
                            })
                        })
                        .collect()
                }

                fn synthesize_nullary_initializer(
                    &mut self,
                    initializer: &NullaryErasedInitializer,
                ) -> typed::expression::ExpressionHandle {
                    let mut members = arena::HandleSpan::empty();
                    let mut member_symbols = arena::HandleSpan::empty();
                    self.target()
                        .push_name_path_member(&mut members, lower_name(&initializer.type_name));
                    self.target()
                        .push_name_path_member_symbol(&mut member_symbols, initializer.type_symbol);
                    self.target()
                        .push_name_path_member(&mut members, lower_name(&initializer.variant_name));
                    self.target().push_name_path_member_symbol(
                        &mut member_symbols,
                        initializer.variant_symbol,
                    );
                    self.target()
                        .insert(typed::expression::ExpressionNode::Name(
                            typed::expression::TableNamePath::new(
                                members,
                                member_symbols,
                                initializer.type_symbol,
                                initializer.variant_symbol,
                            ),
                        ))
                }

                pub(crate) fn lower_application_type(
                    &mut self,
                    application: arena::Handle<resolved::types::TypeReference>,
                    source_expression: resolved::expression::ExpressionHandle,
                ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
                    if !application.is_valid() {
                        return Ok(Default::default());
                    }
                    let program = self.program.ok_or_else(|| {
                        Diagnostic::error("constructor application has no declaration context")
                    })?;
                    lower_type_reference_into_trees_with_exposure(
            program, self.target_trees, program.child_type_reference(application),
            self.source.authored_expression_exposure(source_expression).unwrap_or(
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
                    )
                }

                fn lower_membership_expression(
                    &mut self,
                    membership: &resolved::expression::TableMembershipExpression,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let Some(program) = self.program else {
                        return Err(Diagnostic::error(
                            "cannot lower executable domain membership without a resolved program context",
                        ));
                    };
                    if membership.domain_symbol.is_valid() {
                        let value = self.lower_value(membership.value)?;
                        return lower_domain_membership_expression(
                            program,
                            self.target_trees,
                            value,
                            membership.domain_symbol,
                        );
                    }

                    let value = self.lower_value(membership.value)?;
                    if let Some(lowered) = lower_case_membership_expression(
                        program,
                        self.source,
                        self.target_trees,
                        value,
                        membership.domain,
                        membership.case_type_symbol,
                        membership.case_symbol,
                        membership.is_pattern,
                    ) {
                        return Ok(lowered);
                    }

                    let domain_name =
                        display_name_path(self.source.name_path_members(membership.domain), "::");
                    Err(Diagnostic::error(format!(
                        "unknown domain `{domain_name}` in executable membership expression"
                    )))
                }
            }

            fn quotient_zero_value_target(
                program: &resolved::SymbolResolvedTrees,
                type_reference: &TypeReference,
            ) -> Option<String> {
                resolved_data_definition_for_type(program, type_reference)
                    .filter(|definition| definition.quotient.is_some())
                    .map(|definition| definition.name.as_str().to_owned())
            }

            pub(crate) fn resolved_data_definition_for_type<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                type_reference: &'program TypeReference,
            ) -> Option<&'program resolved::data::DataDefinition> {
                if let TypeReference::Generic(application) = type_reference {
                    let constructor = program
                        .data_definitions
                        .iter()
                        .find(|definition| definition.symbol == application.base_symbol);
                    if constructor.is_some_and(|constructor| {
                        constructor.name.as_str() == "LayoutStorage"
                            && constructor.supply_mode
                                == language_semantics::DataSupplyMode::BoundaryOpaque
                            && program.symbols.symbol_source_origin(constructor.symbol)
                                == Some(source::SourceOrigin::Toolchain)
                            && constructor.conformance_bounds.len() == 1
                            && program.child_type_references(application.arguments).len() == 2
                    }) {
                        return resolved_data_definition_for_type(
                            program,
                            &program.child_type_references(application.arguments)[0],
                        );
                    }
                }
                let (symbol, name) = match type_reference {
                    TypeReference::Named { symbol, name, .. } => (*symbol, Some(name)),
                    TypeReference::Generic(reference) => {
                        (reference.base_symbol, Some(&reference.base_name))
                    }
                    TypeReference::Constrained(reference) => {
                        return resolved_data_definition_for_type(
                            program,
                            program.child_type_reference(reference.base_type),
                        );
                    }
                    _ => return None,
                };
                program.data_definitions.iter().find(|definition| {
                    if symbol.is_valid() {
                        definition.symbol == symbol
                    } else {
                        name.is_some_and(|name| definition.name == *name)
                    }
                })
            }

            fn lower_unary_operator(
                operator: resolved::expression::UnaryOperator,
            ) -> typed::expression::UnaryOperator {
                match operator {
                    resolved::expression::UnaryOperator::BitwiseNot => {
                        typed::expression::UnaryOperator::BitwiseNot
                    }
                    resolved::expression::UnaryOperator::LogicalNot => {
                        typed::expression::UnaryOperator::LogicalNot
                    }
                }
            }
        }
        pub(crate) mod member_binding {
            use super::lowerer::{ExpressionTableLowerer, resolved_data_definition_for_type};
            use crate::expressions::call_results::{
                declared_call_return, declared_type_method_target, peel,
            };
            use crate::lowerer::{exact_field_symbol, exact_top_level_data_symbol};
            use symbol_resolved_trees as resolved;
            use symbols::{SymbolHandle, SymbolKind};

            use resolved::data::{DataField, DataMember};
            use resolved::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
            use resolved::types::TypeReference;

            pub(crate) fn declared_receiver_method_target(
                program: &resolved::SymbolResolvedTrees,
                expressions: &resolved::expression::ExpressionTable,
                receiver: ExpressionHandle,
                target: &resolved::name::DiagnosticName,
            ) -> SymbolHandle {
                declared_type(program, expressions, receiver, 0)
                    .map(|reference| declared_type_method_target(program, reference, target))
                    .unwrap_or_default()
            }

            impl ExpressionTableLowerer<'_, '_, '_> {
                pub(crate) fn selected_member(
                    &self,
                    member: &TableMemberExpression,
                ) -> (
                    typed_trees::expression::MemberSelection,
                    Option<resolved::name::DiagnosticName>,
                ) {
                    use language_semantics::declaration_selection::CollectionMeasure;
                    use typed_trees::expression::MemberSelection;
                    let (field, case) = self.declared_member(member);
                    if field.is_valid() {
                        return (MemberSelection::Field(field), case);
                    }
                    // A same-spelled nominal field keeps its exact selection.
                    // Only an actual builtin collection type gives this
                    // observation its meaning; unknown receivers stay unresolved.
                    let collection = self.program.is_some_and(|program| {
                        declared_type(program, self.source, member.receiver, 0).is_some_and(
                            |reference| {
                                matches!(
                                    peel(program, reference),
                                    TypeReference::Slice(_) | TypeReference::FixedArray(_)
                                )
                            },
                        )
                    });
                    let selected = if collection
                        && case.is_none()
                        && CollectionMeasure::from_authored_spelling(member.member.as_str())
                            == Some(CollectionMeasure::Length)
                    {
                        MemberSelection::CollectionMeasure(CollectionMeasure::Length)
                    } else {
                        MemberSelection::Unresolved
                    };
                    (selected, case)
                }

                pub(crate) fn declared_member(
                    &self,
                    member: &TableMemberExpression,
                ) -> (SymbolHandle, Option<resolved::name::DiagnosticName>) {
                    if member.member_symbol.is_valid() {
                        return (member.member_symbol, member.case_variant.clone());
                    }
                    self.program
                        .and_then(|program| {
                            declared_receiver_field(program, self.source, member, 0)
                        })
                        .map(|field| {
                            let payload = self.program.is_some_and(|program| {
                                let owner = program.symbols.get(field.symbol).parent;
                                program.symbols.get(owner).kind == SymbolKind::Variant
                            });
                            (
                                field.symbol,
                                if payload {
                                    member.case_variant.clone()
                                } else {
                                    None
                                },
                            )
                        })
                        .unwrap_or_else(|| (member.member_symbol, member.case_variant.clone()))
                }
            }

            fn declared_receiver_field<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                expressions: &resolved::expression::ExpressionTable,
                member: &TableMemberExpression,
                depth: usize,
            ) -> Option<&'program DataField> {
                if depth >= 128 || !expressions.expression_is_valid(member.receiver) {
                    return None;
                }
                if let ExpressionNode::StructLiteral(literal) =
                    expressions.expression(member.receiver)
                {
                    return declared_data_field(program, literal.type_symbol, member);
                }
                match declared_type(program, expressions, member.receiver, depth + 1) {
                    Some(receiver) => declared_field(program, receiver, member),
                    None => selected_field(program, member.member_symbol),
                }
            }

            /*
                A member that already carries its field symbol names that
                declaration whatever its receiver is. A case arm's payload
                binding has no declared receiver type here, so a field read
                through it (`inner.x`) reaches `Inner` by this route.
            */
            fn selected_field(
                program: &resolved::SymbolResolvedTrees,
                symbol: SymbolHandle,
            ) -> Option<&DataField> {
                if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
                    return None;
                }
                let owner = program.symbols.get(symbol).parent;
                let data = match program.symbols.get(owner).kind {
                    SymbolKind::Data => owner,
                    SymbolKind::Variant => program.symbols.get(owner).parent,
                    _ => return None,
                };
                let mut definitions = program
                    .data_definitions
                    .iter()
                    .filter(|definition| definition.symbol == data);
                let definition = definitions.next()?;
                if definitions.next().is_some() || !exact_top_level_data_symbol(program, definition)
                {
                    return None;
                }
                let mut fields = program
                    .data_members(definition.members)
                    .iter()
                    .flat_map(|node| match node {
                        DataMember::Field(field) if owner == data => std::slice::from_ref(field),
                        DataMember::Variant(variant) if variant.symbol == owner => {
                            program.data_payload_fields(variant.payload)
                        }
                        _ => &[],
                    })
                    .filter(|field| {
                        field.symbol == symbol && exact_field_symbol(program, owner, field)
                    });
                let field = fields.next()?;
                fields.next().is_none().then_some(field)
            }

            fn declared_type<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                expressions: &resolved::expression::ExpressionTable,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<&'program TypeReference> {
                if !expression.is_valid() || depth >= 128 {
                    return None;
                }
                match expressions.expression(expression) {
                    ExpressionNode::Name(path) => {
                        inferred_capture_type(program, expressions, path.symbol, depth)
                    }
                    ExpressionNode::Call(call) => declared_call_return(program, call.target_symbol),
                    ExpressionNode::ArrayLiteral(_) => {
                        let mut declared = None;
                        for occurrence in expressions.authored_selection_occurrences(expression) {
                            let selection =
                                program.authored_declaration_selections().get(occurrence)?;
                            if selection.source_span() != expressions.source_span(expression) {
                                continue;
                            }
                            let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
                    continue;
                };
                            let Some(constant) =
                                program.const_declaration(selected.selected_symbol())
                            else {
                                continue;
                            };
                            if !matches!(constant.declared_type, TypeReference::FixedArray(_))
                                || declared
                                    .is_some_and(|previous| previous != &constant.declared_type)
                            {
                                return None;
                            }
                            declared = Some(&constant.declared_type);
                        }
                        declared
                    }
                    ExpressionNode::Borrow(borrow) => {
                        declared_type(program, expressions, borrow.target, depth + 1)
                    }
                    ExpressionNode::Indexed(indexed) => {
                        if !indexed.index.is_valid()
                            || matches!(
                                expressions.expression(indexed.index),
                                ExpressionNode::Range(_)
                            )
                        {
                            return None;
                        }
                        let collection =
                            declared_type(program, expressions, indexed.collection, depth + 1)?;
                        let element = match peel(program, collection) {
                            TypeReference::FixedArray(array) => array.element_type,
                            TypeReference::Slice(slice) => slice.element_type,
                            _ => return None,
                        };
                        element
                            .is_valid()
                            .then(|| program.child_type_reference(element))
                    }
                    ExpressionNode::Member(member) => Some(
                        &declared_receiver_field(program, expressions, member, depth + 1)?
                            .type_reference,
                    ),
                    _ => None,
                }
            }

            fn declared_field<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                receiver: &TypeReference,
                member: &TableMemberExpression,
            ) -> Option<&'program DataField> {
                let definition =
                    resolved_data_definition_for_type(program, peel(program, receiver))?;
                declared_data_field(program, definition.symbol, member)
            }

            fn declared_data_field<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                symbol: SymbolHandle,
                member: &TableMemberExpression,
            ) -> Option<&'program DataField> {
                let mut definitions = program
                    .data_definitions
                    .iter()
                    .filter(|definition| definition.symbol == symbol);
                let definition = definitions.next()?;
                if definitions.next().is_some() || !exact_top_level_data_symbol(program, definition)
                {
                    return None;
                }
                if let Some(selected) = &member.case_variant {
                    let mut variants =
                        program
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|node| {
                                let DataMember::Variant(variant) = node else {
                                    return None;
                                };
                                (variant.name.as_str() == selected.as_str()).then_some(variant)
                            });
                    let variant = variants.next()?;
                    if variants.next().is_some()
                        || !variant.symbol.is_valid()
                        || program.symbols.get(variant.symbol).parent != definition.symbol
                    {
                        return None;
                    }
                    let mut payload =
                        program
                            .data_payload_fields(variant.payload)
                            .iter()
                            .filter(|field| {
                                field.name.as_str() == member.member.as_str()
                                    && exact_field_symbol(program, variant.symbol, field)
                            });
                    if let Some(field) = payload.next() {
                        if payload.next().is_some()
                            || (member.member_symbol.is_valid()
                                && member.member_symbol != field.symbol)
                        {
                            return None;
                        }
                        return Some(field);
                    }
                }
                let mut fields =
                    program
                        .data_members(definition.members)
                        .iter()
                        .filter_map(|node| {
                            let DataMember::Field(field) = node else {
                                return None;
                            };
                            (field.name.as_str() == member.member.as_str()
                                && exact_field_symbol(program, definition.symbol, field))
                            .then_some(field)
                        });
                let Some(field) = fields.next() else {
                    return member
                        .case_variant
                        .is_none()
                        .then(|| sole_payload_field(program, definition, member))
                        .flatten();
                };
                if fields.next().is_some()
                    || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
                {
                    return None;
                }
                Some(field)
            }

            /*
                `shape.at` written without its case names a payload field only
                when no common field and exactly one case declares that
                spelling. The member then carries that field's symbol, whose
                parent is the case, exactly as a record member carries its
                field. A spelling two cases share stays unresolved here, and
                whoever rejects or selects it today still does.
            */
            fn sole_payload_field<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                definition: &resolved::data::DataDefinition,
                member: &TableMemberExpression,
            ) -> Option<&'program DataField> {
                let mut selected = None;
                for node in program.data_members(definition.members) {
                    let DataMember::Variant(variant) = node else {
                        continue;
                    };
                    for field in program.data_payload_fields(variant.payload) {
                        if field.name.as_str() != member.member.as_str() {
                            continue;
                        }
                        if selected.is_some() {
                            return None;
                        }
                        selected = Some((variant, field));
                    }
                }
                let (variant, field) = selected?;
                (variant.symbol.is_valid()
                    && program.symbols.get(variant.symbol).parent == definition.symbol
                    && exact_field_symbol(program, variant.symbol, field)
                    && (!member.member_symbol.is_valid() || member.member_symbol == field.symbol))
                    .then_some(field)
            }

            fn inferred_capture_type<'program>(
                program: &'program resolved::SymbolResolvedTrees,
                expressions: &resolved::expression::ExpressionTable,
                symbol: SymbolHandle,
                depth: usize,
            ) -> Option<&'program TypeReference> {
                let declared = declared_symbol_type(program, symbol)?;
                if !matches!(declared, TypeReference::Unit) {
                    return Some(declared);
                }
                if program.symbols.get(symbol).kind != SymbolKind::Local {
                    return Some(declared);
                }
                let parent = program.symbols.get(symbol).parent;
                let state = program
                    .tables
                    .declarations
                    .machine_states
                    .iter()
                    .find_map(|(_, state)| (state.symbol == parent).then_some(state))?;
                let local =
                    program
                        .state_statements(state.statements)
                        .iter()
                        .find_map(|statement| {
                            let resolved::statement::Statement::LocalData(local) = statement else {
                                return None;
                            };
                            (local.symbol == symbol).then_some(local)
                        })?;
                if !local.type_is_inferred
                    || local.role != language_core::LocalBindingRole::Value
                    || !local.initial_value.is_valid()
                {
                    return Some(declared);
                }
                declared_type(program, expressions, local.initial_value, depth + 1)
                    .or(Some(declared))
            }

            pub(crate) fn declared_symbol_type(
                program: &resolved::SymbolResolvedTrees,
                symbol: SymbolHandle,
            ) -> Option<&TypeReference> {
                if !symbol.is_valid() {
                    return None;
                }
                let declaration = program.symbols.get(symbol);
                match declaration.kind {
                    SymbolKind::TypeParameter => program
                        .tables
                        .declarations
                        .data_type_parameters
                        .iter()
                        .find_map(|(_, parameter)| {
                            if parameter.symbol != symbol {
                                return None;
                            }
                            match &parameter.kind {
                                resolved::data::TypeParameterKind::Const { type_reference }
                                | resolved::data::TypeParameterKind::Value { type_reference } => {
                                    Some(type_reference)
                                }
                                _ => None,
                            }
                        }),
                    SymbolKind::Parameter
                        if program.symbols.get(declaration.parent).kind == SymbolKind::Domain =>
                    {
                        program.domain_definitions.iter().find_map(|domain| {
                            (domain.symbol == declaration.parent && domain.subject_symbol == symbol)
                                .then_some(&domain.target_type)
                        })
                    }
                    SymbolKind::Parameter
                        if program.symbols.get(declaration.parent).kind == SymbolKind::Data =>
                    {
                        let definition = program.data_definition_by_symbol(declaration.parent)?;
                        (definition.predicate_subject_symbol == symbol)
                            .then_some(&definition.predicate_subject_type)
                    }
                    SymbolKind::Parameter
                        if program.symbols.get(declaration.parent).kind == SymbolKind::Measure =>
                    {
                        let measure = program
                            .measures
                            .iter()
                            .find(|measure| measure.symbol == declaration.parent)?;
                        let parameter = measure.parameter.as_ref()?;
                        (parameter.symbol == symbol).then_some(&parameter.type_reference)
                    }
                    SymbolKind::Parameter => program
                        .tables
                        .declarations
                        .state_parameters
                        .iter()
                        .find_map(|(_, parameter)| {
                            (parameter.symbol == symbol).then_some(&parameter.type_reference)
                        }),
                    SymbolKind::Local => {
                        let state = program.tables.declarations.machine_states.iter().find_map(
                            |(_, state)| (state.symbol == declaration.parent).then_some(state),
                        )?;
                        program
                            .state_statements(state.statements)
                            .iter()
                            .find_map(|statement| {
                                let resolved::statement::Statement::LocalData(local) = statement
                                else {
                                    return None;
                                };
                                (local.symbol == symbol).then_some(&local.type_reference)
                            })
                    }
                    SymbolKind::Field => {
                        for definition in &program.data_definitions {
                            if let Some(reference) = program
                                .data_members(definition.members)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Field(field)
                                        if field.symbol == symbol
                                            && exact_field_symbol(
                                                program,
                                                definition.symbol,
                                                field,
                                            ) =>
                                    {
                                        Some(&field.type_reference)
                                    }
                                    _ => None,
                                })
                            {
                                return Some(reference);
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
        }
        pub(crate) mod structural_equality {
            use super::lowerer::ExpressionTableLowerer;
            use crate::expressions::equatable::{
                DataEqualityShape, EqualitySelection, FieldEquality, data_definition_by_name_from,
                data_equality_shape, equality_selection, field_equality, implementation_name,
                suggested_equality_declaration, synthesized_equality_row, value_type_base_name,
            };
            use crate::lowerer::name::lower_name;
            use arena::HandleSpan;
            use diagnostics::Diagnostic;
            use resolved::data::{DataDefinition, DataField, DataMember, DataVariant};
            use symbol_resolved_trees as resolved;
            use typed_trees as typed;

            enum Operand {
                Place(typed::expression::ExpressionHandle),
                Literal {
                    case_name: Option<String>,
                    fields: HandleSpan<resolved::expression::TableStructLiteralField>,
                },
            }

            impl<'program, 'target, 'scope> ExpressionTableLowerer<'program, 'target, 'scope> {
                pub(crate) fn try_lower_synthesized_equatable_call(
                    &mut self,
                    expression: resolved::expression::ExpressionHandle,
                    call: &resolved::expression::TableCallExpression,
                ) -> Result<Option<typed::expression::ExpressionHandle>, Diagnostic>
                {
                    let Some(program) = self.program else {
                        return Ok(None);
                    };
                    if !call.target_symbol.is_valid() {
                        return Ok(None);
                    }
                    let Some(data) = synthesized_equality_row(program, call.target_symbol) else {
                        return Ok(None);
                    };
                    let [left, right] = self.source.expression_handles(call.arguments) else {
                        return Ok(None);
                    };
                    let (left, right) = (*left, *right);
                    for operand in [left, right] {
                        if matches!(self.source.expression(operand),
                resolved::expression::ExpressionNode::Borrow(borrow)
                    if borrow.access != language_semantics::ReferenceAccess::Shared)
                        {
                            return Ok(None);
                        }
                        let Some(type_name) = self.operand_data_type_name(operand) else {
                            return Ok(None);
                        };
                        if data_definition_by_name_from(
                            program,
                            &type_name,
                            self.source.source_span(operand),
                        )
                        .is_none_or(|operand_data| operand_data.symbol != data.symbol)
                        {
                            return Ok(None);
                        }
                    }
                    self.finalize_generated_call_selection(expression, call.target_symbol)?;
                    let observed_operand = |operand| match self.source.expression(operand) {
                        resolved::expression::ExpressionNode::Borrow(borrow)
                            if borrow.access == language_semantics::ReferenceAccess::Shared =>
                        {
                            borrow.target
                        }
                        _ => operand,
                    };
                    let equality = resolved::expression::TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: observed_operand(left),
                        operator: resolved::expression::BinaryOperator::Equal,
                        right: observed_operand(right),
                    };
                    if data.quotient.is_none()
                        && data_equality_shape(program, data) == DataEqualityShape::Structural
                    {
                        return self.expand_structural_equality(data, &equality).map(Some);
                    }

                    let left = self.lower(equality.left)?;
                    let right = self.lower(equality.right)?;
                    Ok(Some(self.target().insert(
                        typed::expression::ExpressionNode::Binary(
                            typed::expression::TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left,
                                operator: typed::expression::BinaryOperator::Equal,
                                right,
                            },
                        ),
                    )))
                }

                pub(crate) fn try_lower_structural_equality(
                    &mut self,
                    binary: &resolved::expression::TableBinaryExpression,
                ) -> Result<Option<typed::expression::ExpressionHandle>, Diagnostic>
                {
                    let Some(program) = self.program else {
                        return Ok(None);
                    };

                    if self.classifier_reference_operand(binary.left)
                        || self.classifier_reference_operand(binary.right)
                    {
                        let left_type = self
                            .classifier_reference_type(binary.left)
                            .or_else(|| self.operand_data_type_name(binary.left));
                        let right_type = self
                            .classifier_reference_type(binary.right)
                            .or_else(|| self.operand_data_type_name(binary.right));
                        if let Some((left_name, right_name)) = Self::different_data_type_names(
                            program,
                            &left_type,
                            &right_type,
                            self.source.source_span(binary.left),
                            self.source.source_span(binary.right),
                        ) {
                            return Err(Diagnostic::error(format!(
                                "cannot compare `{left_name}` and `{right_name}` with `==`/`!=`: the operands \
                     are different data types (a case belongs to its own type only)"
                            )));
                        }
                        return Ok(None);
                    }

                    let left_type = self.operand_data_type_name(binary.left);
                    let right_type = self.operand_data_type_name(binary.right);
                    if let Some((left_name, right_name)) = Self::different_data_type_names(
                        program,
                        &left_type,
                        &right_type,
                        self.source.source_span(binary.left),
                        self.source.source_span(binary.right),
                    ) {
                        return Err(Diagnostic::error(format!(
                            "cannot compare `{left_name}` and `{right_name}` with `==`/`!=`: the operands are \
                 different data types (equality requires both operands to be the same type)"
                        )));
                    }
                    let type_name = match left_type.or(right_type) {
                        Some(name) => name,
                        None => return Ok(None),
                    };
                    let Some(data) = data_definition_by_name_from(
                        program,
                        &type_name,
                        self.source.source_span(binary.left),
                    ) else {
                        return Ok(None);
                    };
                    if data.quotient.is_some() {
                        return Err(Diagnostic::error(format!(
                            "cannot compare quotient `{type_name}` values with `==`/`!=`: retained representatives are opaque and have no synthesized structural equality; call a named lifted equality operation"
                        )));
                    }
                    if data_equality_shape(program, data) != DataEqualityShape::Structural {
                        return Ok(None);
                    }

                    if self
                        .equality_scope
                        .is_some_and(|scope| scope.synthesized_row_data == data.symbol)
                    {
                        return self.expand_structural_equality(data, binary).map(Some);
                    }
                    let implementation = match equality_selection(program, data) {
                        EqualitySelection::Selected(implementation) => implementation,
                        EqualitySelection::Missing => {
                            return Err(Diagnostic::error(format!(
                                "cannot compare `{type_name}` values with `==`: structural equality is synthesized only through a named implementation -- declare `{}` to synthesize structural equality, or use `core::Identity<{type_name}>(left, right)` for mathematical identity",
                                suggested_equality_declaration(&type_name)
                            )));
                        }
                        EqualitySelection::Ambiguous(implementations) => {
                            return Err(Diagnostic::error(format!(
                                "cannot compare `{type_name}` values with `==`: {} Equatable implementations satisfy `Equatable<{type_name}>` (`{}`) and the operator selects at most one -- call the intended implementation's `equals` explicitly",
                                implementations.len(),
                                implementations.join("`, `")
                            )));
                        }
                    };
                    if implementation.synthesized {
                        return self.expand_structural_equality(data, binary).map(Some);
                    }
                    if !implementation.equals_state.is_valid() {
                        return Err(Diagnostic::error(format!(
                            "cannot compare `{type_name}` values with `==`: Equatable implementation `{}` retains no `equals` row; declare it as a closed block",
                            implementation_name(program, implementation.conformance)
                        )));
                    }

                    let conformance = implementation.conformance;
                    let mut members = HandleSpan::empty();
                    let receiver_name = conformance.alias.as_ref().map_or_else(
                        || {
                            typed::name::Identifier::generated(implementation_name(
                                program,
                                conformance,
                            ))
                        },
                        lower_name,
                    );
                    self.target()
                        .push_name_path_member(&mut members, receiver_name);
                    let mut member_symbols = HandleSpan::empty();
                    self.target()
                        .push_name_path_member_symbol(&mut member_symbols, conformance.symbol);
                    let receiver = self
                        .target()
                        .insert(typed::expression::ExpressionNode::Name(
                            typed::expression::TableNamePath::new(
                                members,
                                member_symbols,
                                conformance.symbol,
                                conformance.symbol,
                            ),
                        ));
                    let mut arguments = Vec::with_capacity(2);
                    for operand in [binary.left, binary.right] {
                        let target = self.lower(operand)?;
                        let argument =
                            self.target()
                                .insert(typed::expression::ExpressionNode::Borrow(
                                    typed::expression::TableBorrowExpression {
                                        target,
                                        access: language_semantics::ReferenceAccess::Shared,
                                    },
                                ));
                        arguments.push(argument);
                    }
                    let arguments = self.target().insert_expression_handles(arguments);
                    let call = self
            .target()
            .insert(typed::expression::ExpressionNode::Call(
                typed::expression::TableCallExpression {
             authored_call_selection: None,
             authored_call_binding: None,
                    authored_machine_application: Default::default(),
                    requirement_application: Default::default(),
                    exact_coercion_target: typed::types::TypeReferenceHandle::invalid(),
                    receiver,
                    target_symbol: implementation.equals_state,
                    target: typed::name::Identifier::generated_static("equals"),
                    static_machine_parameter: symbols::SymbolHandle::invalid(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
            installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement:
                        language_semantics::CallOperationalAcknowledgement {
                            origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                            ..Default::default()
                        },
                },
            ));
                    Ok(Some(self.with_equality_polarity(call, binary.operator)))
                }

                fn expand_structural_equality(
                    &mut self,
                    data: &'program DataDefinition,
                    binary: &resolved::expression::TableBinaryExpression,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let type_name = data.name.as_str().to_owned();
                    let mut visiting = vec![type_name.clone()];
                    let left = self.classify_operand(binary.left, &type_name)?;
                    let right = self.classify_operand(binary.right, &type_name)?;
                    let equality = self.structural_equality(data, &left, &right, &mut visiting)?;
                    Ok(self.with_equality_polarity(equality, binary.operator))
                }

                fn with_equality_polarity(
                    &mut self,
                    equality: typed::expression::ExpressionHandle,
                    operator: resolved::expression::BinaryOperator,
                ) -> typed::expression::ExpressionHandle {
                    match operator {
                        resolved::expression::BinaryOperator::NotEqual => {
                            let negated = self
                                .target()
                                .insert(typed::expression::ExpressionNode::Boolean(false));
                            self.target()
                                .insert(typed::expression::ExpressionNode::Binary(
                                    typed::expression::TableBinaryExpression {
                                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                        left: equality,
                                        operator: typed::expression::BinaryOperator::Equal,
                                        right: negated,
                                    },
                                ))
                        }
                        _ => equality,
                    }
                }

                fn classifier_reference_operand(
                    &self,
                    operand: resolved::expression::ExpressionHandle,
                ) -> bool {
                    if !operand.is_valid() {
                        return false;
                    }
                    match self.source.expression(operand) {
                        resolved::expression::ExpressionNode::Name(path) => {
                            self.source.name_path_members(path.members).len() >= 2
                        }
                        resolved::expression::ExpressionNode::Borrow(inner) => {
                            self.classifier_reference_operand(inner.target)
                        }
                        _ => false,
                    }
                }

                fn classifier_reference_type(
                    &self,
                    operand: resolved::expression::ExpressionHandle,
                ) -> Option<String> {
                    if !operand.is_valid() {
                        return None;
                    }
                    match self.source.expression(operand) {
                        resolved::expression::ExpressionNode::Name(path) => {
                            let members = self.source.name_path_members(path.members);
                            (members.len() >= 2).then(|| members[0].as_str().to_owned())
                        }
                        resolved::expression::ExpressionNode::Borrow(inner) => {
                            self.classifier_reference_type(inner.target)
                        }
                        _ => None,
                    }
                }

                fn different_data_type_names<'a>(
                    program: &resolved::SymbolResolvedTrees,
                    left_type: &'a Option<String>,
                    right_type: &'a Option<String>,
                    left_span: source::SourceSpan,
                    right_span: source::SourceSpan,
                ) -> Option<(&'a String, &'a String)> {
                    let (Some(left_name), Some(right_name)) = (left_type, right_type) else {
                        return None;
                    };
                    (left_name != right_name
                        && data_definition_by_name_from(program, left_name, left_span).is_some()
                        && data_definition_by_name_from(program, right_name, right_span).is_some())
                    .then_some((left_name, right_name))
                }

                fn operand_data_type_name(
                    &self,
                    operand: resolved::expression::ExpressionHandle,
                ) -> Option<String> {
                    if !operand.is_valid() {
                        return None;
                    }
                    let program = self.program?;
                    match self.source.expression(operand) {
                        resolved::expression::ExpressionNode::StructLiteral(literal) => {
                            Some(literal.type_name.as_str().to_owned())
                        }
                        resolved::expression::ExpressionNode::Borrow(inner) => {
                            self.operand_data_type_name(inner.target)
                        }
                        resolved::expression::ExpressionNode::Name(path) => {
                            if let Some(reference) =
                                super::member_binding::declared_symbol_type(program, path.symbol)
                                && let Some(name) = value_type_base_name(program, reference)
                            {
                                return Some(name);
                            }
                            let members = self.source.name_path_members(path.members);
                            let scope = self.equality_scope?;
                            let [name] = members else {
                                return None;
                            };
                            scope.value_type(name.as_str()).map(str::to_owned)
                        }
                        resolved::expression::ExpressionNode::Member(member) => {
                            let receiver_type = self.operand_data_type_name(member.receiver)?;
                            let data = data_definition_by_name_from(
                                program,
                                &receiver_type,
                                self.source.source_span(member.receiver),
                            )?;
                            let field = program.data_members(data.members).iter().find_map(
                                |data_member| match data_member {
                                    DataMember::Field(field)
                                        if field.name.as_str() == member.member.as_str() =>
                                    {
                                        Some(field)
                                    }
                                    _ => None,
                                },
                            )?;
                            value_type_base_name(program, &field.type_reference)
                        }
                        _ => None,
                    }
                }

                fn classify_operand(
                    &mut self,
                    operand: resolved::expression::ExpressionHandle,
                    type_name: &str,
                ) -> Result<Operand, Diagnostic> {
                    match self.source.expression(operand) {
                        resolved::expression::ExpressionNode::StructLiteral(literal) => {
                            Ok(Operand::Literal {
                                case_name: literal
                                    .case_name
                                    .as_ref()
                                    .map(|name| name.as_str().to_owned()),
                                fields: literal.fields,
                            })
                        }
                        resolved::expression::ExpressionNode::Borrow(inner) => {
                            self.classify_operand(inner.target, type_name)
                        }
                        resolved::expression::ExpressionNode::Name(_)
                        | resolved::expression::ExpressionNode::Member(_)
                        | resolved::expression::ExpressionNode::Indexed(_) => {
                            Ok(Operand::Place(self.lower(operand)?))
                        }
                        _ => Err(Diagnostic::error(format!(
                            "cannot synthesize structural `==` for this `{type_name}` operand: bind the value to a local first"
                        ))),
                    }
                }

                fn structural_equality(
                    &mut self,
                    data: &'program DataDefinition,
                    left: &Operand,
                    right: &Operand,
                    visiting: &mut Vec<String>,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let program = self
                        .program
                        .expect("structural equality requires program context");
                    let members = program.data_members(data.members);

                    let variants: Vec<&DataVariant> = members
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(variant) => Some(variant),
                            _ => None,
                        })
                        .collect();

                    let fields: Vec<&DataField> = members
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect();

                    if variants.is_empty() {
                        if !fields.is_empty()
                            && fields.iter().all(|field| field.relevance.is_erased())
                        {
                            return Err(Diagnostic::error(format!(
                                "cannot synthesize structural `==` for `{}`: every field is `[erased]`, so the record has no runtime value to compare",
                                data.name
                            )));
                        }
                        let compares =
                            self.field_compares(data, &fields, None, left, right, visiting)?;
                        return Ok(self.conjunction(compares));
                    }

                    if !fields.is_empty() {
                        let mut compares =
                            self.field_compares(data, &fields, None, left, right, visiting)?;
                        compares.push(self.sum_equality(data, &variants, left, right, visiting)?);
                        return Ok(self.conjunction(compares));
                    }

                    self.sum_equality(data, &variants, left, right, visiting)
                }

                fn field_compares(
                    &mut self,
                    data: &'program DataDefinition,
                    fields: &[&DataField],
                    case_variant: Option<&DataVariant>,
                    left: &Operand,
                    right: &Operand,
                    visiting: &mut Vec<String>,
                ) -> Result<Vec<typed::expression::ExpressionHandle>, Diagnostic> {
                    let mut compares = Vec::new();
                    for field in fields {
                        if field.relevance.is_erased() {
                            continue;
                        }
                        compares.push(self.field_compare(
                            data,
                            field,
                            case_variant,
                            left,
                            right,
                            visiting,
                        )?);
                    }
                    Ok(compares)
                }

                fn sum_equality(
                    &mut self,
                    data: &'program DataDefinition,
                    variants: &[&DataVariant],
                    left: &Operand,
                    right: &Operand,
                    visiting: &mut Vec<String>,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let program = self
                        .program
                        .expect("structural equality requires program context");

                    match (left, right) {
                        (
                            Operand::Literal {
                                case_name: Some(left_case),
                                ..
                            },
                            Operand::Literal {
                                case_name: Some(right_case),
                                ..
                            },
                        ) => {
                            if left_case != right_case {
                                return Ok(self
                                    .target()
                                    .insert(typed::expression::ExpressionNode::Boolean(false)));
                            }
                            let variant = self.sum_variant(data, variants, left_case)?;
                            let payload: Vec<&DataField> = program
                                .data_payload_fields(variant.payload)
                                .iter()
                                .collect();
                            let compares = self.field_compares(
                                data,
                                &payload,
                                Some(variant),
                                left,
                                right,
                                visiting,
                            )?;
                            Ok(self.conjunction(compares))
                        }
                        (
                            Operand::Literal {
                                case_name: None, ..
                            },
                            Operand::Literal { .. },
                        )
                        | (
                            Operand::Literal { .. },
                            Operand::Literal {
                                case_name: None, ..
                            },
                        ) => Err(Diagnostic::error(format!(
                            "cannot compare `{}` values against a record literal: the type is a sum of cases",
                            data.name
                        ))),
                        (Operand::Place(_), Operand::Literal { case_name, fields })
                        | (Operand::Literal { case_name, fields }, Operand::Place(_)) => {
                            let Some(case_name) = case_name else {
                                return Err(Diagnostic::error(format!(
                                    "cannot compare `{}` values against a record literal: the type is a sum of cases",
                                    data.name
                                )));
                            };
                            let place = match (left, right) {
                                (Operand::Place(place), _) | (_, Operand::Place(place)) => *place,
                                _ => unreachable!("one side is a place in this arm"),
                            };
                            let literal = Operand::Literal {
                                case_name: Some(case_name.clone()),
                                fields: *fields,
                            };
                            let variant = self.sum_variant(data, variants, case_name)?;
                            let tag_compare = self.tag_compare(data, variant, place);
                            let payload: Vec<&DataField> = program
                                .data_payload_fields(variant.payload)
                                .iter()
                                .collect();
                            let place_operand = Operand::Place(place);
                            let mut compares = vec![tag_compare];
                            compares.extend(self.field_compares(
                                data,
                                &payload,
                                Some(variant),
                                &place_operand,
                                &literal,
                                visiting,
                            )?);
                            Ok(self.conjunction(compares))
                        }
                        (Operand::Place(left_place), Operand::Place(right_place)) => {
                            let (left_place, right_place) = (*left_place, *right_place);
                            let mut arms = Vec::new();
                            for variant in variants {
                                let left_tag = self.tag_compare(data, variant, left_place);
                                let right_tag = self.tag_compare(data, variant, right_place);
                                let payload: Vec<&DataField> = program
                                    .data_payload_fields(variant.payload)
                                    .iter()
                                    .collect();
                                let mut compares = vec![left_tag, right_tag];
                                compares.extend(self.field_compares(
                                    data,
                                    &payload,
                                    Some(variant),
                                    &Operand::Place(left_place),
                                    &Operand::Place(right_place),
                                    visiting,
                                )?);
                                arms.push(self.conjunction(compares));
                            }
                            Ok(self.disjunction(arms))
                        }
                    }
                }

                fn sum_variant(
                    &self,
                    data: &'program DataDefinition,
                    variants: &[&'program DataVariant],
                    case_name: &str,
                ) -> Result<&'program DataVariant, Diagnostic> {
                    variants
                        .iter()
                        .copied()
                        .find(|variant| variant.name.as_str() == case_name)
                        .ok_or_else(|| {
                            Diagnostic::error(format!(
                                "`{}` has no case `{case_name}` to compare against",
                                data.name
                            ))
                        })
                }

                fn field_compare(
                    &mut self,
                    data: &'program DataDefinition,
                    field: &DataField,
                    case_variant: Option<&DataVariant>,
                    left: &Operand,
                    right: &Operand,
                    visiting: &mut Vec<String>,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    let program = self
                        .program
                        .expect("structural equality requires program context");
                    let context = || format!("structural equality of `{}`", visiting[0]);
                    match field_equality(program, context, data, field)? {
                        equality @ (FieldEquality::Direct | FieldEquality::Text) => {
                            if matches!(equality, FieldEquality::Text)
                                && (matches!(left, Operand::Literal { .. })
                                    || matches!(right, Operand::Literal { .. }))
                            {
                                return Err(Diagnostic::error(format!(
                                    "cannot synthesize structural `==` for `String` field `{}` of `{}` against a constructed literal: text content equality compares stored values -- bind the literal to a value first",
                                    field.name, data.name
                                )));
                            }
                            let left_value = self.operand_field_value(field, case_variant, left)?;
                            let right_value =
                                self.operand_field_value(field, case_variant, right)?;
                            Ok(self
                                .target()
                                .insert(typed::expression::ExpressionNode::Binary(
                                    typed::expression::TableBinaryExpression {
                                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                        left: left_value,
                                        operator: typed::expression::BinaryOperator::Equal,
                                        right: right_value,
                                    },
                                )))
                        }
                        FieldEquality::Structural(nested) => {
                            let nested_name = nested.name.as_str();
                            if visiting.iter().any(|name| name == nested_name) {
                                return Err(Diagnostic::error(format!(
                                    "{}: type `{nested_name}` is recursive through field `{}` of `{}`: synthesized structural equality would not terminate, so recursive Equatable implementations are not supported yet",
                                    context(),
                                    field.name,
                                    data.name
                                )));
                            }
                            let left_nested =
                                self.operand_field_operand(field, case_variant, left, nested_name)?;
                            let right_nested = self.operand_field_operand(
                                field,
                                case_variant,
                                right,
                                nested_name,
                            )?;
                            visiting.push(nested_name.to_owned());
                            let equality = self.structural_equality(
                                nested,
                                &left_nested,
                                &right_nested,
                                visiting,
                            )?;
                            visiting.pop();
                            Ok(equality)
                        }
                    }
                }

                fn operand_field_value(
                    &mut self,
                    field: &DataField,
                    case_variant: Option<&DataVariant>,
                    operand: &Operand,
                ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
                    match operand {
                        Operand::Place(place) => Ok(self.member_read(*place, field, case_variant)),
                        Operand::Literal { .. } => {
                            let value = self.literal_field_expression(field, operand)?;
                            self.lower(value)
                        }
                    }
                }

                fn operand_field_operand(
                    &mut self,
                    field: &DataField,
                    case_variant: Option<&DataVariant>,
                    operand: &Operand,
                    field_type_name: &str,
                ) -> Result<Operand, Diagnostic> {
                    match operand {
                        Operand::Place(place) => Ok(Operand::Place(self.member_read(
                            *place,
                            field,
                            case_variant,
                        ))),
                        Operand::Literal { .. } => {
                            let value = self.literal_field_expression(field, operand)?;
                            self.classify_operand(value, field_type_name)
                        }
                    }
                }

                fn literal_field_expression(
                    &self,
                    field: &DataField,
                    operand: &Operand,
                ) -> Result<resolved::expression::ExpressionHandle, Diagnostic> {
                    let Operand::Literal { fields, .. } = operand else {
                        unreachable!("literal field access on a place operand")
                    };
                    if let Some(literal_field) = self
                        .source
                        .struct_fields(*fields)
                        .iter()
                        .find(|literal_field| literal_field.name.as_str() == field.name.as_str())
                    {
                        return Ok(literal_field.value);
                    }
                    Err(Diagnostic::error(format!(
                        "cannot synthesize structural `==`: the literal omits field `{}`",
                        field.name
                    )))
                }

                fn member_read(
                    &mut self,
                    place: typed::expression::ExpressionHandle,
                    field: &DataField,
                    case_variant: Option<&DataVariant>,
                ) -> typed::expression::ExpressionHandle {
                    self.target()
                        .insert(typed::expression::ExpressionNode::Member(
                            typed::expression::TableMemberExpression {
                                receiver: place,
                                selection: typed::expression::MemberSelection::Field(field.symbol),
                                member: lower_name(&field.name),
                                case_variant: case_variant.map(|variant| lower_name(&variant.name)),
                            },
                        ))
                }

                fn tag_compare(
                    &mut self,
                    data: &DataDefinition,
                    variant: &DataVariant,
                    place: typed::expression::ExpressionHandle,
                ) -> typed::expression::ExpressionHandle {
                    let mut members = HandleSpan::empty();
                    self.target()
                        .push_name_path_member(&mut members, lower_name(&data.name));
                    self.target()
                        .push_name_path_member(&mut members, lower_name(&variant.name));
                    let mut member_symbols = HandleSpan::empty();
                    self.target()
                        .push_name_path_member_symbol(&mut member_symbols, data.symbol);
                    self.target()
                        .push_name_path_member_symbol(&mut member_symbols, variant.symbol);
                    let case_reference =
                        self.target()
                            .insert(typed::expression::ExpressionNode::Name(
                                typed::expression::TableNamePath::new(
                                    members,
                                    member_symbols,
                                    data.symbol,
                                    variant.symbol,
                                ),
                            ));
                    self.target()
                        .insert(typed::expression::ExpressionNode::Binary(
                            typed::expression::TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left: place,
                                operator: typed::expression::BinaryOperator::CaseMembership,
                                right: case_reference,
                            },
                        ))
                }

                fn conjunction(
                    &mut self,
                    compares: Vec<typed::expression::ExpressionHandle>,
                ) -> typed::expression::ExpressionHandle {
                    self.fold_binary(compares, typed::expression::BinaryOperator::And, true)
                }

                fn disjunction(
                    &mut self,
                    arms: Vec<typed::expression::ExpressionHandle>,
                ) -> typed::expression::ExpressionHandle {
                    self.fold_binary(arms, typed::expression::BinaryOperator::Or, false)
                }

                fn fold_binary(
                    &mut self,
                    parts: Vec<typed::expression::ExpressionHandle>,
                    operator: typed::expression::BinaryOperator,
                    empty_value: bool,
                ) -> typed::expression::ExpressionHandle {
                    let mut parts = parts.into_iter();
                    let Some(mut combined) = parts.next() else {
                        return self
                            .target()
                            .insert(typed::expression::ExpressionNode::Boolean(empty_value));
                    };
                    for part in parts {
                        combined = self
                            .target()
                            .insert(typed::expression::ExpressionNode::Binary(
                                typed::expression::TableBinaryExpression {
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                    left: combined,
                                    operator,
                                    right: part,
                                },
                            ));
                    }
                    combined
                }
            }
        }

        pub(crate) use member_binding::declared_receiver_method_target;

        use crate::expressions::equatable::EqualityScope;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_expression_handle_from_table_with_symbol_substitution(
            program: Option<&resolved::SymbolResolvedTrees>,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            expression: resolved::expression::ExpressionHandle,
            symbol_substitution: Option<(
                symbols::SymbolHandle,
                typed::expression::ExpressionHandle,
            )>,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(program, source, target, symbol_substitution, None)
                .lower(expression)
        }

        pub(crate) fn lower_expression_handle_from_table_in_scope(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            expression: resolved::expression::ExpressionHandle,
            scope: Option<&EqualityScope>,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, scope)
                .lower(expression)
        }

        pub(crate) fn lower_executable_expression_handle_from_table(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            expression: resolved::expression::ExpressionHandle,
            scope: Option<&EqualityScope>,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, scope)
                .admitting_policy_operators(true)
                .lower(expression)
        }

        pub(crate) fn lower_expression_handle_from_table_in_fact_position(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, None)
                .in_fact_position()
                .lower(expression)
        }

        pub(crate) fn lower_scoped_fact_expression(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            expression: resolved::expression::ExpressionHandle,
            scope: Option<&EqualityScope>,
            admits_policy_operators: bool,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, scope)
                .in_fact_position()
                .admitting_policy_operators(admits_policy_operators)
                .lower(expression)
        }

        pub(crate) fn lower_transition_pattern(
            program: &resolved::SymbolResolvedTrees,
            source: &resolved::expression::ExpressionTable,
            target: &mut typed::TypedTrees,
            pattern: resolved::statement::TransitionPattern,
        ) -> Result<typed::statement::TransitionPattern, Diagnostic> {
            lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, None)
                .lower_transition_pattern(pattern)
        }
    }
    pub(crate) use table::declared_receiver_method_target;

    /// Lowers an expression a machine body evaluates. Only these may use a
    /// suffixed policy operator.
    pub(crate) fn lower_executable_expression_handle(
        lowerer: &mut Lowerer,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let Lowerer {
            typed_trees,
            source_trees,
            equality_scope,
            ..
        } = lowerer;
        let source = &source_trees.tables.bodies.expressions;
        table::lower_executable_expression_handle_from_table(
            source_trees,
            source,
            typed_trees,
            expression,
            equality_scope.as_ref(),
        )
    }

    pub(crate) fn lower_expression_handle(
        lowerer: &mut Lowerer,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let Lowerer {
            typed_trees,
            source_trees,
            equality_scope,
            ..
        } = lowerer;
        let source = &source_trees.tables.bodies.expressions;
        table::lower_expression_handle_from_table_in_scope(
            source_trees,
            source,
            typed_trees,
            expression,
            equality_scope.as_ref(),
        )
    }

    pub(crate) fn lower_expression_handle_from_table_with_symbol_substitution(
        program: Option<&resolved::SymbolResolvedTrees>,
        source: &resolved::expression::ExpressionTable,
        target: &mut typed::TypedTrees,
        expression: resolved::expression::ExpressionHandle,
        symbol_substitution: Option<(symbols::SymbolHandle, typed::expression::ExpressionHandle)>,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        table::lower_expression_handle_from_table_with_symbol_substitution(
            program,
            source,
            target,
            expression,
            symbol_substitution,
        )
    }

    /// Lower an expression in proof-fact position. Erased propositions retain
    /// their placement checks; comparisons use the ordinary selection rules.
    pub(crate) fn lower_expression_handle_from_table_in_fact_position(
        program: &resolved::SymbolResolvedTrees,
        source: &resolved::expression::ExpressionTable,
        target: &mut typed::TypedTrees,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        table::lower_expression_handle_from_table_in_fact_position(
            program, source, target, expression,
        )
    }

    pub(crate) fn lower_static_machine_argument(
        program: Option<&resolved::SymbolResolvedTrees>,
        target: &mut typed::TypedTrees,
        exposure: Option<SelectionExposure>,
        argument: &resolved::expression::StaticMachineArgument,
    ) -> Result<typed::expression::StaticMachineArgument, Diagnostic> {
        let type_reference = if argument.type_reference.is_valid() {
            let program = program.ok_or_else(|| {
                Diagnostic::error("structural static type argument requires its resolved program")
            })?;
            crate::type_reference::lower_type_reference_into_trees_with_exposure(
                program,
                target,
                program.child_type_reference(argument.type_reference),
                exposure,
            )?
        } else {
            typed::types::TypeReferenceHandle::invalid()
        };
        let mut selected = typed::expression::StaticMachineArgument {
            type_reference,
            path: argument
                .path
                .iter()
                .map(crate::lowerer::name::lower_name)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            application: argument
                .application
                .as_ref()
                .map(|application| {
                    Ok::<_, Diagnostic>(Box::new(typed::expression::StaticSymbolApplication {
                        member: if application.member.is_valid() {
                            let owner = program.ok_or_else(|| {
                                Diagnostic::error(
                                    "qualified static member requires its resolved source program",
                                )
                            })?;
                            let source = resolved_expressions(owner)
                                .static_member(application.member)
                                .ok_or_else(|| {
                                    Diagnostic::error(
                                        "qualified static member has no owning resolved record",
                                    )
                                })?;
                            let member =
                                lower_static_machine_argument(program, target, exposure, source)?;
                            target.expression_table.insert_static_member(member)
                        } else {
                            arena::Handle::invalid()
                        },
                        lifetime_arguments: application
                            .lifetime_arguments
                            .iter()
                            .map(crate::lowerer::name::lower_name)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments: application
                            .arguments
                            .iter()
                            .map(|argument| {
                                lower_static_machine_argument(program, target, exposure, argument)
                            })
                            .collect::<Result<Vec<_>, Diagnostic>>()?
                            .into_boxed_slice(),
                    }))
                })
                .transpose()?,
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
                typed::expression::EvidenceProjection {
                    term: crate::lowerer::name::lower_name(&projection.term),
                    member: crate::lowerer::name::lower_name(&projection.member),
                }
            }),
            symbol: argument.symbol,
        };
        let mut captured =
            crate::signatures::lexical_context::captured_arguments(program, selected.symbol);
        if !captured.is_empty() {
            let application = selected.application.get_or_insert_with(|| {
                Box::new(typed::expression::StaticSymbolApplication {
                    member: arena::Handle::invalid(),
                    lifetime_arguments: Box::default(),
                    arguments: Box::default(),
                })
            });
            captured.extend(std::mem::take(&mut application.arguments));
            application.arguments = captured.into_boxed_slice();
        }
        if !selected.type_reference.is_valid()
            && selected
                .application
                .as_ref()
                .is_none_or(|application| !application.member.is_valid())
            && selected.symbol.is_valid()
            && target.symbols.get(selected.symbol).kind == symbols::SymbolKind::Data
            && selected.application.is_some()
        {
            let type_reference = target
                .static_operand_type_reference(&selected)
                .map_err(Diagnostic::error)?;
            selected = typed::expression::StaticMachineArgument {
                type_reference,
                ..typed::expression::StaticMachineArgument::default()
            };
        }
        if selected.type_reference.is_valid()
            && (target
                .static_const_literal_from_type_reference(selected.type_reference)
                .is_some()
                || matches!(target.type_reference_table.type_reference(selected.type_reference), typed::types::TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid()))
        {
            selected = target
                .static_operand_from_type_reference(selected.type_reference)
                .map_err(Diagnostic::error)?;
        }
        Ok(selected)
    }

    pub(crate) fn lower_fact_expression(
        lowerer: &mut Lowerer,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        table::lower_scoped_fact_expression(
            lowerer.source_trees,
            &lowerer.source_trees.tables.bodies.expressions,
            &mut lowerer.typed_trees,
            expression,
            lowerer.equality_scope.as_ref(),
            lowerer.admits_contract_policy_operators,
        )
    }
}
