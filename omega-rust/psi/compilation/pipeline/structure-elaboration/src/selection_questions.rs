//! Capture source questions, retaining exact operands without granting permission.

use std::collections::HashSet;
use std::fmt;

type Membership<Key> = HashSet<Key, symbols::BuildSymbolHasher>;

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::callable_values::RuntimeCallBinding;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::StaticArgumentCall;
use typed_trees::references::{DeclarationSelection, TypedReferences};
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawSelectionRole {
    Execution,
    ProductDesignation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSelectionTarget {
    Declaration(SymbolHandle),
    LocalBinding {
        symbol: SymbolHandle,
        owner: SymbolHandle,
    },
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    RuntimeCall {
        node: typed_trees::SourceNode,
        binding: RuntimeCallBinding,
    },
    ProductInput {
        binding: AuthoredDeclarationSelectionLateBinding,
        input: RawDesignationInput,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSelectionQuestion {
    pub lexical_owner: SymbolHandle,
    pub occurrence: AuthoredDeclarationSelectionOccurrenceId,
    pub role: RawSelectionRole,
    pub target: RawSelectionTarget,
    pub designation: Option<RawDesignationInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawDesignationInput {
    pub expression: ExpressionHandle,
    pub request: arena::Handle<typed_trees::expression::BuildSelectionRequest>,
    pub argument_ordinal: u32,
    pub resolved_target: Option<SymbolHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawDependencyKind {
    Declaration,
    Invocation,
    CodeSelection,
    Description,
    Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSelectionDependency {
    pub lexical_owner: SymbolHandle,
    pub target: SymbolHandle,
    pub role: RawSelectionRole,
    pub kind: RawDependencyKind,
}

#[derive(Debug, Clone, Default)]
pub struct RawSelectionQuestions {
    pub declarations: Vec<SymbolHandle>,
    pub questions: Vec<RawSelectionQuestion>,
    pub dependencies: Vec<RawSelectionDependency>,
}

#[derive(Debug, Clone)]
pub struct SelectionCaptureError(String);
impl fmt::Display for SelectionCaptureError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.0)
    }
}
impl std::error::Error for SelectionCaptureError {}

#[derive(Clone, Copy)]
struct RuntimeSelection {
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    node: typed_trees::SourceNode,
    binding: RuntimeCallBinding,
}

struct ExpressionScope {
    expression: ExpressionHandle,
    designated: Vec<(
        AuthoredDeclarationSelectionOccurrenceId,
        RawDesignationInput,
    )>,
    runtime: Option<RuntimeSelection>,
}
struct Capture<'a> {
    typed: &'a TypedTrees,
    admitted: &'a DeclarationSelection,
    output: RawSelectionQuestions,
    owner: SymbolHandle,
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
    seen_expressions: Membership<(SymbolHandle, ExpressionHandle)>,
    seen_types: Membership<(SymbolHandle, TypeReferenceHandle)>,
    seen_facts: Membership<(SymbolHandle, arena::Handle<typed_trees::domain::ProofFact>)>,
    next_authored_symbol: Option<(SymbolHandle, RawSelectionRole)>,
    error: Option<SelectionCaptureError>,
}

impl Capture<'_> {
    fn fail(&mut self, reason: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(SelectionCaptureError(reason.into()));
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
            self.fail(format!(
                "source selection {role} has a missing or stale declaration coordinate {symbol:?}; \
                 lexical owner {:?}, expression {expression:?}, statement {:?}, source {span:?}",
                self.owner, self.statement,
            ));
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
        if !self.valid_symbol(self.owner, "dependency owner")
            || !self.valid_symbol(target, "referenced declaration")
        {
            return;
        }
        let row = RawSelectionDependency {
            lexical_owner: self.owner,
            target,
            role,
            kind,
        };
        if self
            .seen_dependencies
            .insert((self.owner, target, role, kind))
        {
            self.output.dependencies.push(row);
        }
    }

    fn target(&mut self, symbol: SymbolHandle) -> Option<RawSelectionTarget> {
        if !self.valid_symbol(symbol, "authored target") {
            return None;
        }
        let row = self.typed.symbols.get(symbol);
        if matches!(
            row.kind,
            SymbolKind::Parameter
                | SymbolKind::TypeParameter
                | SymbolKind::MachineParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::PropositionParameter
                | SymbolKind::PropositionMachineParameter
                | SymbolKind::Local
        ) {
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
        node: typed_trees::SourceNode,
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
            return Err(SelectionCaptureError(
                "runtime call retains conflicting nominal or static routing".into(),
            ));
        }
        let occurrence = occurrence.ok_or_else(|| {
            SelectionCaptureError("runtime call has no exact authored Call occurrence".into())
        })?;
        if let typed_trees::SourceNode::Expression(expression) = node
            && !self
                .typed
                .expression_table
                .authored_selection_occurrences(expression)
                .any(|attached| attached == occurrence)
        {
            return Err(SelectionCaptureError(
                "runtime call does not retain its own Call occurrence attachment".into(),
            ));
        }
        let row = self
            .typed
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                SelectionCaptureError("runtime call lost its own authored occurrence".into())
            })?;
        if row.kind()
            != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
            || !matches!(
                row.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
                    | AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall
                    )
            )
        {
            return Err(SelectionCaptureError(
                "runtime call occurrence has no computed-call source target".into(),
            ));
        }
        if !self
            .typed
            .expression_table
            .expression_is_valid(binding.callee)
        {
            return Err(SelectionCaptureError(
                "runtime call has a stale evaluated callee".into(),
            ));
        }
        let signature = binding
            .signature(self.typed)
            .map_err(|reason| SelectionCaptureError(reason.into()))?;
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
            return Err(SelectionCaptureError(
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
        use typed_trees::expression::BuildSelectionPhase;
        let Some(inputs) = self
            .typed
            .expression_table
            .build_selection_inputs(expression)
            .map_err(|reason| SelectionCaptureError(reason.into()))?
        else {
            return Ok(Vec::new());
        };
        let ExpressionNode::Call(call) = self.typed.expression_table.expression(expression) else {
            return Err(SelectionCaptureError(
                "Build input commitment is not on a call expression".into(),
            ));
        };
        if inputs.request != call.build_selection || inputs.lexical_owner != self.owner {
            return Err(SelectionCaptureError(
                "Build input commitment names another request or lexical owner".into(),
            ));
        }
        let (owner, _) = self
            .typed
            .trait_machine_signature(call.target_symbol)
            .ok_or_else(|| {
                SelectionCaptureError("Build selection has no exact protocol declaration".into())
            })?;
        if !owner.is_boundary
            || !language_semantics::build_requests::is_build_protocol(
                &self.typed.symbols,
                owner.symbol,
                language_semantics::build_requests::SELECTION_PROTOCOL,
            )
        {
            return Err(SelectionCaptureError(
                "Build selection does not name its compiler-owned protocol".into(),
            ));
        }
        if inputs.phase == BuildSelectionPhase::Preparation {
            if !inputs.inputs.is_empty() {
                return Err(SelectionCaptureError(
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
                    SelectionCaptureError(
                        "Build input commitment lost its exact authored occurrence".into(),
                    )
                })?;
            if row.target() != input.target || !attached.contains(&input.occurrence) {
                return Err(SelectionCaptureError("Build input commitment differs from its retained occurrence target or attachment".into()));
            }
            if let Some(resolved) = input.resolved_target {
                if !resolved.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(resolved)
                {
                    return Err(SelectionCaptureError(
                        "Build input resolution names a stale declaration".into(),
                    ));
                }
                match row.target() {
                    AuthoredDeclarationSelectionTarget::Resolved(selected)
                        if selected.selected_symbol() != resolved =>
                    {
                        return Err(SelectionCaptureError(
                            "Build input resolution conflicts with its resolved ledger target"
                                .into(),
                        ));
                    }
                    AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                        return Err(SelectionCaptureError(
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
                return Err(SelectionCaptureError(
                    "Build input commitment changed the original static argument ordinal".into(),
                ));
            }
            if !seen.insert(input.occurrence) {
                return Err(SelectionCaptureError(
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
                RawDependencyKind::Declaration,
            );
        }
    }
}

impl TypedReferences for Capture<'_> {
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
    fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
        self.seen_signatures.insert(signature.symbol);
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
        let key = (self.owner, expression);
        if !self.seen_expressions.insert(key) {
            return false;
        }
        let designated = match self.designated(expression) {
            Ok(designated) => designated,
            Err(reason) => {
                self.fail(reason.0);
                return false;
            }
        };
        let runtime = match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call) => match self.runtime_call(
                typed_trees::SourceNode::Expression(expression),
                StaticArgumentCall::Expression(expression, call),
            ) {
                Ok(runtime) => runtime,
                Err(reason) => {
                    self.fail(reason.0);
                    return false;
                }
            },
            _ => None,
        };
        self.expressions.push(ExpressionScope {
            expression,
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
            typed_trees::SourceNode::Statement(handle),
            StaticArgumentCall::Statement(call),
        ) {
            Ok(runtime) => {
                self.runtime_statement = runtime;
                true
            }
            Err(reason) => {
                self.fail(reason.0);
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
    fn proof_fact(&mut self, fact: arena::Handle<typed_trees::domain::ProofFact>) -> bool {
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
                AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => {
                    RawSelectionTarget::Intrinsic(intrinsic)
                }
                AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                    let Some(input) = designation else {
                        self.fail(format!(
                            "admitted source selection {:?} remains unresolved as {binding:?}",
                            row.kind()
                        ));
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
        };
        let runtime_key = match target {
            RawSelectionTarget::RuntimeCall {
                node: typed_trees::SourceNode::Expression(handle),
                ..
            } => Some((0, handle.arena_index(), handle.generation())),
            RawSelectionTarget::RuntimeCall {
                node: typed_trees::SourceNode::Statement(handle),
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

pub fn capture(
    typed: &TypedTrees,
    admitted: &DeclarationSelection,
) -> Result<RawSelectionQuestions, SelectionCaptureError> {
    let roots = admitted.explicit_symbols().ok_or_else(|| {
        SelectionCaptureError(
            "source selection capture requires the reader's explicit finite declaration roster"
                .into(),
        )
    })?;
    for &symbol in roots {
        if !symbol.is_valid() || !typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(SelectionCaptureError(
                "admitted source root has a missing or stale declaration coordinate".into(),
            ));
        }
    }
    let mut capture = Capture {
        typed,
        admitted,
        output: RawSelectionQuestions {
            declarations: roots.to_vec(),
            ..RawSelectionQuestions::default()
        },
        owner: SymbolHandle::invalid(),
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
        seen_facts: Membership::default(),
        next_authored_symbol: None,
        error: None,
    };
    typed_trees::references::visit_program(typed, &mut capture);
    // Some logical forms admit one stored signature rather than its entire
    // declaring trait. The root filter is still the reader's exact inventory.
    for &symbol in roots {
        if capture.seen_declarations.contains(&symbol) || capture.seen_signatures.contains(&symbol)
        {
            continue;
        }
        if let Some((_, signature)) = typed.trait_machine_signature(symbol) {
            if capture.declaration(symbol) {
                typed_trees::references::visit_signature(typed, &mut capture, signature);
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
                    typed_trees::references::visit_signature(
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
        None => Ok(capture.output),
    }
}

/// Source custody is a question input, never a package permission or proof.
pub fn declaration_custody(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, SelectionCaptureError> {
    use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
    if !symbol.is_valid() || !typed.symbols.symbols().nodes().is_valid(symbol) {
        return Err(SelectionCaptureError(
            "declaration custody has a missing or stale symbol".into(),
        ));
    }
    Ok(
        match (
            typed.symbols.symbol_package_identity(symbol),
            typed.symbols.symbol_source_origin(symbol),
        ) {
            (Some(package), _) => DeclarationCustody::Package(package),
            (_, Some(source::SourceOrigin::Toolchain)) => DeclarationCustody::Toolchain,
            (_, Some(source::SourceOrigin::ToolchainContract)) => {
                DeclarationCustody::ToolchainContract
            }
            (_, Some(source::SourceOrigin::User)) => DeclarationCustody::UnownedUser,
            _ => DeclarationCustody::Missing,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
    };
    use source::{SourceMap, SourceOrigin, SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbols::{SymbolNameRef, SymbolTableBuilder};
    use typed_trees::expression::{Expression, NamePath};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;

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
    fn admitted_roots_capture_exact_owner_and_occurrence_without_unrelated_body() {
        let (typed, a, b, occurrence) = source();
        let captured = capture(&typed, &DeclarationSelection::from_symbols([a])).unwrap();
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
    fn an_admitted_unresolved_occurrence_refuses_with_its_actual_binding_reason() {
        let (typed, a, b, _) = source();
        let refused = capture(&typed, &DeclarationSelection::from_symbols([a, b])).unwrap_err();
        assert!(refused.to_string().contains("CheckedMember"), "{refused}");
    }

    fn runtime_source(
        statement: bool,
    ) -> (
        TypedTrees,
        SymbolHandle,
        typed_trees::SourceNode,
        RuntimeCallBinding,
    ) {
        use typed_trees::callable_values::RuntimeCallableContract;
        use typed_trees::expression::TableCallExpression;
        use typed_trees::signature::{StateParameter, StateSignature};
        use typed_trees::statement::TableCall;
        use typed_trees::types::TypeReferenceNode;
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
                typed_trees::SourceNode::Statement(body.handles().next().unwrap()),
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
            (body, typed_trees::SourceNode::Expression(call))
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
            let captured = capture(&typed, &DeclarationSelection::from_symbols([a])).unwrap();
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
        let typed_trees::SourceNode::Expression(node) = node else {
            unreachable!()
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(node) else {
            unreachable!()
        };
        let binding = call.runtime_call.take().unwrap();
        let refused = capture(&typed, &DeclarationSelection::from_symbols([a])).unwrap_err();
        assert!(refused.to_string().contains("CheckedCall"), "{refused}");
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(node) else {
            unreachable!()
        };
        call.runtime_call = Some(RuntimeCallBinding {
            contract: Default::default(),
            ..binding
        });
        let refused = capture(&typed, &DeclarationSelection::from_symbols([a])).unwrap_err();
        assert!(
            refused.to_string().contains("current callable contract"),
            "{refused}"
        );
    }
}
