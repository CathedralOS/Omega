//! Capture source questions, retaining exact operands without granting permission.

use std::collections::HashSet;
use std::fmt;

type Membership<Key> = HashSet<Key, arena::BuildIndexHasher>;

use crate::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding;
use crate::declaration_selection::AuthoredDeclarationSelectionTarget;
use crate::source_selections::AuthoredDeclarationSelectionOccurrenceId;
use crate::source_state::TypedTrees;
use crate::source_state::callable_values::RuntimeCallBinding;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use crate::source_state::references::StaticArgumentCall;
use crate::source_state::references::{DeclarationSelection, TypedReferences};
use crate::source_state::statement::{StatementHandle, StatementNode, StatementRootRole};
use crate::source_state::types::{ProductTypeInput, TypeOperandOrigin, TypeReferenceHandle};
use symbols::{SymbolHandle, SymbolKind};

/// Answers produced while this form reads an actual primitive operation.
/// An answer belongs to the exact expression, occurrence and source owner;
/// another use of a copied expression cannot inherit it.
#[derive(Default)]
pub(crate) struct CompletedOperators {
    reads: Vec<CompletedOperatorRead>,
    /// Disjuncts this form did not read because a static type equation
    /// decides them false for its instantiation (generics.md, "Static type
    /// equality": "For an instantiation where the equation is false, that
    /// conjunction is false and its later conjuncts are not evaluated").
    /// Their operators owe no selection.
    vacuous: Vec<(SymbolHandle, ExpressionHandle)>,
    // A shared statement needs every operand that at least one body read.
    transition_reads: Vec<(SymbolHandle, StatementHandle, TransitionRead)>,
}

/// A known-false guard has a checked predicate and no target. After a
/// statically completed arm only another closed type predicate is read.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TransitionRead {
    None,
    Guard,
    GuardAndTarget,
}

fn source_location<T>(owner: SymbolHandle, node: arena::Handle<T>) -> (u32, u32, u32, u32) {
    (
        owner.arena_index(),
        owner.generation(),
        node.arena_index(),
        node.generation(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletedOperatorRead {
    owner: SymbolHandle,
    expression: ExpressionHandle,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
}

impl CompletedOperatorRead {
    fn key(&self) -> (u32, u32, u32, u32, u64) {
        (
            self.owner.arena_index(),
            self.owner.generation(),
            self.expression.arena_index(),
            self.expression.generation(),
            self.occurrence.ordinal(),
        )
    }
}

impl CompletedOperators {
    pub(crate) fn record_builtin(
        &mut self,
        typed: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Result<(), SelectionCaptureError> {
        use crate::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
        use crate::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
        use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding as Binding;
        use crate::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
        if !typed.expression_table.expression_is_valid(expression) {
            return Err(SelectionCaptureError::new(
                "completed operator has a stale expression".into(),
            ));
        }
        for occurrence in typed
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let row = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    SelectionCaptureError::new(
                        "completed operator lost its attached occurrence".into(),
                    )
                })?;
            if row.kind() != Kind::Operator {
                continue;
            }
            if !owner.is_valid() || !typed.symbols.symbols().nodes().is_valid(owner) {
                return Err(SelectionCaptureError::new(
                    "completed operator has no exact source owner".into(),
                ));
            }
            match row.target() {
                Target::LateBound(Binding::CheckedOperator)
                | Target::Intrinsic(Intrinsic::BuiltinOperator) => {
                    self.reads.push(CompletedOperatorRead {
                        owner,
                        expression,
                        occurrence,
                    });
                }
                _ => return Err(SelectionCaptureError::new(
                    "completed primitive operation conflicts with its authored operator selection"
                        .into(),
                )),
            }
        }
        Ok(())
    }

    // This is failure evidence only. Matching another coordinate never
    // authorizes the exact source occurrence queried by `contains`.
    fn diagnostic_reads(
        &self,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> String {
        let related = self
            .reads
            .iter()
            .filter(|read| {
                read.occurrence == occurrence
                    || (read.owner == owner && read.expression == expression)
            })
            .take(4)
            .map(|read| (read.owner, read.expression, read.occurrence))
            .collect::<Vec<_>>();
        format!(
            "expected ({owner:?}, {expression:?}, {occurrence:?}); related completed reads {related:?}"
        )
    }

    /// Record a disjunct the reader skipped as vacuous. Only the reader's
    /// own static decision supplies one; capture never infers it.
    pub(crate) fn record_vacuous(&mut self, owner: SymbolHandle, expression: ExpressionHandle) {
        self.vacuous.push((owner, expression));
    }

    pub(crate) fn record_transition(
        &mut self,
        owner: SymbolHandle,
        statement: StatementHandle,
        read: TransitionRead,
    ) {
        self.transition_reads.push((owner, statement, read));
    }

    fn transition_read(&self, owner: SymbolHandle, statement: StatementHandle) -> TransitionRead {
        self.transition_reads
            .binary_search_by_key(
                &source_location(owner, statement),
                |&(owner, statement, _)| source_location(owner, statement),
            )
            .map_or(TransitionRead::GuardAndTarget, |position| {
                self.transition_reads[position].2
            })
    }

    pub(crate) fn finish(&mut self) {
        self.reads.sort_unstable_by_key(CompletedOperatorRead::key);
        self.reads.dedup();
        self.vacuous
            .sort_unstable_by_key(|&(owner, expression)| source_location(owner, expression));
        self.vacuous.dedup();
        if self
            .transition_reads
            .iter()
            .any(|&(_, _, read)| read != TransitionRead::GuardAndTarget)
        {
            self.transition_reads
                .sort_unstable_by_key(|&(owner, statement, read)| {
                    (source_location(owner, statement), std::cmp::Reverse(read))
                });
            self.transition_reads
                .dedup_by_key(|&mut (owner, statement, _)| source_location(owner, statement));
            self.transition_reads
                .retain(|&(_, _, read)| read != TransitionRead::GuardAndTarget);
        } else {
            self.transition_reads.clear();
        }
    }

    fn is_vacuous(&self, owner: SymbolHandle, expression: ExpressionHandle) -> bool {
        self.vacuous
            .binary_search_by_key(
                &source_location(owner, expression),
                |&(owner, expression)| source_location(owner, expression),
            )
            .is_ok()
    }

    fn contains(
        &self,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> bool {
        self.reads
            .binary_search_by_key(
                &CompletedOperatorRead {
                    owner,
                    expression,
                    occurrence,
                }
                .key(),
                CompletedOperatorRead::key,
            )
            .is_ok()
    }
}

pub use lowered_source_publication::selection_questions::{
    RawDependencyKind, RawDesignationInput, RawSelectionDependency, RawSelectionQuestion,
    RawSelectionQuestions, RawSelectionRole, RawSelectionTarget,
};

/// Give each anonymous declaration this form selects or names, and that emits
/// no row of its own, the invocation identity the lowered form uses for it.
pub(super) fn bind_invocation_declarations(
    mut questions: RawSelectionQuestions,
    typed: &TypedTrees,
    form: &mut super::Elaboration,
    records: &[crate::source_state::SourceRecordCorrespondence],
) -> Result<RawSelectionQuestions, &'static str> {
    if form.module.identities.invocation_scope().is_none() {
        return Ok(questions);
    }
    // These declarations already receive an identity from the row they emit.
    // A record's checked source identity can differ from a callable coordinate.
    let mut emitted = Membership::default();
    emitted.extend(form.machines.iter().map(|(_, symbol)| *symbol));
    emitted.extend(form.boundaries.iter().map(|(_, symbol)| *symbol));
    for record in records {
        emitted.insert(record.declaration);
        emitted.extend(record.fields.iter().map(|field| field.declaration));
    }
    // Publication names a called entry by its emitted machine. A generic
    // helper emits only its applications, so a call to it still selects the
    // template's own entry, which needs an identity here like any other.
    let named_by_emitted_machine = |symbol: SymbolHandle| {
        typed.symbols.get(symbol).kind == SymbolKind::State
            && typed
                .unique_machine_by_entry_state_symbol(symbol)
                .is_some_and(|(machine, _)| emitted.contains(&machine.symbol))
    };
    let selected = questions
        .questions
        .iter()
        .filter(|question| question.designation.is_none())
        .filter_map(|question| match question.target {
            RawSelectionTarget::Declaration(symbol) => Some(symbol),
            _ => None,
        })
        .chain(questions.dependencies.iter().map(|row| row.target));
    let mut named = questions.declarations.clone();
    named.extend(selected);
    named.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    named.dedup();
    let mut identities = std::mem::take(&mut form.module.identities).into_builder();
    let mut bound = Vec::new();
    for symbol in named {
        if !emitted.contains(&symbol)
            && !named_by_emitted_machine(symbol)
            && typed.symbols.symbol_source_origin(symbol)
                == Some(source_admission::SourceOrigin::User)
            && typed.symbols.symbol_package_identity(symbol).is_none()
        {
            let identity = super::declare::invocation_local(typed, symbol, &mut identities)?
                .and_then(|id| identities.text(id))
                .ok_or("anonymous selection declaration lost its invocation identity")?;
            bound.push((symbol, identity.to_owned()));
        }
    }
    form.module.identities = identities.build();
    for (symbol, identity) in bound {
        questions.bind_invocation_declaration(symbol, identity);
    }
    Ok(questions)
}

#[derive(Debug, Clone)]
pub struct SelectionCaptureError {
    detail: String,
    source_span: Option<source_text::SourceSpan>,
}
impl SelectionCaptureError {
    fn new(detail: String) -> Self {
        Self {
            detail,
            source_span: None,
        }
    }

    pub fn source_span(&self) -> Option<source_text::SourceSpan> {
        self.source_span
    }
}
impl fmt::Display for SelectionCaptureError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.detail)
    }
}
impl std::error::Error for SelectionCaptureError {}

#[derive(Clone, Copy)]
struct RuntimeSelection {
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    node: crate::source_state::SourceNode,
    binding: RuntimeCallBinding,
}

struct ExpressionScope {
    expression: ExpressionHandle,
    description: bool,
    designated: Vec<(
        AuthoredDeclarationSelectionOccurrenceId,
        RawDesignationInput,
    )>,
    runtime: Option<RuntimeSelection>,
}
struct Capture<'a> {
    typed: &'a TypedTrees,
    admitted: &'a DeclarationSelection,
    bodies: &'a DeclarationSelection,
    completed_operators: &'a CompletedOperators,
    signature_views: &'a [crate::source_state::signature::StateSignature],
    closed_signature_templates: Vec<SymbolHandle>,
    /// Template requirements whose clauses only their own check form reads
    /// (`Elaboration::generic_templates`), sorted by symbol.
    template_requirements: Vec<SymbolHandle>,
    output: RawSelectionQuestions,
    owner: SymbolHandle,
    /// The machine an applied requirement view is read for, while it is
    /// visited: its references to implementation binders belong there.
    dependency_owner: Option<SymbolHandle>,
    scopes: Vec<SymbolHandle>,
    expressions: Vec<ExpressionScope>,
    statement: Option<StatementHandle>,
    runtime_statement: Option<RuntimeSelection>,
    seen_declarations: Membership<SymbolHandle>,
    seen_signatures: Membership<SymbolHandle>,
    seen_questions: Membership<(
        SymbolHandle,
        AuthoredDeclarationSelectionOccurrenceId,
        RawSelectionRole,
        Option<RawDesignationInput>,
        Option<(u8, u32, u32)>,
    )>,
    seen_dependencies: Membership<(
        SymbolHandle,
        SymbolHandle,
        RawSelectionRole,
        RawDependencyKind,
    )>,
    seen_expressions: Membership<(SymbolHandle, ExpressionHandle, bool)>,
    seen_types: Membership<(SymbolHandle, TypeReferenceHandle)>,
    product_type_inputs: Membership<ProductTypeInput>,
    seen_facts: Membership<(
        SymbolHandle,
        arena::Handle<crate::source_state::domain::ProofFact>,
    )>,
    next_authored_symbol: Option<(SymbolHandle, RawSelectionRole)>,
    error: Option<SelectionCaptureError>,
}

/// The same live selection that supplied the normalized argument must contain
/// this type or domain occurrence. An unrelated argument, a stale lookup or a type
/// merely carrying the same name cannot authorize the rejoined reference.
fn formed_product_type_inputs(typed: &TypedTrees) -> Membership<ProductTypeInput> {
    use crate::declaration_selection::product_operands::ProductOperandConstraint;
    use crate::source_state::types::{TypeConstraintNode, TypeReferenceNode};

    let mut inputs = Membership::default();
    for handle in typed.live_product_operand_applications() {
        let selected = typed.product_operand_applications.get(handle);
        let Some(application) = &selected.argument.application else {
            continue;
        };
        let graphs = selected.operand.application.as_deref().into_iter().chain(
            selected
                .environment
                .iter()
                .map(|environment| &environment.graph),
        );
        let mut names = Membership::default();
        let mut domain_names = Membership::default();
        for graph in graphs {
            names.extend(graph.names.iter().map(|(_, name)| name.source_span));
            for (_, constraint) in graph.constraints.iter() {
                if let ProductOperandConstraint::Domain { name, .. } = constraint
                    && graph.names.is_valid(*name)
                {
                    domain_names.insert(graph.names.get(*name).source_span);
                }
            }
        }
        let mut retain = |input: &ProductTypeInput, declaration, written: &Membership<_>| {
            if input.operand == selected.operand.source_span
                && input.declaration == declaration
                && written.contains(&input.occurrence)
            {
                inputs.insert(*input);
            }
        };
        let parameters = typed
            .data_type_parameters
            .span_or_empty(selected.declaration_parameters);
        for (formal, actual) in parameters.iter().zip(application.arguments.iter()) {
            if formal.kind != crate::source_state::data::TypeParameterKind::Type
                || !actual.type_reference.is_valid()
            {
                continue;
            }
            for reference in
                crate::source_state::references::type_reference_inputs(typed, actual.type_reference)
                    .types
            {
                if let TypeOperandOrigin::ProductInput(input) =
                    typed.type_reference_table.operand_origin(reference)
                {
                    retain(&input, typed.type_reference_symbol(reference), &names);
                }
                if let TypeReferenceNode::Constrained { constraints, .. } =
                    typed.type_reference_node(reference)
                {
                    for constraint in typed.type_reference_table.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint
                            && let Some(input) = domain.product_input.as_deref()
                        {
                            retain(input, domain.symbol, &domain_names);
                        }
                    }
                }
            }
        }
    }
    inputs
}

impl Capture<'_> {
    fn describing(&self) -> bool {
        self.expressions
            .last()
            .is_some_and(|scope| scope.description)
    }

    fn description_operand(&self, expression: ExpressionHandle) -> bool {
        use crate::source_state::references::StaticArgumentRole;
        if self.describing() {
            return true;
        }
        let call = if let Some(scope) = self.expressions.last() {
            let ExpressionNode::Call(call) =
                self.typed.expression_table.expression(scope.expression)
            else {
                return false;
            };
            StaticArgumentCall::Expression(scope.expression, call)
        } else if let Some(statement) = self.statement {
            let StatementNode::Call(call) = self.typed.statement_table.statement(statement) else {
                return false;
            };
            StaticArgumentCall::Statement(call)
        } else {
            return false;
        };
        if crate::source_state::reflection::construction::declaration_query_argument_role(
            self.typed, call,
        ) != StaticArgumentRole::DeclarationDescription
        {
            return false;
        }
        match call {
            StaticArgumentCall::Expression(_, call) => self
                .typed
                .expression_table
                .expression_handles(call.arguments),
            StaticArgumentCall::Statement(call) => self
                .typed
                .statement_table
                .expression_handles(call.arguments),
        }
        .contains(&expression)
    }

    fn fail(&mut self, reason: impl Into<String>) {
        self.fail_at(reason, None);
    }

    fn fail_at(&mut self, reason: impl Into<String>, source_span: Option<source_text::SourceSpan>) {
        if self.error.is_none() {
            self.error = Some(SelectionCaptureError {
                detail: reason.into(),
                source_span,
            });
        }
    }

    fn valid_symbol(&mut self, symbol: SymbolHandle, role: &str) -> bool {
        if !symbol.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            let expression = self.expressions.last().map(|scope| scope.expression);
            let span = expression
                .map(|expression| self.typed.expression_table.source_span(expression))
                .or_else(|| {
                    self.typed
                        .symbols
                        .symbols()
                        .nodes()
                        .is_valid(self.owner)
                        .then(|| self.typed.symbols.symbol_source_span(self.owner))
                        .flatten()
                });
            let mut reason = format!(
                "source selection {role} has a missing or stale declaration coordinate {symbol:?}; \
                 lexical owner {:?}, expression {expression:?}, statement {:?}, source {span:?}",
                self.owner, self.statement,
            );
            if let Some(expression) = expression
                && let ExpressionNode::Name(path) =
                    self.typed.expression_table.expression(expression)
            {
                reason.push_str(&format!(
                    "; name {:?} in {:?}; head {:?}, target {:?}, result owner {:?}, members {:?}, static application {:?}",
                    self.typed.expression_table.display_name(expression),
                    self.typed.symbols.symbols().nodes().is_valid(self.owner)
                        .then(|| self.typed.symbols.display_path(self.owner, "::")),
                    path.head_symbol,
                    path.symbol,
                    path.reserved_result_of,
                    self.typed.expression_table.name_path_member_symbols(path.member_symbols),
                    path.static_application,
                ));
            }
            self.fail(reason);
            false
        } else {
            true
        }
    }

    fn dependency(
        &mut self,
        target: SymbolHandle,
        role: RawSelectionRole,
        kind: RawDependencyKind,
    ) {
        if !self.valid_symbol(target, "referenced declaration") {
            return;
        }
        // An applied header reads substituted types in the supplying scope.
        // Its own formals and binders still belong to the authored signature.
        let owner = if self.typed.symbols.get(target).parent == self.owner {
            self.owner
        } else {
            self.dependency_owner.unwrap_or(self.owner)
        };
        if !self.valid_symbol(owner, "dependency owner") {
            return;
        }
        let row = RawSelectionDependency {
            lexical_owner: owner,
            target,
            role,
            kind,
        };
        if self.seen_dependencies.insert((owner, target, role, kind)) {
            self.output.dependencies.push(row);
        }
    }

    fn target(&mut self, symbol: SymbolHandle) -> Option<RawSelectionTarget> {
        if !self.valid_symbol(symbol, "authored target") {
            return None;
        }
        let row = self.typed.symbols.get(symbol);
        if !self.describing()
            && matches!(
                row.kind,
                SymbolKind::Parameter
                    | SymbolKind::TypeParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
                    | SymbolKind::PropositionParameter
                    | SymbolKind::PropositionMachineParameter
                    | SymbolKind::Local
            )
        {
            if !row.parent.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(row.parent)
            {
                self.fail("local selection has no exact declaration owner");
                return None;
            }
            return Some(RawSelectionTarget::LocalBinding {
                symbol,
                owner: row.parent,
            });
        }
        Some(RawSelectionTarget::Declaration(symbol))
    }

    fn runtime_call(
        &self,
        node: crate::source_state::SourceNode,
        call: StaticArgumentCall<'_>,
    ) -> Result<Option<RuntimeSelection>, SelectionCaptureError> {
        let (binding, occurrence, conflicting) = match call {
            StaticArgumentCall::Expression(_, call) => (
                call.runtime_call,
                call.authored_call_selection,
                call.target_symbol.is_valid()
                    || call.receiver.is_valid()
                    || call.authored_call_binding.is_some()
                    || call.authored_machine_application.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.requirement_application.is_valid()
                    || call.build_selection.is_valid(),
            ),
            StaticArgumentCall::Statement(call) => (
                call.runtime_call,
                call.authored_call_selection,
                call.target_symbol.is_valid()
                    || !call.receiver.is_empty()
                    || call.receiver_root_symbol.is_valid()
                    || call.receiver_symbol.is_valid()
                    || call.authored_machine_application.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some(),
            ),
        };
        let Some(binding) = binding else {
            return Ok(None);
        };
        if conflicting {
            return Err(SelectionCaptureError::new(
                "runtime call retains conflicting nominal or static routing".into(),
            ));
        }
        let occurrence = occurrence.ok_or_else(|| {
            SelectionCaptureError::new("runtime call has no exact authored Call occurrence".into())
        })?;
        if let crate::source_state::SourceNode::Expression(expression) = node
            && !self
                .typed
                .expression_table
                .authored_selection_occurrences(expression)
                .any(|attached| attached == occurrence)
        {
            return Err(SelectionCaptureError::new(
                "runtime call does not retain its own Call occurrence attachment".into(),
            ));
        }
        let row = self
            .typed
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                SelectionCaptureError::new("runtime call lost its own authored occurrence".into())
            })?;
        if row.kind() != crate::declaration_selection::AuthoredDeclarationSelectionKind::Call
            || !matches!(
                row.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
                    | AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall
                    )
            )
        {
            return Err(SelectionCaptureError::new(
                "runtime call occurrence has no computed-call source target".into(),
            ));
        }
        if !self
            .typed
            .expression_table
            .expression_is_valid(binding.callee)
        {
            return Err(SelectionCaptureError::new(
                "runtime call has a stale evaluated callee".into(),
            ));
        }
        let signature = binding
            .signature(self.typed)
            .map_err(|reason| SelectionCaptureError::new(reason.into()))?;
        let contract = self.typed.runtime_callable_contracts.get(binding.contract);
        if !signature.symbol.is_valid()
            || !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(signature.symbol)
            || !contract.lexical_owner.is_valid()
            || !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(contract.lexical_owner)
            || self
                .typed
                .state_parameters
                .span(signature.parameters)
                .is_none()
            || self
                .typed
                .data_type_parameters
                .span(signature.type_parameters)
                .is_none()
            || self
                .typed
                .signature_contracts
                .span(signature.contracts)
                .is_none()
            || self
                .typed
                .signature_invokes
                .span(signature.invokes)
                .is_none()
        {
            return Err(SelectionCaptureError::new(
                "runtime call has no current complete signature namespace".into(),
            ));
        }
        Ok(Some(RuntimeSelection {
            occurrence,
            node,
            binding,
        }))
    }

    fn designated(
        &self,
        expression: ExpressionHandle,
    ) -> Result<
        Vec<(
            AuthoredDeclarationSelectionOccurrenceId,
            RawDesignationInput,
        )>,
        SelectionCaptureError,
    > {
        use crate::source_state::expression::BuildSelectionPhase;
        let Some(inputs) = self
            .typed
            .expression_table
            .build_selection_inputs(expression)
            .map_err(|reason| SelectionCaptureError::new(reason.into()))?
        else {
            return Ok(Vec::new());
        };
        let ExpressionNode::Call(call) = self.typed.expression_table.expression(expression) else {
            return Err(SelectionCaptureError::new(
                "Build input commitment is not on a call expression".into(),
            ));
        };
        if inputs.request != call.build_selection || inputs.lexical_owner != self.owner {
            return Err(SelectionCaptureError::new(
                "Build input commitment names another request or lexical owner".into(),
            ));
        }
        let (owner, _) = self
            .typed
            .trait_machine_signature(call.target_symbol)
            .ok_or_else(|| {
                SelectionCaptureError::new(
                    "Build selection has no exact protocol declaration".into(),
                )
            })?;
        if !owner.is_boundary
            || !crate::source_symbols::is_build_protocol(
                &self.typed.symbols,
                owner.symbol,
                language_core::build_protocol::SELECTION_PROTOCOL,
            )
        {
            return Err(SelectionCaptureError::new(
                "Build selection does not name its compiler-owned protocol".into(),
            ));
        }
        if inputs.phase == BuildSelectionPhase::Preparation {
            if !inputs.inputs.is_empty() {
                return Err(SelectionCaptureError::new(
                    "Build preparation carries commit designation inputs".into(),
                ));
            }
            return Ok(Vec::new());
        }
        let attached = self
            .typed
            .expression_table
            .authored_selection_occurrences(expression)
            .collect::<Membership<_>>();
        let mut designated = Vec::with_capacity(inputs.inputs.len());
        let mut seen = Membership::default();
        for input in &inputs.inputs {
            let row = self
                .typed
                .authored_declaration_selections()
                .get(input.occurrence)
                .ok_or_else(|| {
                    SelectionCaptureError::new(
                        "Build input commitment lost its exact authored occurrence".into(),
                    )
                })?;
            if row.target() != input.target || !attached.contains(&input.occurrence) {
                return Err(SelectionCaptureError::new("Build input commitment differs from its retained occurrence target or attachment".into()));
            }
            if let Some(resolved) = input.resolved_target {
                if !resolved.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(resolved)
                {
                    return Err(SelectionCaptureError::new(
                        "Build input resolution names a stale declaration".into(),
                    ));
                }
                match row.target() {
                    AuthoredDeclarationSelectionTarget::Resolved(selected)
                        if selected.selected_symbol() != resolved =>
                    {
                        return Err(SelectionCaptureError::new(
                            "Build input resolution conflicts with its resolved ledger target"
                                .into(),
                        ));
                    }
                    AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                        return Err(SelectionCaptureError::new(
                            "Build intrinsic input also claims a declaration resolution".into(),
                        ));
                    }
                    _ => {}
                }
            }
            if row
                .top_level_static_argument_position()
                .is_some_and(|position| u32::try_from(position) != Ok(input.argument_ordinal))
            {
                return Err(SelectionCaptureError::new(
                    "Build input commitment changed the original static argument ordinal".into(),
                ));
            }
            if !seen.insert(input.occurrence) {
                return Err(SelectionCaptureError::new(
                    "Build input commitment repeats an authored occurrence".into(),
                ));
            }
            designated.push((
                input.occurrence,
                RawDesignationInput {
                    expression,
                    request: inputs.request,
                    argument_ordinal: input.argument_ordinal,
                    resolved_target: input.resolved_target,
                },
            ));
        }
        Ok(designated)
    }
}

impl Extend<SymbolHandle> for Capture<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            if self
                .next_authored_symbol
                .is_some_and(|(target, _)| target == symbol)
            {
                // Its exact authored question already carries the target,
                // purpose and, for a designation, actual input commitment.
                self.next_authored_symbol = None;
                continue;
            }
            self.dependency(
                symbol,
                RawSelectionRole::Execution,
                if self.describing() {
                    RawDependencyKind::Description
                } else {
                    RawDependencyKind::Declaration
                },
            );
        }
    }
}

impl TypedReferences for Capture<'_> {
    fn inferred_type_references(&self) -> bool {
        false
    }
    fn machine_body(&mut self, machine: &crate::source_state::machine::Machine) -> bool {
        self.bodies.contains(machine.symbol)
    }
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        if !self.admitted.contains(symbol) {
            return false;
        }
        if !self.valid_symbol(symbol, "visited declaration") {
            return false;
        }
        self.owner = symbol;
        self.scopes.clear();
        self.expressions.clear();
        self.statement = None;
        self.runtime_statement = None;
        self.seen_declarations.insert(symbol);
        true
    }
    fn callable_signature(&mut self, signature: &crate::source_state::signature::StateSignature) {
        self.seen_signatures.insert(signature.symbol);
    }
    fn callable_signature_contents(
        &mut self,
        signature: &crate::source_state::signature::StateSignature,
    ) -> bool {
        let mut views = self
            .signature_views
            .iter()
            .filter(|view| view.symbol == signature.symbol);
        match views.next() {
            Some(first) => first == signature || views.any(|view| view == signature),
            // A closed requirement has its own symbol. Its admitted parent
            // also exposes the template, whose clauses are a separate read.
            // So are a template requirement's, which its own check form reads.
            None => {
                let key = (
                    signature.symbol.arena_index(),
                    signature.symbol.generation(),
                );
                let order = |symbol: &SymbolHandle| (symbol.arena_index(), symbol.generation());
                self.closed_signature_templates
                    .binary_search_by_key(&key, order)
                    .is_err()
                    && self
                        .template_requirements
                        .binary_search_by_key(&key, order)
                        .is_err()
            }
        }
    }
    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.scopes.push(self.owner);
        if self.valid_symbol(owner, "lexical scope owner") {
            self.owner = owner;
        }
    }
    fn leave_lexical_scope(&mut self) {
        match self.scopes.pop() {
            Some(owner) => self.owner = owner,
            None => self.fail("source selection left a lexical scope that was not entered"),
        }
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if !self.typed.expression_table.expression_is_valid(expression) {
            self.fail("source selection traversed a stale expression");
            return false;
        }
        // A declaration query names a formal; it does not read that formal's
        // argument value. Preserve the existing Core query classification on
        // its exact operand, without widening ordinary local visibility.
        let description = self.description_operand(expression);
        if self
            .seen_expressions
            .contains(&(self.owner, expression, !description))
        {
            self.fail("one source expression changes its declaration-description role");
            return false;
        }
        let key = (self.owner, expression, description);
        if !self.seen_expressions.insert(key) {
            return false;
        }
        // The reader decided this disjunct false and never read it, so its
        // operators have no source question to answer.
        if self.completed_operators.is_vacuous(self.owner, expression) {
            return false;
        }
        let designated = match self.designated(expression) {
            Ok(designated) => designated,
            Err(reason) => {
                self.fail_at(reason.detail, reason.source_span);
                return false;
            }
        };
        let runtime = match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call) => match self.runtime_call(
                crate::source_state::SourceNode::Expression(expression),
                StaticArgumentCall::Expression(expression, call),
            ) {
                Ok(runtime) => runtime,
                Err(reason) => {
                    self.fail_at(reason.detail, reason.source_span);
                    return false;
                }
            },
            _ => None,
        };
        self.expressions.push(ExpressionScope {
            expression,
            description,
            designated,
            runtime,
        });
        true
    }
    fn leave_expression(&mut self, expression: ExpressionHandle) {
        if !matches!(self.expressions.pop(), Some(scope) if scope.expression == expression) {
            self.fail("source selection expression scopes do not match");
        }
    }
    fn statement_handle(&mut self, statement: StatementHandle) {
        self.runtime_statement = None;
        if !self.typed.statement_table.statement_is_valid(statement) {
            self.fail("source selection traversed a stale statement");
            self.statement = None;
        } else {
            self.statement = Some(statement);
        }
    }
    fn statement_expression(&mut self, role: StatementRootRole) -> bool {
        let required = match role {
            StatementRootRole::TransitionGuard => TransitionRead::Guard,
            StatementRootRole::TransitionValue | StatementRootRole::TransitionArgument => {
                TransitionRead::GuardAndTarget
            }
            _ => return true,
        };
        self.statement.is_none_or(|statement| {
            self.completed_operators
                .transition_read(self.owner, statement)
                >= required
        })
    }
    fn statement(&mut self, statement: &StatementNode) -> bool {
        self.runtime_statement = None;
        let StatementNode::Call(call) = statement else {
            return true;
        };
        if call.runtime_call.is_none() {
            return true;
        }
        let Some(handle) = self.statement else {
            self.fail("runtime call has no exact statement occurrence");
            return false;
        };
        if !std::ptr::eq(statement, self.typed.statement_table.statement(handle)) {
            self.fail("runtime call statement belongs to another source occurrence");
            return false;
        }
        match self.runtime_call(
            crate::source_state::SourceNode::Statement(handle),
            StaticArgumentCall::Statement(call),
        ) {
            Ok(runtime) => {
                self.runtime_statement = runtime;
                true
            }
            Err(reason) => {
                self.fail_at(reason.detail, reason.source_span);
                false
            }
        }
    }
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.fail("source selection traversed a stale type");
            return false;
        }
        let key = (self.owner, reference);
        self.seen_types.insert(key)
    }
    fn proof_fact(&mut self, fact: arena::Handle<crate::source_state::domain::ProofFact>) -> bool {
        if !self.typed.proof_facts.is_valid(fact) {
            self.fail("source selection traversed a stale proof fact");
            return false;
        }
        let key = (self.owner, fact);
        self.seen_facts.insert(key)
    }
    fn authored_selection(&mut self, occurrence: AuthoredDeclarationSelectionOccurrenceId) {
        self.next_authored_symbol = None;
        let Some(row) = self
            .typed
            .authored_declaration_selections()
            .get(occurrence)
            .copied()
        else {
            self.fail("admitted source retains an unknown authored selection occurrence");
            return;
        };
        if !self.valid_symbol(self.owner, "authored selection owner") {
            return;
        }
        let designation = self.expressions.last().and_then(|scope| {
            scope
                .designated
                .iter()
                .find_map(|(selected, input)| (*selected == occurrence).then_some(*input))
        });
        let role = if designation.is_some() {
            RawSelectionRole::ProductDesignation
        } else {
            RawSelectionRole::Execution
        };
        let runtime = if let Some(scope) = self.expressions.last() {
            scope.runtime
        } else {
            self.runtime_statement
        }
        .filter(|runtime| runtime.occurrence == occurrence);
        let target = if let Some(runtime) = runtime {
            if designation.is_some() {
                self.fail("computed runtime call is also a Product designation");
                return;
            }
            RawSelectionTarget::RuntimeCall {
                node: runtime.node,
                binding: runtime.binding,
            }
        } else {
            match row.target() {
                AuthoredDeclarationSelectionTarget::Resolved(target) => {
                    let symbol = target.selected_symbol();
                    self.next_authored_symbol = Some((symbol, role));
                    let Some(target) = self.target(symbol) else {
                        return;
                    };
                    target
                }
                AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::CoreIntegerOperator(application),
                ) => RawSelectionTarget::CoreIntegerOperator(application),
                AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => {
                    RawSelectionTarget::Intrinsic(intrinsic)
                }
                AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                    if binding == AuthoredDeclarationSelectionLateBinding::CheckedOperator
                        && self.expressions.last().is_some_and(|scope| {
                            self.completed_operators.contains(
                                self.owner,
                                scope.expression,
                                occurrence,
                            )
                        })
                    {
                        RawSelectionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        )
                    } else {
                        let Some(input) = designation else {
                            let expression = self.expressions.last().map(|scope| {
                                self.typed.expression_table.display_name(scope.expression)
                            });
                            let completion = self.expressions.last().map(|scope| {
                                let table = &self.typed.expression_table;
                                let node = match table.expression(scope.expression) {
                                    ExpressionNode::Binary(binary) => {
                                        format!("binary {:?}", binary.operator)
                                    }
                                    node => format!("{:?}", std::mem::discriminant(node)),
                                };
                                let views = self
                                    .signature_views
                                    .iter()
                                    .filter(|view| view.symbol == self.owner)
                                    .count();
                                format!(
                                    "{}; node={node}; materialization source={:?}; admitted signature views={views}",
                                    self.completed_operators.diagnostic_reads(
                                        self.owner,
                                        scope.expression,
                                        occurrence,
                                    ),
                                    table.materialization_source(scope.expression),
                                )
                            });
                            self.fail_at(format!(
                                "admitted source selection {:?} remains unresolved as {binding:?} in `{}` at {}; operator completion: {}",
                                row.kind(),
                                self.typed.symbols.display_path(self.owner, "::"),
                                expression.as_deref().unwrap_or("a declaration"),
                                completion.as_deref().unwrap_or("no expression scope"),
                            ), Some(row.source_span()));
                            return;
                        };
                        match input.resolved_target {
                            Some(symbol) => {
                                let Some(target) = self.target(symbol) else {
                                    return;
                                };
                                target
                            }
                            // Recording an unresolved Product input is inert. Its
                            // exact request and application remain owed at Product
                            // admission; this never supplies executable code.
                            None => RawSelectionTarget::ProductInput { binding, input },
                        }
                    }
                }
            }
        };
        let runtime_key = match target {
            RawSelectionTarget::RuntimeCall {
                node: crate::source_state::SourceNode::Expression(handle),
                ..
            } => Some((0, handle.arena_index(), handle.generation())),
            RawSelectionTarget::RuntimeCall {
                node: crate::source_state::SourceNode::Statement(handle),
                ..
            } => Some((1, handle.arena_index(), handle.generation())),
            _ => None,
        };
        let question = RawSelectionQuestion {
            lexical_owner: self.owner,
            occurrence,
            role,
            target,
            designation,
        };
        if self
            .seen_questions
            .insert((self.owner, occurrence, role, designation, runtime_key))
        {
            // A folded constant has no emitted row. Retain its checked source
            // declaration so publication can authenticate the selected occurrence.
            if role == RawSelectionRole::Execution
                && let RawSelectionTarget::Declaration(symbol) = target
                && self.typed.symbols.get(symbol).kind == SymbolKind::Const
            {
                self.output.declarations.push(symbol);
            }
            self.output.questions.push(question);
        }
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        self.dependency(
            symbol,
            RawSelectionRole::Execution,
            RawDependencyKind::Invocation,
        );
    }
    fn code_selection(&mut self, symbol: SymbolHandle) {
        self.dependency(
            symbol,
            RawSelectionRole::Execution,
            RawDependencyKind::CodeSelection,
        );
    }
    fn static_application(
        &mut self,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        formation_owner: SymbolHandle,
    ) {
        if let Err(reason) = self.typed.static_record_source_receiver(application) {
            self.fail(format!("invalid retained const-record receiver: {reason}"));
        }
        // A Product use can share a header formed by Build. Retain and validate
        // that forming declaration without claiming the use selects its code.
        self.formation_owner(formation_owner);
    }
    fn product_type_input(&mut self, input: ProductTypeInput, symbol: SymbolHandle) {
        if input.declaration != symbol || !self.product_type_inputs.contains(&input) {
            self.fail("a retained Product type does not match its selected operand");
        } else {
            // The Build already issued the designation. Reading its resulting
            // type in a prepared contract does not select code in Build.
            self.formation_owner(symbol);
        }
    }
    fn formation_owner(&mut self, owner: SymbolHandle) {
        if self.valid_symbol(owner, "application formation owner") {
            self.output.declarations.push(owner);
        }
    }
    fn declaration_description_dependency(&mut self, symbol: SymbolHandle) {
        self.dependency(
            symbol,
            RawSelectionRole::Execution,
            RawDependencyKind::Description,
        );
    }
    fn layout_realization_dependency(&mut self, symbol: SymbolHandle) {
        self.dependency(
            symbol,
            RawSelectionRole::Execution,
            RawDependencyKind::Layout,
        );
    }
}

#[cfg(test)]
pub(crate) fn capture(
    typed: &TypedTrees,
    admitted: &DeclarationSelection,
    completed_operators: &CompletedOperators,
    signature_views: &[crate::source_state::signature::StateSignature],
) -> Result<RawSelectionQuestions, SelectionCaptureError> {
    let owners = signature_views
        .iter()
        .map(|view| view.symbol)
        .collect::<Vec<_>>();
    capture_with_bodies(
        typed,
        admitted,
        admitted,
        completed_operators,
        signature_views,
        &owners,
        &[],
    )
}

pub(crate) fn capture_with_bodies(
    typed: &TypedTrees,
    admitted: &DeclarationSelection,
    bodies: &DeclarationSelection,
    completed_operators: &CompletedOperators,
    signature_views: &[crate::source_state::signature::StateSignature],
    view_owners: &[SymbolHandle],
    template_requirements: &[SymbolHandle],
) -> Result<RawSelectionQuestions, SelectionCaptureError> {
    if view_owners.len() != signature_views.len() {
        return Err(SelectionCaptureError::new(
            "source selection capture lost the owner of an admitted signature view".into(),
        ));
    }
    let roots = admitted.explicit_symbols().ok_or_else(|| {
        SelectionCaptureError::new(
            "source selection capture requires the reader's explicit finite declaration roster"
                .into(),
        )
    })?;
    let body_roots = bodies.explicit_symbols().ok_or_else(|| {
        SelectionCaptureError::new(
            "source body capture requires an explicit finite body roster".into(),
        )
    })?;
    if body_roots.iter().any(|symbol| !admitted.contains(*symbol)) {
        return Err(SelectionCaptureError::new(
            "source body capture names a declaration this form did not admit".into(),
        ));
    }
    for &symbol in roots {
        if !symbol.is_valid() || !typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(SelectionCaptureError::new(
                "admitted source root has a missing or stale declaration coordinate".into(),
            ));
        }
    }
    let mut closed_signature_templates = signature_views
        .iter()
        .filter_map(|view| typed.closed_requirement_application(view.symbol))
        .map(|closed| closed.template)
        .collect::<Vec<_>>();
    closed_signature_templates
        .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    closed_signature_templates.dedup();
    let mut template_requirements = template_requirements.to_vec();
    template_requirements
        .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    template_requirements.dedup();
    let mut capture = Capture {
        typed,
        admitted,
        bodies,
        completed_operators,
        signature_views,
        closed_signature_templates,
        template_requirements,
        output: RawSelectionQuestions::for_declarations(roots.to_vec()),
        owner: SymbolHandle::invalid(),
        dependency_owner: None,
        scopes: Vec::new(),
        expressions: Vec::new(),
        statement: None,
        runtime_statement: None,
        seen_declarations: Membership::default(),
        seen_signatures: Membership::default(),
        seen_questions: Membership::default(),
        seen_dependencies: Membership::default(),
        seen_expressions: Membership::default(),
        seen_types: Membership::default(),
        product_type_inputs: formed_product_type_inputs(typed),
        seen_facts: Membership::default(),
        next_authored_symbol: None,
        error: None,
    };
    // A prepared application retains its declaration symbol but has its own
    // parameter and clause handles. Visit that actual read before considering
    // declaration-wide traversal or the standalone-signature fallback.
    for (signature, owner) in signature_views.iter().zip(view_owners) {
        if capture.declaration(signature.symbol) {
            capture.dependency_owner = (*owner != signature.symbol).then_some(*owner);
            crate::source_state::references::visit_signature(typed, &mut capture, signature);
            capture.dependency_owner = None;
        }
    }
    crate::source_state::references::visit_program(typed, &mut capture);
    // Some logical forms admit one stored signature rather than its entire
    // declaring trait. The root filter is still the reader's exact inventory.
    for &symbol in roots {
        if capture.seen_declarations.contains(&symbol) || capture.seen_signatures.contains(&symbol)
        {
            continue;
        }
        if let Some((_, signature)) = typed.trait_machine_signature(symbol) {
            if capture.declaration(symbol) {
                crate::source_state::references::visit_signature(typed, &mut capture, signature);
            }
            continue;
        }
        let mut contracts = typed
            .runtime_callable_contracts
            .iter()
            .filter(|(_, contract)| contract.signature.symbol == symbol);
        match (contracts.next(), contracts.next()) {
            (Some((_, contract)), None) => {
                if capture.declaration(symbol) {
                    crate::source_state::references::visit_signature(
                        typed,
                        &mut capture,
                        &contract.signature,
                    );
                }
            }
            _ => {
                capture.fail("admitted source root has no unique retained declaration or signature")
            }
        }
    }
    match capture.error {
        Some(error) => Err(error),
        None => {
            capture
                .output
                .declarations
                .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            capture.output.declarations.dedup();
            Ok(capture.output)
        }
    }
}

/// Source custody is a question input, never a package permission or proof.
pub fn declaration_custody(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, SelectionCaptureError> {
    lowered_source_publication::selection_questions::declaration_custody(typed, symbol)
        .map_err(|detail| SelectionCaptureError::new(detail.into()))
}

#[cfg(test)]
#[path = "selection_layout_query_tests.rs"]
mod layout_query_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    use crate::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
    use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding as LateBinding;
    use crate::source_inputs::SourceMap;
    use crate::source_state::expression::{Expression, NamePath};
    use crate::source_state::machine::Machine;
    use crate::source_state::name::Identifier;
    use crate::source_state::state::State;
    use crate::source_state::statement::StatementNode;
    use crate::source_symbols::SymbolTableBuilder;
    use source_admission::SourceOrigin;
    use source_text::{SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbols::SymbolNameRef;

    fn source() -> (
        TypedTrees,
        SymbolHandle,
        SymbolHandle,
        AuthoredDeclarationSelectionOccurrenceId,
    ) {
        let mut sources = SourceMap::default();
        let source = sources
            .add_with_metadata(
                PathBuf::from("package/questions.omg"),
                "A B entry B unknown Callback callable".into(),
                PathBuf::from("package"),
                semantic_vocabulary::PackageKeyIdentity::from_digest([0x51; 32]),
                SourceOrigin::User,
            )
            .source_id;
        let span = |start, end| SourceSpan::new(source, Span::new(start, end));
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let names = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Source(span(0, 1))),
                (SymbolKind::Machine, SymbolNameRef::Source(span(2, 3))),
                (
                    SymbolKind::CallableContract,
                    SymbolNameRef::Source(span(20, 28)),
                ),
            ],
        );
        let (a, b) = (names.handle_at(0), names.handle_at(1));
        let a_entry = builder
            .insert_children(a, [(SymbolKind::State, SymbolNameRef::Source(span(4, 9)))])
            .handle_at(0);
        builder.insert_children(
            a_entry,
            [(SymbolKind::Parameter, SymbolNameRef::Source(span(29, 37)))],
        );
        let b_entry = builder
            .insert_children(b, [(SymbolKind::State, SymbolNameRef::Source(span(4, 9)))])
            .handle_at(0);
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let selected = typed
            .record_resolved_authored_declaration_selection_once(
                span(10, 11),
                Exposure::PrivateImplementation,
                Kind::StaticArgument,
                b,
            )
            .unwrap();
        let unresolved = typed
            .record_late_bound_authored_declaration_selection(
                span(12, 19),
                Exposure::PrivateImplementation,
                Kind::MemberAccess,
                LateBinding::CheckedMember,
            )
            .unwrap();
        for (symbol, state, occurrence, target) in
            [(a, a_entry, selected, b), (b, b_entry, unresolved, b)]
        {
            let expression =
                typed
                    .expression_table
                    .insert_tree(&Expression::Name(NamePath::resolved(
                        vec![Identifier::generated("B")],
                        target,
                        target,
                    )));
            typed
                .expression_table
                .attach_authored_selection_occurrences(expression, [occurrence]);
            let statement_nodes = typed
                .statement_table
                .insert_statements([StatementNode::Expression(expression)]);
            let states = typed.machine_states.insert_many([State {
                symbol: state,
                statement_nodes,
                ..Default::default()
            }]);
            typed.push_machine(Machine {
                symbol,
                states,
                ..Default::default()
            });
        }
        (typed, a, b, selected)
    }

    #[test]
    fn a_marked_product_type_without_a_selecting_operand_is_refused() {
        use crate::source_state::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};

        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("build")),
                (SymbolKind::Data, SymbolNameRef::Static("Role")),
                (SymbolKind::Domain, SymbolNameRef::Static("Qualified")),
            ],
        );
        let owner = children.handle_at(0);
        let role = children.handle_at(1);
        let domain = children.handle_at(2);
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let carrier = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: role,
            name: Identifier::generated("Role"),
        });
        let constraints =
            typed
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    symbol: domain,
                    ..Default::default()
                })]);
        let reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: carrier,
                constraints,
            });
        typed.push_machine(Machine {
            symbol: owner,
            attached_data_application: reference,
            ..Default::default()
        });
        let admitted = DeclarationSelection::from_symbols([owner]);
        let completed = CompletedOperators::default();
        let lexical = capture(&typed, &admitted, &completed, &[]).unwrap();
        for symbol in [role, domain] {
            assert!(lexical.dependencies.iter().any(|row| row.target == symbol));
        }
        let unselected = |declaration| {
            Arc::new(ProductTypeInput {
                operand: Default::default(),
                occurrence: Default::default(),
                declaration,
            })
        };
        let assert_refused = |typed: &TypedTrees| {
            let refused = capture(typed, &admitted, &completed, &[]).unwrap_err();
            assert!(
                refused
                    .to_string()
                    .contains("does not match its selected operand"),
                "{refused}"
            );
        };
        typed
            .type_reference_table
            .set_operand_origin(carrier, TypeOperandOrigin::ProductInput(unselected(role)));
        assert_refused(&typed);
        typed
            .type_reference_table
            .set_operand_origin(carrier, TypeOperandOrigin::Lexical);
        let TypeConstraintNode::Domain(qualification) =
            &mut typed.type_reference_table.constraints_mut(constraints)[0]
        else {
            unreachable!()
        };
        qualification.product_input = Some(unselected(domain));
        assert_refused(&typed);
    }

    #[test]
    fn admitted_roots_capture_exact_owner_and_occurrence_without_unrelated_body() {
        let (typed, a, b, occurrence) = source();
        let captured = capture(
            &typed,
            &DeclarationSelection::from_symbols([a]),
            &CompletedOperators::default(),
            &[],
        )
        .unwrap();
        assert_eq!(captured.declarations, vec![a]);
        assert_eq!(
            captured.questions,
            vec![RawSelectionQuestion {
                lexical_owner: a,
                occurrence,
                role: RawSelectionRole::Execution,
                target: RawSelectionTarget::Declaration(b),
                designation: None,
            }]
        );
    }

    #[test]
    fn cast_domain_dependencies_distinguish_absent_selected_and_invalid_coordinates() {
        use crate::source_state::types::TypeReferenceNode;
        use numeric_literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
        use numerics::integer_carrier::FixedInteger;

        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain([
                    (SymbolKind::Machine, SymbolNameRef::Static("convert")),
                    (SymbolKind::Domain, SymbolNameRef::Static("SelectedDomain")),
                ]),
        );
        let owner = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let domain = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 1)
            .unwrap();
        let state = builder
            .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
            .handle_at(0);
        let mut symbols = builder.finish();
        symbols.install_integer_types([FixedInteger::U8, FixedInteger::U64]);
        let mut typed = TypedTrees {
            symbols,
            ..Default::default()
        };
        let target_symbol = typed
            .symbols
            .builtin_atom_symbol(symbols::BuiltinTypeAtom::U64)
            .unwrap();
        let target = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: target_symbol,
            name: Identifier::generated("u64"),
        });
        assert_eq!(
            typed
                .type_reference_table
                .primitive_type(target, &typed.symbols),
            Some(symbols::PrimitiveType::U64)
        );
        let domain_id = typed.semantic_domains.intern("SelectedDomain");
        typed.push_domain_definition(crate::source_state::domain::DomainDefinition {
            symbol: domain,
            name: Identifier::generated("SelectedDomain"),
            target_type: target,
            semantic_id: domain_id,
            ..Default::default()
        });
        let value = typed.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_value(42).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            }),
        ));
        let cast = typed.expression_table.insert(ExpressionNode::Cast(
            crate::source_state::expression::TableCastExpression {
                value,
                target_type: target,
                result_type: target,
                target_label: Default::default(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: Default::default(),
                semantic_domain_arguments: Default::default(),
                semantic_domain_symbol: SymbolHandle::invalid(),
                semantic_domain_id: semantic_vocabulary::SemanticDomainId::NULL,
                form: language_core::cast_form::CastForm::Value,
            },
        ));
        let statements = typed
            .statement_table
            .insert_statements([StatementNode::Expression(cast)]);
        let states = typed.machine_states.insert_many([State {
            symbol: state,
            statement_nodes: statements,
            ..Default::default()
        }]);
        typed.push_machine(Machine {
            symbol: owner,
            states,
            ..Default::default()
        });
        let admitted = DeclarationSelection::from_symbols([owner]);
        let operators = CompletedOperators::default();
        let ordinary = capture(&typed, &admitted, &operators, &[]).unwrap();
        assert!(ordinary.dependencies.iter().all(|dependency| {
            dependency.target != domain && dependency.target != SymbolHandle::invalid()
        }));

        let mut domain_name = arena::HandleSpan::empty();
        typed
            .expression_table
            .push_name_path_member(&mut domain_name, Identifier::generated("SelectedDomain"));
        let ExpressionNode::Cast(node) = typed.expression_table.expression_mut(cast) else {
            unreachable!()
        };
        node.semantic_domain = domain_name;
        node.semantic_domain_id = domain_id;
        let refusal = capture(&typed, &admitted, &operators, &[]).unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("missing or stale declaration coordinate"),
            "{refusal}"
        );

        let ExpressionNode::Cast(node) = typed.expression_table.expression_mut(cast) else {
            unreachable!()
        };
        node.semantic_domain_symbol = domain;
        let selected = capture(&typed, &admitted, &operators, &[]).unwrap();
        assert!(selected.dependencies.contains(&RawSelectionDependency {
            lexical_owner: owner,
            target: domain,
            role: RawSelectionRole::Execution,
            kind: RawDependencyKind::Declaration,
        }));

        let ExpressionNode::Cast(node) = typed.expression_table.expression_mut(cast) else {
            unreachable!()
        };
        node.semantic_domain_symbol = SymbolHandle::from_parts(u32::MAX, 1);
        let refusal = capture(&typed, &admitted, &operators, &[]).unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("missing or stale declaration coordinate"),
            "{refusal}"
        );
    }

    #[test]
    fn an_admitted_unresolved_occurrence_refuses_with_its_actual_binding_reason() {
        let (typed, a, b, _) = source();
        let refused = capture(
            &typed,
            &DeclarationSelection::from_symbols([a, b]),
            &CompletedOperators::default(),
            &[],
        )
        .unwrap_err();
        assert!(refused.to_string().contains("CheckedMember"), "{refused}");
    }

    fn cast_source(
        form: language_core::cast_form::CastForm,
    ) -> (TypedTrees, SymbolHandle, SymbolHandle, ExpressionHandle) {
        let (mut typed, owner, _, _) = source();
        let state = typed.machine_by_symbol(owner).unwrap().states.start();
        let StatementNode::Expression(value) = typed
            .statement_table
            .statement(typed.machine_states.get(state).statement_nodes.start())
        else {
            panic!("the source fixture owns one expression");
        };
        let value = *value;
        let target = typed
            .symbols
            .insert_generated_root_from(owner, SymbolKind::Data, "Target");
        let target_type = typed.type_reference_table.insert(
            crate::source_state::types::TypeReferenceNode::Named {
                symbol: target,
                name: Identifier::generated("Target"),
            },
        );
        let cast = typed.expression_table.insert(ExpressionNode::Cast(
            crate::source_state::expression::TableCastExpression {
                value,
                target_type,
                result_type: target_type,
                target_label: Default::default(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: Default::default(),
                semantic_domain_arguments: Default::default(),
                semantic_domain_symbol: SymbolHandle::invalid(),
                semantic_domain_id: semantic_vocabulary::SemanticDomainId::NULL,
                form,
            },
        ));
        let body = typed
            .statement_table
            .insert_statements([StatementNode::Expression(cast)]);
        typed.machine_states.get_mut(state).statement_nodes = body;
        (typed, owner, target, cast)
    }

    #[test]
    fn intrinsic_casts_keep_operand_selections_and_target_dependencies_without_a_domain() {
        use language_core::{
            ReferenceAccess,
            cast_form::{CastEntry, CastEntryKind, CastForm},
        };
        use numerics::arithmetic::ArithmeticDomain;
        let mut cases = vec![
            (CastForm::Value, ArithmeticDomain::Exact),
            (
                CastForm::Recast(ReferenceAccess::Shared),
                ArithmeticDomain::Exact,
            ),
        ];
        for kind in [
            CastEntryKind::Add,
            CastEntryKind::Keep,
            CastEntryKind::Remove,
        ] {
            for policy in [
                ArithmeticDomain::Wrapping,
                ArithmeticDomain::Saturating,
                ArithmeticDomain::Trapping,
            ] {
                cases.push((
                    CastForm::Entry(CastEntry {
                        kind,
                        first: true,
                        after_carrier: false,
                    }),
                    policy,
                ));
            }
        }
        for (form, policy) in cases {
            let (mut typed, owner, target, expression) = cast_source(form);
            let ExpressionNode::Cast(cast) = typed.expression_table.expression_mut(expression)
            else {
                panic!("the source fixture owns one cast");
            };
            cast.domain = policy;
            let captured = capture(
                &typed,
                &DeclarationSelection::from_symbols([owner]),
                &CompletedOperators::default(),
                &[],
            )
            .expect("an intrinsic cast has no semantic-domain declaration dependency");
            assert_eq!(
                captured.questions.len(),
                1,
                "the operand's genuine authored selection survives"
            );
            assert_eq!(captured.questions[0].lexical_owner, owner);
            assert!(captured.dependencies.iter().any(|row| row.target == target));
        }
    }

    #[test]
    fn declared_cast_domains_remain_dependencies_and_incomplete_coordinates_still_refuse() {
        use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
        let (mut typed, owner, target, expression) = cast_source(CastForm::Value);
        let domain = typed
            .symbols
            .insert_generated_root_from(owner, SymbolKind::Domain, "D");
        let ExpressionNode::Cast(cast) = typed.expression_table.expression_mut(expression) else {
            panic!("the source fixture owns one cast");
        };
        cast.semantic_domain_symbol = domain;
        let captured = capture(
            &typed,
            &DeclarationSelection::from_symbols([owner]),
            &CompletedOperators::default(),
            &[],
        )
        .expect("a declared semantic domain keeps its exact dependency");
        assert!(captured.dependencies.iter().any(|row| row.target == target));
        assert!(captured.dependencies.iter().any(|row| row.target == domain));
        for malformed in 0..7 {
            let (mut typed, owner, _, expression) = cast_source(CastForm::Value);
            let mut domain_name = arena::HandleSpan::empty();
            typed
                .expression_table
                .push_name_path_member(&mut domain_name, Identifier::generated("D"));
            let missing_type = typed.type_reference_table.insert(
                crate::source_state::types::TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated("Missing"),
                },
            );
            let arguments = typed
                .type_reference_table
                .insert_type_reference_handles([missing_type]);
            let ExpressionNode::Cast(cast) = typed.expression_table.expression_mut(expression)
            else {
                panic!("the source fixture owns one cast");
            };
            match malformed {
                0 => cast.semantic_domain = domain_name,
                1 => cast.semantic_domain_arguments = arguments,
                2 => cast.semantic_domain_id = semantic_vocabulary::SemanticDomainId(1),
                3 => {
                    cast.form = CastForm::Entry(CastEntry {
                        kind: CastEntryKind::Add,
                        first: true,
                        after_carrier: false,
                    })
                }
                4 => cast.semantic_domain_symbol = SymbolHandle::from_parts(9_999, 1),
                5 => cast.target_type = missing_type,
                _ => {
                    cast.semantic_domain = domain_name;
                    cast.domain = numerics::arithmetic::ArithmeticDomain::Wrapping;
                    cast.form = CastForm::Entry(CastEntry {
                        kind: CastEntryKind::Add,
                        first: true,
                        after_carrier: false,
                    });
                }
            }
            let refused = capture(
                &typed,
                &DeclarationSelection::from_symbols([owner]),
                &CompletedOperators::default(),
                &[],
            )
            .expect_err("a declared cast coordinate must remain exact");
            assert!(
                refused
                    .to_string()
                    .contains("missing or stale declaration coordinate"),
                "{refused}"
            );
        }
    }

    fn runtime_source(
        statement: bool,
    ) -> (
        TypedTrees,
        SymbolHandle,
        crate::source_state::SourceNode,
        RuntimeCallBinding,
    ) {
        use crate::source_state::callable_values::RuntimeCallableContract;
        use crate::source_state::expression::TableCallExpression;
        use crate::source_state::signature::{StateParameter, StateSignature};
        use crate::source_state::statement::TableCall;
        use crate::source_state::types::TypeReferenceNode;
        let (mut typed, a, _, _) = source();
        let holder = typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .find_map(|(symbol, row)| (row.kind == SymbolKind::Parameter).then_some(symbol))
            .unwrap();
        let signature_symbol = typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .find_map(|(symbol, row)| (row.kind == SymbolKind::CallableContract).then_some(symbol))
            .unwrap();
        let contract = typed
            .runtime_callable_contracts
            .insert(RuntimeCallableContract {
                signature: StateSignature {
                    symbol: signature_symbol,
                    ..Default::default()
                },
                lexical_owner: a,
                resolved_source: None,
            });
        let carrier = typed
            .type_reference_table
            .insert(TypeReferenceNode::Callable { contract });
        let parameters = typed.state_parameters.insert_many([StateParameter {
            symbol: holder,
            type_reference: carrier,
            ..Default::default()
        }]);
        let callee = typed
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("callable")],
                holder,
                holder,
            )));
        let binding = RuntimeCallBinding { callee, contract };
        let occurrence = typed
            .record_late_bound_authored_declaration_selection(
                typed.symbols.symbol_source_span(holder).unwrap(),
                Exposure::PrivateImplementation,
                Kind::Call,
                LateBinding::CheckedCall,
            )
            .unwrap();
        let (body, node) = if statement {
            let body = typed
                .statement_table
                .insert_statements([StatementNode::Call(TableCall {
                    runtime_call: Some(binding),
                    authored_call_selection: Some(occurrence),
                    ..Default::default()
                })]);
            (
                body,
                crate::source_state::SourceNode::Statement(body.handles().next().unwrap()),
            )
        } else {
            let call = typed
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    runtime_call: Some(binding),
                    authored_call_selection: Some(occurrence),
                    ..Default::default()
                }));
            typed
                .expression_table
                .attach_authored_selection_occurrences(call, [occurrence]);
            let body = typed
                .statement_table
                .insert_statements([StatementNode::Expression(call)]);
            (body, crate::source_state::SourceNode::Expression(call))
        };
        let state = typed
            .machine_by_symbol(a)
            .unwrap()
            .states
            .handles()
            .next()
            .unwrap();
        let state = typed.machine_states.get_mut(state);
        state.parameters = parameters;
        state.statement_nodes = body;
        (typed, a, node, binding)
    }

    #[test]
    fn computed_call_questions_retain_exact_runtime_binding_for_both_call_forms() {
        for statement in [false, true] {
            let (typed, a, node, binding) = runtime_source(statement);
            let captured = capture(
                &typed,
                &DeclarationSelection::from_symbols([a]),
                &CompletedOperators::default(),
                &[],
            )
            .unwrap();
            let calls = captured
                .questions
                .iter()
                .filter(|question| {
                    matches!(question.target, RawSelectionTarget::RuntimeCall { .. })
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0].target,
                RawSelectionTarget::RuntimeCall { node, binding }
            );
            assert!(
                captured
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.lexical_owner == a
                        && dependency.target
                            == match typed.expression_table.expression(binding.callee) {
                                ExpressionNode::Name(path) => path.symbol,
                                _ => unreachable!(),
                            })
            );
        }
    }

    #[test]
    fn a_computed_call_without_its_binding_or_current_contract_is_refused() {
        let (mut typed, a, node, _) = runtime_source(false);
        let crate::source_state::SourceNode::Expression(node) = node else {
            unreachable!()
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(node) else {
            unreachable!()
        };
        let binding = call.runtime_call.take().unwrap();
        let refused = capture(
            &typed,
            &DeclarationSelection::from_symbols([a]),
            &CompletedOperators::default(),
            &[],
        )
        .unwrap_err();
        assert!(refused.to_string().contains("CheckedCall"), "{refused}");
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(node) else {
            unreachable!()
        };
        call.runtime_call = Some(RuntimeCallBinding {
            contract: Default::default(),
            ..binding
        });
        let refused = capture(
            &typed,
            &DeclarationSelection::from_symbols([a]),
            &CompletedOperators::default(),
            &[],
        )
        .unwrap_err();
        assert!(
            refused.to_string().contains("current callable contract"),
            "{refused}"
        );
    }

    #[test]
    fn casts_omit_only_absent_semantic_domain_coordinates() {
        use crate::source_state::expression::TableCastExpression;
        for fault in 0..3 {
            let (mut typed, owner, _, _) = source();
            let value = typed.expression_table.insert(ExpressionNode::Boolean(true));
            let expression =
                typed
                    .expression_table
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value,
                        target_type: Default::default(),
                        result_type: Default::default(),
                        target_label: Default::default(),
                        domain: numerics::arithmetic::ArithmeticDomain::Wrapping,
                        semantic_domain: Default::default(),
                        semantic_domain_arguments: Default::default(),
                        semantic_domain_symbol: if fault == 1 {
                            SymbolHandle::from_arena_index(9999)
                        } else {
                            SymbolHandle::invalid()
                        },
                        semantic_domain_id: semantic_vocabulary::SemanticDomainId(u32::from(
                            fault == 2,
                        )),
                        form: Default::default(),
                    }));
            let state = typed
                .machine_by_symbol(owner)
                .unwrap()
                .states
                .handles()
                .next()
                .unwrap();
            typed.machine_states.get_mut(state).statement_nodes = typed
                .statement_table
                .insert_statements([StatementNode::Expression(expression)]);
            // This tests source-coordinate capture, before cast type checking.
            // Missing and stale declared-domain coordinates are not omitted.
            let result = capture(
                &typed,
                &DeclarationSelection::from_symbols([owner]),
                &CompletedOperators::default(),
                &[],
            );
            if fault == 0 {
                result.unwrap();
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("missing or stale declaration coordinate")
                );
            }
        }
    }

    #[test]
    fn completed_operator_answers_require_the_exact_owner_expression_and_occurrence() {
        use crate::source_state::expression::{BinaryOperator, TableBinaryExpression};
        let (mut typed, a, b, selected) = source();
        let span = typed
            .authored_declaration_selections()
            .get(selected)
            .unwrap()
            .source_span();
        let occurrence = typed
            .record_late_bound_authored_declaration_selection(
                span,
                Exposure::PrivateImplementation,
                Kind::Operator,
                LateBinding::CheckedOperator,
            )
            .unwrap();
        let value = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let node = ExpressionNode::Binary(TableBinaryExpression {
            left: value,
            operator: BinaryOperator::Equal,
            right: value,
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            negation: false,
        });
        let expression = typed.expression_table.insert(node.clone());
        typed
            .expression_table
            .attach_authored_selection_occurrences(expression, [occurrence]);
        let state = typed
            .machine_by_symbol(a)
            .unwrap()
            .states
            .handles()
            .next()
            .unwrap();
        let body = typed
            .statement_table
            .insert_statements([StatementNode::Expression(expression)]);
        typed.machine_states.get_mut(state).statement_nodes = body;
        let admitted = DeclarationSelection::from_symbols([a]);
        let mut answers = CompletedOperators::default();
        let refused = capture(&typed, &admitted, &answers, &[]).unwrap_err();
        assert!(refused.to_string().contains("CheckedOperator"));
        assert_eq!(refused.source_span(), Some(span));
        answers.record_builtin(&typed, b, expression).unwrap();
        answers.finish();
        assert!(
            capture(&typed, &admitted, &answers, &[])
                .unwrap_err()
                .to_string()
                .contains("CheckedOperator")
        );
        answers.record_builtin(&typed, a, expression).unwrap();
        answers.finish();
        let captured = capture(&typed, &admitted, &answers, &[]).unwrap();
        assert!(
            captured
                .questions
                .iter()
                .any(|question| question.occurrence == occurrence
                    && question.lexical_owner == a
                    && question.target
                        == RawSelectionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ))
        );

        // A copied expression retains the source occurrence, but has not been read.
        let copied = typed.expression_table.insert(node);
        typed
            .expression_table
            .attach_authored_selection_occurrences(copied, [occurrence]);
        let copied_body = typed
            .statement_table
            .insert_statements([StatementNode::Expression(copied)]);
        typed.machine_states.get_mut(state).statement_nodes = copied_body;
        assert!(
            capture(&typed, &admitted, &answers, &[])
                .unwrap_err()
                .to_string()
                .contains("CheckedOperator")
        );
        answers.record_builtin(&typed, a, copied).unwrap();
        answers.finish();
        capture(&typed, &admitted, &answers, &[]).unwrap();

        // Neither a new occurrence nor an explicit declaration choice inherits it.
        let other = typed
            .record_late_bound_authored_declaration_selection(
                span,
                Exposure::PrivateImplementation,
                Kind::Operator,
                LateBinding::CheckedOperator,
            )
            .unwrap();
        assert!(!answers.contains(a, copied, other));
        typed
            .expression_table
            .attach_authored_selection_occurrences(copied, [other]);
        assert!(
            capture(&typed, &admitted, &answers, &[])
                .unwrap_err()
                .to_string()
                .contains("CheckedOperator")
        );
        let mut conflicting = typed.clone();
        conflicting
            .finalize_authored_declaration_selection(occurrence, LateBinding::CheckedOperator, b)
            .unwrap();
        assert!(
            answers
                .record_builtin(&conflicting, a, expression)
                .unwrap_err()
                .to_string()
                .contains("conflicts")
        );
        typed.expression_table.clear();
        typed.expression_table.insert(ExpressionNode::Boolean(true));
        assert!(
            answers
                .record_builtin(&typed, a, expression)
                .unwrap_err()
                .to_string()
                .contains("stale expression")
        );
    }
}
