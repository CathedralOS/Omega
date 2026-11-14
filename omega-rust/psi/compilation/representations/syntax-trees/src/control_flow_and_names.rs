/*
We represent parsed executable statements (`StatementNode`), inline assembly
blocks (`AssemblyBlock`), spanned identifier occurrences (`Identifier`), and
paged source-text ownership (`SyntaxTextOwner`) for `SyntaxTrees`.
*/

pub mod control_flow {
    pub mod assembly {
        use crate::expression::ExpressionHandle;
        use crate::identifier::Identifier;
        use crate::paged_roster::paged_roster;
        use crate::statement::StatementHandle;
        use arena::{Handle, HandleSpan};

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct AssemblyControlFlow {
            pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
            pub primitives: HandleSpan<ExpressionHandle>,
            pub scratch: [ExpressionHandle; language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
            pub blocks: HandleSpan<AssemblyBlock>,
        }
        impl Default for AssemblyControlFlow {
            fn default() -> Self {
                Self {
                    target_requirement: Default::default(),
                    primitives: Default::default(),
                    scratch: [ExpressionHandle::invalid();
                        language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                    blocks: Default::default(),
                }
            }
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct AssemblyBlock {
            pub label: Identifier,
            pub requires: HandleSpan<ExpressionHandle>,
            pub ensures: HandleSpan<ExpressionHandle>,
            pub instructions: HandleSpan<StatementHandle>,
            pub exit: AssemblyExit,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub enum AssemblyExit {
            #[default]
            Fallthrough,
            Jump(Identifier),
            ZeroFlag(Identifier, bool),
            EnclosingTransition,
        }

        paged_roster! {
            pub(crate) struct AssemblyTable => pub(crate) AssemblyTableAppendCheckpoint {
                arena graphs: AssemblyControlFlow,
                arena blocks: AssemblyBlock,
                arena instructions: StatementHandle,
                arena expressions: ExpressionHandle,
            }
        }

        impl AssemblyTable {
            pub(crate) fn insert_graph(
                &mut self,
                graph: AssemblyControlFlow,
            ) -> Handle<AssemblyControlFlow> {
                self.graphs.insert(graph)
            }

            pub(crate) fn graph(
                &self,
                handle: Handle<AssemblyControlFlow>,
            ) -> &AssemblyControlFlow {
                self.graphs.get(handle)
            }

            pub(crate) fn insert_blocks(
                &mut self,
                blocks: impl IntoIterator<Item = AssemblyBlock>,
            ) -> HandleSpan<AssemblyBlock> {
                self.blocks.insert_many(blocks)
            }

            pub(crate) fn blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
                self.blocks.span_or_empty(span)
            }

            pub(crate) fn insert_instructions(
                &mut self,
                instructions: impl IntoIterator<Item = StatementHandle>,
            ) -> HandleSpan<StatementHandle> {
                self.instructions.insert_many(instructions)
            }

            pub(crate) fn instructions(
                &self,
                span: HandleSpan<StatementHandle>,
            ) -> &[StatementHandle] {
                self.instructions.span_or_empty(span)
            }

            pub(crate) fn insert_expressions(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.expressions.insert_many(expressions)
            }

            pub(crate) fn expressions(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.expressions.span_or_empty(span)
            }
        }
    }
    pub mod statement {
        use super::assembly::AssemblyTable;
        pub use super::assembly::{AssemblyBlock, AssemblyControlFlow, AssemblyExit};
        use crate::expression::ExpressionHandle;
        use crate::identifier::Identifier;
        use crate::paged_roster::paged_roster;
        use arena::{Handle, HandleSpan};
        use source::SourceSpan;

        pub type StatementHandle = Handle<StatementNode>;

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct RootBinding {
            pub receiver: ExpressionHandle,
            pub slot: Box<[Identifier]>,
            pub implementation: Box<[Identifier]>,
            pub implementation_operand: ExpressionHandle,
            pub source_span: SourceSpan,
        }
        pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

        paged_roster! {
            pub struct StatementTable => pub(crate) StatementTableAppendCheckpoint {
                arena statements: StatementNode,
                arena expression_handles: ExpressionHandle,
                arena identifier_path_members: Identifier,
                arena transition_targets: TransitionTargetNode,
                arena transition_patterns: TransitionPattern,
                arena outcome_proof_selectors: TableOutcomeProofSelector,
                roster assembly: AssemblyTable => super::assembly::AssemblyTableAppendCheckpoint,
            }
        }

        impl StatementTable {
            pub fn insert_transition_patterns(
                &mut self,
                patterns: impl IntoIterator<Item = TransitionPattern>,
            ) -> HandleSpan<TransitionPattern> {
                self.transition_patterns.insert_many(patterns)
            }

            pub fn transition_patterns(
                &self,
                patterns: HandleSpan<TransitionPattern>,
            ) -> &[TransitionPattern] {
                self.transition_patterns.span_or_empty(patterns)
            }

            pub fn transition_pattern_mut(
                &mut self,
                handle: Handle<TransitionPattern>,
            ) -> &mut TransitionPattern {
                self.transition_patterns.get_mut(handle)
            }

            pub fn new() -> Self {
                Self::new_with_generation(1)
            }

            pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
                self.statements.append(statement)
            }

            pub fn insert_assembly(
                &mut self,
                graph: AssemblyControlFlow,
            ) -> Handle<AssemblyControlFlow> {
                self.assembly.insert_graph(graph)
            }

            pub fn assembly(&self, handle: Handle<AssemblyControlFlow>) -> &AssemblyControlFlow {
                self.assembly.graph(handle)
            }

            pub fn insert_assembly_blocks(
                &mut self,
                blocks: impl IntoIterator<Item = AssemblyBlock>,
            ) -> HandleSpan<AssemblyBlock> {
                self.assembly.insert_blocks(blocks)
            }

            pub fn assembly_blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
                self.assembly.blocks(span)
            }

            pub fn insert_assembly_instructions(
                &mut self,
                instructions: impl IntoIterator<Item = StatementHandle>,
            ) -> HandleSpan<StatementHandle> {
                self.assembly.insert_instructions(instructions)
            }

            pub fn assembly_instructions(
                &self,
                span: HandleSpan<StatementHandle>,
            ) -> &[StatementHandle] {
                self.assembly.instructions(span)
            }

            pub fn insert_assembly_expressions(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.assembly.insert_expressions(expressions)
            }

            pub fn assembly_expressions(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.assembly.expressions(span)
            }

            pub fn append_expression_handle(
                &mut self,
                expression: ExpressionHandle,
            ) -> Handle<ExpressionHandle> {
                self.expression_handles.append(expression)
            }

            pub fn insert_expression_handles(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.expression_handles.insert_many(expressions)
            }

            pub fn append_identifier_path_member(
                &mut self,
                member: Identifier,
            ) -> Handle<Identifier> {
                self.identifier_path_members.append(member)
            }

            pub fn insert_transition_target(
                &mut self,
                target: TransitionTargetNode,
            ) -> TransitionTargetHandle {
                self.transition_targets.insert(target)
            }

            pub fn insert_outcome_proof_selectors(
                &mut self,
                selectors: impl IntoIterator<Item = TableOutcomeProofSelector>,
            ) -> HandleSpan<TableOutcomeProofSelector> {
                self.outcome_proof_selectors.insert_many(selectors)
            }

            pub fn outcome_proof_selectors(
                &self,
                span: HandleSpan<TableOutcomeProofSelector>,
            ) -> &[TableOutcomeProofSelector] {
                self.outcome_proof_selectors.span_or_empty(span)
            }

            pub fn replace_statement(&mut self, handle: StatementHandle, statement: StatementNode) {
                *self.statements.get_mut(handle) = statement;
            }

            pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
                self.statements.get(handle)
            }

            pub fn expression_handles(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.expression_handles.span_or_empty(span)
            }

            pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
                self.identifier_path_members.span_or_empty(span)
            }

            pub fn transition_target(
                &self,
                handle: TransitionTargetHandle,
            ) -> &TransitionTargetNode {
                self.transition_targets.get(handle)
            }

            pub fn statement_count(&self) -> usize {
                self.statements.len()
            }

            pub fn transition_target_count(&self) -> usize {
                self.transition_targets.len()
            }
        }

        impl Default for StatementTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum StatementNode {
            Assembly(Handle<AssemblyControlFlow>),
            RootBinding(RootBinding),
            AssemblyFact(TableAssemblyFact),
            Assignment(TableAssignment),
            Call(TableCall),
            ProofOutputBindingStatement(TableProofOutputBindingStatement),
            Expression(ExpressionHandle),
            LocalData(TableLocalData),
            Transition(TableTransition),
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableProofOutputBindingStatement {
            pub bindings: Box<[TableProofOutputSelector]>,
            pub call: ExpressionHandle,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableProofOutputSelector {
            pub output_field: Identifier,
            pub binding: Identifier,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct TableOutcomeProofSelector {
            pub output_field: Identifier,
            pub binding: Identifier,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableAssemblyFact {
            pub kind: AssemblyFactKind,
            pub expression: ExpressionHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum AssemblyFactKind {
            Requires,
            Ensures,
        }

        impl Default for StatementNode {
            fn default() -> Self {
                Self::Expression(ExpressionHandle::invalid())
            }
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableAssignment {
            pub target: ExpressionHandle,
            pub value: ExpressionHandle,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct TableCall {
            pub callee: ExpressionHandle,
            pub receiver: HandleSpan<Identifier>,
            pub target: Identifier,
            pub target_is_static: bool,
            pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
            pub evidence_arguments: Box<[Identifier]>,
            pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
            pub discards_result: bool,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct TableLocalData {
            pub role: language_core::LocalBindingRole,
            pub name: Identifier,
            pub type_reference: crate::types::TypeReferenceHandle,
            pub initial_value: ExpressionHandle,
            pub is_mutable: bool,
            pub relevance: language_core::BindingRelevance,
            /// The fields a record pattern names, in authored order, each
            /// bound or waived. Empty on every local that is not a
            /// `LocalBindingRole::RecordPattern` marker.
            pub pattern_fields: HandleSpan<crate::expression::TableMatchField>,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TransitionPattern {
            pub is_structural: bool,
            pub subject: ExpressionHandle,
            pub capture: Handle<crate::expression::MatchSubjectCapture>,
            pub pattern: crate::expression::TableDestructurePattern,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableTransition {
            pub patterns: HandleSpan<TransitionPattern>,
            pub target: TransitionTargetHandle,
            pub continuation: TransitionTargetHandle,
            pub guard: TransitionGuardNode,
            pub proof_selectors: HandleSpan<TableOutcomeProofSelector>,
            pub exit: TransitionExit,
            pub ends_dispatch: bool,
            pub source_span: SourceSpan,
        }

        impl TableTransition {
            pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
                [self.target, self.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
            }
        }

        impl Default for TableTransition {
            fn default() -> Self {
                Self {
                    patterns: HandleSpan::empty(),
                    target: TransitionTargetHandle::invalid(),
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: Default::default(),
                    ends_dispatch: true,
                    source_span: SourceSpan::default(),
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionExit {
            #[default]
            Ordinary,
            Crash(crate::item::CrashCause),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionGuardNode {
            Always,
            When(ExpressionHandle),
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub enum TransitionTargetNode {
            Named {
                operational_acknowledgement: language_core::CallOperationalAcknowledgement,
                path: HandleSpan<Identifier>,
                arguments: HandleSpan<ExpressionHandle>,
                evidence_arguments: Box<[Identifier]>,
                source_span: SourceSpan,
            },
            Value(ExpressionHandle),
            #[default]
            Terminal,
        }

        #[cfg(test)]
        mod tests {
            use super::{
                StatementNode, StatementTable, TableTransition, TransitionGuardNode,
                TransitionTargetNode,
            };
            use crate::expression::{ExpressionNode, ExpressionTable};
            use crate::identifier::Identifier;
            use arena::HandleSpan;

            #[test]
            fn statement_table_stores_transition_payloads_as_handles() {
                let mut statements = StatementTable::new();
                let mut expressions = ExpressionTable::new();
                let path_start =
                    statements.append_identifier_path_member(Identifier::generated("next"));
                let path = HandleSpan::from_parts(path_start, 1);
                let argument_one = expressions.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(1),
                ));
                let argument_one = statements.append_expression_handle(argument_one);
                let argument_two = expressions.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(2),
                ));
                let _argument_two = statements.append_expression_handle(argument_two);
                let arguments = HandleSpan::from_parts(argument_one, 2);
                let target = statements.insert_transition_target(TransitionTargetNode::Named {
                    operational_acknowledgement: Default::default(),
                    path,
                    arguments,
                    evidence_arguments: Box::default(),
                    source_span: Default::default(),
                });
                let guard = expressions.insert(ExpressionNode::Boolean(true));
                let statement = statements.insert(StatementNode::Transition(TableTransition {
                    patterns: Default::default(),
                    target,
                    continuation: super::TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::When(guard),
                    proof_selectors: HandleSpan::empty(),
                    exit: Default::default(),
                    ends_dispatch: true,
                    source_span: Default::default(),
                }));

                let StatementNode::Transition(transition) = statements.statement(statement) else {
                    panic!("statement should lower to a table transition");
                };
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = statements.transition_target(transition.target)
                else {
                    panic!("transition target should be named");
                };

                assert_eq!(path.count(), 1);
                assert_eq!(arguments.count(), 2);
                assert_eq!(expressions.expression_count(), 3);

                let mut acknowledged = statements.transition_target(target).clone();
                let TransitionTargetNode::Named {
                    operational_acknowledgement,
                    ..
                } = &mut acknowledged
                else {
                    panic!("named target");
                };
                operational_acknowledgement.acknowledges_block = true;
                let bytes = compiler_storage::encode(&acknowledged, Default::default()).unwrap();
                let restored: TransitionTargetNode =
                    compiler_storage::decode(&bytes, Default::default()).unwrap();
                let TransitionTargetNode::Named {
                    operational_acknowledgement,
                    path: restored_path,
                    arguments: restored_arguments,
                    ..
                } = restored
                else {
                    panic!("record changed the named call form");
                };
                assert!(operational_acknowledgement.acknowledges_block);
                assert!(!operational_acknowledgement.acknowledges_suspend);
                assert_eq!(restored_path, *path);
                assert_eq!(restored_arguments, *arguments);
            }
        }
    }
}

pub mod names {
    pub mod identifier {
        use super::text_owner::{RetainedSource, SyntaxTextContext};
        use arena::Handle;
        use std::fmt;

        use source::SourceSpan;

        #[derive(Clone, Default, compiler_storage::Record)]
        pub struct Identifier {
            text: IdentifierText,
            source_span: SourceSpan,
        }

        #[derive(Clone, Default, compiler_storage::Record)]
        enum IdentifierText {
            #[default]
            Missing,
            Owned(Box<str>),
            SourceBacked {
                source: Handle<RetainedSource>,
            },
        }

        impl Identifier {
            pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
                Self {
                    text: IdentifierText::Owned(text.into().into_boxed_str()),
                    source_span,
                }
            }

            pub fn generated(text: impl Into<String>) -> Self {
                Self {
                    text: IdentifierText::Owned(text.into().into_boxed_str()),
                    source_span: SourceSpan::default(),
                }
            }

            pub fn source_backed(source: Handle<RetainedSource>, source_span: SourceSpan) -> Self {
                Self {
                    text: IdentifierText::SourceBacked { source },
                    source_span,
                }
            }

            #[inline]
            pub fn as_str<'text>(
                &'text self,
                context: &'text impl SyntaxTextContext,
            ) -> &'text str {
                match &self.text {
                    IdentifierText::Missing => "",
                    IdentifierText::Owned(text) => text.as_ref(),
                    IdentifierText::SourceBacked { source } => {
                        let span = self.source_span.span;
                        &context.source_text(*source)[span.start..span.end]
                    }
                }
            }

            pub fn source_span(&self) -> SourceSpan {
                self.source_span
            }

            pub fn trailing_span(&self, length: usize) -> SourceSpan {
                let mut span = self.source_span();
                if span.span.end >= length {
                    span.span.start = span.span.end - length;
                }
                span
            }

            pub fn is_self_receiver(&self, context: &impl SyntaxTextContext) -> bool {
                language_core::is_self_receiver(self.as_str(context))
            }

            pub fn is_source_backed(&self) -> bool {
                self.source_span.span.start != self.source_span.span.end
            }

            pub fn into_string(self, context: &impl SyntaxTextContext) -> String {
                match self.text {
                    IdentifierText::Missing => String::new(),
                    IdentifierText::Owned(text) => text.into_string(),
                    IdentifierText::SourceBacked { source } => {
                        let span = self.source_span.span;
                        context.source_text(source)[span.start..span.end].to_owned()
                    }
                }
            }
        }

        impl From<&str> for Identifier {
            fn from(text: &str) -> Self {
                Self::generated(text)
            }
        }

        impl fmt::Debug for Identifier {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut value = formatter.debug_struct("Identifier");
                match &self.text {
                    IdentifierText::Missing => {
                        value.field("text", &"");
                    }
                    IdentifierText::Owned(text) => {
                        value.field("text", text);
                    }
                    IdentifierText::SourceBacked { source } => {
                        value.field("source", source);
                    }
                }
                value.field("source_span", &self.source_span).finish()
            }
        }

        pub trait JoinedSyntaxPath {
            fn joined_path_in(&self, context: &impl SyntaxTextContext) -> String {
                self.joined_path_with_in(context, "::")
            }
            fn joined_path_with_in(
                &self,
                context: &impl SyntaxTextContext,
                separator: &str,
            ) -> String;
        }
        impl JoinedSyntaxPath for [Identifier] {
            fn joined_path_with_in(
                &self,
                context: &impl SyntaxTextContext,
                separator: &str,
            ) -> String {
                let size = self
                    .iter()
                    .map(|name| name.as_str(context).len())
                    .sum::<usize>()
                    + separator.len().saturating_mul(self.len().saturating_sub(1));
                let mut result = String::with_capacity(size);
                for (index, name) in self.iter().enumerate() {
                    if index != 0 {
                        result.push_str(separator);
                    }
                    result.push_str(name.as_str(context));
                }
                result
            }
        }
    }

    pub mod text_owner {
        use arena::Handle;
        use std::sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        };

        static NEXT_SOURCE_GENERATION: AtomicU32 = AtomicU32::new(1);

        #[derive(Clone)]
        pub struct RetainedSource {
            generation: u32,
            text: Arc<str>,
        }

        pub trait SyntaxTextContext {
            fn source_text(&self, source: Handle<RetainedSource>) -> &str;
        }

        #[derive(Clone, Default)]
        pub struct SyntaxTextOwner {
            sources: Vec<RetainedSource>,
        }

        impl SyntaxTextOwner {
            pub fn retain(&mut self, text: Arc<str>) -> Handle<RetainedSource> {
                let generation = NEXT_SOURCE_GENERATION
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                        current.checked_add(1)
                    })
                    .expect("retained syntax source generation exhausted");
                self.sources.push(RetainedSource { generation, text });
                Handle::from_parts(1, generation)
            }

            pub fn retain_from(&mut self, other: &Self) {
                for source in &other.sources {
                    if let Err(position) = self
                        .sources
                        .binary_search_by_key(&source.generation, |source| source.generation)
                    {
                        self.sources.insert(position, source.clone());
                    }
                }
            }

            pub fn adopt(&mut self, other: Self) {
                for source in other.sources {
                    if let Err(position) = self
                        .sources
                        .binary_search_by_key(&source.generation, |source| source.generation)
                    {
                        self.sources.insert(position, source);
                    }
                }
            }
        }

        impl SyntaxTextContext for SyntaxTextOwner {
            #[inline]
            fn source_text(&self, source: Handle<RetainedSource>) -> &str {
                assert_eq!(
                    source.arena_index(),
                    1,
                    "invalid retained syntax source handle"
                );
                if let Some(last) = self.sources.last()
                    && last.generation == source.generation()
                {
                    return &last.text;
                }
                let position = self
                    .sources
                    .binary_search_by_key(&source.generation(), |source| source.generation)
                    .expect("retained syntax source does not belong to this forest");
                &self.sources[position].text
            }
        }

        impl std::fmt::Debug for SyntaxTextOwner {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("SyntaxTextOwner")
                    .field("source_count", &self.sources.len())
                    .finish()
            }
        }
        impl std::fmt::Debug for RetainedSource {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("RetainedSource")
                    .field("generation", &self.generation)
                    .field("bytes", &self.text.len())
                    .finish()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::{SyntaxTextContext, SyntaxTextOwner};
            use std::sync::Arc;

            #[test]
            fn adoption_keeps_independent_page_identities_and_moves_source_ownership() {
                let text: Arc<str> = Arc::from("retained");
                let mut destination = SyntaxTextOwner::default();
                let first = destination.retain(Arc::from("original"));
                let mut source = SyntaxTextOwner::default();
                let second = source.retain(text.clone());
                destination.adopt(source);
                assert_eq!(Arc::strong_count(&text), 2, "adoption moves the owner");
                assert_eq!(destination.source_text(first), "original");
                assert_eq!(destination.source_text(second), "retained");
            }

            #[test]
            #[should_panic(expected = "does not belong to this forest")]
            fn unrelated_forest_cannot_resolve_a_name_page() {
                let mut source = SyntaxTextOwner::default();
                let handle = source.retain(Arc::from("owned"));
                SyntaxTextOwner::default().source_text(handle);
            }
        }

        impl RetainedSource {
            fn read_storage<Context>(
                input: &mut compiler_storage::RecordReader<'_, Context>,
            ) -> Result<(u32, Self), compiler_storage::RecordError> {
                let original: u32 = compiler_storage::Record::read(input)?;
                if original == 0 {
                    return Err(input.error("retained syntax source has a zero generation"));
                }
                let text: Arc<str> = compiler_storage::Record::read(input)?;
                let generation = input.define_generation::<Self, _>(original, &text, || {
                    NEXT_SOURCE_GENERATION
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                            current.checked_add(1)
                        })
                        .map_err(|_| "retained syntax source generation exhausted")
                })?;
                Ok((original, Self { generation, text }))
            }
        }
        impl compiler_storage::Record for RetainedSource {
            fn write(
                &self,
                output: &mut compiler_storage::RecordWriter,
            ) -> Result<(), compiler_storage::RecordError> {
                compiler_storage::Record::write(&self.generation, output)?;
                compiler_storage::Record::write(&self.text, output)
            }
            fn read<Context>(
                input: &mut compiler_storage::RecordReader<'_, Context>,
            ) -> Result<Self, compiler_storage::RecordError> {
                Self::read_storage(input).map(|(_, source)| source)
            }
        }
        impl compiler_storage::Record for SyntaxTextOwner {
            fn write(
                &self,
                output: &mut compiler_storage::RecordWriter,
            ) -> Result<(), compiler_storage::RecordError> {
                compiler_storage::Record::write(&self.sources, output)
            }
            fn read<Context>(
                input: &mut compiler_storage::RecordReader<'_, Context>,
            ) -> Result<Self, compiler_storage::RecordError> {
                input.remap_generations::<RetainedSource>();
                let mut sources = input.record(|input| {
                    let length = input.sequence::<RetainedSource>()?;
                    let mut sources = Vec::with_capacity(length);
                    let mut previous = 0;
                    for _ in 0..length {
                        let (original, source) = RetainedSource::read_storage(input)?;
                        if original <= previous {
                            return Err(input.error(
                                "retained syntax sources are not in unique generation order",
                            ));
                        }
                        previous = original;
                        sources.push(source);
                    }
                    Ok(sources)
                })?;
                sources.sort_unstable_by_key(|source| source.generation);
                Ok(Self { sources })
            }
        }

        #[cfg(test)]
        mod storage_tests {
            use super::*;
            use compiler_storage::{RecordLimits, decode, encode};

            #[test]
            fn restored_forests_keep_shared_pages_without_colliding_with_live_names() {
                let mut first = SyntaxTextOwner::default();
                let first_name = first.retain(Arc::from("first"));
                let mut second = SyntaxTextOwner::default();
                let second_name = second.retain(Arc::from("second"));
                let mut both = first.clone();
                both.retain_from(&second);
                let bytes = encode(
                    &(second, both, first_name, second_name),
                    RecordLimits::default(),
                )
                .unwrap();
                let mut live = SyntaxTextOwner::default();
                let live_name = live.retain(Arc::from("live"));
                let (second, both, restored_first, restored_second): (
                    SyntaxTextOwner,
                    SyntaxTextOwner,
                    Handle<RetainedSource>,
                    Handle<RetainedSource>,
                ) = decode(&bytes, RecordLimits::default()).unwrap();
                assert_ne!(first_name, restored_first);
                assert_ne!(second_name, restored_second);
                assert_eq!(both.source_text(restored_first), "first");
                assert_eq!(both.source_text(restored_second), "second");
                assert_eq!(second.source_text(restored_second), "second");
                live.retain_from(&both);
                assert_eq!(live.source_text(live_name), "live");
                assert_eq!(live.source_text(restored_first), "first");
                assert_eq!(live.source_text(restored_second), "second");
            }

            #[test]
            fn duplicate_page_identity_cannot_change_its_retained_text() {
                let mut first = SyntaxTextOwner::default();
                let handle = first.retain(Arc::from("original"));
                let conflicting = SyntaxTextOwner {
                    sources: vec![RetainedSource {
                        generation: handle.generation(),
                        text: Arc::from("substituted"),
                    }],
                };
                let bytes = encode(&(first, conflicting), RecordLimits::default()).unwrap();
                assert!(
                    decode::<(SyntaxTextOwner, SyntaxTextOwner)>(&bytes, RecordLimits::default())
                        .is_err()
                );
            }
        }
    }
}
