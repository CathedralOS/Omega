#![forbid(unsafe_code)]

/*
We represent parsed Omega syntax trees (`SyntaxTrees`, `SyntaxTreeRoots`, and
`SyntaxTreeTables`) before name and symbol resolution. Authored identifiers and
float spellings borrow their text through `SyntaxTextOwner` page handles and
spans, while arena tables store items, statements, expressions, and type
references.
*/

/*
We separate parsed surface structure from symbol and type assignment so the
parser (`Psi 01`) can checkpoint, rewind, and share immutable page baselines
without allocating per-identifier strings or coupling grammar recovery to
semantic tables. `SyntaxEquivalent` compares forests structurally through both
text owners so Build continuation sharing observes every retained row.
*/

/*
- Produced by: `Psi 01` (`01_tokens-to-syntax-trees`).
- Consumed by: `Psi 02` (`02_syntax-trees-to-symbol-resolved-trees`), `build-time-evaluation`, `build-declarations`, and `package-compilation`.
- Flat module layout:
  - `lib.rs`: `SyntaxTrees`, `SyntaxTreeRoots`, `SyntaxTreeTables`, `SyntaxEquivalent`, `ParsedSources`, and `PagedRoster`.
  - `declarations.rs`: top-level item declarations (`data`, `machine`, `trait`, `capability`, `proof`, `type_aliases`).
  - `control_flow_and_names.rs`: executable statements, inline assembly blocks, identifiers, and `SyntaxTextOwner`.
  - `values_and_types.rs`: surface expressions, float spellings, and syntactic type references.
  - `copies.rs`: cross-forest item and body copying with expression-handle rejoin (`item_copies`, `body_copies`).
  - `inspection.rs`: deterministic `SyntaxTreesSnapshot` formatting for compiler phase inspection.
*/

pub(crate) mod equality {

    use crate::{SyntaxTextOwner, SyntaxTrees};
    use arena::{Arena, Handle, HandleSpan, PagedArenaSet};
    use std::sync::Arc;

    pub trait SyntaxEquivalent {
        fn same_syntax(
            &self,
            other: &Self,
            left_text: &SyntaxTextOwner,
            right_text: &SyntaxTextOwner,
        ) -> bool;
    }

    impl SyntaxEquivalent for crate::identifier::Identifier {
        fn same_syntax(
            &self,
            other: &Self,
            left_text: &SyntaxTextOwner,
            right_text: &SyntaxTextOwner,
        ) -> bool {
            self.as_str(left_text) == other.as_str(right_text)
        }
    }
    impl PartialEq for SyntaxTrees {
        fn eq(&self, other: &Self) -> bool {
            let Self {
                source_id,
                roots,
                tables,
            } = self;
            let crate::SyntaxTreeTables {
                text,
                items,
                expressions,
                statements,
                type_references,
            } = tables;
            *source_id == other.source_id
                && *roots == other.roots
                && items.same_syntax(&other.tables.items, text, &other.text)
                && expressions.same_syntax(&other.tables.expressions, text, &other.text)
                && statements.same_syntax(&other.tables.statements, text, &other.text)
                && type_references.same_syntax(&other.tables.type_references, text, &other.text)
        }
    }
    impl Eq for SyntaxTrees {}

    impl<T> SyntaxEquivalent for Handle<T> {
        fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool {
            self == other
        }
    }
    impl<T> SyntaxEquivalent for HandleSpan<T> {
        fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool {
            self == other
        }
    }
    impl<T: SyntaxEquivalent> SyntaxEquivalent for Vec<T> {
        fn same_syntax(
            &self,
            other: &Self,
            left: &SyntaxTextOwner,
            right: &SyntaxTextOwner,
        ) -> bool {
            self.len() == other.len()
                && self.iter().zip(other).all(|(left_value, right_value)| {
                    left_value.same_syntax(right_value, left, right)
                })
        }
    }
    impl<T: SyntaxEquivalent> SyntaxEquivalent for Option<T> {
        fn same_syntax(
            &self,
            other: &Self,
            left: &SyntaxTextOwner,
            right: &SyntaxTextOwner,
        ) -> bool {
            match (self, other) {
                (Some(left_value), Some(right_value)) => {
                    left_value.same_syntax(right_value, left, right)
                }
                (None, None) => true,
                _ => false,
            }
        }
    }
    macro_rules! indirect {
    ($($container:ident),*) => { $(impl<T: SyntaxEquivalent + ?Sized> SyntaxEquivalent for $container<T> {
        fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool { (**self).same_syntax(&**other,left,right) }
    })* };
}
    indirect!(Box, Arc);
    impl<A: SyntaxEquivalent, B: SyntaxEquivalent> SyntaxEquivalent for (A, B) {
        fn same_syntax(
            &self,
            other: &Self,
            left: &SyntaxTextOwner,
            right: &SyntaxTextOwner,
        ) -> bool {
            self.0.same_syntax(&other.0, left, right) && self.1.same_syntax(&other.1, left, right)
        }
    }
    macro_rules! arena_equality {
    ($($arena:ident),*) => { $(impl<T: Default + SyntaxEquivalent> SyntaxEquivalent for $arena<T> {
        fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
            self.equal_by(other, |left_value,right_value| left_value.same_syntax(right_value,left,right))
        }
    })* };
}
    arena_equality!(Arena, PagedArenaSet);
    macro_rules! plain {
    ($($kind:ty),* $(,)?) => { $(impl SyntaxEquivalent for $kind {
        fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool { self == other }
    })* };
}
    plain!(
        bool,
        language_core::PatternComponent,
        usize,
        u8,
        u16,
        u32,
        u64,
        i64,
        String,
        source::SourceSpan,
        source::Span,
        source::SourceId,
        source::SourceBytes
    );

    impl<T: SyntaxEquivalent> SyntaxEquivalent for [T] {
        fn same_syntax(
            &self,
            other: &Self,
            left: &SyntaxTextOwner,
            right: &SyntaxTextOwner,
        ) -> bool {
            self.len() == other.len()
                && self.iter().zip(other).all(|(left_value, right_value)| {
                    left_value.same_syntax(right_value, left, right)
                })
        }
    }
    plain!(
        language_core::OperatorSpelling,
        numerics::literals::FloatFormat,
        numerics::literals::IntegerLiteral
    );
    plain!(
        language_core::LocalBindingRole,
        language_core::BindingRelevance,
        language_core::DataSupplyMode,
        language_core::CallOperationalAcknowledgement,
        language_core::DomainPredicatePresence,
        language_core::ReferenceAccess,
        language_core::DomainClassification,
        numerics::arithmetic::ArithmeticDomain
    );
}
pub use equality::SyntaxEquivalent;

mod copies;
pub(crate) use copies::item_copies;
mod control_flow_and_names;
pub use control_flow_and_names::{control_flow, names};
pub mod declarations;
pub mod inspection;
pub(crate) mod paged_roster {
    macro_rules! paged_roster {
    (
        $(#[$table_meta:meta])*
        $table_vis:vis struct $table:ident => $checkpoint_vis:vis $checkpoint:ident {
            $( arena $field:ident: $ty:ty, )*
            $( roster $nested:ident: $nested_ty:ty => $nested_checkpoint:ty, )*
        }
    ) => {
        $(#[$table_meta])*
        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        $table_vis struct $table {
            $( $field: arena::PagedArenaSet<$ty>, )*
            $( $nested: $nested_ty, )*
        }

        $checkpoint_vis struct $checkpoint {
            $( $field: arena::PagedArenaFrontier<$ty>, )*
            $( $nested: $nested_checkpoint, )*
        }

        impl $table {
            pub(crate) fn append_checkpoint(&self) -> $checkpoint {
                $checkpoint {
                    $( $field: self.$field.frontier(), )*
                    $( $nested: self.$nested.append_checkpoint(), )*
                }
            }

            pub(crate) fn rewind_appended(&mut self, checkpoint: $checkpoint) {
                $( self.$field.rewind_appended(checkpoint.$field); )*
                $( self.$nested.rewind_appended(checkpoint.$nested); )*
            }

            pub(crate) fn share_pages(&mut self) {
                $( self.$field.share_pages(); )*
                $( self.$nested.share_pages(); )*
            }

            pub(crate) fn new_with_generation(generation: u32) -> Self {
                Self {
                    $( $field: arena::PagedArenaSet::with_generation(generation), )*
                    $( $nested: <$nested_ty>::new_with_generation(generation), )*
                }
            }

            pub(crate) fn adopt(&mut self, other: Self) {
                $( self.$field.adopt(other.$field); )*
                $( self.$nested.adopt(other.$nested); )*
            }

            pub(crate) fn begin_page(&mut self, generation: u32) {
                $( self.$field.begin_page(generation); )*
                $( self.$nested.begin_page(generation); )*
            }
        }
    };
}

    pub(crate) use paged_roster;
}
pub mod source_files {
    use crate::item::ItemHandle;
    use arena::{Arena, HandleSpan};
    use source::{DependencyScope, SourceId, SourceOrigin};
    use std::ops::Range;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ParsedRootItems {
        pub items: Vec<ItemHandle>,
        pub import_range: Range<usize>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ParsedSource {
        pub source_id: SourceId,
        pub path: PathBuf,
        pub source: Arc<str>,
        pub origin: Option<SourceOrigin>,
        pub root_items: Vec<ItemHandle>,
        pub import_root_range: Range<usize>,
        pub scope: DependencyScope,
    }

    impl Default for ParsedSource {
        fn default() -> Self {
            Self {
                source_id: SourceId::default(),
                path: PathBuf::default(),
                source: Arc::from(""),
                origin: None,
                root_items: Vec::new(),
                import_root_range: 0..0,
                scope: DependencyScope::Product,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ParsedSources {
        pub sources: Arena<ParsedSource>,
        pub batch: HandleSpan<ParsedSource>,
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::SyntaxTrees;
    use crate::SyntaxEquivalent;
    use crate::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableMemberExpression,
    };
    use crate::identifier::Identifier;
    use crate::item::{
        DataDefinition, Item, Machine, ModuleDeclaration, State, StateHandle, StateSignature,
        TraitDefinition,
    };
    use crate::snapshot::ItemSnapshot;
    use crate::statement::{StatementNode, TableAssignment, TableCall};
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use arena::HandleSpan;
    use numerics::literals::IntegerLiteral;
    use source::{SourceId, SourceSpan, Span};

    fn shared_case_subject_syntax() -> (SyntaxTrees, ExpressionHandle, ExpressionHandle) {
        use crate::expression::{TableIndexedExpression, TableMembershipExpression};
        let mut source = SyntaxTrees::new(Default::default());
        let collection = source.expressions.insert(ExpressionNode::Boolean(false));
        let selector = source
            .expressions
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let subject = source
            .expressions
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index: selector,
            }));
        let classifier = source
            .expressions
            .append_identifier_path_member(Identifier::generated("Both"));
        let membership =
            source
                .expressions
                .insert(ExpressionNode::Membership(TableMembershipExpression {
                    value: subject,
                    domain: HandleSpan::from_parts(classifier, 1),
                }));
        let payload = source
            .expressions
            .insert(ExpressionNode::Member(TableMemberExpression {
                access_source_span: SourceSpan::new(SourceId(7), Span::new(11, 15)),
                receiver: subject,
                member: Identifier::generated("a"),
                case_variant: Some(Identifier::generated("Both")),
            }));
        (source, membership, payload)
    }

    fn copied_case_subject(
        destination: &SyntaxTrees,
        membership: ExpressionHandle,
        payload: ExpressionHandle,
    ) -> ExpressionHandle {
        let ExpressionNode::Membership(membership) = destination.expressions.expression(membership)
        else {
            panic!("copied membership");
        };
        let ExpressionNode::Member(payload) = destination.expressions.expression(payload) else {
            panic!("copied payload");
        };
        assert_eq!(membership.value, payload.receiver);
        assert_eq!(
            payload.access_source_span,
            SourceSpan::new(SourceId(7), Span::new(11, 15))
        );
        membership.value
    }

    #[test]
    fn expression_copy_preserves_shared_subject_and_isolates_transactions() {
        let (mut source, membership, payload) = shared_case_subject_syntax();
        let ExpressionNode::Membership(observation) = source.expressions.expression(membership)
        else {
            panic!("source membership");
        };
        let independent = source.expressions.expression(observation.value).clone();
        let independent = source.expressions.insert(independent);
        let independent_payload =
            source
                .expressions
                .insert(ExpressionNode::Member(TableMemberExpression {
                    access_source_span: SourceSpan::new(SourceId(7), Span::new(11, 15)),
                    receiver: independent,
                    member: Identifier::generated("a"),
                    case_variant: Some(Identifier::generated("Both")),
                }));
        let children = source.expressions.insert_expression_handles([
            membership,
            payload,
            independent_payload,
        ]);
        let root = source
            .expressions
            .insert(ExpressionNode::ArrayLiteral(children));
        let mut destination = SyntaxTrees::new(Default::default());
        let first = destination.copy_expression_from(&source, root);
        let second = destination.copy_expression_from(&source, root);
        let subject = |copied| {
            let ExpressionNode::ArrayLiteral(children) = destination.expressions.expression(copied)
            else {
                panic!("copied array root");
            };
            let children = destination.expressions.expression_handles(*children);
            let subject = copied_case_subject(&destination, children[0], children[1]);
            let ExpressionNode::Member(independent) =
                destination.expressions.expression(children[2])
            else {
                panic!("copied independent payload");
            };
            assert_ne!(subject, independent.receiver);
            subject
        };
        assert_ne!(subject(first), subject(second));
    }

    #[test]
    fn item_copy_preserves_shared_body_subject_and_isolates_instances() {
        let (mut source, membership, payload) = shared_case_subject_syntax();
        let statement = source
            .statements
            .insert(StatementNode::Assignment(TableAssignment {
                target: payload,
                value: membership,
            }));
        let statement = source.items.append_statement_handle(statement);
        let state = source.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            statements: HandleSpan::from_parts(statement, 1),
        });
        let state = source.items.append_state_handle(state);
        let fact = source
            .items
            .append_proof_fact(crate::item::ProofFact::Expression(membership));
        let contract = source
            .items
            .append_capability_contract(crate::item::CapabilityContract {
                kind: crate::item::CapabilityContractKind::Requires,
                keyword_source_span: None,
                binding: None,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 0,
            });
        let mut machine = single_state_machine(state);
        machine.contracts = HandleSpan::from_parts(contract, 1);
        let item = Item::Machine(machine);
        let mut destination = SyntaxTrees::new(Default::default());
        let first = destination.copy_item_from(&source, &item);
        let second = destination.copy_item_from(&source, &item);
        let subject = |copied: &Item| {
            let Item::Machine(machine) = copied else {
                panic!("copied machine");
            };
            let state = destination.items.state_handles(machine.states)[0];
            let statements = destination.items.state(state).statements;
            let statement = destination.items.statements(statements)[0];
            let StatementNode::Assignment(assignment) = destination.statements.statement(statement)
            else {
                panic!("copied assignment");
            };
            let crate::item::ProofFact::Expression(fact) =
                &destination.items.requirement_facts(machine.contracts)[0]
            else {
                panic!("copied expression fact");
            };
            assert_eq!(*fact, assignment.value);
            copied_case_subject(&destination, assignment.value, assignment.target)
        };
        assert_ne!(subject(&first), subject(&second));
    }

    #[test]
    fn adopts_parser_pages_without_remapping_roots() {
        let mut first = SyntaxTrees::new_with_generation(SourceId(10), 1);
        let first_root = first.push_root_item(Item::Module(ModuleDeclaration::default()));
        let mut second = SyntaxTrees::new_with_generation(SourceId(20), 2);
        let second_root = second.push_root_item(Item::Module(ModuleDeclaration::default()));

        first.adopt_page(second);
        assert_eq!(first.root_item_handles(), [first_root, second_root]);
        assert!(matches!(first.root_item(first_root), Item::Module(_)));
        assert!(matches!(first.root_item(second_root), Item::Module(_)));
        assert_eq!(first.page_count(), 2);

        first.begin_page(3);
        let generated = first.push_root_item(Item::Module(ModuleDeclaration::default()));
        assert_eq!(generated.generation(), 3);
        assert_eq!(first.page_count(), 3);
    }

    fn assembled_first_statement(
        file: &SyntaxTrees,
        expect: &'static str,
    ) -> (SyntaxTrees, crate::statement::StatementHandle) {
        let mut assembled = SyntaxTrees::new(Default::default());
        assembled.extend_from(file);

        let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
            panic!("expected machine root item");
        };
        let state_handle = assembled
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state handle");
        let state = assembled.items.state(state_handle);
        let statement_handle = assembled
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect(expect);
        (assembled, statement_handle)
    }

    fn single_state_machine(state: arena::Handle<StateHandle>) -> Machine {
        Machine {
            nested_machines: HandleSpan::empty(),
            name: Identifier::generated("main"),
            generic_data_template: Default::default(),
            default_body_trait: Default::default(),
            attached_data: None,
            spelling: None,
            is_public: false,
            target: None,
            authored_target: None,
            boundary: false,
            bodyless: false,
            target_family_interface: false,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            terminates_guarantee: false,
            ranking_subjects: HandleSpan::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: crate::expression::ExpressionHandle::invalid(),
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            suspends: false,
            blocks: false,
            admitted_same_stack_bytes: 0,
            declares_no_crash_route: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(state, 1),
        }
    }

    #[test]
    fn constant_initializer_normalization_survives_copy_and_clone() {
        let span = |start| SourceSpan::new(SourceId(3), Span::new(start, start + 4));
        let mut source = SyntaxTrees::new(Default::default());
        let operand = source.expressions.insert(ExpressionNode::Boolean(false));
        source.expressions.set_source_span(operand, span(24));
        let authored_expression = source.expressions.insert(ExpressionNode::Unary(
            crate::expression::TableUnaryExpression {
                operator: crate::expression::UnaryOperator::LogicalNot,
                operand,
            },
        ));
        source
            .expressions
            .set_source_span(authored_expression, span(20));
        let value = source.expressions.insert(ExpressionNode::Boolean(true));
        source.expressions.set_source_span(value, span(20));
        let normalization = crate::item::ConstInitializerNormalization {
            authored_custody: Default::default(),
            declaration_custody: Default::default(),
            authored_expression,
            result_value: "boolean4:true".to_owned().into(),
            selections: vec![crate::types::ConstArgumentOrigin {
                reference: span(24),
                declaration: span(4),
                initializer: span(12),
                evaluated_value: "boolean5:false".to_owned().into(),
            }],
            builtin_operators: vec![span(20)],
            call_selections: Vec::new(),
        };
        let constant = Item::Const(crate::item::ConstDefinition {
            name: Identifier::generated("ENABLED"),
            value,
            normalization: Some(normalization.clone()),
            ..Default::default()
        });
        source.push_root_item(constant.clone());
        let mut cloned = source.clone();
        assert_eq!(cloned, source);
        cloned
            .expressions
            .replace_expression(operand, ExpressionNode::Boolean(true));
        assert!(source.expressions.expression(operand).same_syntax(
            &ExpressionNode::Boolean(false),
            &source.text,
            &source.text
        ));

        let mut destination = SyntaxTrees::new(Default::default());
        for _ in 0..8 {
            destination
                .expressions
                .insert(ExpressionNode::Boolean(false));
        }
        let first = destination.copy_item_from(&source, &constant);
        destination.extend_from(&source);
        let second = destination
            .root_items()
            .next()
            .expect("extended constant")
            .clone();
        let mut copied_operands = Vec::new();
        for copied in [first, second] {
            let Item::Const(copied) = copied else {
                panic!("copied constant");
            };
            let receipt = copied.normalization.expect("retained normalization");
            assert_eq!(receipt.result_value, normalization.result_value);
            assert_eq!(receipt.selections, normalization.selections);
            assert_eq!(receipt.builtin_operators, normalization.builtin_operators);
            assert_ne!(copied.value, value);
            assert_ne!(receipt.authored_expression, authored_expression);
            assert_ne!(receipt.authored_expression, copied.value);
            assert!(
                destination
                    .expressions
                    .expression(copied.value)
                    .same_syntax(
                        &ExpressionNode::Boolean(true),
                        &destination.text,
                        &destination.text
                    )
            );
            assert_eq!(destination.expressions.source_span(copied.value), span(20));
            assert_eq!(
                destination
                    .expressions
                    .source_span(receipt.authored_expression),
                span(20)
            );
            let ExpressionNode::Unary(unary) = destination
                .expressions
                .expression(receipt.authored_expression)
            else {
                panic!("retained authored unary expression");
            };
            assert_eq!(unary.operator, crate::expression::UnaryOperator::LogicalNot);
            assert_ne!(unary.operand, operand);
            assert!(
                destination
                    .expressions
                    .expression(unary.operand)
                    .same_syntax(
                        &ExpressionNode::Boolean(false),
                        &destination.text,
                        &destination.text
                    )
            );
            assert_eq!(destination.expressions.source_span(unary.operand), span(24));
            copied_operands.push(unary.operand);
        }
        assert_ne!(copied_operands[0], copied_operands[1]);
        destination
            .expressions
            .replace_expression(copied_operands[0], ExpressionNode::Boolean(true));
        assert!(
            destination
                .expressions
                .expression(copied_operands[1])
                .same_syntax(
                    &ExpressionNode::Boolean(false),
                    &destination.text,
                    &destination.text
                )
        );
        assert!(source.expressions.expression(operand).same_syntax(
            &ExpressionNode::Boolean(false),
            &source.text,
            &source.text
        ));
    }

    #[test]
    fn semantic_nan_declaration_survives_copy_without_encoding() {
        use language_core::constant_value::EvaluatedConstant;
        use numerics::literals::FloatFormat;
        for format in [FloatFormat::F32, FloatFormat::F64] {
            let mut source = SyntaxTrees::new(Default::default());
            let value = source
                .expressions
                .insert(ExpressionNode::UndeterminedNaN(format));
            let constant = Item::Const(crate::item::ConstDefinition {
                value,
                normalization: Some(crate::item::ConstInitializerNormalization {
                    authored_expression: value,
                    result_value: EvaluatedConstant::UndeterminedNaN(format),
                    ..Default::default()
                }),
                ..Default::default()
            });
            let mut destination = SyntaxTrees::new(Default::default());
            let Item::Const(copied) = destination.copy_item_from(&source, &constant) else {
                panic!("copied constant");
            };
            assert!(
                destination
                    .expressions
                    .expression(copied.value)
                    .same_syntax(
                        &ExpressionNode::UndeterminedNaN(format),
                        &destination.text,
                        &destination.text
                    )
            );
            let receipt = copied.normalization.expect("retained semantic result");
            assert_eq!(
                receipt.result_value,
                EvaluatedConstant::UndeterminedNaN(format)
            );
            assert_eq!(receipt.result_value.encoding(), None);
        }
    }

    #[test]
    fn normalized_structural_expression_copy_remaps_children_and_preserves_source() {
        let mut source = SyntaxTrees::new(Default::default());
        let child = source.expressions.insert(ExpressionNode::Boolean(true));
        let children = source.expressions.insert_expression_handles([child]);
        let expression = source
            .expressions
            .insert(ExpressionNode::ArrayLiteral(children));
        let source_span = SourceSpan::new(SourceId(3), Span::new(10, 16));
        source.expressions.set_source_span(expression, source_span);
        let argument = source
            .type_references
            .insert_named(Identifier::generated("normalized-array"));
        source.type_references.retain_const_argument_normalization(
            argument,
            source_span,
            "array9:[bool; 1]13:boolean4:true".to_owned(),
            [],
            [],
        );
        source
            .type_references
            .retain_const_argument_expression(argument, expression);
        let mut destination = SyntaxTrees::new(Default::default());
        for _ in 0..8 {
            destination
                .expressions
                .insert(ExpressionNode::Boolean(false));
        }
        let first = destination.copy_type_reference_handle(&source, argument);
        let second = destination.copy_type_reference_handle(&source, argument);
        let mut copied_children = Vec::new();
        for copied in [first, second] {
            let copied_expression = destination
                .type_references
                .const_argument_normalization(copied)
                .unwrap()
                .authored_expression;
            assert_ne!(copied_expression, expression);
            assert_eq!(
                destination.expressions.source_span(copied_expression),
                source_span
            );
            let ExpressionNode::ArrayLiteral(children) =
                destination.expressions.expression(copied_expression)
            else {
                panic!("copied array");
            };
            let copied_child = destination.expressions.expression_handles(*children)[0];
            assert_ne!(copied_child, child);
            assert!(matches!(
                destination.expressions.expression(copied_child),
                ExpressionNode::Boolean(true)
            ));
            copied_children.push(copied_child);
        }
        assert_ne!(copied_children[0], copied_children[1]);
    }

    #[test]
    fn constant_argument_origin_survives_deep_copy_and_root_extension() {
        let span =
            |source_id, start| SourceSpan::new(SourceId(source_id), Span::new(start, start + 4));
        let origin = crate::types::ConstArgumentOrigin {
            reference: span(1, 40),
            declaration: span(2, 12),
            initializer: span(2, 24),
            evaluated_value: "integer3:u641:2".to_owned().into(),
        };
        let mut source = SyntaxTrees::new(Default::default());
        let second = crate::types::ConstArgumentOrigin {
            reference: span(1, 48),
            ..origin.clone()
        };
        let origins = [origin.clone(), second];
        let operators = [span(1, 44), span(1, 52)];
        let result_encoding = "integer3:u641:4";
        let argument = source
            .type_references
            .insert_named(Identifier::generated("4"));
        source.type_references.retain_const_argument_normalization(
            argument,
            span(1, 40),
            result_encoding.to_owned(),
            origins.clone(),
            operators,
        );
        let arguments = source
            .type_references
            .insert_type_reference_handles([argument]);
        let application = source
            .type_references
            .insert_generic(Identifier::generated("Buffer"), arguments);
        let instance = source
            .type_references
            .insert_named(Identifier::generated("Buffer<2>"));
        source
            .type_references
            .retain_generic_application_origin(instance, application);

        let mut destination = SyntaxTrees::new(Default::default());
        for _ in 0..16 {
            destination.type_references.insert_unit();
        }
        let copied = destination.copy_type_reference_handle(&source, instance);
        let copied_application = destination
            .type_references
            .generic_application_origin(copied);
        let TypeReferenceNode::Generic { arguments, .. } = destination
            .type_references
            .type_reference(copied_application)
        else {
            panic!("copied application");
        };
        let copied_argument = destination
            .type_references
            .type_reference_handles(*arguments)[0];
        assert_ne!(copied_argument, argument);
        let copied_normalization = destination
            .type_references
            .const_argument_normalization(copied_argument)
            .expect("copied normalization");
        assert_eq!(
            copied_normalization.canonical_result_encoding,
            result_encoding
        );
        assert_eq!(
            destination
                .type_references
                .const_argument_builtin_operators(copied_normalization.builtin_operators),
            &operators
        );
        assert_eq!(
            destination
                .type_references
                .const_argument_origins(copied_normalization.selections),
            &origins
        );

        source.push_root_item(Item::Data(DataDefinition {
            cleanup_selection: None,
            name: Identifier::generated("Buffer<2>"),
            is_public: false,
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            generic_instance: Some(application),
            properties: Default::default(),
            quotient: None,
            requires_facts: HandleSpan::empty(),
            members: HandleSpan::empty(),
        }));
        destination.extend_from(&source);
        let Item::Data(data) = destination.root_items().next().expect("extended data") else {
            panic!("extended data");
        };
        let TypeReferenceNode::Generic { arguments, .. } = destination
            .type_references
            .type_reference(data.generic_instance.expect("instance application"))
        else {
            panic!("extended application");
        };
        let extended_argument = destination
            .type_references
            .type_reference_handles(*arguments)[0];
        assert_ne!(extended_argument, copied_argument);
        let extended_normalization = destination
            .type_references
            .const_argument_normalization(extended_argument)
            .expect("extended normalization");
        assert_eq!(
            extended_normalization.canonical_result_encoding,
            result_encoding
        );
        assert_eq!(
            destination
                .type_references
                .const_argument_builtin_operators(extended_normalization.builtin_operators),
            &operators
        );
        assert_eq!(
            destination
                .type_references
                .const_argument_origins(extended_normalization.selections),
            &origins
        );
        let original_normalization = source
            .type_references
            .const_argument_normalization(argument)
            .expect("original normalization");
        assert_eq!(
            source
                .type_references
                .const_argument_origins(original_normalization.selections),
            &origins
        );
    }

    #[test]
    #[should_panic]
    fn duplicate_constant_argument_origin_cannot_overwrite_custody() {
        let mut syntax = SyntaxTrees::new(Default::default());
        let argument = syntax
            .type_references
            .insert_named(Identifier::generated("2"));
        syntax.type_references.retain_const_argument_normalization(
            argument,
            Default::default(),
            String::new(),
            [],
            [],
        );
        syntax.type_references.retain_const_argument_normalization(
            argument,
            Default::default(),
            String::new(),
            [],
            [],
        );
    }

    #[test]
    fn copying_nested_generic_origins_remaps_handles_and_preserves_occurrence_tokens() {
        let token = |name: &str, start| {
            Identifier::new(
                name,
                SourceSpan::new(Default::default(), Span::new(start, start + name.len())),
            )
        };
        let mut source = SyntaxTrees::new(Default::default());
        let secret = source.type_references.insert_named(token("Secret", 40));
        let arguments = source
            .type_references
            .insert_type_reference_handles([secret]);
        let inner_application = source
            .type_references
            .insert_generic(token("Envelope", 30), arguments);
        let inner = source
            .type_references
            .insert_named(token("Envelope<Secret>", 30));
        source
            .type_references
            .retain_generic_application_origin(inner, inner_application);
        let arguments = source
            .type_references
            .insert_type_reference_handles([inner]);
        let outer_application = source
            .type_references
            .insert_generic(token("Envelope", 20), arguments);
        let outer = source
            .type_references
            .insert_named(token("Envelope<Envelope<Secret>>", 20));
        source
            .type_references
            .retain_generic_application_origin(outer, outer_application);

        let mut destination = SyntaxTrees::new(Default::default());
        for _ in 0..16 {
            destination.type_references.insert_unit();
        }
        let copied = destination.copy_type_reference_handle(&source, outer);
        assert_ne!(copied, outer);
        let application = destination
            .type_references
            .generic_application_origin(copied);
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = destination.type_references.type_reference(application)
        else {
            panic!("outer application");
        };
        assert_eq!(base_name.source_span(), token("Envelope", 20).source_span());
        let inner = destination
            .type_references
            .type_reference_handles(*arguments)[0];
        let application = destination
            .type_references
            .generic_application_origin(inner);
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = destination.type_references.type_reference(application)
        else {
            panic!("inner application");
        };
        assert_eq!(base_name.source_span(), token("Envelope", 30).source_span());
        let secret = destination
            .type_references
            .type_reference_handles(*arguments)[0];
        let TypeReferenceNode::Named(name) = destination.type_references.type_reference(secret)
        else {
            panic!("exact private payload");
        };
        assert_eq!(name.as_str(&destination), "Secret");
        assert_eq!(name.source_span(), token("Secret", 40).source_span());
        assert!(
            destination.type_references.generic_nodes().is_empty(),
            "inert origins are not new synthesis work"
        );
    }

    #[test]
    fn syntax_copy_and_snapshot_preserve_generic_instance_origin() {
        let mut source = SyntaxTrees::new(Default::default());
        let argument = source
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("Message")));
        let arguments = source
            .type_references
            .insert_type_reference_handles([argument]);
        let origin = source.type_references.insert(TypeReferenceNode::Generic {
            base_name: Identifier::generated("Carrier"),
            lifetime_arguments: Vec::new(),
            arguments,
        });
        source.push_root_item(Item::Data(DataDefinition {
            cleanup_selection: None,
            name: Identifier::generated("irrelevant synthetic name"),
            is_public: false,
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            generic_instance: Some(origin),
            properties: Default::default(),
            quotient: None,
            requires_facts: HandleSpan::empty(),
            members: HandleSpan::empty(),
        }));

        let mut copied = SyntaxTrees::new(Default::default());
        copied.extend_from(&source);
        let Item::Data(data) = copied.root_items().next().expect("copied data") else {
            panic!("copied root must remain data");
        };
        let origin = data.generic_instance.expect("copied generic origin");
        assert!(matches!(
            copied.type_references.type_reference(origin),
            TypeReferenceNode::Generic { base_name, .. } if base_name.as_str(&copied) == "Carrier"
        ));

        let snapshot = copied.snapshot();
        assert!(matches!(
            &snapshot.root_items[0],
            ItemSnapshot::Data {
                conformance_bounds,
                generic_instance: Some(crate::snapshot::TypeReferenceSnapshot::Generic { .. }),
                ..
            } if conformance_bounds.is_empty()
        ));
    }

    #[test]
    fn syntax_trees_extend_from_preserves_trait_and_data_visibility() {
        let mut file = SyntaxTrees::new(Default::default());
        file.push_root_item(Item::Trait(TraitDefinition {
            is_boundary: false,
            is_public: true,
            name: Identifier::generated("PublicTrait"),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            parents: HandleSpan::empty(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
            refines: None,
            refinement_clauses: Vec::new(),
        }));
        file.push_root_item(Item::Data(DataDefinition {
            cleanup_selection: None,
            name: Identifier::generated("PublicData"),
            is_public: true,
            members: HandleSpan::empty(),
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: arena::HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            generic_instance: None,
            properties: Default::default(),
            quotient: None,
            requires_facts: arena::HandleSpan::empty(),
        }));

        let mut assembled = SyntaxTrees::new(Default::default());
        assembled.extend_from(&file);

        let mut roots = assembled.root_items();
        let Item::Trait(trait_definition) = roots.next().expect("trait root") else {
            panic!("expected trait root item");
        };
        let Item::Data(data_definition) = roots.next().expect("data root") else {
            panic!("expected data root item");
        };
        assert!(trait_definition.is_public);
        assert!(data_definition.is_public);

        let snapshot = assembled.snapshot();
        assert!(matches!(
            &snapshot.root_items[0],
            ItemSnapshot::Trait {
                is_public: true,
                ..
            }
        ));
        assert!(matches!(
            &snapshot.root_items[1],
            ItemSnapshot::Data {
                is_public: true,
                ..
            }
        ));
    }

    #[test]
    fn syntax_trees_extend_from_preserves_root_payload_handles() {
        let mut file = SyntaxTrees::new(Default::default());
        let suspends_keyword_source_span = SourceSpan::new(Default::default(), Span::new(10, 18));
        let blocks_keyword_source_span = SourceSpan::new(Default::default(), Span::new(20, 26));
        let return_type = file
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let state = file.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type,
            contracts: HandleSpan::empty(),
            statements: HandleSpan::empty(),
        });
        let state = file.items.append_state_handle(state);
        file.push_root_item(Item::Machine(Machine {
            nested_machines: HandleSpan::empty(),
            name: Identifier::generated("main"),
            generic_data_template: Default::default(),
            default_body_trait: Default::default(),
            attached_data: None,
            spelling: None,
            is_public: true,
            target: None,
            authored_target: None,
            boundary: false,
            bodyless: true,
            target_family_interface: false,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            terminates_guarantee: false,
            ranking_subjects: HandleSpan::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: crate::expression::ExpressionHandle::invalid(),
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: vec![suspends_keyword_source_span],
            blocks_keyword_source_spans: vec![blocks_keyword_source_span],
            suspends: true,
            blocks: true,
            admitted_same_stack_bytes: 0,
            declares_no_crash_route: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(state, 1),
        }));

        let mut assembled = SyntaxTrees::new(Default::default());
        assembled.extend_from(&file);

        let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
            panic!("expected machine root item");
        };
        assert!(machine.is_public, "syntax assembly must retain visibility");
        assert!(matches!(
            &assembled.snapshot().root_items[0],
            ItemSnapshot::Machine {
                is_public: true,
                ..
            }
        ));
        assert_eq!(
            machine.suspends_keyword_source_spans,
            [suspends_keyword_source_span]
        );
        assert_eq!(
            machine.blocks_keyword_source_spans,
            [blocks_keyword_source_span]
        );
        let state_handle = assembled
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state handle");
        let state = assembled.items.state(state_handle);
        assert_eq!(state.name.as_str(&assembled), "entry");
        assert!(state.return_type.is_valid());
    }

    #[test]
    fn syntax_signature_copy_preserves_operational_keyword_sources() {
        let mut source = SyntaxTrees::new(Default::default());
        let suspends_keyword_source_span = SourceSpan::new(Default::default(), Span::new(30, 38));
        let blocks_keyword_source_span = SourceSpan::new(Default::default(), Span::new(40, 46));
        let signature = source.items.insert_state_signature(&StateSignature {
            name: Identifier::generated("wait"),
            spelling: None,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            is_default: true,
            parameters: HandleSpan::empty(),
            native_callback_parameters: Vec::new(),
            return_type: TypeReferenceHandle::invalid(),
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: vec![suspends_keyword_source_span],
            blocks_keyword_source_spans: vec![blocks_keyword_source_span],
            suspends: true,
            blocks: true,
            contracts: HandleSpan::empty(),
            default_body: HandleSpan::empty(),
            terminates_guarantee: false,
        });

        let mut copied_trees = SyntaxTrees::new(Default::default());
        let copied = copied_trees
            .copy_state_signature_from(&source, source.items.state_signature(signature));

        assert_eq!(
            copied.suspends_keyword_source_spans,
            [suspends_keyword_source_span]
        );
        assert_eq!(
            copied.blocks_keyword_source_spans,
            [blocks_keyword_source_span]
        );
        assert!(copied.suspends);
        assert!(copied.blocks);
    }

    #[test]
    fn syntax_trees_extend_from_preserves_statement_call_arguments() {
        let mut file = SyntaxTrees::new(Default::default());
        let receiver = file
            .statements
            .append_identifier_path_member(Identifier::generated("self"));
        let receiver = HandleSpan::from_parts(receiver, 1);
        let argument = file
            .expressions
            .insert(crate::expression::ExpressionNode::Integer(
                IntegerLiteral::from_value(0),
            ));
        let argument = file.statements.append_expression_handle(argument);
        let call = file.statements.insert(StatementNode::Call(TableCall {
            target_is_static: false,
            receiver,
            target: Identifier::generated("take_non_negative"),
            machine_arguments: Box::default(),
            arguments: HandleSpan::from_parts(argument, 1),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }));
        let call = file.items.append_statement_handle(call);
        let state = file.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            statements: HandleSpan::from_parts(call, 1),
        });
        let state = file.items.append_state_handle(state);
        file.push_root_item(Item::Machine(single_state_machine(state)));

        let (assembled, statement_handle) = assembled_first_statement(&file, "call statement");
        let StatementNode::Call(call) = assembled.statements.statement(statement_handle) else {
            panic!("expected call statement");
        };
        assert_eq!(
            assembled
                .statements
                .expression_handles(call.arguments)
                .len(),
            1
        );
    }

    #[test]
    fn syntax_trees_extend_from_preserves_nested_expression_argument_spans() {
        let mut file = SyntaxTrees::new(Default::default());
        let target_name = file
            .expressions
            .append_identifier_path_member(Identifier::generated("xp"));
        let target = file
            .expressions
            .insert(ExpressionNode::Name(HandleSpan::from_parts(target_name, 1)));

        let player_name = file
            .expressions
            .append_identifier_path_member(Identifier::generated("player"));
        let player = file
            .expressions
            .insert(ExpressionNode::Name(HandleSpan::from_parts(player_name, 1)));
        let player_level = file
            .expressions
            .insert(ExpressionNode::Member(TableMemberExpression {
                access_source_span: SourceSpan::new(SourceId(7), Span::new(11, 15)),
                receiver: player,
                member: Identifier::generated("level"),
                case_variant: None,
            }));

        let nested_arguments = file.expressions.insert_expression_handles([player_level]);
        let nested_call = file
            .expressions
            .insert(ExpressionNode::Call(TableCallExpression {
                target_is_static: false,
                receiver: player,
                target: Identifier::generated("xp_required"),
                machine_arguments: Box::default(),
                arguments: nested_arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }));

        let zero = file
            .expressions
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
        let max_arguments = file
            .expressions
            .insert_expression_handles([zero, nested_call]);
        let max_call = file
            .expressions
            .insert(ExpressionNode::Call(TableCallExpression {
                target_is_static: false,
                receiver: ExpressionHandle::invalid(),
                target: Identifier::generated("max"),
                machine_arguments: Box::default(),
                arguments: max_arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }));

        let statement = file
            .statements
            .insert(StatementNode::Assignment(TableAssignment {
                target,
                value: max_call,
            }));
        let statement = file.items.append_statement_handle(statement);
        let state = file.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            statements: HandleSpan::from_parts(statement, 1),
        });
        let state = file.items.append_state_handle(state);
        file.push_root_item(Item::Machine(single_state_machine(state)));

        let (assembled, statement_handle) =
            assembled_first_statement(&file, "assignment statement");
        let StatementNode::Assignment(assignment) =
            assembled.statements.statement(statement_handle)
        else {
            panic!("expected assignment statement");
        };

        assert_eq!(
            assembled.display_expression_name(assignment.value),
            "max(0, player.xp_required(player.level))"
        );
    }

    #[test]
    fn retained_name_pages_survive_clone_copy_and_owner_retirement() {
        use source::{SourceId, SourceSpan, Span};
        use std::sync::Arc;
        let source: Arc<str> = Arc::from("first second");
        let mut original = SyntaxTrees::new(SourceId(1));
        let page = original.text.retain(source.clone());
        let first = Identifier::source_backed(page, SourceSpan::new(SourceId(1), Span::new(0, 5)));
        let original_name = original
            .expressions
            .append_identifier_path_member(first.clone());
        let original_expression =
            original
                .expressions
                .insert(ExpressionNode::Name(HandleSpan::from_parts(
                    original_name,
                    1,
                )));
        let checkpoint = original.clone();
        assert_eq!(
            Arc::strong_count(&source),
            3,
            "one reference per forest, independent of name count"
        );
        let mut destination = SyntaxTrees::new(SourceId(1));
        let other_text: Arc<str> = Arc::from("other");
        let other_page = destination.text.retain(other_text);
        let other_name =
            Identifier::source_backed(other_page, SourceSpan::new(SourceId(1), Span::new(0, 5)));
        let copied = destination.copy_expression_from(&original, original_expression);
        drop(original);
        drop(source);
        assert_eq!(first.as_str(&checkpoint), "first");
        assert_eq!(first.as_str(&destination), "first");
        assert_eq!(other_name.as_str(&destination), "other");
        assert_eq!(destination.display_expression_name(copied), "first");
        let generated = Identifier::new("generated", first.source_span());
        assert_eq!(
            generated.as_str(&destination),
            "generated",
            "authored coordinates do not define generated spelling"
        );
        drop(checkpoint);
        assert_eq!(destination.display_expression_name(copied), "first");
    }

    #[test]
    fn contextual_syntax_equality_includes_detached_nodes_and_logical_name_text() {
        use source::{SourceId, SourceSpan, Span};
        use std::sync::Arc;
        fn forest(spelling: &str) -> SyntaxTrees {
            let mut syntax = SyntaxTrees::new(SourceId(1));
            let page = syntax.text.retain(Arc::from(spelling));
            syntax
                .expressions
                .append_identifier_path_member(Identifier::source_backed(
                    page,
                    SourceSpan::new(SourceId(1), Span::new(0, spelling.len())),
                ));
            syntax
        }
        let left = forest("same");
        let right = forest("same");
        assert_eq!(
            left, right,
            "independent owner identities do not change logical equality"
        );
        assert_ne!(
            left,
            forest("else"),
            "unreachable arena rows remain part of completion input"
        );
        let mut generated = SyntaxTrees::new(SourceId(1));
        generated
            .expressions
            .append_identifier_path_member(Identifier::generated("same"));
        assert_eq!(
            left, generated,
            "name equality retains its existing spelling-only contract"
        );
        let mut changed = right.clone();
        changed.expressions.insert(ExpressionNode::Boolean(true));
        assert_ne!(left, changed);
    }

    #[test]
    fn constant_destination_debt_is_remapped_without_settling_or_refreshing_stale_inputs() {
        let mut source = SyntaxTrees::new(Default::default());
        let destination_type = source
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let value = source
            .expressions
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
        let other = source.expressions.insert(ExpressionNode::Boolean(false));
        let name = Identifier::generated("PENDING");
        let obligation = crate::item::ConstDestinationObligation {
            declaration: name.source_span(),
            destination: destination_type,
            initializer: value,
        };
        let mut constant = crate::item::ConstDefinition {
            name,
            type_reference: destination_type,
            value,
            pending_qualification: Some(obligation.clone()),
            ..Default::default()
        };
        let mut destination = SyntaxTrees::new(Default::default());
        for _ in 0..8 {
            destination
                .expressions
                .insert(ExpressionNode::Boolean(true));
        }
        destination
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("u64")));
        let Item::Const(copied) =
            destination.copy_item_from(&source, &Item::Const(constant.clone()))
        else {
            panic!("copied constant");
        };
        assert_ne!(copied.value, value);
        assert!(copied.qualification.is_none());
        let pending = copied
            .pending_qualification
            .expect("pending destination retained");
        assert_eq!(pending.declaration, copied.name.source_span());
        assert_eq!(pending.destination, copied.type_reference);
        assert_eq!(pending.initializer, copied.value);

        let stale = crate::item::ConstDestinationObligation {
            initializer: other,
            ..obligation
        };
        constant.pending_qualification = Some(stale.clone());
        let Item::Const(copied) = destination.copy_item_from(&source, &Item::Const(constant))
        else {
            panic!("copied stale constant");
        };
        assert!(copied.qualification.is_none());
        let pending = copied
            .pending_qualification
            .expect("stale debt cannot vanish");
        assert_eq!(pending, stale);
        assert_ne!(pending.initializer, copied.value);
    }

    #[test]
    fn copying_const_result_preserves_distinct_computation_and_materialized_value() {
        let mut source = SyntaxTrees::new(Default::default());
        let authored = source.expressions.insert(ExpressionNode::Boolean(true));
        let materialized = source.expressions.insert(ExpressionNode::Boolean(false));
        let argument = source
            .type_references
            .insert_named(Identifier::generated("result"));
        source.type_references.retain_const_argument_normalization(
            argument,
            Default::default(),
            String::new(),
            [],
            [],
        );
        source
            .type_references
            .retain_const_argument_expression(argument, authored);
        source
            .type_references
            .retain_const_argument_materialized_expression(argument, materialized);
        let mut copied = SyntaxTrees::new(Default::default());
        for _ in 0..8 {
            copied.expressions.insert(ExpressionNode::Boolean(true));
        }
        let argument = copied.copy_type_reference_handle(&source, argument);
        let normalization = copied
            .type_references
            .const_argument_normalization(argument)
            .unwrap();
        assert_ne!(normalization.authored_expression, authored);
        assert_ne!(normalization.materialized_expression, materialized);
        assert!(matches!(
            copied
                .expressions
                .expression(normalization.authored_expression),
            ExpressionNode::Boolean(true)
        ));
        assert!(matches!(
            copied
                .expressions
                .expression(normalization.materialized_expression),
            ExpressionNode::Boolean(false)
        ));
    }
}
mod values_and_types;
pub use values_and_types::{type_system, values};

use crate::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::item::{
    Item, ItemHandle, ItemTable, Machine, MathematicalDefinition, MathematicalDefinitionHandle,
    TraitDefinition,
};
use crate::statement::StatementTable;
use crate::types::TypeReferenceTable;
use source::SourceId;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, compiler_storage::Record)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub roots: SyntaxTreeRoots,
    pub tables: SyntaxTreeTables,
}

#[derive(Debug, Clone, compiler_storage::Record)]
pub struct SyntaxTreeTables {
    pub text: crate::SyntaxTextOwner,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct SyntaxTreeRoots {
    pub items: Vec<ItemHandle>,
    pub constant_extent_probes: Vec<(ExpressionHandle, bool)>,
    pub mathematical_definitions: Vec<MathematicalDefinitionHandle>,
}

pub struct SyntaxAppendCheckpoint {
    items: declarations::item::ItemTableAppendCheckpoint,
    expressions: values::expression::ExpressionTableAppendCheckpoint,
    statements: control_flow::statement::StatementTableAppendCheckpoint,
    types: type_system::types::TypeReferenceTableAppendCheckpoint,
    root_items: usize,
    mathematical_definitions: usize,
    constant_extent_probes: usize,
}

impl SyntaxTrees {
    pub fn append_checkpoint(&self) -> SyntaxAppendCheckpoint {
        SyntaxAppendCheckpoint {
            items: self.items.append_checkpoint(),
            expressions: self.expressions.append_checkpoint(),
            statements: self.statements.append_checkpoint(),
            types: self.type_references.append_checkpoint(),
            root_items: self.roots.items.len(),
            mathematical_definitions: self.roots.mathematical_definitions.len(),
            constant_extent_probes: self.roots.constant_extent_probes.len(),
        }
    }

    pub fn rewind_appended(&mut self, checkpoint: SyntaxAppendCheckpoint) {
        self.items.rewind_appended(checkpoint.items);
        self.expressions.rewind_appended(checkpoint.expressions);
        self.statements.rewind_appended(checkpoint.statements);
        self.type_references.rewind_appended(checkpoint.types);
        self.roots.items.truncate(checkpoint.root_items);
        self.roots
            .mathematical_definitions
            .truncate(checkpoint.mathematical_definitions);
        self.roots
            .constant_extent_probes
            .truncate(checkpoint.constant_extent_probes);
    }

    pub fn share_pages(&mut self) {
        self.items.share_pages();
        self.expressions.share_pages();
        self.statements.share_pages();
        self.type_references.share_pages();
    }

    pub fn new(source_id: SourceId) -> Self {
        Self::new_with_generation(source_id, 1)
    }

    pub fn new_with_generation(source_id: SourceId, generation: u32) -> Self {
        Self {
            source_id,
            roots: SyntaxTreeRoots::new_with_generation(generation),
            tables: SyntaxTreeTables::new_with_generation(generation),
        }
    }

    pub fn display_expression_name(&self, handle: ExpressionHandle) -> String {
        self.expressions.expression(handle).display_name(self)
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        let handle = self.insert_item(item);
        self.roots.items.push(handle);
        handle
    }

    pub fn const_expression_contains_name(&self, expression: ExpressionHandle) -> bool {
        match self.expressions.expression(expression) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Binary(binary) => {
                self.const_expression_contains_name(binary.left)
                    || self.const_expression_contains_name(binary.right)
            }
            ExpressionNode::Unary(unary) => self.const_expression_contains_name(unary.operand),
            _ => false,
        }
    }

    pub fn push_root_mathematical_definition(
        &mut self,
        definition: MathematicalDefinition,
    ) -> MathematicalDefinitionHandle {
        let handle = self.items.append_mathematical_definition(definition);
        self.roots.mathematical_definitions.push(handle);
        handle
    }

    pub fn root_mathematical_definition_handles(&self) -> &[MathematicalDefinitionHandle] {
        &self.roots.mathematical_definitions
    }

    pub fn root_mathematical_definition(
        &self,
        handle: MathematicalDefinitionHandle,
    ) -> &MathematicalDefinition {
        self.items.mathematical_definition(handle)
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        &self.roots.items
    }

    #[inline]
    pub fn root_item(&self, handle: ItemHandle) -> &Item {
        self.items.item(handle)
    }

    pub fn root_items(&self) -> impl Iterator<Item = &Item> {
        self.root_item_handles()
            .iter()
            .map(|handle| self.root_item(*handle))
    }

    pub fn root_item_count(&self) -> usize {
        self.roots.items.len()
    }

    pub fn constant_initializer_owns_selection(&self, reference: source::SourceSpan) -> bool {
        self.root_items().any(|item| {
            let Item::Const(definition) = item else {
                return false;
            };
            definition
                .normalization
                .as_ref()
                .is_some_and(|normalization| {
                    normalization
                        .selections
                        .iter()
                        .any(|origin| origin.reference == reference)
                        || normalization.builtin_operators.contains(&reference)
                        || normalization
                            .call_selections
                            .iter()
                            .any(|selection| selection.0 == reference)
                })
        })
    }

    pub fn extend_from(&mut self, other: &SyntaxTrees) {
        let frontier = self.type_references.supplied_operand_owner_frontier();
        let mut declarations = Vec::with_capacity(other.root_item_count());
        for handle in other.root_item_handles() {
            let copied = self.push_copied_root_item(other, *handle);
            declarations.push((*handle, copied));
        }
        for handle in other.root_mathematical_definition_handles() {
            let definition = self
                .copy_mathematical_definition(other, other.root_mathematical_definition(*handle));
            self.push_root_mathematical_definition(definition);
        }
        self.type_references
            .remap_supplied_operand_owners(frontier, &declarations);
    }

    pub fn adopt_page(&mut self, other: SyntaxTrees) {
        self.roots.items.extend(other.roots.items);
        self.roots
            .mathematical_definitions
            .extend(other.roots.mathematical_definitions);
        self.tables.adopt(other.tables);
    }

    pub fn begin_page(&mut self, generation: u32) {
        self.tables.begin_page(generation);
    }

    pub fn page_count(&self) -> usize {
        self.tables.page_count()
    }

    fn insert_item(&mut self, item: Item) -> ItemHandle {
        match &item {
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Trait(trait_definition) => self.insert_trait_definition(trait_definition),
            Item::Capability(_)
            | Item::Conformance(_)
            | Item::Const(_)
            | Item::Data(_)
            | Item::TypeAlias(_)
            | Item::Domain(_)
            | Item::Measure(_)
            | Item::Module(_)
            | Item::Operator(_)
            | Item::Package(_)
            | Item::Proposition(_)
            | Item::Use(_) => {}
        }

        self.items.append_item(item)
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine(machine);
    }

    fn insert_trait_definition(&mut self, trait_definition: &TraitDefinition) {
        self.items.insert_trait_definition(trait_definition);
    }

    fn push_copied_root_item(&mut self, other: &SyntaxTrees, handle: ItemHandle) -> ItemHandle {
        let item = self.copy_item_from(other, other.root_item(handle));
        self.push_root_item(item)
    }
}

impl SyntaxTreeTables {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn new_with_generation(generation: u32) -> Self {
        Self {
            text: crate::SyntaxTextOwner::default(),
            items: ItemTable::new_with_generation(generation),
            expressions: ExpressionTable::new_with_generation(generation),
            statements: StatementTable::new_with_generation(generation),
            type_references: TypeReferenceTable::new_with_generation(generation),
        }
    }

    fn adopt(&mut self, other: Self) {
        self.text.adopt(other.text);
        self.items.adopt(other.items);
        self.expressions.adopt(other.expressions);
        self.statements.adopt(other.statements);
        self.type_references.adopt(other.type_references);
    }

    fn begin_page(&mut self, generation: u32) {
        self.items.begin_page(generation);
        self.expressions.begin_page(generation);
        self.statements.begin_page(generation);
        self.type_references.begin_page(generation);
    }

    fn page_count(&self) -> usize {
        self.items.page_count()
    }
}

impl SyntaxTreeRoots {
    fn new_with_generation(_generation: u32) -> Self {
        Self {
            items: Vec::new(),
            constant_extent_probes: Vec::new(),
            mathematical_definitions: Vec::new(),
        }
    }
}

impl Default for SyntaxTreeTables {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SyntaxTrees {
    type Target = SyntaxTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for SyntaxTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}

impl Default for SyntaxTrees {
    fn default() -> Self {
        Self::new(SourceId::default())
    }
}

impl crate::SyntaxTextContext for SyntaxTrees {
    fn source_text(&self, source: arena::Handle<crate::RetainedSource>) -> &str {
        self.text.source_text(source)
    }
}

pub use control_flow::statement;
pub use declarations::item;
pub use inspection::snapshot;
pub use language_core::operator_spelling;
pub use names::identifier;
pub use names::text_owner::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};
pub use snapshot::SyntaxTreesSnapshot;
pub use source_files::{ParsedRootItems, ParsedSource, ParsedSources};
pub use type_system::types;
pub use values::expression;
pub use values::float_spelling::FloatSpelling;

pub mod syntax_trees {
    pub use crate::*;
}
