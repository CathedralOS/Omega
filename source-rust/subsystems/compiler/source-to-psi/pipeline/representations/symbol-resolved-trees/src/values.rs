/*
We represent symbol-resolved value expressions (`values::expression`), static
member projections, and symbol/table subtree copying in `SymbolResolvedTrees`.
*/

pub mod expression {

    pub(crate) mod display {
        use diagnostics::{bracketed_display_names, comma_join_display};

        use crate::source_resolved::expression::{
            BinaryOperator, ExpressionNode, ExpressionTable, StaticMachineArgument,
            TableBinaryExpression, TableCallExpression, TableCastExpression, TableUnaryExpression,
            UnaryOperator,
        };
        use crate::source_resolved::name::DiagnosticName;

        impl ExpressionNode {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                match self {
                    Self::Match(dispatch) => {
                        let arms = table
                            .match_arms(dispatch.arms)
                            .iter()
                            .map(|arm| {
                                let pattern = match arm.pattern {
                                    crate::source_resolved::expression::MatchPattern::Value(value) => {
                                        table.display_name(value)
                                    }
                                    crate::source_resolved::expression::MatchPattern::Destructure(pattern) => {
                                        display_destructure_pattern(table, pattern)
                                    }
                                    crate::source_resolved::expression::MatchPattern::Wildcard => "_".to_owned(),
                                };
                                format!("{pattern} => {}", table.display_name(arm.value))
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(
                            "match {} {{ {arms} }}",
                            table.display_name(dispatch.subject)
                        )
                    }
                    Self::SavedSubject(saved) => {
                        format!("<saved subject #{}>", saved.capture.arena_index())
                    }
                    Self::ArrayLiteral(values) => {
                        bracketed_display_names(table.expression_handles(*values).iter(), |value| {
                            table.display_name(*value)
                        })
                    }
                    Self::Atomic(atomic) => format!(
                        "atomic[{:?}]({})",
                        atomic.ordering,
                        table.display_name(atomic.value)
                    ),
                    Self::Binary(binary) => binary.display_name(table),
                    Self::Boolean(value) => value.to_string(),
                    Self::Cast(cast) => cast.display_name(table),
                    Self::Call(call) => call.display_name(table),
                    Self::Float(value) => value.to_string(),
                    Self::UndeterminedNaN(format) => {
                        format!("<undetermined NaN {}>", format.name())
                    }
                    Self::Indexed(indexed) => {
                        format!(
                            "{}[{}]",
                            table.display_name(indexed.collection),
                            table.display_name(indexed.index)
                        )
                    }
                    Self::Integer(value) => value.to_string(),
                    Self::Membership(membership) => {
                        format!(
                            "{} in {}",
                            table.display_name(membership.value),
                            display_name_path(table.name_path_members(membership.domain), "::")
                        )
                    }
                    Self::Member(member) => {
                        format!("{}.{}", table.display_name(member.receiver), member.member)
                    }
                    Self::Borrow(expression) => format!(
                        "{}{}",
                        expression.access.borrow_prefix(),
                        table.display_name(expression.target)
                    ),
                    Self::Name(path) => {
                        if let Some(argument) = table.static_member(path.static_application) {
                            argument.display_name_in(table)
                        } else if path.static_application.is_valid() {
                            "<missing applied declaration>".to_owned()
                        } else {
                            display_name_path(table.name_path_members(path.members), "::")
                        }
                    }
                    Self::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
                        (true, true) => format!(
                            "{}..{}",
                            table.display_name(range.start),
                            table.display_name(range.end)
                        ),
                        (true, false) => format!("{}..", table.display_name(range.start)),
                        (false, true) => format!("..{}", table.display_name(range.end)),
                        (false, false) => "..".to_string(),
                    },
                    Self::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
                    Self::String(value) => source_text::display_literal_bytes(value),
                    Self::Unary(unary) => unary.display_name(table),
                    Self::TypeExpression(_) => "type".to_owned(),
                    Self::ZeroValue(_) => "zero_value<type>()".to_owned(),
                }
            }
        }

        fn display_destructure_pattern(
            table: &ExpressionTable,
            pattern: crate::source_resolved::expression::TableDestructurePattern,
        ) -> String {
            use crate::source_resolved::expression::MatchFieldAction;
            let mut fields = table
                .match_fields(pattern.fields)
                .iter()
                .map(|field| {
                    let member = field.member.as_str();
                    match &field.action {
                        MatchFieldAction::Bind(binding) if binding.as_str() == member => {
                            member.to_owned()
                        }
                        MatchFieldAction::Bind(binding) => {
                            format!("{member} as {}", binding.as_str())
                        }
                        MatchFieldAction::Equal(value) => {
                            format!("{member}: {}", table.display_name(*value))
                        }
                        MatchFieldAction::Waive => format!("{member} as _"),
                    }
                })
                .collect::<Vec<_>>();
            if pattern.has_rest {
                fields.push("..".to_owned());
            }
            let selectors = table.match_proof_selectors(pattern.proof_selectors);
            let proofs = if selectors.is_empty() {
                String::new()
            } else {
                format!(
                    "; {}",
                    selectors
                        .iter()
                        .map(|selector| format!(
                            "{}: {}",
                            selector.output_field.as_str(),
                            selector.binding.as_str()
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let guard = if pattern.guard.is_valid() {
                format!(" if {}", table.display_name(pattern.guard))
            } else {
                String::new()
            };
            format!(
                "{} {{ {}{proofs} }}{guard}",
                table.display_name(pattern.head),
                fields.join(", ")
            )
        }

        pub fn display_name_path(path: &[DiagnosticName], separator: &str) -> String {
            let byte_count = path.iter().map(|name| name.as_str().len()).sum::<usize>()
                + separator.len().saturating_mul(path.len().saturating_sub(1));
            let mut display_name = String::with_capacity(byte_count);

            for (index, name) in path.iter().enumerate() {
                if index > 0 {
                    display_name.push_str(separator);
                }

                display_name.push_str(name.as_str());
            }

            display_name
        }

        impl TableBinaryExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                format!(
                    "{} {}{} {}",
                    table.display_name(self.left),
                    self.operator.display_name(),
                    self.policy.operator_suffix(),
                    table.display_name(self.right)
                )
            }
        }

        impl TableUnaryExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                format!(
                    "{}{}",
                    self.operator.display_name(),
                    table.display_name(self.operand)
                )
            }
        }

        impl TableCastExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                let target_type =
                    display_name_path(table.name_path_members(self.target_label), "::");
                format!("{} as {}", table.display_name(self.value), target_type)
            }
        }

        impl TableCallExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                let machine_arguments = if self.machine_arguments.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        self.machine_arguments
                            .iter()
                            .map(|argument| argument.display_name_in(table))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let arguments =
                    comma_join_display(table.expression_handles(self.arguments), |argument| {
                        table.display_name(*argument)
                    });

                if self.receiver.is_valid() {
                    format!(
                        "{}.{}{machine_arguments}({arguments})",
                        table.display_name(self.receiver),
                        self.target
                    )
                } else {
                    format!("{}{machine_arguments}({arguments})", self.target)
                }
            }
        }

        impl StaticMachineArgument {
            pub fn display_name_in(&self, table: &ExpressionTable) -> String {
                self.display_name_with_owner(Some(table))
            }

            fn display_name_with_owner(&self, table: Option<&ExpressionTable>) -> String {
                if self.type_reference.is_valid() {
                    return "<structural type>".to_owned();
                }
                if let Some(literal) = &self.const_literal {
                    return literal.to_string();
                }
                if let Some(projection) = &self.evidence_projection {
                    return format!("{}.{}", projection.term, projection.member);
                }
                let mut rendered = display_name_path(&self.path, "::");
                if let Some(application) = &self.application {
                    let mut arguments = application
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| format!("'{lifetime}"))
                        .collect::<Vec<_>>();
                    arguments.extend(
                        application
                            .arguments
                            .iter()
                            .map(|argument| argument.display_name_with_owner(table)),
                    );
                    rendered.push('<');
                    rendered.push_str(&arguments.join(", "));
                    rendered.push('>');
                    if application.member.is_valid() {
                        rendered.push_str("::");
                        match table.and_then(|table| table.static_member(application.member)) {
                            Some(member) => {
                                rendered.push_str(&member.display_name_with_owner(table))
                            }
                            None => rendered.push_str("<qualified member requires owning table>"),
                        }
                    }
                }
                rendered
            }
        }

        impl BinaryOperator {
            pub fn display_name(self) -> &'static str {
                match self {
                    Self::Add => "+",
                    Self::And => "&&",
                    Self::BitwiseAnd => "&",
                    Self::BitwiseOr => "|",
                    Self::BitwiseXor => "^",
                    Self::Divide => "/",
                    Self::Equal => "==",
                    Self::Greater => ">",
                    Self::GreaterOrEqual => ">=",
                    Self::Less => "<",
                    Self::LessOrEqual => "<=",
                    Self::Modulo => "%",
                    Self::Multiply => "*",
                    Self::NotEqual => "!=",
                    Self::Or => "||",
                    Self::ShiftLeft => "<<",
                    Self::ShiftRight => ">>",
                    Self::Subtract => "-",
                }
            }
        }

        impl UnaryOperator {
            pub fn display_name(self) -> &'static str {
                match self {
                    Self::BitwiseNot => "~",
                    Self::LogicalNot => "!",
                }
            }
        }
    }
    pub(crate) mod static_members {
        use super::{ExpressionTable, StaticMachineArgument};
        use symbols::SymbolHandle;

        impl ExpressionTable {
            pub fn copy_static_argument_from(
                &mut self,
                source: &Self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(Some(source), argument)
            }

            pub fn copy_static_argument_from_self(
                &mut self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(None, argument)
            }

            fn copy_static_argument_members(
                &mut self,
                source: Option<&Self>,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                let mut copied = argument.clone();
                if let Some(application) = &mut copied.application {
                    application.arguments = application
                        .arguments
                        .iter()
                        .map(|argument| self.copy_static_argument_members(source, argument))
                        .collect();
                    if application.member.is_valid() {
                        let member = source
                            .unwrap_or(self)
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its source expression table",
                            )
                            .clone();
                        let copied_member = self.copy_static_argument_members(source, &member);
                        application.member = self.insert_static_member(copied_member);
                    }
                }
                copied
            }

            pub fn remap_static_argument_symbols(
                &mut self,
                argument: &mut StaticMachineArgument,
                symbols: &[(SymbolHandle, SymbolHandle)],
            ) {
                argument.authored_selection_target =
                    argument.authored_selection_target.map(|target| {
                        symbols
                            .iter()
                            .find(|(source, _)| *source == target)
                            .map_or(target, |(_, replacement)| *replacement)
                    });
                if let Some((_, replacement)) = symbols
                    .iter()
                    .find(|(source, _)| *source == argument.symbol)
                {
                    argument.symbol = *replacement;
                }
                if let Some(Ok(selected)) = &mut argument.signature_free_requirement {
                    for symbol in [
                        &mut selected.trait_definition,
                        &mut selected.declaring_trait,
                        &mut selected.requirement,
                    ] {
                        if let Some((_, replacement)) =
                            symbols.iter().find(|(source, _)| *source == *symbol)
                        {
                            *symbol = *replacement;
                        }
                    }
                }
                if let Some(application) = &mut argument.application {
                    for argument in application.arguments.iter_mut() {
                        self.remap_static_argument_symbols(argument, symbols);
                    }
                    if application.member.is_valid() {
                        let mut member = self
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its receiving expression table",
                            )
                            .clone();
                        self.remap_static_argument_symbols(&mut member, symbols);
                        *self.static_member_mut(application.member).expect(
                            "qualified static member remains in its receiving expression table",
                        ) = member;
                    }
                }
            }
        }
    }
    pub(crate) mod symbol_copies;
    pub(crate) mod table_copies;
    pub(crate) use table_copies::ExpressionSource;
    #[cfg(test)]
    pub(crate) mod tests {
        use super::{
            BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
            TableStructLiteral, TableStructLiteralField,
        };
        use crate::source_resolved::name::DiagnosticName;
        use crate::source_resolved::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
            AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelections,
        };
        use crate::source_selections::CompilerDerivedSelectionPartition;
        use arena::HandleSpan;
        use source_text::{SourceId, SourceSpan, Span};
        use symbols::SymbolHandle;

        #[test]
        fn pattern_dependencies_retain_the_combined_condition_once() {
            use super::{MatchFieldAction, MatchPattern, TableDestructurePattern, TableMatchField};

            let mut table = ExpressionTable::new();
            let head = table.insert(ExpressionNode::Boolean(true));
            let observed = table.insert(ExpressionNode::Boolean(false));
            let expected = table.insert(ExpressionNode::Boolean(true));
            let equality = table.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: observed,
                operator: BinaryOperator::Equal,
                right: expected,
            }));
            let guard = table.insert(ExpressionNode::Boolean(true));
            let condition = table.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: equality,
                operator: BinaryOperator::And,
                right: guard,
            }));
            let fields = table.insert_match_fields([TableMatchField {
                action: MatchFieldAction::Equal(expected),
                ..Default::default()
            }]);
            let pattern = MatchPattern::Destructure(TableDestructurePattern {
                head,
                fields,
                guard,
                condition,
                ..Default::default()
            });
            assert_eq!(
                table
                    .match_pattern_dependencies(pattern)
                    .collect::<Vec<_>>(),
                [head, condition]
            );
            let ExpressionNode::Binary(combined) = table.expression(condition) else {
                panic!("retained condition contains both predicate inputs");
            };
            assert_eq!([combined.left, combined.right], [equality, guard]);
            let ExpressionNode::Binary(comparison) = table.expression(combined.left) else {
                panic!("field comparison is retained inside the condition");
            };
            assert_eq!(comparison.right, expected);
            assert_eq!(
                table
                    .match_pattern_dependencies(MatchPattern::Value(equality))
                    .collect::<Vec<_>>(),
                [equality]
            );
            assert_eq!(
                table
                    .match_pattern_dependencies(MatchPattern::Wildcard)
                    .count(),
                0
            );
            assert_eq!(
                table
                    .match_pattern_dependencies(MatchPattern::Destructure(Default::default()))
                    .count(),
                0
            );
        }

        fn authored_selection_occurrences() -> [AuthoredDeclarationSelectionOccurrenceId; 2] {
            let mut selections = AuthoredDeclarationSelections::default();
            let first = selections
                .record_resolved(
                    SourceSpan::default(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::MemberAccess,
                    SymbolHandle::from_arena_index(31),
                )
                .expect("valid selected symbol");
            let second = selections
                .record_resolved(
                    SourceSpan::default(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::Call,
                    SymbolHandle::from_arena_index(32),
                )
                .expect("valid selected symbol");
            [first, second]
        }

        #[test]
        fn clearing_expression_storage_retires_its_handles_and_payload_spans() {
            let mut table = ExpressionTable::new();
            let original = table.insert(ExpressionNode::Boolean(true));
            let original_arguments = table.insert_expression_handles([original]);

            table.clear();
            let current = table.insert(ExpressionNode::Boolean(false));
            let current_arguments = table.insert_expression_handles([current]);
            assert_eq!(original.arena_index(), current.arena_index());
            assert_ne!(original.generation(), current.generation());
            assert!(!table.expression_is_valid(original));
            assert!(table.expression_is_valid(current));
            assert!(
                table
                    .spans
                    .expression_handles
                    .span(original_arguments)
                    .is_none()
            );
            assert_eq!(table.expression_handles(current_arguments), [current]);
        }

        #[test]
        fn expression_occurrences_support_multiple_ids_and_survive_resolved_copies() {
            let occurrences = authored_selection_occurrences();
            assert_eq!(occurrences[0].ordinal(), 0);

            let mut source = ExpressionTable::new();
            let expression = source.insert(ExpressionNode::Integer(
                numeric_literals::IntegerLiteral::from_value(7),
            ));
            source.set_authored_expression_exposure(
                expression,
                AuthoredDeclarationSelectionExposure::PublicInterface,
            );
            let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
            source.set_source_span(expression, source_span);
            let partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(9);
            source.set_compiler_selection_partition(expression, partition);
            source.attach_authored_selection_occurrences(expression, occurrences);

            assert_eq!(
                source
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
                occurrences
            );

            let mut copied = ExpressionTable::new();
            let copied_expression = copied.copy_from(&source, expression);
            assert_eq!(
                copied
                    .authored_selection_occurrences(copied_expression)
                    .collect::<Vec<_>>(),
                occurrences
            );
            assert_eq!(
                copied.authored_expression_exposure(copied_expression),
                Some(AuthoredDeclarationSelectionExposure::PublicInterface)
            );
            assert_eq!(copied.source_span(copied_expression), source_span);
            assert_eq!(
                copied.compiler_selection_partition(copied_expression),
                Some(partition)
            );

            let self_copied_expression = source.copy_from_self(expression);
            assert_eq!(
                source
                    .authored_selection_occurrences(self_copied_expression)
                    .collect::<Vec<_>>(),
                occurrences
            );
            assert_eq!(
                source.authored_expression_exposure(self_copied_expression),
                Some(AuthoredDeclarationSelectionExposure::PublicInterface)
            );
            assert_eq!(source.source_span(self_copied_expression), source_span);
            assert_eq!(
                source.compiler_selection_partition(self_copied_expression),
                Some(partition)
            );
        }

        #[test]
        fn expression_table_copies_table_payloads_without_tree_roundtrip() {
            let room_symbol = SymbolHandle::from_arena_index(3);
            let field_symbol = SymbolHandle::from_arena_index(4);

            let mut source = ExpressionTable::new();
            let room_members = source.reserve_name_path_members(1);
            source.set_name_path_member_at_offset(
                room_members,
                0,
                DiagnosticName::generated("room"),
            );
            let room_member_symbols = source.reserve_name_path_member_symbols(1);
            source.set_name_path_member_symbol_at_offset(room_member_symbols, 0, room_symbol);
            let room = source.insert(ExpressionNode::Name(TableNamePath {
                members: room_members,
                member_symbols: room_member_symbols,
                head_symbol: room_symbol,
                symbol: room_symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }));

            let field_members = source.reserve_name_path_members(2);
            source.set_name_path_member_at_offset(
                field_members,
                0,
                DiagnosticName::generated("room"),
            );
            source.set_name_path_member_at_offset(
                field_members,
                1,
                DiagnosticName::generated("field"),
            );
            let field_member_symbols = source.reserve_name_path_member_symbols(2);
            source.set_name_path_member_symbol_at_offset(field_member_symbols, 0, room_symbol);
            source.set_name_path_member_symbol_at_offset(field_member_symbols, 1, field_symbol);
            let field = source.insert(ExpressionNode::Name(TableNamePath {
                members: field_members,
                member_symbols: field_member_symbols,
                head_symbol: room_symbol,
                symbol: field_symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }));

            let hall = source.insert(ExpressionNode::String((&b"Hall"[..]).into()));
            let open = source.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: room,
                operator: BinaryOperator::Equal,
                right: field,
            }));
            let fields = source.insert_struct_fields([
                TableStructLiteralField {
                    name: DiagnosticName::generated("name"),
                    field_symbol: SymbolHandle::from_arena_index(6),
                    value: hall,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("open"),
                    field_symbol: SymbolHandle::from_arena_index(7),
                    value: open,
                },
            ]);
            let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                zero_omitted_fields: false,
                application_type: Default::default(),
                type_name: DiagnosticName::generated("Room"),
                type_symbol: SymbolHandle::from_arena_index(5),
                case_name: None,
                case_symbol: None,
                fields,
            }));

            let mut copied = ExpressionTable::new();
            let copied_root = copied.copy_from(&source, root);

            assert_eq!(source.display_name(root), copied.display_name(copied_root));

            let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root)
            else {
                panic!("copied root should remain a struct literal");
            };
            assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);
            assert_eq!(
                struct_literal.type_symbol,
                SymbolHandle::from_arena_index(5)
            );
            assert_eq!(
                copied.struct_fields(struct_literal.fields)[0].field_symbol,
                SymbolHandle::from_arena_index(6)
            );

            let open_field = &copied.struct_fields(struct_literal.fields)[1];
            let ExpressionNode::Binary(binary) = copied.expression(open_field.value) else {
                panic!("copied field should keep its binary expression");
            };
            let ExpressionNode::Name(TableNamePath {
                members,
                head_symbol,
                symbol,
                ..
            }) = copied.expression(binary.right)
            else {
                panic!("copied binary rhs should keep its name path");
            };

            assert_eq!(*head_symbol, room_symbol);
            assert_eq!(*symbol, field_symbol);
            assert_eq!(copied.name_path_members(*members).len(), 2);
            assert_eq!(
                copied.name_path_member_symbols(match copied.expression(binary.right) {
                    ExpressionNode::Name(path) => path.member_symbols,
                    _ => unreachable!(),
                }),
                [room_symbol, field_symbol]
            );
        }
        #[test]
        fn match_arms_preserve_order_source_and_nonnumeric_values() {
            use super::{MatchPattern, TableMatchArm, TableMatchExpression};
            let mut table = ExpressionTable::new();
            let subject = table.insert(ExpressionNode::Boolean(true));
            let pattern = table.insert(ExpressionNode::Boolean(false));
            let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
            let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
            let source_span = source_text::SourceSpan::new(
                source_text::SourceId(7),
                source_text::Span::new(11, 29),
            );
            let arms = table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Value(pattern),
                    value: first,
                    source_span,
                },
                TableMatchArm {
                    pattern: MatchPattern::Wildcard,
                    value: fallback,
                    source_span,
                },
            ]);
            let expression = table.insert(ExpressionNode::Match(TableMatchExpression {
                subject,
                arms,
                ..Default::default()
            }));
            table.set_source_span(expression, source_span);
            assert_eq!(
                table.match_arms(arms)[0].pattern,
                MatchPattern::Value(pattern)
            );
            assert_eq!(table.match_arms(arms)[1].pattern, MatchPattern::Wildcard);
            assert_eq!(table.match_arms(arms)[0].source_span, source_span);
            assert_eq!(
                table.display_name(expression),
                "match true { false => \"first\", _ => \"fallback\" }"
            );

            let mut copied = ExpressionTable::new();
            let copied_expression = copied.copy_from(&table, expression);
            assert_eq!(
                copied.display_name(copied_expression),
                table.display_name(expression)
            );
            assert_eq!(copied.source_span(copied_expression), source_span);
            let ExpressionNode::Match(dispatch) = copied.expression(copied_expression) else {
                panic!("expected match");
            };
            assert_eq!(copied.match_arms(dispatch.arms)[0].source_span, source_span);
            let own_copy = table.copy_from_self(expression);
            assert_eq!(
                table.display_name(own_copy),
                copied.display_name(copied_expression)
            );
            assert_eq!(table.source_span(own_copy), source_span);
        }

        struct RowFrontier {
            expressions: usize,
            occurrence_ids: usize,
            expression_handles: usize,
            name_path_members: usize,
            name_path_member_symbols: usize,
            struct_fields: usize,
            match_arms: usize,
        }

        impl RowFrontier {
            fn of(table: &ExpressionTable) -> Self {
                Self {
                    expressions: table.nodes.expressions.storage_slice().len(),
                    occurrence_ids: table
                        .spans
                        .authored_selection_occurrence_ids
                        .storage_slice()
                        .len(),
                    expression_handles: table.spans.expression_handles.storage_slice().len(),
                    name_path_members: table.spans.name_path_members.storage_slice().len(),
                    name_path_member_symbols: table
                        .spans
                        .name_path_member_symbols
                        .storage_slice()
                        .len(),
                    struct_fields: table.spans.struct_fields.storage_slice().len(),
                    match_arms: table.spans.match_arms.storage_slice().len(),
                }
            }
        }

        fn render_rows_past(table: &ExpressionTable, frontier: &RowFrontier) -> String {
            let mut lines = Vec::new();
            for (handle, node) in table.iter_expressions().skip(frontier.expressions) {
                let span = table.source_span(handle);
                let occurrences =
                    table.nodes.authored_selection_occurrences[super::source_span_index(handle)];
                lines.push(format!(
            "expression {handle:?} = {node:?} at {}:{}..{} exposure {:?} partition {:?} occurrences {occurrences:?}",
            span.source_id.0,
            span.span.start,
            span.span.end,
            table.authored_expression_exposure(handle),
            table.compiler_selection_partition(handle),
        ));
            }
            let occurrence_ids = table
                .spans
                .authored_selection_occurrence_ids
                .storage_slice()
                .iter()
                .map(|stored| stored.occurrence())
                .collect::<Vec<_>>();
            push_rows_past(
                &mut lines,
                "occurrence",
                &occurrence_ids,
                frontier.occurrence_ids,
            );
            push_rows_past(
                &mut lines,
                "handle",
                table.spans.expression_handles.storage_slice(),
                frontier.expression_handles,
            );
            push_rows_past(
                &mut lines,
                "member",
                table.spans.name_path_members.storage_slice(),
                frontier.name_path_members,
            );
            push_rows_past(
                &mut lines,
                "member symbol",
                table.spans.name_path_member_symbols.storage_slice(),
                frontier.name_path_member_symbols,
            );
            push_rows_past(
                &mut lines,
                "field",
                table.spans.struct_fields.storage_slice(),
                frontier.struct_fields,
            );
            push_rows_past(
                &mut lines,
                "arm",
                table.spans.match_arms.storage_slice(),
                frontier.match_arms,
            );
            shorten_handles(&lines.join("\n"))
        }

        fn push_rows_past<Row: std::fmt::Debug>(
            lines: &mut Vec<String>,
            label: &str,
            rows: &[Row],
            frontier: usize,
        ) {
            for (row_index, row) in rows.iter().enumerate().skip(frontier) {
                lines.push(format!("{label} #{} = {row:?}", row_index + 1));
            }
        }

        fn shorten_handles(rendered: &str) -> String {
            let handles = shorten_records(
                rendered,
                "Handle { arena_index: ",
                ", generation: ",
                |index, generation| {
                    if generation == "1" {
                        format!("#{index}")
                    } else {
                        format!("#{index}@{generation}")
                    }
                },
            );
            shorten_records(
                &handles,
                "HandleSpan { start: ",
                ", count: ",
                |start, count| format!("{start}+{count}"),
            )
        }

        fn shorten_records(
            rendered: &str,
            opening: &str,
            separator: &str,
            shorten: impl Fn(&str, &str) -> String,
        ) -> String {
            let mut shortened = String::with_capacity(rendered.len());
            let mut rest = rendered;
            while let Some(start) = rest.find(opening) {
                shortened.push_str(&rest[..start]);
                let fields = &rest[start + opening.len()..];
                let (first, after_first) = fields
                    .split_once(separator)
                    .expect("debug record keeps its field separator");
                let (second, after_record) = after_first
                    .split_once(" }")
                    .expect("debug record keeps its closing brace");
                shortened.push_str(&shorten(first, second));
                rest = after_record;
            }
            shortened.push_str(rest);
            shortened
        }

        fn insert_members(
            table: &mut ExpressionTable,
            members: &[&str],
        ) -> HandleSpan<DiagnosticName> {
            let span = table.reserve_name_path_members(members.len() as u32);
            for (offset, member) in members.iter().enumerate() {
                table.set_name_path_member_at_offset(
                    span,
                    offset as u32,
                    DiagnosticName::generated(*member),
                );
            }
            span
        }

        fn insert_name(
            table: &mut ExpressionTable,
            members: &[&str],
            member_symbols: &[SymbolHandle],
            symbol: SymbolHandle,
        ) -> super::ExpressionHandle {
            let members = insert_members(table, members);
            let symbol_span = table.reserve_name_path_member_symbols(member_symbols.len() as u32);
            for (offset, member_symbol) in member_symbols.iter().enumerate() {
                table.set_name_path_member_symbol_at_offset(
                    symbol_span,
                    offset as u32,
                    *member_symbol,
                );
            }
            table.insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols: symbol_span,
                head_symbol: member_symbols.first().copied().unwrap_or_default(),
                symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }))
        }

        fn insert_every_node_kind(table: &mut ExpressionTable) -> super::ExpressionHandle {
            use super::{
                MatchPattern, TableAtomicExpression, TableBorrowExpression, TableCallExpression,
                TableCastExpression, TableIndexedExpression, TableMatchArm, TableMatchExpression,
                TableMemberExpression, TableMembershipExpression, TableRangeExpression,
                TableUnaryExpression, UnaryOperator,
            };
            use language_core::atomic::{
                AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
            };
            use numeric_literals::{FloatFormat, FloatLiteral, IntegerLiteral};

            let [root_occurrence, call_occurrence] = authored_selection_occurrences();
            let room_symbol = SymbolHandle::from_arena_index(3);
            let count_symbol = SymbolHandle::from_arena_index(4);
            let span = |start| SourceSpan::new(SourceId(7), Span::new(start, start + 5));

            let receiver = insert_name(
                table,
                &["self", "room"],
                &[room_symbol, room_symbol],
                room_symbol,
            );
            table.attach_authored_selection_occurrences(receiver, [root_occurrence]);
            table.set_authored_expression_exposure(
                receiver,
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
            );
            let one = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
            let half = table.insert(ExpressionNode::Float(FloatLiteral::new(2.5)));
            let array_values = table.insert_expression_handles([one, half]);
            let array = table.insert(ExpressionNode::ArrayLiteral(array_values));
            let flag = table.insert(ExpressionNode::Boolean(true));
            let not_flag = table.insert(ExpressionNode::Unary(TableUnaryExpression {
                operator: UnaryOperator::LogicalNot,
                operand: flag,
            }));
            let target_label = insert_members(table, &["u8"]);
            let semantic_domain = insert_members(table, &["Meters"]);
            let cast = table.insert(ExpressionNode::Cast(TableCastExpression {
                value: not_flag,
                target_type: arena::Handle::from_arena_index(5),
                target_label,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain,
                semantic_domain_arguments: arena::HandleSpan::empty(),
                semantic_domain_symbol: SymbolHandle::from_arena_index(6),
                form: language_core::cast_form::CastForm::Value,
            }));
            table.set_source_span(cast, span(40));
            table.set_compiler_selection_partition(
                cast,
                CompilerDerivedSelectionPartition::from_compiler_ordinal(9),
            );
            let arguments = table.insert_expression_handles([array, cast]);
            let call = table.insert(ExpressionNode::Call(TableCallExpression {
                // This nominal call carries its declaration in target_symbol.
                callee: super::ExpressionHandle::invalid(),
                target_is_static: true,
                authored_call_binding: None,
                receiver,
                target_symbol: SymbolHandle::from_arena_index(7),
                target: DiagnosticName::generated("measure"),
                requirement_application: arena::Handle::invalid(),
                machine_arguments: Box::default(),
                arguments,
                evidence_arguments: Box::from([DiagnosticName::generated("witness")]),
                operational_acknowledgement: Default::default(),
            }));
            table.set_source_span(call, span(30));
            table.attach_authored_selection_occurrences(call, [call_occurrence, root_occurrence]);
            table.set_authored_expression_exposure(
                call,
                AuthoredDeclarationSelectionExposure::PublicInterface,
            );

            let zero = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
            let range = table.insert(ExpressionNode::Range(TableRangeExpression {
                start: zero,
                end: super::ExpressionHandle::invalid(),
                end_inclusive: false,
            }));
            let hall = table.insert(ExpressionNode::String((&b"Hall"[..]).into()));
            let eleven = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(11)));
            let thirteen = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(13)));
            let proof = table.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: eleven,
                operator: BinaryOperator::Add,
                right: thirteen,
            }));
            let grid = insert_name(table, &["grid"], &[room_symbol], room_symbol);
            let three = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
            let cell = table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: grid,
                index: three,
            }));
            let fields = table.insert_struct_fields([
                TableStructLiteralField {
                    name: DiagnosticName::generated("name"),
                    field_symbol: SymbolHandle::from_arena_index(8),
                    value: hall,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("proof"),
                    field_symbol: SymbolHandle::from_arena_index(9),
                    value: proof,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("size"),
                    field_symbol: SymbolHandle::from_arena_index(10),
                    value: cell,
                },
            ]);
            let literal = table.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                zero_omitted_fields: false,
                application_type: Default::default(),
                type_name: DiagnosticName::generated("Room"),
                type_symbol: SymbolHandle::from_arena_index(11),
                case_name: Some(DiagnosticName::generated("Open")),
                case_symbol: Some(SymbolHandle::from_arena_index(12)),
                fields,
            }));
            table.set_source_span(literal, span(60));

            let counter = insert_name(table, &["cell"], &[], SymbolHandle::invalid());
            let count = table.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: counter,
                member_symbol: count_symbol,
                member: DiagnosticName::generated("count"),
                case_variant: Some(DiagnosticName::generated("Full")),
            }));
            let borrowed = table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                target: count,
                access: language_core::ReferenceAccess::Mutable,
            }));
            let previous = insert_name(table, &["previous"], &[count_symbol], count_symbol);
            let atomic = table.insert(ExpressionNode::Atomic(TableAtomicExpression {
                value: borrowed,
                result: previous,
                ordering: AtomicOrderingPlan::Load(MemoryOrdering::GlobalOrder),
                result_custody: AtomicExpressionResultCustody::Scalar,
            }));
            let nan = table.insert(ExpressionNode::UndeterminedNaN(FloatFormat::F32));
            let domain = insert_members(table, &["Small"]);
            let membership = table.insert(ExpressionNode::Membership(TableMembershipExpression {
                is_pattern: false,
                value: nan,
                domain,
                domain_symbol: SymbolHandle::from_arena_index(14),
                case_type_symbol: SymbolHandle::invalid(),
                case_symbol: SymbolHandle::invalid(),
            }));
            let zero_value = table.insert(ExpressionNode::ZeroValue(
                arena::Handle::from_arena_index(13),
            ));

            let arms = table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Value(range),
                    value: literal,
                    source_span: span(70),
                },
                TableMatchArm {
                    pattern: MatchPattern::Value(atomic),
                    value: membership,
                    source_span: span(80),
                },
                TableMatchArm {
                    pattern: MatchPattern::Wildcard,
                    value: zero_value,
                    source_span: span(90),
                },
            ]);
            let root = table.insert(ExpressionNode::Match(TableMatchExpression {
                subject: call,
                arms,
                ..Default::default()
            }));
            table.set_source_span(root, span(20));
            table.attach_authored_selection_occurrences(root, [root_occurrence]);
            table.set_compiler_selection_partition(
                root,
                CompilerDerivedSelectionPartition::from_compiler_ordinal(4),
            );
            root
        }

        #[test]
        fn table_and_own_copies_append_identical_rows_in_child_first_order() {
            let mut source = ExpressionTable::new();
            let root = insert_every_node_kind(&mut source);
            let frontier = RowFrontier::of(&source);

            let mut across = source.clone();
            let across_root = across.copy_from(&source, root);
            let mut within = source.clone();
            let within_root = within.copy_from_self(root);

            assert_eq!(across_root, within_root);
            let rendered = render_rows_past(&across, &frontier);
            assert_eq!(rendered, render_rows_past(&within, &frontier));
            assert!(across == within, "both copies must leave identical tables");
            assert_eq!(rendered, EXPECTED_COPY_ROWS.trim());
        }

        const EXPECTED_COPY_ROWS: &str = r#"
expression #28 = Name(TableNamePath { members: #9+2, member_symbols: #5+2, head_symbol: #3, symbol: #3, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure Some(PrivateImplementation) partition None occurrences #5+1
expression #29 = Integer(IntegerLiteral { text: "1", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #30 = Float(FloatLiteral { text: "2.5", landing: None, presented: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #31 = ArrayLiteral(#7+2) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #32 = Boolean(true) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #33 = Unary(TableUnaryExpression { operator: LogicalNot, operand: #32 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #34 = Cast(TableCastExpression { value: #33, target_type: #5, target_label: #11+1, domain: Exact, semantic_domain: #12+1, semantic_domain_arguments: #0@0+0, semantic_domain_symbol: #6, form: Value }) at 7:40..45 exposure None partition Some(CompilerDerivedSelectionPartition(9)) occurrences #0@0+0
expression #35 = Call(TableCallExpression { callee: #0@0, target_is_static: true, authored_call_binding: None, receiver: #28, target_symbol: #7, target: "measure", requirement_application: #0@0, machine_arguments: [], arguments: #5+2, evidence_arguments: ["witness"], operational_acknowledgement: CallOperationalAcknowledgement { origin: Source, acknowledges_suspend: false, acknowledges_block: false } }) at 7:30..35 exposure Some(PublicInterface) partition None occurrences #6+2
expression #36 = Integer(IntegerLiteral { text: "0", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #37 = Range(TableRangeExpression { start: #36, end: #0@0, end_inclusive: false }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #38 = String([72, 97, 108, 108]) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #39 = Integer(IntegerLiteral { text: "11", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #40 = Integer(IntegerLiteral { text: "13", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #41 = Binary(TableBinaryExpression { left: #39, operator: Add, right: #40, policy: Exact }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #42 = Name(TableNamePath { members: #13+1, member_symbols: #7+1, head_symbol: #3, symbol: #3, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #43 = Integer(IntegerLiteral { text: "3", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #44 = Indexed(TableIndexedExpression { collection: #42, index: #43 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #45 = StructLiteral(TableStructLiteral { zero_omitted_fields: false, application_type: #0@0, type_name: "Room", type_symbol: #11, case_name: Some("Open"), case_symbol: Some(#12), fields: #4+3 }) at 7:60..65 exposure None partition None occurrences #0@0+0
expression #46 = Name(TableNamePath { members: #14+1, member_symbols: #0@0+0, head_symbol: #0@0, symbol: #0@0, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #47 = Member(TableMemberExpression { receiver: #46, member_symbol: #4, member: "count", case_variant: Some("Full") }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #48 = Borrow(TableBorrowExpression { target: #47, access: Mutable }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #49 = Name(TableNamePath { members: #15+1, member_symbols: #8+1, head_symbol: #4, symbol: #4, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #50 = Atomic(TableAtomicExpression { value: #48, result: #49, ordering: Load(GlobalOrder), result_custody: Scalar }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #51 = UndeterminedNaN(F32) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #52 = Membership(TableMembershipExpression { is_pattern: false, value: #51, domain: #16+1, domain_symbol: #14, case_type_symbol: #0@0, case_symbol: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #53 = ZeroValue(#13) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #54 = Match(TableMatchExpression { subject: #35, capture: #0@0, arms: #4+3 }) at 7:20..25 exposure None partition Some(CompilerDerivedSelectionPartition(4)) occurrences #8+1
occurrence #5 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #6 = AuthoredDeclarationSelectionOccurrenceId(1)
occurrence #7 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #8 = AuthoredDeclarationSelectionOccurrenceId(0)
handle #5 = #31
handle #6 = #34
handle #7 = #29
handle #8 = #30
member #9 = "self"
member #10 = "room"
member #11 = "u8"
member #12 = "Meters"
member #13 = "grid"
member #14 = "cell"
member #15 = "previous"
member #16 = "Small"
member symbol #5 = #3
member symbol #6 = #3
member symbol #7 = #3
member symbol #8 = #4
field #4 = TableStructLiteralField { name: "name", field_symbol: #8, value: #38 }
field #5 = TableStructLiteralField { name: "proof", field_symbol: #9, value: #41 }
field #6 = TableStructLiteralField { name: "size", field_symbol: #10, value: #44 }
arm #4 = TableMatchArm { pattern: Value(#37), value: #45, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 70, end: 75 } } }
arm #5 = TableMatchArm { pattern: Value(#50), value: #52, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 80, end: 85 } } }
arm #6 = TableMatchArm { pattern: Wildcard, value: #53, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 90, end: 95 } } }
"#;

        #[test]
        fn materialized_value_copy_retains_its_distinct_authored_computation() {
            let mut source = ExpressionTable::new();
            let authored = source.insert(ExpressionNode::Boolean(false));
            let value = source.insert(ExpressionNode::Boolean(true));
            source.retain_materialization_source(value, authored);
            let own = source.copy_from_self(value);
            let mut destination = ExpressionTable::new();
            for _ in 0..8 {
                destination.insert(ExpressionNode::Boolean(true));
            }
            let copied = destination.copy_from(&source, value);
            for (table, copied) in [(&source, own), (&destination, copied)] {
                let authored = table.materialization_source(copied);
                assert!(authored.is_valid() && authored != copied);
                assert_eq!(table.expression(copied), &ExpressionNode::Boolean(true));
                assert_eq!(table.expression(authored), &ExpressionNode::Boolean(false));
            }
        }
    }

    pub use display::display_name_path;
    pub use numeric_literals::FloatLiteral;

    use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
    use crate::source_resolved::AuthoredDeclarationSelectionOccurrenceId;
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_selections::CompilerDerivedSelectionPartition;
    use arena::{Arena, Handle, HandleSpan};
    use numeric_literals::IntegerLiteral;
    use source_text::SourceSpan;
    use symbols::SymbolHandle;

    pub type ExpressionHandle = Handle<ExpressionNode>;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    struct StoredAuthoredSelectionOccurrenceId(Option<AuthoredDeclarationSelectionOccurrenceId>);

    impl StoredAuthoredSelectionOccurrenceId {
        fn new(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> Self {
            Self(Some(occurrence))
        }

        fn occurrence(self) -> AuthoredDeclarationSelectionOccurrenceId {
            self.0
                .expect("expression occurrence spans cannot contain the arena dummy sentinel")
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct ExpressionTable {
        nodes: ExpressionNodeStorage,
        spans: ExpressionSpanStorage,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    struct ExpressionNodeStorage {
        expressions: Arena<ExpressionNode>,
        source_spans: Vec<SourceSpan>,
        unresolved_names: Vec<crate::source_symbols::UnresolvedName>,
        materialization_sources: Vec<ExpressionHandle>,

        authored_expression_exposures: Vec<Option<AuthoredDeclarationSelectionExposure>>,
        compiler_selection_partitions: Vec<Option<CompilerDerivedSelectionPartition>>,
        authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    struct ExpressionSpanStorage {
        expression_handles: Arena<ExpressionHandle>,
        name_path_members: Arena<DiagnosticName>,
        name_path_member_symbols: Arena<SymbolHandle>,
        struct_fields: Arena<TableStructLiteralField>,
        match_arms: Arena<TableMatchArm>,
        match_fields: Arena<TableMatchField>,
        match_subjects: Arena<MatchSubjectCapture>,
        static_members: Arena<StaticMachineArgument>,
        match_proof_selectors: Arena<crate::source_resolved::statement::OutcomeProofSelector>,
        authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
    }

    pub(crate) struct PreparedStaticArgumentRebase {
        arguments: Box<[StaticMachineArgument]>,
        members: Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
    }

    pub(crate) struct PreparedExpressionSelectionRebase {
        remapped: Vec<Option<Vec<AuthoredDeclarationSelectionOccurrenceId>>>,
        arguments: Vec<(
            ExpressionHandle,
            Option<Handle<StaticMachineArgument>>,
            PreparedStaticArgumentRebase,
        )>,
    }

    macro_rules! expression_node_accessors {
    ($($(#[$doc:meta])* $name:ident($variant:ident) -> $node_type:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(&self, handle: ExpressionHandle) -> Option<&$node_type> {
                match self.expression(handle) {
                    ExpressionNode::$variant(node) => Some(node),
                    _ => None,
                }
            }
        )*
    };
}

    impl ExpressionTable {
        pub fn insert_static_member(
            &mut self,
            member: StaticMachineArgument,
        ) -> Handle<StaticMachineArgument> {
            self.spans.static_members.insert(member)
        }

        pub fn static_member(
            &self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&StaticMachineArgument> {
            self.spans
                .static_members
                .is_valid(member)
                .then(|| self.spans.static_members.get(member))
        }

        pub fn static_member_mut(
            &mut self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&mut StaticMachineArgument> {
            if self.spans.static_members.is_valid(member) {
                Some(self.spans.static_members.get_mut(member))
            } else {
                None
            }
        }

        pub fn insert_match_arms(
            &mut self,
            arms: impl IntoIterator<Item = TableMatchArm>,
        ) -> HandleSpan<TableMatchArm> {
            self.spans.match_arms.insert_many(arms)
        }

        pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
            self.spans.match_arms.span_or_empty(arms)
        }

        pub fn match_arm_mut(&mut self, arm: Handle<TableMatchArm>) -> &mut TableMatchArm {
            self.spans.match_arms.get_mut(arm)
        }

        pub fn insert_match_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableMatchField>,
        ) -> HandleSpan<TableMatchField> {
            self.spans.match_fields.insert_many(fields)
        }

        pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
            self.spans.match_fields.span_or_empty(fields)
        }

        pub fn match_pattern_dependencies(
            &self,
            pattern: MatchPattern,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            // Resolution combines field equalities and the authored guard into
            // condition. Traverse that graph once, not its retained source fields too.
            let roots = match pattern {
                MatchPattern::Value(value) => [value, ExpressionHandle::invalid()],
                MatchPattern::Destructure(pattern) => [pattern.head, pattern.condition],
                MatchPattern::Wildcard => [ExpressionHandle::invalid(); 2],
            };
            roots.into_iter().filter(|root| root.is_valid())
        }

        pub fn match_field_mut(&mut self, field: Handle<TableMatchField>) -> &mut TableMatchField {
            self.spans.match_fields.get_mut(field)
        }

        pub fn insert_match_subject(
            &mut self,
            subject: ExpressionHandle,
        ) -> Handle<MatchSubjectCapture> {
            self.spans.match_subjects.insert(MatchSubjectCapture {
                purpose: SubjectCapturePurpose::Match,
                subject,
            })
        }

        pub fn insert_transition_subject(
            &mut self,
            subject: ExpressionHandle,
        ) -> Handle<MatchSubjectCapture> {
            self.spans.match_subjects.insert(MatchSubjectCapture {
                purpose: SubjectCapturePurpose::Transition,
                subject,
            })
        }

        pub fn match_subject(&self, capture: Handle<MatchSubjectCapture>) -> &MatchSubjectCapture {
            self.spans.match_subjects.get(capture)
        }

        pub fn insert_match_proof_selectors(
            &mut self,
            selectors: impl IntoIterator<Item = crate::source_resolved::statement::OutcomeProofSelector>,
        ) -> HandleSpan<crate::source_resolved::statement::OutcomeProofSelector> {
            self.spans.match_proof_selectors.insert_many(selectors)
        }

        pub fn match_proof_selectors(
            &self,
            selectors: HandleSpan<crate::source_resolved::statement::OutcomeProofSelector>,
        ) -> &[crate::source_resolved::statement::OutcomeProofSelector] {
            self.spans.match_proof_selectors.span_or_empty(selectors)
        }

        pub fn new() -> Self {
            Self {
                nodes: ExpressionNodeStorage {
                    expressions: Arena::new(),
                    source_spans: Vec::new(),
                    unresolved_names: Vec::new(),
                    materialization_sources: Vec::new(),
                    authored_expression_exposures: Vec::new(),
                    compiler_selection_partitions: Vec::new(),
                    authored_selection_occurrences: Vec::new(),
                },
                spans: ExpressionSpanStorage {
                    expression_handles: Arena::new(),
                    name_path_members: Arena::new(),
                    name_path_member_symbols: Arena::new(),
                    struct_fields: Arena::new(),
                    match_arms: Arena::new(),
                    match_fields: Arena::new(),
                    match_subjects: Arena::new(),
                    static_members: Arena::new(),
                    match_proof_selectors: Arena::new(),
                    authored_selection_occurrence_ids: Arena::new(),
                },
            }
        }

        pub fn clear(&mut self) {
            self.nodes.expressions.retire_all_retain_capacity();
            self.nodes.source_spans.clear();
            self.nodes.unresolved_names.clear();
            self.nodes.materialization_sources.clear();
            self.nodes.authored_expression_exposures.clear();
            self.nodes.compiler_selection_partitions.clear();
            self.nodes.authored_selection_occurrences.clear();
            self.spans.expression_handles.retire_all_retain_capacity();
            self.spans.name_path_members.retire_all_retain_capacity();
            self.spans
                .name_path_member_symbols
                .retire_all_retain_capacity();
            self.spans.struct_fields.retire_all_retain_capacity();
            self.spans.match_arms.retire_all_retain_capacity();
            self.spans.match_fields.retire_all_retain_capacity();
            self.spans.match_subjects.retire_all_retain_capacity();
            self.spans.static_members.retire_all_retain_capacity();
            self.spans
                .match_proof_selectors
                .retire_all_retain_capacity();
            self.spans
                .authored_selection_occurrence_ids
                .retire_all_retain_capacity();
        }

        pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
            let handle = self.nodes.expressions.insert(expression);
            self.nodes.source_spans.push(SourceSpan::default());
            self.nodes.unresolved_names.push(Default::default());
            self.nodes
                .materialization_sources
                .push(ExpressionHandle::invalid());
            self.nodes.authored_expression_exposures.push(None);
            self.nodes.compiler_selection_partitions.push(None);
            self.nodes
                .authored_selection_occurrences
                .push(HandleSpan::empty());
            debug_assert_eq!(source_span_index(handle), self.nodes.source_spans.len() - 1);
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.authored_expression_exposures.len() - 1
            );
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.compiler_selection_partitions.len() - 1
            );
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.authored_selection_occurrences.len() - 1
            );
            handle
        }

        pub fn materialization_source(&self, handle: ExpressionHandle) -> ExpressionHandle {
            self.nodes
                .materialization_sources
                .get(source_span_index(handle))
                .copied()
                .unwrap_or_default()
        }

        pub fn retain_materialization_source(
            &mut self,
            value: ExpressionHandle,
            authored: ExpressionHandle,
        ) {
            assert!(value.is_valid() && authored.is_valid() && value != authored);
            assert!(!self.materialization_source(value).is_valid());
            self.nodes.materialization_sources[source_span_index(value)] = authored;
        }

        pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
            self.nodes.source_spans[source_span_index(handle)]
        }

        /// Resolution's outcome for this exact occurrence, not another source's name.
        pub fn unresolved_name(
            &self,
            handle: ExpressionHandle,
        ) -> crate::source_symbols::UnresolvedName {
            if !self.nodes.expressions.is_valid(handle) {
                return Default::default();
            }
            self.nodes.unresolved_names[source_span_index(handle)]
        }

        pub fn set_unresolved_name(
            &mut self,
            handle: ExpressionHandle,
            outcome: crate::source_symbols::UnresolvedName,
        ) {
            self.nodes.unresolved_names[source_span_index(handle)] = outcome;
        }

        pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
            self.nodes.source_spans[source_span_index(handle)] = source_span;
        }

        pub fn authored_expression_exposure(
            &self,
            handle: ExpressionHandle,
        ) -> Option<AuthoredDeclarationSelectionExposure> {
            self.nodes.authored_expression_exposures[source_span_index(handle)]
        }

        pub fn set_authored_expression_exposure(
            &mut self,
            handle: ExpressionHandle,
            exposure: AuthoredDeclarationSelectionExposure,
        ) {
            let slot = &mut self.nodes.authored_expression_exposures[source_span_index(handle)];
            if let Some(existing) = *slot {
                assert_eq!(
                    existing, exposure,
                    "one expression handle cannot represent two authored visibility positions"
                );
            }
            *slot = Some(exposure);
        }

        pub fn compiler_selection_partition(
            &self,
            handle: ExpressionHandle,
        ) -> Option<CompilerDerivedSelectionPartition> {
            self.nodes.compiler_selection_partitions[source_span_index(handle)]
        }

        pub fn set_compiler_selection_partition(
            &mut self,
            handle: ExpressionHandle,
            partition: CompilerDerivedSelectionPartition,
        ) {
            let slot = &mut self.nodes.compiler_selection_partitions[source_span_index(handle)];
            if let Some(existing) = *slot {
                assert_eq!(
                    existing, partition,
                    "one expression handle cannot belong to two compiler selection partitions"
                );
            }
            *slot = Some(partition);
        }

        pub fn attach_authored_selection_occurrences(
            &mut self,
            handle: ExpressionHandle,
            occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
        ) {
            let mut combined = self
                .authored_selection_occurrences(handle)
                .collect::<Vec<_>>();

            for occurrence in occurrences {
                if !combined.contains(&occurrence) {
                    combined.push(occurrence);
                }
            }

            self.replace_authored_selection_occurrences(handle, combined);
        }

        pub fn replace_authored_selection_occurrences(
            &mut self,
            handle: ExpressionHandle,
            occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
        ) {
            let mut unique = Vec::new();
            for occurrence in occurrences {
                if !unique.contains(&occurrence) {
                    unique.push(occurrence);
                }
            }
            self.nodes.authored_selection_occurrences[source_span_index(handle)] =
                self.spans.authored_selection_occurrence_ids.insert_many(
                    unique
                        .into_iter()
                        .map(StoredAuthoredSelectionOccurrenceId::new),
                );
        }

        pub fn authored_selection_occurrences(
            &self,
            handle: ExpressionHandle,
        ) -> impl ExactSizeIterator<Item = AuthoredDeclarationSelectionOccurrenceId> + '_ {
            self.spans
                .authored_selection_occurrence_ids
                .span_or_empty(self.nodes.authored_selection_occurrences[source_span_index(handle)])
                .iter()
                .copied()
                .map(StoredAuthoredSelectionOccurrenceId::occurrence)
        }

        pub(crate) fn prepare_static_argument_rebase(
            &self,
            arguments: &[StaticMachineArgument],
            remap: impl Fn(
                AuthoredDeclarationSelectionOccurrenceId,
            ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
        ) -> Result<PreparedStaticArgumentRebase, ()> {
            fn argument(
                table: &ExpressionTable,
                value: &mut StaticMachineArgument,
                remap: &impl Fn(
                    AuthoredDeclarationSelectionOccurrenceId,
                ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
                members: &mut Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
                active: &mut Vec<Handle<StaticMachineArgument>>,
            ) -> Result<(), ()> {
                if let Some(occurrence) = value.authored_selection {
                    value.authored_selection = Some(remap(occurrence).ok_or(())?);
                }
                if let Some(application) = &mut value.application {
                    for child in &mut application.arguments {
                        argument(table, child, remap, members, active)?;
                    }
                    let handle = application.member;
                    if handle.is_valid() {
                        let key = |handle: Handle<StaticMachineArgument>| {
                            (handle.arena_index(), handle.generation())
                        };
                        if members
                            .binary_search_by_key(&key(handle), |(handle, _)| key(*handle))
                            .is_err()
                        {
                            if active.contains(&handle) {
                                return Err(());
                            }
                            active.push(handle);
                            let mut child = table.static_member(handle).ok_or(())?.clone();
                            argument(table, &mut child, remap, members, active)?;
                            active.pop();
                            let at =
                                members.partition_point(|(current, _)| key(*current) < key(handle));
                            members.insert(at, (handle, child));
                        }
                    }
                }
                Ok(())
            }
            let mut values = arguments.to_vec().into_boxed_slice();
            let mut members = Vec::new();
            for value in &mut values {
                argument(self, value, &remap, &mut members, &mut Vec::new())?;
            }
            Ok(PreparedStaticArgumentRebase {
                arguments: values,
                members,
            })
        }

        pub(crate) fn apply_static_argument_rebase(
            &mut self,
            prepared: PreparedStaticArgumentRebase,
        ) -> Box<[StaticMachineArgument]> {
            for (handle, member) in prepared.members {
                *self
                    .static_member_mut(handle)
                    .expect("prepared static member remains current") = member;
            }
            prepared.arguments
        }

        pub(crate) fn prepare_authored_selection_rebase(
            &self,
            expression_frontier: usize,
            rebase: crate::source_selections::AuthoredDeclarationSelectionSuffixRebase,
        ) -> Result<PreparedExpressionSelectionRebase, ()> {
            if expression_frontier > self.expression_count() {
                return Err(());
            }
            let remapped = self
                .iter_expressions()
                .enumerate()
                .map(|(index, (handle, _))| {
                    let occurrences = self
                        .authored_selection_occurrences(handle)
                        .map(|occurrence| {
                            if index < expression_frontier {
                                rebase.retain_base(occurrence)
                            } else {
                                rebase.rebase_appended(occurrence)
                            }
                            .ok_or(())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((index >= expression_frontier).then_some(occurrences))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut arguments = Vec::new();
            for (index, (handle, node)) in self.iter_expressions().enumerate() {
                let (members, values) = match node {
                    ExpressionNode::Call(call) => (None, call.machine_arguments.as_ref()),
                    ExpressionNode::Name(name) if name.static_application.is_valid() => (
                        Some(name.static_application),
                        std::slice::from_ref(
                            self.static_member(name.static_application).ok_or(())?,
                        ),
                    ),
                    _ => continue,
                };
                let prepared = self.prepare_static_argument_rebase(values, |occurrence| {
                    if index < expression_frontier {
                        rebase.retain_base(occurrence)
                    } else {
                        rebase.rebase_appended(occurrence)
                    }
                })?;
                if index >= expression_frontier {
                    arguments.push((handle, members, prepared));
                }
            }
            Ok(PreparedExpressionSelectionRebase {
                remapped,
                arguments,
            })
        }

        pub(crate) fn apply_authored_selection_rebase(
            &mut self,
            prepared: PreparedExpressionSelectionRebase,
        ) {
            let PreparedExpressionSelectionRebase {
                remapped,
                arguments,
            } = prepared;
            for (handle, member, prepared) in arguments {
                let mut arguments = self.apply_static_argument_rebase(prepared);
                if let Some(member) = member {
                    *self
                        .static_member_mut(member)
                        .expect("prepared applied name remains current") = arguments[0].clone();
                } else {
                    let ExpressionNode::Call(call) = self.expression_mut(handle) else {
                        unreachable!("prepared call remains current")
                    };
                    call.machine_arguments = std::mem::take(&mut arguments);
                }
            }
            for (index, occurrences) in remapped.into_iter().enumerate() {
                let Some(occurrences) = occurrences else {
                    continue;
                };
                self.nodes.authored_selection_occurrences[index] =
                    self.spans.authored_selection_occurrence_ids.insert_many(
                        occurrences
                            .into_iter()
                            .map(StoredAuthoredSelectionOccurrenceId::new),
                    );
            }
        }

        pub fn insert_expression_handles(
            &mut self,
            expressions: impl IntoIterator<Item = ExpressionHandle>,
        ) -> HandleSpan<ExpressionHandle> {
            self.spans.expression_handles.insert_many(expressions)
        }

        pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
            self.spans.expression_handles.insert_many(
                std::iter::repeat_with(ExpressionHandle::invalid)
                    .take(usize::try_from(count).expect("expression handle span count overflow")),
            )
        }

        pub fn set_expression_handle_at_offset(
            &mut self,
            expressions: HandleSpan<ExpressionHandle>,
            offset: u32,
            expression: ExpressionHandle,
        ) {
            *self
                .spans
                .expression_handles
                .get_mut(expressions.handle_at(offset)) = expression;
        }

        pub fn push_expression_handle(
            &mut self,
            span: &mut HandleSpan<ExpressionHandle>,
            expression: ExpressionHandle,
        ) {
            self.spans
                .expression_handles
                .append_to_span(span, expression);
        }

        pub fn insert_struct_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableStructLiteralField>,
        ) -> HandleSpan<TableStructLiteralField> {
            self.spans.struct_fields.insert_many(fields)
        }

        pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
            self.spans.struct_fields.insert_many(
                std::iter::repeat_with(TableStructLiteralField::default).take(
                    usize::try_from(count).expect("struct literal field span count overflow"),
                ),
            )
        }

        pub fn set_struct_field_at_offset(
            &mut self,
            fields: HandleSpan<TableStructLiteralField>,
            offset: u32,
            field: TableStructLiteralField,
        ) {
            *self.spans.struct_fields.get_mut(fields.handle_at(offset)) = field;
        }

        pub fn push_struct_field(
            &mut self,
            span: &mut HandleSpan<TableStructLiteralField>,
            field: TableStructLiteralField,
        ) {
            self.spans.struct_fields.append_to_span(span, field);
        }

        pub fn push_name_path_member(
            &mut self,
            span: &mut HandleSpan<DiagnosticName>,
            member: DiagnosticName,
        ) {
            self.spans.name_path_members.append_to_span(span, member);
        }

        pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<DiagnosticName> {
            self.spans.name_path_members.insert_many(
                std::iter::repeat_with(DiagnosticName::default)
                    .take(usize::try_from(count).expect("name path member span count overflow")),
            )
        }

        pub fn set_name_path_member_at_offset(
            &mut self,
            members: HandleSpan<DiagnosticName>,
            offset: u32,
            member: DiagnosticName,
        ) {
            *self
                .spans
                .name_path_members
                .get_mut(members.handle_at(offset)) = member;
        }

        pub fn reserve_name_path_member_symbols(&mut self, count: u32) -> HandleSpan<SymbolHandle> {
            self.spans.name_path_member_symbols.insert_many(
                std::iter::repeat_with(SymbolHandle::invalid).take(
                    usize::try_from(count).expect("name path member symbol span count overflow"),
                ),
            )
        }

        pub fn set_name_path_member_symbol_at_offset(
            &mut self,
            member_symbols: HandleSpan<SymbolHandle>,
            offset: u32,
            member_symbol: SymbolHandle,
        ) {
            *self
                .spans
                .name_path_member_symbols
                .get_mut(member_symbols.handle_at(offset)) = member_symbol;
        }

        pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
            self.nodes.expressions.get(handle)
        }

        expression_node_accessors! {
            name_path_node(Name) -> TableNamePath;

            member_node(Member) -> TableMemberExpression;

            indexed_node(Indexed) -> TableIndexedExpression;

            cast_node(Cast) -> TableCastExpression;

            binary_node(Binary) -> TableBinaryExpression;

            call_node(Call) -> TableCallExpression;

            unary_node(Unary) -> TableUnaryExpression;

            integer_node(Integer) -> IntegerLiteral;

        }

        pub fn expression_is_valid(&self, handle: ExpressionHandle) -> bool {
            self.nodes.expressions.is_valid(handle)
        }

        pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
            self.nodes.expressions.get_mut(handle)
        }

        pub fn iter_expressions(
            &self,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
            self.nodes.expressions.iter()
        }

        pub fn expression_handles(
            &self,
            span: HandleSpan<ExpressionHandle>,
        ) -> &[ExpressionHandle] {
            self.spans.expression_handles.span_or_empty(span)
        }

        pub fn expression_handle_at_offset(
            &self,
            expressions: HandleSpan<ExpressionHandle>,
            offset: u32,
        ) -> &ExpressionHandle {
            self.spans
                .expression_handles
                .get(expressions.handle_at(offset))
        }

        pub fn struct_fields(
            &self,
            span: HandleSpan<TableStructLiteralField>,
        ) -> &[TableStructLiteralField] {
            self.spans.struct_fields.span_or_empty(span)
        }

        pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
            self.spans.name_path_members.span_or_empty(span)
        }

        pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
            self.spans.name_path_member_symbols.span_or_empty(span)
        }

        pub fn expression_count(&self) -> usize {
            self.nodes.expressions.len()
        }

        pub fn display_name(&self, handle: ExpressionHandle) -> String {
            self.expression(handle).display_name(self)
        }
    }

    fn source_span_index(handle: ExpressionHandle) -> usize {
        usize::try_from(handle.arena_index())
            .expect("expression index overflow")
            .checked_sub(1)
            .expect("invalid expression handle has no source span")
    }

    impl Default for ExpressionTable {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub enum ExpressionNode {
        Match(TableMatchExpression),
        SavedSubject(TableSavedSubject),
        ArrayLiteral(HandleSpan<ExpressionHandle>),
        Atomic(TableAtomicExpression),
        Binary(TableBinaryExpression),
        Boolean(bool),
        Cast(TableCastExpression),
        Call(TableCallExpression),
        Float(FloatLiteral),
        UndeterminedNaN(numeric_literals::FloatFormat),
        Indexed(TableIndexedExpression),
        Integer(IntegerLiteral),
        Membership(TableMembershipExpression),
        Member(TableMemberExpression),
        Borrow(TableBorrowExpression),
        Name(TableNamePath),
        Range(TableRangeExpression),
        StructLiteral(TableStructLiteral),
        String(source_text::SourceBytes),
        Unary(TableUnaryExpression),
        TypeExpression(arena::Handle<crate::source_resolved::types::TypeReference>),
        ZeroValue(arena::Handle<crate::source_resolved::types::TypeReference>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableBorrowExpression {
        pub target: ExpressionHandle,
        pub access: language_core::ReferenceAccess,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableAtomicExpression {
        pub value: ExpressionHandle,
        pub result: ExpressionHandle,
        pub ordering: language_core::atomic::AtomicOrderingPlan,
        pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
    }

    impl Default for ExpressionNode {
        fn default() -> Self {
            Self::Integer(IntegerLiteral::zero())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableBinaryExpression {
        pub left: ExpressionHandle,
        pub operator: BinaryOperator,
        pub right: ExpressionHandle,
        /// The policy an operator suffix chose for this one operation
        /// (`+%`, `+|`, `+!`); `Exact` for the bare operator.
        pub policy: numerics::arithmetic::ArithmeticDomain,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableUnaryExpression {
        pub operator: UnaryOperator,
        pub operand: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableCastExpression {
        pub value: ExpressionHandle,
        pub target_type: arena::Handle<crate::source_resolved::types::TypeReference>,
        pub target_label: HandleSpan<DiagnosticName>,
        pub domain: numerics::arithmetic::ArithmeticDomain,
        pub semantic_domain: HandleSpan<DiagnosticName>,
        pub semantic_domain_arguments: HandleSpan<crate::source_resolved::types::TypeReference>,
        pub semantic_domain_symbol: SymbolHandle,
        pub form: language_core::cast_form::CastForm,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableIndexedExpression {
        pub collection: ExpressionHandle,
        pub index: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableRangeExpression {
        pub start: ExpressionHandle,
        pub end: ExpressionHandle,
        pub end_inclusive: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct TableMembershipExpression {
        pub is_pattern: bool,
        pub value: ExpressionHandle,
        pub domain: HandleSpan<DiagnosticName>,
        pub domain_symbol: SymbolHandle,
        pub case_type_symbol: SymbolHandle,
        pub case_symbol: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct TableMemberExpression {
        pub receiver: ExpressionHandle,
        pub member_symbol: SymbolHandle,
        pub member: DiagnosticName,
        pub case_variant: Option<DiagnosticName>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct TableCallExpression {
        pub callee: ExpressionHandle,
        /// Keep static qualification until lexical binding can distinguish a
        /// namespace from a parameter or local with the same name.
        pub target_is_static: bool,
        pub authored_call_binding:
            Option<crate::declaration_selection::ResolvedAuthoredCallBinding>,
        pub receiver: ExpressionHandle,
        pub target_symbol: SymbolHandle,
        pub target: DiagnosticName,
        pub requirement_application: arena::Handle<crate::source_resolved::types::TypeReference>,
        pub machine_arguments: Box<[StaticMachineArgument]>,
        pub arguments: HandleSpan<ExpressionHandle>,
        pub evidence_arguments: Box<[DiagnosticName]>,
        pub operational_acknowledgement: declaration_contracts::CallOperationalAcknowledgement,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum SignatureFreeRequirementResolutionError {
        InvalidPath,
        TraitNotUnique,
        RequirementNotUnique,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub struct ResolvedSignatureFreeRequirement {
        pub trait_definition: SymbolHandle,
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct StaticMachineArgument {
        /// Exact ledger occurrence attached when resolution mints this input.
        pub authored_selection:
            Option<crate::source_selections::AuthoredDeclarationSelectionOccurrenceId>,
        pub authored_selection_target: Option<SymbolHandle>,
        pub type_reference: arena::Handle<crate::source_resolved::types::TypeReference>,
        pub path: Box<[DiagnosticName]>,
        pub application: Option<Box<StaticSymbolApplication>>,
        pub const_literal: Option<numeric_literals::IntegerLiteral>,
        pub evidence_projection: Option<EvidenceProjection>,
        pub symbol: SymbolHandle,
        /// Resolution records the whole qualified operand's requirement lookup.
        /// A refusal is retained too: typing must not retry with a call signature.
        pub signature_free_requirement: Option<
            Result<ResolvedSignatureFreeRequirement, SignatureFreeRequirementResolutionError>,
        >,
    }

    impl record_storage::Record for StaticMachineArgument {
        fn write(
            &self,
            output: &mut record_storage::RecordWriter,
        ) -> Result<(), record_storage::RecordError> {
            b"ARGSEL2".write(output)?;
            self.authored_selection.write(output)?;
            self.authored_selection_target.write(output)?;
            self.type_reference.write(output)?;
            self.path.write(output)?;
            self.application.write(output)?;
            self.const_literal.write(output)?;
            self.evidence_projection.write(output)?;
            self.symbol.write(output)?;
            self.signature_free_requirement.write(output)
        }

        fn read<Context>(
            input: &mut record_storage::RecordReader<'_, Context>,
        ) -> Result<Self, record_storage::RecordError> {
            if <[u8; 7]>::read(input)? != *b"ARGSEL2" {
                return Err(input.error("static input selection storage has another version"));
            }
            Ok(Self {
                authored_selection: Option::read(input)?,
                authored_selection_target: Option::read(input)?,
                type_reference: Handle::read(input)?,
                path: Box::read(input)?,
                application: Option::read(input)?,
                const_literal: Option::read(input)?,
                evidence_projection: Option::read(input)?,
                symbol: SymbolHandle::read(input)?,
                signature_free_requirement: Option::read(input)?,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct StaticSymbolApplication {
        pub member: Handle<StaticMachineArgument>,
        pub lifetime_arguments: Box<[DiagnosticName]>,
        pub arguments: Box<[StaticMachineArgument]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct EvidenceProjection {
        pub term: DiagnosticName,
        pub member: DiagnosticName,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableNamePath {
        pub members: HandleSpan<DiagnosticName>,
        pub member_symbols: HandleSpan<SymbolHandle>,
        pub head_symbol: SymbolHandle,
        pub symbol: SymbolHandle,
        pub static_application: arena::Handle<StaticMachineArgument>,
        /// The callable whose reserved `result` this name denotes, or invalid
        /// when it denotes no result. Stage 02 sets it once, where it decides a
        /// contract name is the reserved result occurrence
        /// (`wiki/spec/terminal-psi/calls_and_outcomes.md`: "The reserved
        /// result occurrence belongs to its contract owner, not any local or
        /// parameter spelled `result`"), so a later reader takes the result
        /// by this identity instead of comparing the spelling. `symbol` stays
        /// invalid, as before, so readers of the spelling route are unchanged.
        pub reserved_result_of: SymbolHandle,
    }

    impl TableNamePath {
        pub const fn new(
            members: HandleSpan<DiagnosticName>,
            member_symbols: HandleSpan<SymbolHandle>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            Self {
                members,
                member_symbols,
                head_symbol,
                symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
    pub struct TableStructLiteral {
        /// A final source `..` requests ordinary zero values for missing runtime fields.
        pub zero_omitted_fields: bool,
        pub application_type: Handle<crate::source_resolved::types::TypeReference>,
        pub type_name: DiagnosticName,
        pub type_symbol: SymbolHandle,
        pub case_name: Option<DiagnosticName>,
        pub case_symbol: Option<SymbolHandle>,
        pub fields: HandleSpan<TableStructLiteralField>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
    pub struct TableStructLiteralField {
        pub name: DiagnosticName,
        pub field_symbol: SymbolHandle,
        pub value: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum BinaryOperator {
        Add,
        And,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        Divide,
        Equal,
        Greater,
        GreaterOrEqual,
        Less,
        LessOrEqual,
        Modulo,
        Multiply,
        NotEqual,
        Or,
        ShiftLeft,
        ShiftRight,
        Subtract,
    }

    impl BinaryOperator {
        pub fn is_comparison(self) -> bool {
            matches!(
                self,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        }

        pub fn result_is_boolean(self) -> bool {
            self.is_comparison() || matches!(self, BinaryOperator::And | BinaryOperator::Or)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum UnaryOperator {
        BitwiseNot,
        LogicalNot,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableMatchExpression {
        pub subject: ExpressionHandle,
        pub capture: Handle<MatchSubjectCapture>,
        pub arms: HandleSpan<TableMatchArm>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub enum SubjectCapturePurpose {
        #[default]
        Match,
        Transition,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct MatchSubjectCapture {
        pub purpose: SubjectCapturePurpose,
        pub subject: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableSavedSubject {
        pub component: language_core::PatternComponent,
        pub capture: Handle<MatchSubjectCapture>,
        pub pattern_head: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableMatchArm {
        pub pattern: MatchPattern,
        pub value: ExpressionHandle,
        pub source_span: SourceSpan,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub enum MatchPattern {
        Value(ExpressionHandle),
        Destructure(TableDestructurePattern),
        #[default]
        Wildcard,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableDestructurePattern {
        pub application_type: Handle<crate::source_resolved::types::TypeReference>,
        pub head: ExpressionHandle,
        pub fields: HandleSpan<TableMatchField>,
        pub has_rest: bool,
        pub guard: ExpressionHandle,
        pub condition: ExpressionHandle,
        pub proof_selectors: HandleSpan<crate::source_resolved::statement::OutcomeProofSelector>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct TableMatchField {
        pub component: language_core::PatternComponent,
        pub member: DiagnosticName,
        pub field_symbol: SymbolHandle,
        pub action: MatchFieldAction,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum MatchFieldAction {
        Bind(DiagnosticName),
        Equal(ExpressionHandle),
        #[default]
        Waive,
    }
}

#[cfg(test)]
mod static_input_record_tests {
    use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
    use crate::declaration_selection::AuthoredDeclarationSelectionKind;
    use crate::source_resolved::expression::StaticMachineArgument;
    use crate::source_selections::AuthoredDeclarationSelections;
    use record_storage::{RecordLimits, decode, encode};
    use symbols::SymbolHandle;

    #[test]
    fn exact_static_input_attachment_round_trips_and_refuses_the_old_marker() {
        let selected = SymbolHandle::from_parts(3, 5);
        let mut ledger = AuthoredDeclarationSelections::default();
        let occurrence = ledger
            .record_resolved(
                source_text::SourceSpan::new(
                    source_text::SourceId(0),
                    source_text::Span::new(1, 2),
                ),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::StaticArgument,
                selected,
            )
            .unwrap();
        let argument = StaticMachineArgument {
            authored_selection: Some(occurrence),
            authored_selection_target: Some(selected),
            symbol: selected,
            signature_free_requirement: Some(Ok(
                crate::source_resolved::expression::ResolvedSignatureFreeRequirement {
                    trait_definition: SymbolHandle::from_parts(4, 5),
                    declaring_trait: SymbolHandle::from_parts(6, 5),
                    requirement: selected,
                },
            )),
            ..Default::default()
        };
        let mut bytes = encode(&argument, RecordLimits::default()).unwrap();
        assert_eq!(
            decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).unwrap(),
            argument
        );
        bytes[8] ^= 1;
        assert!(decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).is_err());
    }
}
