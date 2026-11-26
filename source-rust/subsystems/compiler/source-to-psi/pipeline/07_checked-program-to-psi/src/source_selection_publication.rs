//! Normalize the exact selections of every accepted source form. The output
//! is a question packet; only receiving execution admission grants permission.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::source_state::TypedTrees;
use crate::source_state::callable_values::RuntimeCallableContractHandle;
use crate::source_state::references::StaticArgumentRole;
use application_identity::type_identity::encoding::callable_declaration_path;
use diagnostics::Diagnostic;
use lowered_source_publication::selection_questions::{
    RawDependencyKind, RawDesignationInput, RawSelectionQuestion, RawSelectionQuestions,
    RawSelectionRole, RawSelectionTarget,
};
use numerics::intrinsics::CoreIntegerOperatorSignature;
use record_storage::Record;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use semantic_vocabulary::source_selections::{
    BuildCallSite, SelectionBindingKind, SelectionDeclaration, SelectionForm, SelectionJoin,
    SelectionNode, SelectionOccurrence, SelectionProductDesignation, SelectionPurpose,
    SelectionQuestion, SelectionQuestions, SelectionReference, SelectionRole,
};
use semantic_vocabulary::{
    CallableInvocationSite, ContractId, InvocationSite, MachineId, SelectionDeclarationId,
    SelectionQuestionId, StructuralTypeId, TargetMachineCallable,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_psi::{OperationKind, TerminalModule};

use crate::AcceptedTerminalSource;

// The source arena bounds the slot table. Each occupied row retains the full
// handle so a reused arena slot cannot name an earlier declaration generation.
struct SymbolRows<Value> {
    positions: Vec<usize>,
    rows: Vec<(SymbolHandle, Value)>,
}

impl<Value> SymbolRows<Value> {
    fn new(symbols: usize) -> Self {
        Self {
            positions: vec![0; symbols + 1],
            rows: Vec::new(),
        }
    }

    fn get(&self, symbol: &SymbolHandle) -> Option<&Value> {
        let position = *self.positions.get(symbol.arena_index() as usize)?;
        let (retained, value) = self.rows.get(position.checked_sub(1)?)?;
        (retained == symbol).then_some(value)
    }

    fn insert(&mut self, symbol: SymbolHandle, value: Value) -> Result<(), Diagnostic> {
        let position = self
            .positions
            .get_mut(symbol.arena_index() as usize)
            .filter(|_| symbol.is_valid())
            .ok_or_else(|| refused("source declaration is outside its symbol arena"))?;
        if *position == 0 {
            self.rows.push((symbol, value));
            *position = self.rows.len();
        } else {
            let (retained, row) = &mut self.rows[*position - 1];
            if *retained != symbol {
                return Err(refused("source declaration changed its symbol generation"));
            }
            *row = value;
        }
        Ok(())
    }

    fn get_or_insert_with(
        &mut self,
        symbol: SymbolHandle,
        make: impl FnOnce() -> Value,
    ) -> Result<&mut Value, Diagnostic> {
        if self.get(&symbol).is_none() {
            self.insert(symbol, make())?;
        }
        let position = self.positions[symbol.arena_index() as usize];
        Ok(&mut self.rows[position - 1].1)
    }
}

fn refused(detail: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!("source declaration-selection admission: {detail}"))
}

fn selection_context(mut diagnostic: Diagnostic, context: impl std::fmt::Display) -> Diagnostic {
    diagnostic.message = format!("{}; {context}", diagnostic.message);
    diagnostic
}

pub(crate) fn prepare(
    source: &AcceptedTerminalSource,
    module: &TerminalModule,
    consent: Option<&dyn crate::source_inputs::InvocationSourceConsent>,
) -> Result<PreparedSelections, Diagnostic> {
    let mut prepared = Questions::new(source, module)?;
    prepared.retain_build_sites()?;
    for (ordinal, form) in source.selection_forms().enumerate() {
        let raw = form.as_ref().ok_or_else(|| {
            refused("an accepted source form has no declaration-selection correspondence")
        })?;
        let form = if ordinal == 0 {
            SelectionForm::Published
        } else {
            SelectionForm::Semantic {
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| refused("accepted form ordinal exceeds its namespace"))?,
            }
        };
        prepared
            .retain_form(form, raw)
            .map_err(|error| selection_context(error, format_args!("accepted form {form:?}")))?;
    }
    prepared
        .retain_physical_joins()
        .map_err(|error| selection_context(error, "joining emitted declarations to source"))?;
    let invocation_consent = prepared.invocation_consent(consent)?;
    let captured_entry = module.entry.filter(|&entry| {
        let TargetMachineCallable::Machine(id) = entry else {
            return false;
        };
        let Some(consent) = consent else { return false };
        let symbols = &source.source().symbols;
        let Some(span) = source
            .closure()
            .elaboration()
            .machines
            .get(id)
            .and_then(|symbol| symbols.symbol_provenance_source_span(*symbol))
        else {
            return false;
        };
        symbols.source_map().is_some_and(|map| {
            consent.allows_selection(&map, span, span, source_provenance::DependencyScope::Build)
        })
    });
    prepared.output.dependency_calls = source.dependency_calls();
    Ok(PreparedSelections {
        captured_entry,
        questions: prepared.output,
        invocation_consent,
    })
}

pub(crate) struct PreparedSelections {
    pub(crate) captured_entry: Option<TargetMachineCallable>,
    pub(crate) questions: SelectionQuestions,
    pub(crate) invocation_consent: Vec<SelectionQuestionId>,
}

struct CleanupSelectionBinding {
    carrier: StructuralTypeId,
    author: SelectionDeclarationId,
    question: SelectionQuestionId,
    target: MachineId,
}

struct Questions<'source> {
    source: &'source AcceptedTerminalSource,
    module: &'source TerminalModule,
    declarations: Declarations<'source>,
    exact_toolchain_sources:
        std::cell::OnceCell<Result<Vec<(source_text::SourceId, [u8; 32])>, String>>,
    output: SelectionQuestions,
    build_sites: HashMap<
        (
            SymbolHandle,
            crate::source_state::expression::ExpressionHandle,
        ),
        BuildCallSite,
        arena::BuildIndexHasher,
    >,
    requirements: HashMap<
        (MachineId, ContractId, MachineId),
        &'source terminal_check_tables::program::Selected,
        arena::BuildIndexHasher,
    >,
}

impl<'source> Questions<'source> {
    fn new(
        source: &'source AcceptedTerminalSource,
        module: &'source TerminalModule,
    ) -> Result<Self, Diagnostic> {
        let mut requirements = HashMap::default();
        for selection in &source.closure().elaboration().selections {
            let Some(required) = selection.required_contract else {
                continue;
            };
            let key = (selection.caller, required, selection.implementation);
            if let Some(previous) = requirements.insert(key, selection)
                && previous != selection
            {
                return Err(refused(
                    "requirement call has competing retained selections",
                ));
            }
        }
        Ok(Self {
            source,
            module,
            declarations: Declarations::new(source, module)?,
            exact_toolchain_sources: Default::default(),
            output: SelectionQuestions::default(),
            build_sites: HashMap::default(),
            requirements,
        })
    }

    fn invocation_consent(
        &self,
        consent: Option<&dyn crate::source_inputs::InvocationSourceConsent>,
    ) -> Result<Vec<SelectionQuestionId>, Diagnostic> {
        let Some(consent) = consent else {
            return Ok(Vec::new());
        };
        if self.module.identities.invocation_scope().is_none() {
            return Err(refused("captured source consent requires an invocation"));
        }
        let sources = self
            .source
            .source()
            .symbols
            .source_map()
            .ok_or_else(|| refused("captured source consent lost its actual source map"))?;
        let mut questions = Vec::new();
        for (id, row) in self.output.questions.iter() {
            let requester = self
                .output
                .declarations
                .get(row.occurrence.lexical_owner)
                .ok_or_else(|| refused("captured selection lost its lexical declaration"))?;
            if requester.custody != DeclarationCustody::UnownedUser
                || requester.purpose != SelectionPurpose::Build
            {
                continue;
            }
            let requester_span = self
                .declarations
                .source_span(row.occurrence.lexical_owner)?;
            let selected_span = match &row.selected {
                SelectionReference::Declaration(selected)
                    if row.purpose == SelectionPurpose::Build =>
                {
                    let selected_row =
                        self.output.declarations.get(*selected).ok_or_else(|| {
                            refused("captured selection lost its selected declaration")
                        })?;
                    match selected_row.custody {
                        DeclarationCustody::UnownedUser
                            if selected_row.purpose == SelectionPurpose::Build =>
                        {
                            self.declarations.source_span(*selected)?
                        }
                        DeclarationCustody::Package(_)
                            if selected_row.purpose == SelectionPurpose::Product =>
                        {
                            let selected_span = self.declarations.source_span(*selected)?;
                            if consent.allows_supplied_library(
                                &sources,
                                requester_span,
                                selected_span,
                            ) {
                                questions.push(id);
                            }
                            continue;
                        }
                        DeclarationCustody::Toolchain => requester_span,
                        _ => continue,
                    }
                }
                SelectionReference::Intrinsic { .. }
                | SelectionReference::LocalBinding { .. }
                | SelectionReference::RuntimeContract { .. }
                    if row.purpose == SelectionPurpose::Build =>
                {
                    requester_span
                }
                SelectionReference::ProductInput { .. }
                    if row.purpose == SelectionPurpose::Product
                        && matches!(row.role, SelectionRole::ProductDesignation(_)) =>
                {
                    // Retain the Build author's inert request. No Product
                    // declaration or package edge is admitted by this consent.
                    requester_span
                }
                _ => continue,
            };
            if consent.allows_selection(
                &sources,
                requester_span,
                selected_span,
                source_provenance::DependencyScope::Build,
            ) {
                questions.push(id);
            }
        }
        Ok(questions)
    }

    fn typed_origin(
        &self,
        machine: MachineId,
        site: InvocationSite,
    ) -> Result<crate::source_state::SourceCallCorrespondence, Diagnostic> {
        let mut calls = self
            .source
            .correspondence()
            .calls
            .iter()
            .filter(|call| call.machine == machine && call.site == site);
        let call = calls.next().ok_or_else(|| {
            let declaration = self
                .source
                .closure()
                .elaboration()
                .machines
                .get(machine)
                .map(|symbol| self.source.source().symbols.display_path(*symbol, "::"));
            refused(format_args!(
                "source invocation lost its typed occurrence or original lexical owner: machine {machine:?}, site {site:?}, declaration {declaration:?}"
            ))
        })?;
        if calls.next().is_some() {
            return Err(refused(
                "source invocation has competing authored correspondences",
            ));
        }
        Ok(*call)
    }

    fn source_target(&self, target: TargetMachineCallable) -> Result<SymbolHandle, Diagnostic> {
        let elaboration = &self.source.closure().elaboration();
        match target {
            TargetMachineCallable::Machine(machine) => elaboration.machines.get(machine),
            TargetMachineCallable::Boundary(boundary) => elaboration.boundaries.get(boundary),
        }
        .copied()
        .ok_or_else(|| refused("invoked declaration has no exact source correspondence"))
    }

    fn node(node: crate::source_state::SourceNode) -> SelectionNode {
        match node {
            crate::source_state::SourceNode::Expression(expression) => SelectionNode::Expression {
                index: expression.arena_index(),
                generation: expression.generation(),
            },
            crate::source_state::SourceNode::Statement(statement) => SelectionNode::Statement {
                index: statement.arena_index(),
                generation: statement.generation(),
            },
        }
    }

    fn execution_question(
        &mut self,
        owner: SymbolHandle,
        node: crate::source_state::SourceNode,
        selected: SelectionReference,
        role: SelectionRole,
    ) -> Result<SelectionQuestionId, Diagnostic> {
        let lexical_owner = self.declarations.declaration(&mut self.output, owner)?;
        Ok(self.output.questions.push(SelectionQuestion {
            form: SelectionForm::Published,
            occurrence: SelectionOccurrence {
                lexical_owner,
                node: Self::node(node),
            },
            selected,
            purpose: self.declarations.purpose(owner)?,
            role,
        }))
    }

    fn invocation_reference(
        &mut self,
        owner: SymbolHandle,
        node: crate::source_state::SourceNode,
        target: semantic_vocabulary::source_selections::SelectionCallTarget,
    ) -> Result<SelectionReference, Diagnostic> {
        let typed = self.source.source();
        let (runtime, parameter, dispatch, source_target) = match node {
            crate::source_state::SourceNode::Expression(expression) => {
                if !typed.expression_table.expression_is_valid(expression) {
                    return Err(refused("invocation source expression is stale"));
                }
                let crate::source_state::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(expression)
                else {
                    return Err(refused("emitted invocation source is no call expression"));
                };
                (
                    call.runtime_call,
                    call.static_machine_parameter,
                    call.static_requirement_dispatch.as_ref(),
                    call.target_symbol,
                )
            }
            crate::source_state::SourceNode::Statement(statement) => {
                if !typed.statement_table.statement_is_valid(statement) {
                    return Err(refused("invocation source statement is stale"));
                }
                match typed.statement_table.statement(statement) {
                    crate::source_state::statement::StatementNode::Call(call) => (
                        call.runtime_call,
                        call.static_machine_parameter,
                        call.static_requirement_dispatch.as_ref(),
                        call.target_symbol,
                    ),
                    crate::source_state::statement::StatementNode::Transition(transition) => {
                        if !typed
                            .statement_table
                            .transition_target_is_valid(transition.target)
                        {
                            return Err(refused("invocation source transition target is stale"));
                        }
                        let crate::source_state::statement::TransitionTargetNode::Named {
                            path,
                            static_machine_parameter,
                            ..
                        } = typed.statement_table.transition_target(transition.target)
                        else {
                            return Err(refused(
                                "emitted invocation source is no named transition call",
                            ));
                        };
                        (None, *static_machine_parameter, None, path.symbol)
                    }
                    _ => return Err(refused("emitted invocation source is no call statement")),
                }
            }
        };
        match target {
            semantic_vocabulary::source_selections::SelectionCallTarget::Selected {
                contract,
                ..
            } => {
                let runtime = runtime.ok_or_else(|| {
                    refused("selected invocation has no actual runtime source callee")
                })?;
                runtime.signature(typed).map_err(refused)?;
                let identity = typed
                    .runtime_callable_contract_identity(runtime.contract, &[])
                    .map_err(refused)?;
                let retained = self.module.execution.callables.contract(contract).ok_or_else(|| {
                    refused("selected invocation names no retained callable contract")
                })?;
                if self.module.identities.text(retained.identity) != Some(identity.as_str()) {
                    return Err(refused("selected invocation changes its source contract"));
                }
                // Calling a structural interface does not select its lexical
                // definition's package or an implementation body. The actual
                // holder and header references retain their own source reads.
                Ok(SelectionReference::RuntimeContract { identity })
            }
            semantic_vocabulary::source_selections::SelectionCallTarget::Declared(target) => {
                if runtime.is_some() {
                    return Err(refused(
                        "runtime source callee became a declared invocation",
                    ));
                }
                let selected = self.source_target(target)?;
                if parameter.is_valid() {
                    return self.declarations.symbol_reference(
                        &mut self.output,
                        owner,
                        parameter,
                        StaticArgumentRole::ValueDependency,
                    );
                }
                if let Some(dispatch) = dispatch {
                    if dispatch.realization_machine != selected {
                        return Err(refused(
                            "static delegation selected a different implementation body",
                        ));
                    }
                    if dispatch.evidence_parameter.is_valid() {
                        return self.declarations.symbol_reference(
                            &mut self.output,
                            owner,
                            dispatch.evidence_parameter,
                            StaticArgumentRole::ValueDependency,
                        );
                    }
                } else if source_target != selected {
                    // Source calls may name an entry while publication names
                    // its machine, including a bodyless boundary machine. Use
                    // the dictionary's exact emitted-entry correspondence for
                    // both physical forms; another declaration still refuses.
                    if self.declarations.dictionary_symbol(source_target)? != selected {
                        return Err(refused(format_args!(
                            "declared invocation differs from its exact source selection: source {} ({source_target:?}), emitted {} ({selected:?}), owner {} ({owner:?}), occurrence {node:?}",
                            typed.symbols.display_path(source_target, "::"),
                            typed.symbols.display_path(selected, "::"),
                            typed.symbols.display_path(owner, "::"),
                        )));
                    }
                }
                self.declarations.symbol_reference(
                    &mut self.output,
                    owner,
                    selected,
                    StaticArgumentRole::ValueDependency,
                )
            }
        }
    }

    // A named call selects a public requirement and a separate body. Read the
    // retained association rather than treating the requirement as that body.
    fn requirement_reference(
        &mut self,
        source: crate::source_state::SourceCallCorrespondence,
        required: ContractId,
        target: semantic_vocabulary::source_selections::SelectionCallTarget,
    ) -> Result<SelectionReference, Diagnostic> {
        use crate::source_state::references::StaticArgumentCall;
        use semantic_vocabulary::source_selections::SelectionCallTarget;

        let SelectionCallTarget::Declared(TargetMachineCallable::Machine(body)) = target else {
            return Err(refused(
                "public requirement has no declared implementation body",
            ));
        };
        let typed = self.source.source();
        let elaboration = &self.source.closure().elaboration();
        if elaboration.machines.get(source.source_member) != Some(&source.owner)
            || !typed.symbols.symbols().nodes().is_valid(source.owner)
        {
            return Err(refused("requirement call lost its original source owner"));
        }
        let selection = self
            .requirements
            .get(&(source.source_member, required, body))
            .copied()
            .ok_or_else(|| {
                refused(
                    "requirement call lost its exact caller, public header and body association",
                )
            })?;
        let call = match source.source {
            crate::source_state::SourceNode::Expression(expression)
                if typed.expression_table.expression_is_valid(expression) =>
            {
                let crate::source_state::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(expression)
                else {
                    return Err(refused("requirement source is no call expression"));
                };
                if call.runtime_call.is_some() {
                    return Err(refused("runtime source callee became a requirement call"));
                }
                StaticArgumentCall::Expression(expression, call)
            }
            crate::source_state::SourceNode::Statement(statement)
                if typed.statement_table.statement_is_valid(statement) =>
            {
                let crate::source_state::statement::StatementNode::Call(call) =
                    typed.statement_table.statement(statement)
                else {
                    return Err(refused("requirement source is no call statement"));
                };
                if call.runtime_call.is_some() {
                    return Err(refused("runtime source callee became a requirement call"));
                }
                StaticArgumentCall::Statement(call)
            }
            _ => return Err(refused("requirement call has a stale source occurrence")),
        };
        let lexical_owner = call.lexical_owner(typed);
        let owner = if lexical_owner == SymbolHandle::invalid() {
            source.owner
        } else {
            lexical_owner
        };
        if !typed.symbols.symbols().nodes().is_valid(owner) {
            return Err(refused("requirement call lost its supplying declaration"));
        }
        let signature = typed
            .source_call_requirement_signature(owner, call)
            .map_err(refused)?;
        let (declaring, _) = typed
            .trait_machine_signature(signature.symbol)
            .ok_or_else(|| refused("requirement call lost its exact declaring trait"))?;
        let identity = typed.trait_requirement_callable_identity_text(declaring, signature);
        let declaration = self
            .module
            .declaration_contract(required)
            .and_then(|contract| contract.declaration.as_ref())
            .ok_or_else(|| refused("requirement call lost its retained public declaration"))?;
        if self.module.identities.text(declaration.callable) != Some(identity.as_str()) {
            return Err(refused(
                "requirement call changes its source public signature",
            ));
        }
        let (mut operand, mut supplier) = typed
            .source_call_selected_operand(owner, call)
            .map_err(refused)?;
        let retained = selection
            .source_operand
            .as_ref()
            .ok_or_else(|| refused("requirement call lost its retained implementation operand"))?;
        // A copied helper names the selected implementation, but its enclosing
        // application supplied the record value. Rejoin that same argument
        // before comparing the supplier retained during source checking.
        let record = typed
            .source_call_record_operand(
                source.owner,
                call,
                &operand,
                supplier,
                self.exact_toolchain_sources
                    .get_or_init(|| {
                        source_admission::source_identity::toolchain_source_identities(
                            typed.symbols.source_files(),
                        )
                    })
                    .as_deref()
                    .map_err(refused)?,
            )
            .map_err(refused)?;
        match (record, retained.static_record.as_ref()) {
            (Some(record), Some(expected))
                if record.parameter == expected.parameter
                    && record.application == expected.application =>
            {
                operand = std::borrow::Cow::Owned(record.operand);
                supplier = record.supplier;
            }
            (None, None) => {}
            _ => {
                return Err(refused(
                    "requirement call changes its retained record parameter or schema",
                ));
            }
        }
        let identity = typed
            .declared_static_operand_identity(
                crate::source_state::type_identity::DeclaredStaticOperandIdentityRequest {
                    operand:
                        crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                            &operand,
                        ),
                    lexical_owner: supplier,
                    binders: &[],
                    lifetime_binders: &[],
                    exact_toolchain_sources: self
                        .exact_toolchain_sources
                        .get_or_init(|| {
                            source_admission::source_identity::toolchain_source_identities(
                                typed.symbols.source_files(),
                            )
                        })
                        .as_deref()
                        .map_err(refused)?,
                },
            )
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refused("requirement call lost its complete implementation operand identity")
            })?;
        if identity != retained.identity || supplier != retained.supplier {
            return Err(refused(
                "requirement call changes its retained implementation operand or supplier",
            ));
        }
        let selected = if selection.parameter.is_valid() {
            let dispatch = match call {
                StaticArgumentCall::Expression(_, call) => {
                    call.static_requirement_dispatch.as_ref()
                }
                StaticArgumentCall::Statement(call) => call.static_requirement_dispatch.as_ref(),
            }
            .ok_or_else(|| refused("requirement call lost its actual parameter dispatch"))?;
            if dispatch.evidence_parameter != selection.parameter
                || dispatch.declaring_trait != selection.declaring_trait
                || dispatch.requirement != selection.requirement
                || dispatch.realization_machine
                    != self.source_target(TargetMachineCallable::Machine(body))?
            {
                return Err(refused(
                    "requirement call changes its retained parameter selection",
                ));
            }
            let (bound, supplying) = typed
                .selected_implementation_operand(owner, selection.parameter)
                .map_err(refused)?;
            if operand.as_ref() != bound || supplier != supplying {
                return Err(refused(
                    "requirement call changes its implementation operand or supplier",
                ));
            }
            selection.parameter
        } else {
            self.source_target(TargetMachineCallable::Machine(body))?
        };
        self.declarations.symbol_reference(
            &mut self.output,
            source.owner,
            selected,
            StaticArgumentRole::ValueDependency,
        )
    }

    fn retain_invocation(
        &mut self,
        machine: MachineId,
        site: InvocationSite,
        target: semantic_vocabulary::source_selections::SelectionCallTarget,
        required: Option<ContractId>,
    ) -> Result<(), Diagnostic> {
        let source = self.typed_origin(machine, site)?;
        let (owner, node) = (source.owner, source.source);
        let selected = if let Some(required) = required {
            self.requirement_reference(source, required, target)?
        } else {
            self.invocation_reference(owner, node, target)?
        };
        let question = self.execution_question(owner, node, selected, SelectionRole::Executable)?;
        let original_caller = self.declarations.declaration(&mut self.output, owner)?;
        self.output.joins.push(SelectionJoin::Invocation {
            question,
            original_caller,
            machine,
            site: CallableInvocationSite::Invocation(site),
            target,
        });
        Ok(())
    }

    /// Cleanup calls use the carrier author's actual choice, including its
    /// type application and static arguments, rather than a caller expression.
    fn cleanup_bindings(&mut self) -> Result<Vec<CleanupSelectionBinding>, Diagnostic> {
        let source = self.source;
        let typed = source.source();
        let mut questions = self
            .output
            .questions
            .iter()
            .filter_map(|(id, question)| {
                let SelectionReference::Declaration(target) = question.selected else {
                    return None;
                };
                (question.form == SelectionForm::Published).then_some((
                    (
                        question.occurrence.lexical_owner,
                        question.occurrence.node,
                        target,
                    ),
                    id,
                ))
            })
            .collect::<Vec<_>>();
        questions.sort_unstable_by_key(|(key, _)| *key);
        let mut bindings = Vec::new();
        for record in &source.correspondence().records {
            let Some(cleanup) = &record.cleanup else {
                continue;
            };
            let subject = typed
                .closed_data_field_definition(cleanup.selection.subject)
                .ok_or_else(|| refused("cleanup source lost its actual carrier application"))?;
            if subject.symbol != record.declaration {
                return Err(refused(
                    "cleanup source differs from its actual carrier application",
                ));
            }
            if let Some(application) = &record.application {
                let actual = typed
                    .closed_data_field_definition(application.reference)
                    .ok_or_else(|| {
                        refused("cleanup carrier has no retained complete application")
                    })?;
                if actual != subject {
                    return Err(refused("cleanup carrier changed its complete application"));
                }
            } else if subject.generic_instance.is_some() || !subject.type_parameters.is_empty() {
                return Err(refused(
                    "applied cleanup carrier lost its complete type arguments",
                ));
            }
            let application = cleanup.selection.application;
            if !typed
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err(refused(
                    "cleanup source lost its selected static application",
                ));
            }
            let registered = typed
                .expression_table
                .static_machine_application(application)
                .registration_operation;
            let target = self.source_target(TargetMachineCallable::Machine(cleanup.target))?;
            if self.declarations.dictionary_symbol(registered)? != target {
                return Err(refused(
                    "cleanup application differs from its selected executable",
                ));
            }
            let occurrence = cleanup
                .selection
                .authorization
                .ok_or_else(|| refused("cleanup source lost its authored selection occurrence"))?;
            let ledger = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| refused("cleanup source selection is outside its typed owner"))?;
            if ledger.kind() != crate::declaration_selection::AuthoredDeclarationSelectionKind::DomainCleanupSelection {
                return Err(refused("cleanup source occurrence is not a cleanup choice"));
            }
            let author = self
                .declarations
                .declaration(&mut self.output, record.declaration)?;
            let selected = self.declarations.declaration(&mut self.output, target)?;
            let node = SelectionNode::AuthoredSelection {
                occurrence: occurrence.ordinal(),
                partition: ledger
                    .compiler_partition()
                    .map(|partition| partition.ordinal()),
            };
            let key = (author, node, selected);
            let start = questions.partition_point(|(candidate, _)| *candidate < key);
            let end = questions.partition_point(|(candidate, _)| *candidate <= key);
            let [(_, question)] = &questions[start..end] else {
                return Err(refused(
                    "cleanup application has no unique captured source question",
                ));
            };
            bindings.push(CleanupSelectionBinding {
                carrier: record.record,
                author,
                question: *question,
                target: cleanup.target,
            });
        }
        bindings.sort_unstable_by_key(|binding| binding.carrier);
        if bindings
            .windows(2)
            .any(|pair| pair[0].carrier == pair[1].carrier)
        {
            return Err(refused(
                "cleanup carrier has competing source application choices",
            ));
        }
        Ok(bindings)
    }

    fn retain_physical_joins(&mut self) -> Result<(), Diagnostic> {
        use semantic_vocabulary::source_selections::SelectionCallTarget;
        let cleanup_bindings = self.cleanup_bindings()?;
        let module = self.module;
        let elaboration = &self.source.closure().elaboration();
        for machine in &module.machines {
            let sources = elaboration.sources.get(machine.id).ok_or_else(|| {
                refused("source machine lost its emitted identity correspondence")
            })?;
            if sources.heads.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return Err(refused(
                    "source component heads are not in exact block order",
                ));
            }
            let symbol = *elaboration
                .machines
                .get(machine.id)
                .ok_or_else(|| refused("emitted machine has no source declaration"))?;
            let declaration = self.declarations.declaration(&mut self.output, symbol)?;
            self.output.joins.push(SelectionJoin::Machine {
                declaration,
                machine: machine.id,
                head: machine.entry,
            });
            for &(head, original) in &sources.heads {
                if head == machine.entry && original == machine.id {
                    continue;
                }
                let symbol = *elaboration
                    .machines
                    .get(original)
                    .ok_or_else(|| refused("folded head has no original source declaration"))?;
                let declaration = self.declarations.declaration(&mut self.output, symbol)?;
                self.output.joins.push(SelectionJoin::Machine {
                    declaration,
                    machine: machine.id,
                    head,
                });
            }
            for block in &machine.blocks {
                for operation in &block.operations {
                    let site = InvocationSite::Operation(operation.id);
                    if let Some(call) = operation.declared_invocation() {
                        self.retain_invocation(
                            machine.id,
                            site,
                            SelectionCallTarget::Declared(call.callee),
                            call.required_contract,
                        )?;
                    } else if let Some(call) = operation.kind.callable_invocation() {
                        self.retain_invocation(
                            machine.id,
                            site,
                            SelectionCallTarget::Selected {
                                contract: call.contract,
                                selector: call.callee,
                            },
                            None,
                        )?;
                    }
                    if let OperationKind::SelectMachine { descriptor } = operation.kind {
                        let mut sources = self.source.correspondence().constructions.iter().filter(
                            |(emitted, selected, _, _)| {
                                *emitted == machine.id && *selected == operation.id
                            },
                        );
                        let (_, _, owner, expression) =
                            sources.next().copied().ok_or_else(|| {
                                refused("machine selection has no exact expression occurrence")
                            })?;
                        if sources.next().is_some() {
                            return Err(refused(
                                "machine selection has competing source occurrences",
                            ));
                        }
                        let node = crate::source_state::SourceNode::Expression(expression);
                        if !matches!(
                            self.source.source().expression_table.expression(expression),
                            crate::source_state::expression::ExpressionNode::MachineValue(_)
                        ) {
                            return Err(refused(
                                "descriptor constructor differs from its source expression",
                            ));
                        }
                        let selected = module
                            .execution.callables
                            .descriptor(descriptor)
                            .ok_or_else(|| refused("selected descriptor has no retained row"))?;
                        let target =
                            self.source_target(TargetMachineCallable::Machine(selected.machine))?;
                        let declaration =
                            self.declarations.declaration(&mut self.output, target)?;
                        let question = self.execution_question(
                            owner,
                            node,
                            SelectionReference::Declaration(declaration),
                            SelectionRole::MachineValue,
                        )?;
                        self.output.joins.push(SelectionJoin::Descriptor {
                            question,
                            descriptor,
                            selected: declaration,
                            machine: machine.id,
                            operation: operation.id,
                        });
                    }
                }
                for (edge, actions) in block.terminator.cleanup_rosters() {
                    for (ordinal, action) in actions.iter().enumerate() {
                        let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) =
                            action
                        else {
                            continue;
                        };
                        let position = cleanup_bindings
                            .binary_search_by_key(&cleanup.structural_type, |binding| {
                                binding.carrier
                            })
                            .map_err(|_| {
                                refused("cleanup action lost its exact carrier source application")
                            })?;
                        let binding = &cleanup_bindings[position];
                        if binding.target != cleanup.cleanup_machine {
                            return Err(refused(
                                "cleanup action changed its selected source target",
                            ));
                        }
                        self.output.joins.push(SelectionJoin::Invocation {
                            question: binding.question,
                            original_caller: binding.author,
                            machine: machine.id,
                            site: CallableInvocationSite::CleanupAction {
                                edge,
                                action_ordinal: u32::try_from(ordinal).map_err(|_| {
                                    refused("cleanup action ordinal exceeds its namespace")
                                })?,
                            },
                            target: SelectionCallTarget::Declared(TargetMachineCallable::Machine(
                                binding.target,
                            )),
                        });
                    }
                }
                if let Some((site, call)) = block.terminator.declared_invocation() {
                    self.retain_invocation(
                        machine.id,
                        site,
                        SelectionCallTarget::Declared(call.callee),
                        call.required_contract,
                    )?;
                }
            }
            let mut successors = machine
                .blocks
                .iter()
                .flat_map(|block| block.terminator.successor_edges())
                .collect::<Vec<_>>();
            successors.sort_unstable_by_key(|(edge, _)| *edge);
            for &(emitted, edge, owner, node) in &self.source.correspondence().folded_calls {
                if emitted != machine.id {
                    continue;
                }
                let Ok(position) = successors.binary_search_by_key(&edge, |(edge, _)| *edge) else {
                    continue;
                };
                let head = successors[position].1;
                let Ok(head_index) = sources
                    .heads
                    .binary_search_by_key(&head, |(block, _)| *block)
                else {
                    continue;
                };
                let original = sources.heads[head_index].1;
                let target = *elaboration
                    .machines
                    .get(original)
                    .ok_or_else(|| refused("folded target lost its original source declaration"))?;
                let selected = self.invocation_reference(
                    owner,
                    node,
                    SelectionCallTarget::Declared(TargetMachineCallable::Machine(original)),
                )?;
                let question =
                    self.execution_question(owner, node, selected, SelectionRole::Executable)?;
                let original_caller = self.declarations.declaration(&mut self.output, owner)?;
                let original_target = self.declarations.declaration(&mut self.output, target)?;
                self.output.joins.push(SelectionJoin::FoldedCall {
                    question,
                    original_caller,
                    original_target,
                    machine: machine.id,
                    edge,
                    head,
                });
            }
        }
        for (boundary, symbol) in elaboration.boundaries.iter() {
            let declaration = self.declarations.declaration(&mut self.output, *symbol)?;
            self.output.joins.push(SelectionJoin::Boundary {
                declaration,
                boundary,
            });
        }
        for record in &self.source.correspondence().records {
            let declaration = self
                .declarations
                .declaration(&mut self.output, record.declaration)?;
            self.output.joins.push(SelectionJoin::Structure {
                declaration,
                structural_type: record.record,
                storage_identity: record.storage_identity.clone(),
            });
        }
        Ok(())
    }
    fn retain_build_sites(&mut self) -> Result<(), Diagnostic> {
        for machine in &self.module.machines {
            for call in machine.boundary_invocations() {
                let InvocationSite::Operation(operation) = call.site else {
                    continue;
                };
                let mut origins = self.source.correspondence().calls.iter().filter(|call| {
                    call.machine == machine.id && call.site == InvocationSite::Operation(operation)
                });
                let Some(origin) = origins.next() else {
                    continue;
                };
                if origins.next().is_some() {
                    return Err(refused(
                        "one Build input occurrence has competing source sites",
                    ));
                }
                let (owner, crate::source_state::SourceNode::Expression(expression)) =
                    (origin.owner, origin.source)
                else {
                    continue;
                };
                if self
                    .source
                    .source()
                    .expression_table
                    .build_selection_inputs(expression)
                    .map_err(refused)?
                    .is_some()
                {
                    let site = BuildCallSite {
                        machine: machine.id,
                        operation,
                        boundary: call.boundary,
                    };
                    if self.build_sites.insert((owner, expression), site).is_some() {
                        return Err(refused(
                            "one Build input occurrence has multiple emitted sites",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn retain_form(
        &mut self,
        form: SelectionForm,
        raw: &RawSelectionQuestions,
    ) -> Result<(), Diagnostic> {
        for &declaration in &raw.declarations {
            self.declarations
                .declaration(&mut self.output, declaration)
                .map_err(|error| {
                    selection_context(error, format_args!("captured declaration {declaration:?}"))
                })?;
        }
        for question in &raw.questions {
            self.authored_question(form, question).map_err(|error| {
                selection_context(
                    error,
                    format_args!(
                        "authored occurrence {:?}, lexical owner {:?}, target {:?}",
                        question.occurrence, question.lexical_owner, question.target,
                    ),
                )
            })?;
        }
        for dependency in &raw.dependencies {
            if dependency.role == RawSelectionRole::ProductDesignation {
                return Err(refused(
                    "Product dependency has no exact retained input commitment",
                ));
            }
            let owner = self
                .declarations
                .declaration(&mut self.output, dependency.lexical_owner)
                .map_err(|error| {
                    selection_context(
                        error,
                        format_args!("dependency owner {:?}", dependency.lexical_owner),
                    )
                })?;
            let selected = self
                .declarations
                .symbol_reference(
                    &mut self.output,
                    dependency.lexical_owner,
                    dependency.target,
                    if dependency.kind == RawDependencyKind::Description {
                        StaticArgumentRole::DeclarationDescription
                    } else {
                        StaticArgumentRole::ValueDependency
                    },
                )
                .map_err(|error| {
                    selection_context(
                        error,
                        format_args!(
                            "dependency from {:?} to {:?} ({:?})",
                            dependency.lexical_owner, dependency.target, dependency.kind,
                        ),
                    )
                })?;
            self.output.questions.push(SelectionQuestion {
                form,
                occurrence: SelectionOccurrence {
                    lexical_owner: owner,
                    node: SelectionNode::Declaration {
                        index: dependency.lexical_owner.arena_index(),
                        generation: dependency.lexical_owner.generation(),
                    },
                },
                selected,
                purpose: self.declarations.purpose(dependency.lexical_owner)?,
                // A reference inventory is not itself an invocation. Actual
                // calls and code construction receive exact physical joins.
                role: match dependency.kind {
                    RawDependencyKind::Layout => SelectionRole::Type,
                    _ => SelectionRole::Declaration,
                },
            });
        }
        Ok(())
    }

    fn authored_question(
        &mut self,
        form: SelectionForm,
        raw: &RawSelectionQuestion,
    ) -> Result<(), Diagnostic> {
        let typed = self.source.source();
        let ledger = typed
            .authored_declaration_selections()
            .get(raw.occurrence)
            .ok_or_else(|| refused("captured authored selection lost its exact ledger row"))?;
        let owner = self
            .declarations
            .declaration(&mut self.output, raw.lexical_owner)?;
        let designation = match (raw.role, raw.designation) {
            (RawSelectionRole::Execution, None) => None,
            (RawSelectionRole::ProductDesignation, Some(input)) => {
                Some(self.product_designation(form, raw, input)?)
            }
            _ => {
                return Err(refused(
                    "selection role disagrees with its retained Product input",
                ));
            }
        };
        let selected = match raw.target {
            RawSelectionTarget::Declaration(symbol) => {
                // Build records a request; Product selects its declaration after
                // the package join. The input commitment retains this exact symbol.
                if let Some(designation) = designation.as_ref()
                    && self.module.identities.invocation_scope().is_some()
                    && self.declarations.purpose(raw.lexical_owner)? == SelectionPurpose::Build
                {
                    if raw.designation.and_then(|input| input.resolved_target) != Some(symbol) {
                        return Err(refused(
                            "resolved Product designation differs from its exact retained input",
                        ));
                    }
                    SelectionReference::ProductInput {
                        input_selection: designation.input_selection,
                    }
                } else {
                    self.declarations.symbol_reference(
                        &mut self.output,
                        raw.lexical_owner,
                        symbol,
                        StaticArgumentRole::DeclarationDescription,
                    )?
                }
            }
            RawSelectionTarget::LocalBinding { symbol, owner } => self.declarations.local_binding(
                &mut self.output,
                raw.lexical_owner,
                symbol,
                owner,
            )?,
            RawSelectionTarget::CoreIntegerOperator(application) => {
                SelectionReference::Declaration(
                    self.declarations
                        .core_integer_operator(&mut self.output, application)?,
                )
            }
            RawSelectionTarget::Intrinsic(intrinsic) => SelectionReference::Intrinsic {
                identity: intrinsic_identity(&intrinsic, "authored-intrinsic-v1:")?,
            },
            RawSelectionTarget::ProductInput { input, .. } => {
                if raw.designation != Some(input) {
                    return Err(refused(
                        "inert Product reference names another input occurrence",
                    ));
                }
                SelectionReference::ProductInput {
                    input_selection: designation
                        .as_ref()
                        .ok_or_else(|| {
                            refused("unresolved reference has no authenticated Product designation")
                        })?
                        .input_selection,
                }
            }
            RawSelectionTarget::RuntimeCall { binding, .. } => {
                binding.signature(typed).map_err(refused)?;
                SelectionReference::RuntimeContract {
                    identity: typed
                        .runtime_callable_contract_identity(binding.contract, &[])
                        .map_err(refused)?,
                }
            }
        };
        let (purpose, role) = match designation {
            Some(designation) => (
                SelectionPurpose::Product,
                SelectionRole::ProductDesignation(designation),
            ),
            None => (
                self.declarations.purpose(raw.lexical_owner)?,
                if matches!(raw.target, RawSelectionTarget::RuntimeCall { .. }) {
                    SelectionRole::Contract
                } else {
                    SelectionRole::Declaration
                },
            ),
        };
        self.output.questions.push(SelectionQuestion {
            form,
            occurrence: SelectionOccurrence {
                lexical_owner: owner,
                node: SelectionNode::AuthoredSelection {
                    occurrence: raw.occurrence.ordinal(),
                    partition: ledger
                        .compiler_partition()
                        .map(|partition| partition.ordinal()),
                },
            },
            selected,
            purpose,
            role,
        });
        Ok(())
    }

    fn product_designation(
        &mut self,
        form: SelectionForm,
        raw: &RawSelectionQuestion,
        input: RawDesignationInput,
    ) -> Result<SelectionProductDesignation, Diagnostic> {
        use ::wire::digest_encoder::DomainDigestEncoder;
        let typed = self.source.source();
        let retained = typed
            .expression_table
            .build_selection_inputs(input.expression)
            .map_err(refused)?
            .ok_or_else(|| refused("Product input has no exact source association"))?;
        if retained.lexical_owner != raw.lexical_owner || retained.request != input.request {
            return Err(refused(
                "Product input belongs to another source owner or request",
            ));
        }
        let mut inputs = retained.inputs.iter().filter(|selected| {
            selected.occurrence == raw.occurrence
                && selected.argument_ordinal == input.argument_ordinal
        });
        let (Some(selected), None) = (inputs.next(), inputs.next()) else {
            return Err(refused(
                "Product input has no unique exact occurrence and ordinal",
            ));
        };
        if selected.resolved_target != input.resolved_target {
            return Err(refused(
                "Product input changed its resolver-owned selected declaration",
            ));
        }
        // A generic helper's own check form shares the authored call with
        // each published application, so only the published form may read
        // the emitted site. The check form's request never runs.
        let site = if form == SelectionForm::Published {
            Some(
                *self
                    .build_sites
                    .get(&(raw.lexical_owner, input.expression))
                    .ok_or_else(|| refused("Product input has no exact emitted Build site"))?,
            )
        } else {
            None
        };
        let encoded = record_storage::encode(selected, Default::default()).map_err(refused)?;
        let owner = self.declarations.identity(raw.lexical_owner)?;
        let request = typed
            .expression_table
            .build_selection_request(input.request)
            .ok_or_else(|| refused("Product input lost its complete request"))?;
        let request_bytes =
            record_storage::encode(request, Default::default()).map_err(refused)?;
        let mut digest = DomainDigestEncoder::new(b"omega.source-product-input.v1\0");
        digest.bytes(owner.as_bytes());
        digest.bytes(&encoded);
        digest.bytes(&request_bytes);
        let input_selection = digest.finish();
        let crate::source_state::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(input.expression)
        else {
            return Err(refused("Product input is attached to no actual call"));
        };
        let static_application = if call.authored_machine_application.is_valid() {
            if !typed
                .expression_table
                .static_machine_application_is_valid(call.authored_machine_application)
            {
                return Err(refused("Product input has a stale static application"));
            }
            let application = typed
                .expression_table
                .static_machine_application(call.authored_machine_application);
            let encoded =
                record_storage::encode(application, Default::default()).map_err(refused)?;
            let mut digest =
                DomainDigestEncoder::new(b"omega.source-product-static-application.v1\0");
            digest.bytes(owner.as_bytes());
            digest.bytes(&encoded);
            Some(digest.finish())
        } else {
            None
        };
        Ok(SelectionProductDesignation {
            site,
            operand: input.argument_ordinal,
            input_selection,
            static_application,
        })
    }
}

fn intrinsic_identity(value: &impl Record, domain: &str) -> Result<String, Diagnostic> {
    let encoded = record_storage::encode(value, Default::default())
        .map_err(|error| refused(format!("intrinsic identity cannot be retained: {error}")))?;
    let mut identity = String::with_capacity(domain.len() + encoded.len() * 2);
    identity.push_str(domain);
    for byte in encoded {
        write!(identity, "{byte:02x}").expect("writing an owned string cannot fail");
    }
    Ok(identity)
}

struct DeclarationIdentity {
    text: String,
    emitted: bool,
}

impl SymbolRows<DeclarationIdentity> {
    fn retain_identity(
        &mut self,
        symbol: SymbolHandle,
        identity: &str,
        emitted: bool,
    ) -> Result<(), Diagnostic> {
        let row = self.get_or_insert_with(symbol, || DeclarationIdentity {
            text: identity.to_owned(),
            emitted,
        })?;
        if row.text != identity {
            return Err(refused(
                "one source declaration has competing checked identities",
            ));
        }
        row.emitted |= emitted;
        Ok(())
    }

    fn is_emitted(&self, symbol: SymbolHandle) -> bool {
        self.get(&symbol).is_some_and(|row| row.emitted)
    }
}

struct Declarations<'source> {
    typed: &'source TypedTrees,
    build_entry_source: Option<&'source source_admission::SourceFile>,
    ids: SymbolRows<SelectionDeclarationId>,
    identities: SymbolRows<DeclarationIdentity>,
    contracts: SymbolRows<Vec<RuntimeCallableContractHandle>>,
    callable_declarations: HashMap<(SelectionPurpose, String), SelectionDeclarationId>,
    sources: Vec<(SelectionDeclarationId, source_text::SourceSpan)>,
    core_operators: BTreeMap<CoreIntegerOperatorSignature, SelectionDeclarationId>,
}

impl<'source> Declarations<'source> {
    fn new(
        source: &'source AcceptedTerminalSource,
        module: &TerminalModule,
    ) -> Result<Self, Diagnostic> {
        let mut identities = SymbolRows::new(source.source().symbols.symbols().nodes().len());
        let elaboration = &source.closure().elaboration();
        for machine in &module.machines {
            let symbol = elaboration
                .machines
                .get(machine.id)
                .copied()
                .ok_or_else(|| refused("emitted machine has no exact source declaration"))?;
            let declaration = machine
                .declaration
                .as_ref()
                .ok_or_else(|| refused("emitted source machine has no checked declaration"))?;
            let identity = module
                .identities
                .text(declaration.semantic_declaration)
                .ok_or_else(|| refused("checked machine declaration lost its semantic identity"))?;
            identities.retain_identity(symbol, identity, true)?;
        }
        for (boundary, symbol) in elaboration.boundaries.iter() {
            let declaration = module
                .boundary_machine(boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .ok_or_else(|| refused("source boundary has no exact checked declaration"))?;
            let identity = module
                .identities
                .text(declaration.source.semantic_declaration)
                .ok_or_else(|| refused("checked boundary lost its semantic identity"))?;
            identities.retain_identity(*symbol, identity, true)?;
        }
        // Anonymous Build records already have checked declaration identities.
        // Reuse their exact source associations, as for emitted machines; a
        // second hermetic-name lookup would discard that invocation custody.
        for source_record in &source.correspondence().records {
            let record = module
                .structural_type(source_record.record)
                .ok_or_else(|| refused("source record has no checked structural declaration"))?;
            let mut retain = |symbol, identity| -> Result<(), Diagnostic> {
                let identity = module
                    .identities
                    .text(identity)
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        refused("checked source record lost its declaration identity")
                    })?;
                identities.retain_identity(symbol, identity, true)
            };
            retain(source_record.declaration, record.declaration_identity)?;
            let mut fields = record.shape.fields().collect::<Vec<_>>();
            fields.sort_unstable_by_key(|field| field.id);
            for field in &source_record.fields {
                let position = fields
                    .binary_search_by_key(&field.field, |field| field.id)
                    .map_err(|_| refused("source field has no checked structural declaration"))?;
                retain(field.declaration, fields[position].declaration_identity)?;
            }
        }
        for form in source.selection_forms().flatten() {
            for (symbol, identity) in form.invocation_declarations() {
                if module.identities.invocation_scope().is_none() {
                    return Err(refused(
                        "portable selection contains an invocation declaration",
                    ));
                }
                identities.retain_identity(*symbol, identity, false)?;
            }
        }
        // Prepare this association once. Only an admitted contract's meaning
        // is normalized, so unrelated generic contracts never become work.
        let mut contracts: SymbolRows<Vec<RuntimeCallableContractHandle>> =
            SymbolRows::new(source.source().symbols.symbols().nodes().len());
        for (handle, contract) in source.source().runtime_callable_contracts.iter() {
            contracts
                .get_or_insert_with(contract.signature.symbol, Vec::new)?
                .push(handle);
        }
        let build_entry_source = module
            .entry
            .and_then(|entry| match entry {
                TargetMachineCallable::Machine(machine) => elaboration.machines.get(machine),
                TargetMachineCallable::Boundary(_) => None,
            })
            .and_then(|symbol| {
                source
                    .source()
                    .symbols
                    .symbol_provenance_source_span(*symbol)
            })
            .and_then(|span| source.source().symbols.source_file(span))
            .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build);
        Ok(Self {
            typed: source.source(),
            build_entry_source,
            ids: SymbolRows::new(source.source().symbols.symbols().nodes().len()),
            identities,
            contracts,
            callable_declarations: HashMap::new(),
            sources: Vec::new(),
            core_operators: BTreeMap::new(),
        })
    }

    fn core_integer_operator(
        &mut self,
        output: &mut SelectionQuestions,
        application: CoreIntegerOperatorSignature,
    ) -> Result<SelectionDeclarationId, Diagnostic> {
        if !application.is_valid() {
            return Err(refused(
                "core integer operator has an invalid catalog application",
            ));
        }
        if let Some(id) = self.core_operators.get(&application) {
            return Ok(*id);
        }
        let identity = intrinsic_identity(&application, "core-integer-policy-operator-v1:")?;
        let id = output.declarations.push(SelectionDeclaration {
            identity: identity.clone(),
            callable_identity: Some(identity),
            name: format!("Core integer {:?}", application.operation()),
            custody: DeclarationCustody::Toolchain,
            purpose: SelectionPurpose::Product,
        });
        self.core_operators.insert(application, id);
        Ok(id)
    }

    fn source_span(
        &self,
        declaration: SelectionDeclarationId,
    ) -> Result<source_text::SourceSpan, Diagnostic> {
        self.sources
            .binary_search_by_key(&declaration, |(id, _)| *id)
            .ok()
            .map(|index| self.sources[index].1)
            .ok_or_else(|| refused("captured selection lost its source declaration"))
    }

    fn valid(&self, symbol: SymbolHandle, role: &str) -> Result<(), Diagnostic> {
        if !symbol.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(refused(format!(
                "source selection {role} has a missing or stale declaration coordinate {symbol:?}"
            )));
        }
        Ok(())
    }

    fn purpose(&self, symbol: SymbolHandle) -> Result<SelectionPurpose, Diagnostic> {
        self.valid(symbol, "purpose owner")?;
        let source = self
            .typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| self.typed.symbols.source_file(span))
            .ok_or_else(|| refused("source declaration has no retained source purpose"))?;
        let purpose = match source.dependency_scope {
            source_provenance::DependencyScope::Build => SelectionPurpose::Build,
            source_provenance::DependencyScope::Product => SelectionPurpose::Product,
        };
        Ok(match (self.build_entry_source, source.package_identity) {
            (Some(entry), Some(owner)) => {
                purpose.for_declaring_package(entry.package_identity, owner)
            }
            _ => purpose,
        })
    }

    fn dictionary_symbol(&self, symbol: SymbolHandle) -> Result<SymbolHandle, Diagnostic> {
        self.valid(symbol, "declaration")?;
        if self.typed.symbols.get(symbol).kind != SymbolKind::State
            || self.identities.is_emitted(symbol)
        {
            return Ok(symbol);
        }
        let Some((machine, _)) = self.typed.unique_machine_by_entry_state_symbol(symbol) else {
            let has_entry = self.typed.machine_states_named(symbol).any(|(machine, _)| {
                self.typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == symbol)
            });
            if has_entry {
                return Err(refused(format!(
                    "source selection state `{}` ({symbol:?}) has competing registered machine entries; source {:?}",
                    self.typed.symbols.display_path(symbol, "::"),
                    self.typed.symbols.symbol_provenance_source_span(symbol),
                )));
            }
            // Stored signatures retain their own declaration scope without
            // registering an executable machine. Its ordinary identity below
            // still authenticates the exact source/package provenance.
            return Ok(symbol);
        };
        self.valid(machine.symbol, "entry declaration")?;
        if !self.identities.is_emitted(machine.symbol) {
            // Only an actual emitted declaration supplies an entry alias.
            // An un-emitted source description keeps its genuine identity.
            return Ok(symbol);
        }
        if self.purpose(symbol)? != self.purpose(machine.symbol)? {
            return Err(refused(
                "source selection entry changes its owning declaration's purpose",
            ));
        }
        // The entry and its owning machine are one selected declaration, not
        // two dictionary rows with a copied identity. The original occurrence
        // and physical call still authenticate the exact entry independently.
        Ok(machine.symbol)
    }

    fn identity(&mut self, symbol: SymbolHandle) -> Result<String, Diagnostic> {
        let symbol = self.dictionary_symbol(symbol)?;
        if let Some(identity) = self.identities.get(&symbol) {
            return Ok(identity.text.clone());
        }
        let identity = self
            .typed
            .normalized_hermetic_symbol_identity(symbol)
            .map_err(refused)?;
        self.identities.retain_identity(symbol, &identity, false)?;
        Ok(identity)
    }

    fn callable_identity(&self, symbol: SymbolHandle) -> Result<Option<String>, Diagnostic> {
        if self.typed.symbols.get(symbol).kind == SymbolKind::CallableContract {
            let contracts = self
                .contracts
                .get(&symbol)
                .ok_or_else(|| refused("callable declaration has no retained full contract"))?;
            let mut identity = None;
            for &contract in contracts {
                let candidate = self
                    .typed
                    .runtime_callable_contract_identity(contract, &[])
                    .map_err(|error| refused(format!("callable selection identity: {error}")))?;
                if identity
                    .as_ref()
                    .is_some_and(|previous| previous != &candidate)
                {
                    return Err(refused(
                        "one callable declaration names different retained contracts",
                    ));
                }
                identity = Some(candidate);
            }
            identity
                .map(Some)
                .ok_or_else(|| refused("callable declaration has no contract row"))
        } else {
            Ok(None)
        }
    }

    fn written_callable(&self, mut symbol: SymbolHandle) -> Result<SymbolHandle, Diagnostic> {
        // Applying a signature creates fresh binders and a fresh contract
        // symbol. Its derivation points at the exact declaration it copied.
        loop {
            let origin = self.typed.symbols.get(symbol).generated_from;
            if !origin.is_valid() {
                return Ok(symbol);
            }
            self.valid(origin, "applied callable origin")?;
            if origin.arena_index() >= symbol.arena_index() {
                return Err(refused(
                    "applied callable has no earlier declaration origin",
                ));
            }
            if self.typed.symbols.get(origin).kind != SymbolKind::CallableContract {
                return Ok(symbol);
            }
            symbol = origin;
        }
    }

    fn declaration(
        &mut self,
        output: &mut SelectionQuestions,
        symbol: SymbolHandle,
    ) -> Result<SelectionDeclarationId, Diagnostic> {
        let symbol = self.dictionary_symbol(symbol)?;
        if let Some(id) = self.ids.get(&symbol) {
            return Ok(*id);
        }
        let callable_identity = self.callable_identity(symbol)?;
        let written = if callable_identity.is_some() {
            self.written_callable(symbol)?
        } else {
            symbol
        };
        let occurrence = self.identity(written)?;
        let identity = match &callable_identity {
            Some(meaning) => callable_declaration_path(&occurrence, meaning),
            None => occurrence,
        };
        let custody = lowered_source_publication::selection_questions::declaration_custody(
            self.typed, symbol,
        )
        .map_err(refused)?;
        let purpose = self.purpose(symbol)?;
        let span = self
            .typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .ok_or_else(|| refused("selected declaration has no retained source span"))?;
        let declaration = SelectionDeclaration {
            identity,
            name: self.typed.symbols.display_path(written, "::"),
            custody,
            purpose,
            callable_identity,
        };
        // Use the written declaration and its applied meaning. Equal copies
        // share a row; a different occurrence or contract needs its own row.
        let id = if declaration.callable_identity.is_some() {
            match self
                .callable_declarations
                .entry((purpose, declaration.identity.clone()))
            {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let id = *entry.get();
                    if output.declarations.get(id) != Some(&declaration)
                        || self.source_span(id)? != span
                    {
                        return Err(refused(
                            "one callable declaration identity has competing source evidence",
                        ));
                    }
                    self.ids.insert(symbol, id)?;
                    return Ok(id);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    *entry.insert(output.declarations.push(declaration))
                }
            }
        } else {
            output.declarations.push(declaration)
        };
        self.sources.push((id, span));
        self.ids.insert(symbol, id)?;
        Ok(id)
    }

    fn local_binding(
        &mut self,
        output: &mut SelectionQuestions,
        lexical_owner: SymbolHandle,
        symbol: SymbolHandle,
        declared_owner: SymbolHandle,
    ) -> Result<SelectionReference, Diagnostic> {
        self.valid(symbol, "local binding")?;
        self.valid(lexical_owner, "local binding lexical owner")?;
        self.valid(declared_owner, "local binding declaration owner")
            .map_err(|error| {
                let provenance = self
                    .typed
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .map(|span| {
                        (
                            self.typed.symbols.source_file(span).map(|file| &file.path),
                            span,
                        )
                    });
                selection_context(
                    error,
                    format_args!(
                        "binding {} ({:?}), referenced from {}, source {provenance:?}",
                        self.typed.symbols.display_path(symbol, "::"),
                        self.typed.symbols.get(symbol).kind,
                        self.typed.symbols.display_path(lexical_owner, "::"),
                    ),
                )
            })?;
        if self.typed.symbols.get(symbol).parent != declared_owner {
            return Err(refused(
                "local selection changed its actual declaration owner",
            ));
        }
        let kind = match self.typed.symbols.get(symbol).kind {
            SymbolKind::Parameter | SymbolKind::Local => SelectionBindingKind::Value,
            SymbolKind::TypeParameter if self.typed.proof_parameter_contract(symbol).is_some() => {
                SelectionBindingKind::Proof
            }
            SymbolKind::TypeParameter => SelectionBindingKind::Type,
            SymbolKind::MachineParameter => SelectionBindingKind::Machine,
            SymbolKind::ConformanceParameter => SelectionBindingKind::Conformance,
            SymbolKind::PropositionParameter => SelectionBindingKind::Proof,
            SymbolKind::PropositionMachineParameter => SelectionBindingKind::PropositionMachine,
            _ => return Err(refused("resolved local selection names no actual binding")),
        };
        // A formal belongs to the entry state, and the body is checked
        // in its machine's lexical scope. Follow the source declarations
        // first. When an enclosing static binder was copied, check the
        // actual parent chain of that specialization as well.
        let mut scopes = Vec::new();
        let mut sees_binding = |follow_copied_parents: bool| -> Result<bool, Diagnostic> {
            let first = scopes.len();
            let mut scope = lexical_owner;
            while scope.is_valid() {
                self.valid(scope, "enclosing local scope")?;
                if scopes[first..].contains(&scope) {
                    return Err(refused("local selection has a cyclic lexical scope"));
                }
                scopes.push(scope);
                if scope == declared_owner
                    || self.typed.machine_by_symbol(scope).is_some_and(|machine| {
                        self.typed
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == declared_owner)
                    })
                {
                    return Ok(true);
                }
                scope = if self.typed.symbols.get(scope).kind == SymbolKind::CallableContract {
                    // The anonymous signature symbol belongs to the symbol-table root.
                    // Its exact contract occurrence records the source scope it captures.
                    let mut parents = self
                        .contracts
                        .get(&scope)
                        .into_iter()
                        .flatten()
                        .map(|&handle| self.typed.runtime_callable_contract(handle).lexical_owner);
                    let parent = parents
                        .next()
                        .ok_or_else(|| refused("callable scope has no retained contract"))?;
                    self.valid(parent, "callable's enclosing source declaration")?;
                    if parents.any(|other| other != parent) {
                        return Err(refused(
                            "callable scope has ambiguous retained lexical parents",
                        ));
                    }
                    parent
                } else if let Some((owner, _)) = self.typed.owned_conformance_realization(scope) {
                    owner.symbol
                } else if !follow_copied_parents
                    && let Some(template) = self
                        .typed
                        .executable_application_template(scope)
                        .ok()
                        .flatten()
                {
                    template
                } else {
                    self.typed.symbols.get(scope).parent
                };
            }
            Ok(false)
        };
        let visible = sees_binding(false)?
            || (!matches!(kind, SelectionBindingKind::Value)
                && self
                    .typed
                    .executable_application_template(declared_owner)
                    .is_ok_and(|application| application.is_some())
                && sees_binding(true)?);
        if !visible {
            let description = |symbol: SymbolHandle| {
                let provenance = self
                    .typed
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .map(|span| {
                        (
                            self.typed.symbols.source_file(span).map(|file| &file.path),
                            span,
                        )
                    });
                (
                    self.typed.symbols.display_path(symbol, "::"),
                    symbol,
                    self.typed.symbols.get(symbol).kind,
                    provenance,
                )
            };
            return Err(selection_context(
                refused("selected local binding is outside its actual lexical context"),
                format_args!(
                    "lexical owner {:?}; selected binding {:?}; declared owner {:?}; visited lexical scopes {:?}",
                    description(lexical_owner),
                    description(symbol),
                    description(declared_owner),
                    scopes.into_iter().map(description).collect::<Vec<_>>(),
                ),
            ));
        }
        Ok(SelectionReference::LocalBinding {
            owner: self.declaration(output, lexical_owner)?,
            kind,
            index: symbol.arena_index(),
            generation: symbol.generation(),
        })
    }

    fn symbol_reference(
        &mut self,
        output: &mut SelectionQuestions,
        owner: SymbolHandle,
        symbol: SymbolHandle,
        role: StaticArgumentRole,
    ) -> Result<SelectionReference, Diagnostic> {
        self.valid(symbol, "reference target")?;
        if let Some(atom) = self.typed.symbols.builtin_type_atom(symbol) {
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&atom, "builtin-type-v1:")?,
            });
        }
        // The open-ended integer family (`u<N>`, `i<N>`) is builtin like each
        // width it names; it has no authored source or package.
        if let Some(sign) = self.typed.symbols.integer_family(symbol) {
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&sign, "builtin-integer-family-v1:")?,
            });
        }
        if let Some(function) = self.typed.symbols.builtin_function_for_symbol(symbol) {
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&function, "builtin-function-v1:")?,
            });
        }
        if let Some(member) = self.typed.symbols.builtin_type_member(symbol) {
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&member, "builtin-type-member-v1:")?,
            });
        }
        if let Some(declaration) = self.typed.symbols.fixed_core_declaration_for_symbol(symbol) {
            // Kernel vocabulary is installed at exact root slots. It has no
            // authored source/package and grants no invocation or proof rule.
            let ordinal = u8::try_from(declaration.ordinal())
                .map_err(|_| refused("fixed core declaration exceeds its closed ordinal"))?;
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&ordinal, "fixed-core-declaration-v1:")?,
            });
        }
        match self.typed.symbols.get(symbol).kind {
            SymbolKind::Parameter
            | SymbolKind::Local
            | SymbolKind::TypeParameter
            | SymbolKind::MachineParameter
            | SymbolKind::ConformanceParameter
            | SymbolKind::PropositionParameter
            | SymbolKind::PropositionMachineParameter
                if role == StaticArgumentRole::ValueDependency =>
            {
                self.local_binding(output, owner, symbol, self.typed.symbols.get(symbol).parent)
            }
            _ => Ok(SelectionReference::Declaration(
                self.declaration(output, symbol)?,
            )),
        }
    }
}

#[cfg(test)]
mod symbol_rows_tests {
    use super::*;

    #[test]
    fn fixed_kernel_references_keep_their_slots_without_authored_provenance() {
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::{FixedCoreDeclaration, SymbolNameRef};
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain(crate::source_symbols::builtin_function_symbols())
                .chain(crate::source_symbols::fixed_core_declaration_symbols())
                .chain([(
                    SymbolKind::MathematicalDefinition,
                    SymbolNameRef::Static("core::Identity"),
                )]),
        );
        let counterfeit = children.handle_at(children.count() - 1);
        let typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let count = typed.symbols.symbols().nodes().len();
        let mut declarations = Declarations {
            callable_declarations: HashMap::new(),
            typed: &typed,
            build_entry_source: None,
            ids: SymbolRows::new(count),
            identities: SymbolRows::new(count),
            contracts: SymbolRows::new(count),
            sources: Vec::new(),
            core_operators: BTreeMap::new(),
        };
        let mut output = SelectionQuestions::default();
        let mut identities = std::collections::BTreeSet::new();
        for declaration in FixedCoreDeclaration::ALL {
            let symbol = typed
                .symbols
                .fixed_core_declaration_symbol(declaration)
                .unwrap();
            let selected = declarations
                .symbol_reference(
                    &mut output,
                    root,
                    symbol,
                    StaticArgumentRole::ValueDependency,
                )
                .unwrap();
            let SelectionReference::Intrinsic { identity } = &selected else {
                panic!("fixed vocabulary became an authored declaration");
            };
            assert!(identity.starts_with("fixed-core-declaration-v1:"));
            assert!(identities.insert(identity.clone()));
            assert_eq!(
                declarations
                    .symbol_reference(
                        &mut output,
                        root,
                        symbol,
                        StaticArgumentRole::ValueDependency
                    )
                    .unwrap(),
                selected
            );
            let stale = SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
            assert!(
                declarations
                    .symbol_reference(
                        &mut output,
                        root,
                        stale,
                        StaticArgumentRole::ValueDependency
                    )
                    .is_err()
            );
        }
        let error = declarations
            .symbol_reference(
                &mut output,
                root,
                counterfeit,
                StaticArgumentRole::ValueDependency,
            )
            .unwrap_err();
        assert!(
            error
                .message
                .contains("no retained source/package provenance")
        );
        assert!(declarations.sources.is_empty());
        assert!(output.declarations.is_empty());
    }

    #[test]
    fn source_declaration_rows_keep_symbol_generations_and_arena_bounds() {
        let original = SymbolHandle::from_parts(1, 1);
        let replacement = SymbolHandle::from_parts(1, 2);
        let mut rows = SymbolRows::new(2);
        rows.insert(original, "declared identity").unwrap();
        assert_eq!(rows.get(&original), Some(&"declared identity"));
        assert_eq!(rows.get(&replacement), None);
        assert!(rows.insert(replacement, "other identity").is_err());
        assert_eq!(rows.get(&original), Some(&"declared identity"));
        assert!(
            rows.insert(SymbolHandle::from_parts(3, 1), "outside")
                .is_err()
        );
        assert_eq!(rows.rows.len(), 1);
    }
}

#[cfg(test)]
mod invocation_tests {
    use super::*;
    use std::sync::Arc;

    fn named_calls() -> AcceptedTerminalSource {
        const TEXT: &str = r#"
trait FirstRequirement {
    machine pick(value: u64) -> u64 reaches none crashes none;
}
First satisfies FirstRequirement {
    machine pick(value: u64) -> u64 { value }
}
trait SecondRequirement {
    machine pick(value: u64) -> u64 reaches none crashes none;
}
Second satisfies SecondRequirement {
    machine pick(value: u64) -> u64 { value }
}
machine entry() -> u64 crashes none {
    let first: u64 = First::pick(7);
    let repeated: u64 = First::pick(first);
    Second::pick(repeated)
}
"#;
        accepted_source(TEXT, &[])
    }

    fn accepted_source(text: &str, metadata_names: &[&str]) -> AcceptedTerminalSource {
        accepted_source_with_core(text, metadata_names, &[])
    }

    fn accepted_source_with_publication(
        text: &str,
        metadata_names: &[&str],
        publication: terminal_codec::TerminalPublication,
    ) -> AcceptedTerminalSource {
        accepted_source_with_inputs(text, metadata_names, &[], publication)
    }

    fn accepted_source_with_core(
        text: &str,
        metadata_names: &[&str],
        core: &[(&str, &str, &[u8])],
    ) -> AcceptedTerminalSource {
        accepted_source_with_inputs(
            text,
            metadata_names,
            core,
            terminal_codec::TerminalPublication::Invocation,
        )
    }

    fn accepted_source_with_inputs(
        text: &str,
        metadata_names: &[&str],
        core: &[(&str, &str, &[u8])],
        publication: terminal_codec::TerminalPublication,
    ) -> AcceptedTerminalSource {
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add_checked_instance(
                "selection/main.omg".into(),
                text,
                "selection".into(),
                Some(semantic_vocabulary::PackageKeyIdentity::from_digest([0xc3; 32]).unwrap()),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let mut syntax =
            crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../../source/library/core");
        let mut bindings = Vec::new();
        for &(file, declaration, bytes) in core {
            let text = std::str::from_utf8(bytes).unwrap();
            let source = sources
                .add_with_metadata(
                    root.join(file),
                    text.to_owned(),
                    root.clone(),
                    None,
                    source_admission::SourceOrigin::Toolchain,
                )
                .source_id;
            let tokens = crate::frontend_fixture::Lexer::new(text)
                .tokenize()
                .unwrap();
            crate::frontend_fixture::parse(&mut syntax, source, &tokens).unwrap();
            bindings.push(
                crate::source_symbols::SourceScopedTopLevelBinding::logical_module_import(
                    id,
                    source,
                    &format!("omega::language::core::{declaration}"),
                    3,
                ),
            );
        }
        let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: bindings,
            retained_base: None,
        })
        .unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "entry")
            .unwrap()
            .symbol;
        let metadata = metadata_names
            .iter()
            .map(|name| {
                typed
                    .traits()
                    .iter()
                    .find(|declaration| declaration.name.as_str() == *name)
                    .expect("the fixture names its actual declaration")
                    .symbol
            })
            .collect::<Vec<_>>();
        let mut selected = metadata.clone();
        for &symbol in &metadata {
            let owner = typed.trait_definition_by_symbol(symbol).unwrap();
            selected.extend(
                typed
                    .trait_machine_signatures(owner)
                    .iter()
                    .map(|signature| signature.symbol),
            );
        }
        let frozen = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
        crate::judge_fixture(
            frozen,
            crate::lowering::Compilation {
                declaration_metadata: &metadata,
                entries: &[entry],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &selected,
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            publication,
        )
        .unwrap()
    }

    fn closed_requirement_formed_in_build(
        calls_build: bool,
    ) -> (
        crate::lowering::PreparedTypedTrees,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) {
        use crate::declaration_selection::ProductDeclarationOperand;
        use crate::declaration_selection::product_operands::{
            ProductStaticApplication, ProductStaticOperand, ProductStaticOperandGraph,
        };
        const BUILD: &str = "public machine build() { let four: u64 = 4; }";
        let product = format!(
            r#"
boundary trait Probe {{
    machine take<const Limit: u64>(value: u32) -> u32
    reaches none requires value <= Limit crashes none;
}}
machine entry() -> u32 crashes none {{
    {}
    let first: u32 = Probe::take::<4>(3);
    let second: u32 = Probe::take::<8>(7);
    first
}}
"#,
            if calls_build { "build();" } else { "" }
        );
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xc4; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let mut syntax = None;
        let mut build_source = None;
        for (path, text, scope) in [
            (
                "scope/build.omg",
                BUILD,
                source_provenance::DependencyScope::Build,
            ),
            (
                "scope/main.omg",
                product.as_str(),
                source_provenance::DependencyScope::Product,
            ),
        ] {
            let id = sources
                .add_checked_instance(
                    path.into(),
                    text,
                    "scope".into(),
                    Some(package),
                    source_admission::SourceOrigin::User,
                    source_admission::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            if scope == source_provenance::DependencyScope::Build {
                build_source = Some(id);
            }
            let tokens = crate::frontend_fixture::Lexer::new(text)
                .tokenize()
                .unwrap();
            match &mut syntax {
                None => {
                    syntax = Some(
                        crate::frontend_fixture::parse_syntax_trees_with_id(
                            id, &tokens,
                        )
                        .unwrap(),
                    )
                }
                Some(syntax) => {
                    crate::frontend_fixture::parse(syntax, id, &tokens).unwrap();
                }
            }
        }
        let mut request = crate::resolution::ResolutionRequest::new(syntax.unwrap());
        request.sources = Some(Arc::new(sources));
        let resolved = crate::resolution::resolve(request).unwrap();
        let mut program = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let author = program.realized_machine_named("build").unwrap().symbol;
        let (literal, span) = program
            .expression_table
            .iter_expressions()
            .find_map(|(expression, node)| {
                let crate::source_state::expression::ExpressionNode::Integer(value) = node else {
                    return None;
                };
                let span = program.expression_table.source_span(expression);
                (Some(span.source_id) == build_source && value.value_u64() == Some(4))
                    .then(|| (value.clone(), span))
            })
            .unwrap();
        let mut graph = ProductStaticOperandGraph::default();
        let value = graph.operands.insert(ProductStaticOperand {
            integer: Some(literal),
            ..Default::default()
        });
        let arguments = graph.static_arguments.insert_many([value]);
        let application = graph.applications.insert(ProductStaticApplication {
            arguments,
            ..Default::default()
        });
        graph.root = graph.operands.insert(ProductStaticOperand {
            application,
            ..Default::default()
        });
        let operand = ProductDeclarationOperand {
            path: "Probe::take".into(),
            source_span: span,
            application: Some(Arc::new(graph)),
        };
        let input = program.clone();
        let lookups = crate::source_state::product_operand::ProductSourceLookups::new(&input);
        let prepared = typed_trees::product_operand::formation::prepare_product_requirement_operand(
            &mut program,
            &lookups,
            &operand,
            author,
            None,
        )
        .unwrap();
        let application = program
            .closed_requirement_application(prepared.closed_requirement.unwrap())
            .unwrap()
            .application;
        assert_eq!(
            program
                .expression_table
                .static_machine_application(application)
                .lexical_owner,
            author
        );
        let program = crate::preparation::prepare_typed_trees(program, &[], &[]).unwrap();
        (program, application)
    }

    fn accept_formation_program(
        program: crate::lowering::PreparedTypedTrees,
    ) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
        let entry = program
            .trees()
            .realized_machine_named("entry")
            .unwrap()
            .symbol;
        let package = program
            .trees()
            .symbols
            .symbol_package_identity(entry)
            .unwrap();
        crate::judge_fixture(
            program,
            crate::lowering::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[Some(package)],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    }

    #[test]
    fn static_formation_scope_is_metadata_not_a_product_selection() {
        let (program, _) = closed_requirement_formed_in_build(false);
        let source = accept_formation_program(program).unwrap();
        let prepared = prepare(&source, source.module(), None).unwrap();
        psi_authorization::selection_questions::admit_selection_questions(
            source.module(),
            &prepared.questions,
            &psi_authorization::selection_questions::SelectionAuthorities {
                build: None,
                product: None,
            },
        )
        .unwrap();
        let (build, _) = prepared
            .questions
            .declarations
            .iter()
            .find(|(_, row)| row.name == "build" && row.purpose == SelectionPurpose::Build)
            .expect("formation source stays in the captured inventory");
        assert!(
            !prepared
                .questions
                .questions
                .iter()
                .any(
                    |(_, question)| question.purpose == SelectionPurpose::Product
                        && question.selected == SelectionReference::Declaration(build)
                )
        );

        // A written call still owes visibility before it reaches admission.
        let (program, _) = closed_requirement_formed_in_build(true);
        let sources = program.trees().symbols.source_map().unwrap();
        let errors = accept_formation_program(program).err().unwrap();
        assert_eq!(errors.len(), 1, "{errors:?}");
        let refused = &errors[0];
        assert!(
            refused
                .message
                .contains("this call has no visible declaration in its source scope"),
            "{refused:?}"
        );
        assert_eq!(sources.text_at(refused.source_span.unwrap()), "build");

        for owner in [
            SymbolHandle::invalid(),
            SymbolHandle::from_parts(u32::MAX, 1),
        ] {
            let (program, application) = closed_requirement_formed_in_build(false);
            let mut program = program.into_inner();
            program
                .expression_table
                .static_machine_application_mut(application)
                .lexical_owner = owner;
            let prepared = crate::preparation::prepare_typed_trees(program, &[], &[]).unwrap();
            let refusal = accept_formation_program(prepared).err().unwrap();
            assert!(
                format!("{refusal:?}")
                    .contains("closed requirement call lost its supplying declaration"),
                "{refusal:?}"
            );
        }
    }

    #[test]
    fn generated_cleanup_calls_keep_the_carriers_actual_selection() {
        let source = accepted_source(
            r#"
data First dropped by end_first { value: u64; }
data Second dropped by end_second { value: u64; }
machine end_first(owned: First) { let { value as _ } = move owned; }
machine end_second(owned: Second) { let { value as _ } = move owned; }
machine entry() {
    let first: First = First { value: 7u64 };
    let second: Second = Second { value: 7u64 };
}
"#,
            &[],
        );
        let module = source.module();
        let prepared = prepare(&source, module, None).expect("cleanup choices must publish");
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .expect("each generated call must retain its carrier author's exact selection");
        let calls = prepared
            .questions
            .joins
            .iter()
            .enumerate()
            .filter_map(|(position, join)| match join {
                SelectionJoin::Invocation {
                    question,
                    original_caller,
                    site: CallableInvocationSite::CleanupAction { .. },
                    target,
                    ..
                } => Some((position, *question, *original_caller, *target)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [first, second] = calls.as_slice() else {
            panic!("two exact cleanup calls: {calls:?}")
        };
        assert_ne!(first.1, second.1);
        assert_ne!(first.2, second.2);
        assert_ne!(first.3, second.3);

        let mut wrong_author = prepared.questions.clone();
        wrong_author
            .questions
            .get_mut(first.1)
            .unwrap()
            .occurrence
            .lexical_owner = second.2;
        let SelectionJoin::Invocation {
            original_caller, ..
        } = &mut wrong_author.joins[first.0]
        else {
            unreachable!()
        };
        *original_caller = second.2;

        let mut wrong_target = prepared.questions.clone();
        let SelectionJoin::Invocation { target, .. } = &mut wrong_target.joins[first.0] else {
            unreachable!()
        };
        *target = second.3;

        let mut wrong_application = prepared.questions.clone();
        let structures = wrong_application
            .joins
            .iter()
            .enumerate()
            .filter_map(|(position, join)| match join {
                SelectionJoin::Structure {
                    declaration,
                    storage_identity,
                    ..
                } if *declaration == first.2 || *declaration == second.2 => {
                    Some((position, *declaration, storage_identity.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(structures.len(), 2, "two carrier applications");
        let (first_structure, _, _) = structures
            .iter()
            .find(|(_, declaration, _)| *declaration == first.2)
            .unwrap();
        let (_, _, second_identity) = structures
            .iter()
            .find(|(_, declaration, _)| *declaration == second.2)
            .unwrap();
        let SelectionJoin::Structure {
            storage_identity, ..
        } = &mut wrong_application.joins[*first_structure]
        else {
            unreachable!()
        };
        *storage_identity = second_identity.clone();

        // Collect every verdict before asserting, so a failure reports all
        // mismatched ownership and target relationships in one test run.
        let cases = [
            (
                "carrier author",
                wrong_author,
                "cleanup selection differs from its exact carrier application or author",
            ),
            (
                "selected target",
                wrong_target,
                "invocation selection differs from its actual emitted target",
            ),
            (
                "carrier application",
                wrong_application,
                "source selection structure differs from its applied storage identity",
            ),
        ];
        let observations = cases
            .into_iter()
            .map(|(name, packet, expected)| {
                (
                    name,
                    expected,
                    psi_authorization::selection_questions::validate_selection_questions(
                        module, &packet,
                    ),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            observations.iter().all(|(_, expected, result)| {
                result
                    .as_ref()
                    .is_err_and(|reason| reason.contains(expected))
            }),
            "{observations:?}"
        );
    }

    #[test]
    fn recursive_declarations_keep_both_entry_and_folded_head_origins() {
        let source = accepted_source(
            r#"
machine repeat(index: u64) {
    transition index < 3 {
        true -> repeat(index + 1),
        _ -> {}
    }
}
machine entry() { repeat(0); }
"#,
            &[],
        );
        let module = source.module();
        let prepared = prepare(&source, module, None).unwrap();
        let validate = |questions: &SelectionQuestions| {
            psi_authorization::selection_questions::validate_selection_questions(module, questions)
        };
        validate(&prepared.questions).unwrap();
        let (machine, declaration, head) = prepared
            .questions
            .joins
            .iter()
            .find_map(|join| match join {
                SelectionJoin::FoldedCall {
                    machine,
                    original_target,
                    head,
                    ..
                } => Some((*machine, *original_target, *head)),
                _ => None,
            })
            .expect("the real recursive call must retain its folded target");
        let entry = module.machine(machine).unwrap().entry;
        assert_ne!(entry, head);
        let is_origin = |join: &SelectionJoin, expected| {
            matches!(join, SelectionJoin::Machine {
                declaration: source, machine: body, head,
            } if *source == declaration && *body == machine && *head == expected)
        };
        assert!(
            prepared
                .questions
                .joins
                .iter()
                .any(|join| is_origin(join, entry))
        );
        let origin = prepared
            .questions
            .joins
            .iter()
            .find(|join| is_origin(join, head))
            .unwrap()
            .clone();
        let mut duplicate = prepared.questions.clone();
        duplicate.joins.push(origin);
        assert!(
            validate(&duplicate)
                .unwrap_err()
                .contains("repeats a machine origin")
        );
        let mut missing_head = prepared.questions.clone();
        missing_head.joins.retain(|join| !is_origin(join, head));
        assert!(
            validate(&missing_head)
                .unwrap_err()
                .contains("folded selection lost its original caller, target or actual head")
        );
    }

    #[test]
    fn specialized_callbacks_publish_the_written_declaration_with_its_applied_meaning() {
        let source = accepted_source(
            r#"
public data Observe = machine(value: u64) -> u64 reaches none crashes none;
machine prepare() reaches none crashes none {}
machine read(value: u64) -> u64 reaches none crashes none { value }
public machine choose<machine Prepare>(operation: Observe, value: u64) -> u64
requires machine Prepare() crashes none;
{ Prepare(); operation(value) }
machine entry() -> u64 crashes none { choose::<prepare>(read, 47u64) }
"#,
            &[],
        );
        let module = source.module();
        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();

        let typed = source.source();
        let mut reapplied = 0;
        for (handle, contract) in typed.runtime_callable_contracts.iter() {
            let symbol = contract.signature.symbol;
            let origin = typed.symbols.get(symbol).generated_from;
            if !origin.is_valid()
                || typed.symbols.get(origin).kind != SymbolKind::CallableContract
                || typed.symbols.get(origin).generated_from.is_valid()
            {
                continue;
            }
            let meaning = typed
                .runtime_callable_contract_identity(handle, &[])
                .unwrap();
            let original = typed.normalized_hermetic_symbol_identity(origin).unwrap();
            let applied = typed.normalized_hermetic_symbol_identity(symbol).unwrap();
            let written = callable_declaration_path(&original, &meaning);
            let generated = callable_declaration_path(&applied, &meaning);
            assert_ne!(
                written, generated,
                "the fixture must distinguish the fresh symbol"
            );
            let rows = &prepared.questions.declarations;
            if rows.iter().any(|(_, row)| row.identity == written) {
                assert!(
                    !rows.iter().any(|(_, row)| row.identity == generated),
                    "the compiler-created binder must not become a second authored declaration"
                );
                reapplied += 1;
            }
        }
        assert!(
            reapplied > 0,
            "the concrete invocation must retain an applied callable"
        );
    }

    #[test]
    fn anonymous_callable_occurrences_keep_distinct_source_identity() {
        const TEXT: &str = r#"
trait Pick {
    machine pick(value: u64) -> u64 reaches none crashes none
    requires value <= 100 ensures result <= value;
}
Identity satisfies Pick {
    machine pick(value: u64) -> u64 reaches none crashes none
    requires value <= 100 ensures result <= value { value }
}
data TransformValue = machine(value: u64) -> u64 reaches none crashes none
    requires value <= 100 ensures result <= value;
data TransformAmount = machine(amount: u64) -> u64 reaches none crashes none
    requires amount <= 100 ensures result <= amount;
machine apply(operation: TransformValue, input: u64) -> u64
reaches none crashes none requires input <= 100 ensures result <= input {
    operation(input)
}
machine entry() -> u64 reaches none crashes none {
    let selected: TransformAmount = Identity::pick;
    apply(selected, 7)
}
"#;
        let source = accepted_source(TEXT, &[]);
        let typed = source.source();
        let occurrences = typed
            .runtime_callable_contracts
            .iter()
            .filter_map(|(handle, contract)| {
                let symbol = contract.signature.symbol;
                if typed.symbols.get(symbol).kind != SymbolKind::CallableContract
                    || typed.symbols.get(contract.lexical_owner).kind == SymbolKind::TypeAlias
                {
                    return None;
                }
                let parameters = typed.state_signature_parameters(&contract.signature);
                let [formal] = parameters.as_array()?;
                matches!(formal.name.as_str(), "value" | "amount").then_some((
                    symbol,
                    (handle, formal.symbol, formal.name.as_str().to_owned()),
                ))
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(occurrences.len(), 2);
        let mut occurrences = occurrences.into_iter();
        let (first, (first_contract, first_formal, first_name)) = occurrences.next().unwrap();
        let (second, (second_contract, second_formal, second_name)) = occurrences.next().unwrap();
        assert_ne!(first, second);
        assert_ne!(first_formal, second_formal);
        assert_ne!(first_name, second_name);
        assert_ne!(
            typed.symbols.symbol_provenance_source_span(first).unwrap(),
            typed.symbols.symbol_provenance_source_span(second).unwrap()
        );
        let identities = [first, second]
            .map(|symbol| typed.normalized_hermetic_symbol_identity(symbol).unwrap());
        assert_ne!(identities[0], identities[1]);
        let meanings = [first_contract, second_contract].map(|contract| {
            typed
                .runtime_callable_contract_identity(contract, &[])
                .unwrap()
        });
        assert_eq!(meanings[0], meanings[1]);

        // Adding a comment after the types must leave their identities unchanged.
        let revised_text = format!("{TEXT}\n// An unrelated trailing source comment.\n");
        let revised = accepted_source(&revised_text, &[]);
        let revised_typed = revised.source();
        let revised_occurrences = revised_typed
            .runtime_callable_contracts
            .iter()
            .filter_map(|(handle, contract)| {
                let symbol = contract.signature.symbol;
                if revised_typed.symbols.get(symbol).kind != SymbolKind::CallableContract
                    || revised_typed.symbols.get(contract.lexical_owner).kind == SymbolKind::TypeAlias
                {
                    return None;
                }
                let parameters = revised_typed.state_signature_parameters(&contract.signature);
                let [formal] = parameters.as_array()?;
                if !matches!(formal.name.as_str(), "value" | "amount") {
                    return None;
                }
                Some((
                    formal.name.as_str().to_owned(),
                    (
                        revised_typed
                            .normalized_hermetic_symbol_identity(symbol)
                            .unwrap(),
                        revised_typed
                            .runtime_callable_contract_identity(handle, &[])
                            .unwrap(),
                    ),
                ))
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(revised_occurrences.len(), 2);
        for (name, identity, meaning) in [
            (&first_name, &identities[0], &meanings[0]),
            (&second_name, &identities[1], &meanings[1]),
        ] {
            let (revised_identity, revised_meaning) = revised_occurrences.get(name).unwrap();
            assert_eq!(identity, revised_identity);
            assert_eq!(meaning, revised_meaning);
        }

        let module = source.module();
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| operation.kind.callable_invocation().is_some()),
            "the source must retain a runtime call through its callable parameter"
        );
        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
        let rows = [0, 1].map(|index| {
            let identity = callable_declaration_path(&identities[index], &meanings[index]);
            let rows = prepared
                .questions
                .declarations
                .iter()
                .filter(|(_, row)| row.identity.as_str() == identity.as_str())
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1);
            rows[0]
        });
        assert_ne!(rows[0].0, rows[1].0);
        assert_eq!(rows[0].1.callable_identity.as_ref(), Some(&meanings[0]));
        assert_eq!(rows[1].1.callable_identity.as_ref(), Some(&meanings[1]));
        let mut duplicate = prepared.questions.clone();
        duplicate.declarations.get_mut(rows[1].0).unwrap().identity = rows[0].1.identity.clone();
        assert!(
            psi_authorization::selection_questions::validate_selection_questions(
                module, &duplicate,
            )
            .unwrap_err()
            .contains("source selection dictionary repeats a declaration in one purpose"),
            "equal callable meaning must not permit a duplicated source declaration"
        );
    }

    #[test]
    fn domain_declarations_keep_their_subjects_without_splitting_their_meaning() {
        let source = accepted_source_with_publication(
            r#"
domain [u8; 2]::Ready requires true;
domain [u8; 3]::Ready requires true;
machine entry() {
    let first: [u8; 2] in Ready = [1u8, 2u8] as +Ready;
    let second: [u8; 3] in Ready = [3u8, 4u8, 5u8] as +Ready;
}
"#,
            &[],
            terminal_codec::TerminalPublication::Portable,
        );
        let typed = source.source();
        let domains = typed
            .domain_definitions()
            .iter()
            .filter(|domain| domain.name.as_str().ends_with("::Ready"))
            .collect::<Vec<_>>();
        assert_eq!(
            domains.len(),
            2,
            "the fixture must retain both qualified Ready declarations: {:?}",
            typed
                .domain_definitions()
                .iter()
                .map(|domain| domain.name.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            domains[0].semantic_id, domains[1].semantic_id,
            "the same normalized predicate keeps its meaning across capacities"
        );
        let identities = domains
            .iter()
            .map(|domain| {
                typed
                    .normalized_hermetic_symbol_identity(domain.symbol)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_ne!(identities[0], identities[1]);
        let exact_sources = source_admission::source_identity::toolchain_source_identities(
            typed.symbols.source_files(),
        )
        .unwrap();
        let nominal = domains
            .iter()
            .map(|domain| {
                typed
                    .exact_owner_nominal_identity(domain.symbol, &exact_sources)
                    .and_then(|identity| typed.type_identity_text(identity))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_ne!(nominal[0], nominal[1]);
        let module = source.module();
        let bytes = terminal_codec::encode_module(module).unwrap();
        let decoded = terminal_codec::decode_module(&bytes).unwrap();
        assert_eq!(module.types.structural_domains, decoded.types.structural_domains);
        assert_eq!(module.types.structural_domains.len(), 2);
        let mut collapsed = module.clone();
        collapsed.types.structural_domains[1].identity = collapsed.types.structural_domains[0].identity;
        let invalid = terminal_codec::encode_module(&collapsed).unwrap();
        assert!(matches!(
            terminal_codec::decode_module(&invalid),
            Err(terminal_codec::CodecError::MalformedStructuralFoundation(
                "structural domain identity"
            ))
        ));
        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
        let rows = prepared
            .questions
            .declarations
            .iter()
            .filter_map(|(id, row)| identities.contains(&row.identity).then_some(id))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        let mut duplicate = prepared.questions.clone();
        duplicate.declarations.get_mut(rows[1]).unwrap().identity = prepared
            .questions
            .declarations
            .get(rows[0])
            .unwrap()
            .identity
            .clone();
        assert!(
            psi_authorization::selection_questions::validate_selection_questions(
                module, &duplicate
            )
            .is_err(),
            "different subjects must not bypass the duplicate declaration guard"
        );
    }

    #[test]
    fn requirement_overloads_keep_their_signatures_and_refuse_header_swaps() {
        // Publish both complete headers directly. A signature-free named call
        // would test overload selection before reaching this identity owner.
        let source = accepted_source(
            r#"
boundary trait Input {
    machine pick(value: u64) -> u64 reaches none crashes none;
    machine pick(value: bool) -> bool reaches none crashes none;
}
machine entry() reaches Input {}
"#,
            &["Input"],
        );
        let module = source.module();
        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
        let typed = source.source();
        let owner = typed
            .traits()
            .iter()
            .find(|owner| owner.name.as_str() == "Input")
            .unwrap();
        let identities = typed
            .trait_machine_signatures(owner)
            .iter()
            .map(|signature| {
                typed
                    .normalized_hermetic_symbol_identity(signature.symbol)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(identities.len(), 2);
        assert_ne!(identities[0], identities[1]);
        let headers = prepared
            .questions
            .joins
            .iter()
            .filter_map(|join| match join {
                SelectionJoin::Boundary {
                    declaration,
                    boundary,
                } => Some((*declaration, *boundary)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(headers.len(), 2);
        assert_ne!(headers[0].0, headers[1].0);
        for (original, other) in [(headers[0], headers[1]), (headers[1], headers[0])] {
            let mut changed = prepared.questions.clone();
            let joined = changed
                .joins
                .iter_mut()
                .find_map(|join| match join {
                    SelectionJoin::Boundary {
                        declaration,
                        boundary,
                    } if *boundary == original.1 => Some(declaration),
                    _ => None,
                })
                .unwrap();
            *joined = other.0;
            let error = psi_authorization::selection_questions::validate_selection_questions(
                module, &changed,
            )
            .unwrap_err();
            assert!(
                error.contains("boundary differs from its checked semantic declaration"),
                "{error}"
            );
        }
    }

    #[test]
    fn describing_a_formal_does_not_make_its_argument_value_visible() {
        let source = named_calls();
        let published = source
            .produce(
                crate::TerminalProductionCustody::default(),
                &mut artifacts::compile_timings::CompileTimings::default(),
            )
            .unwrap();
        assert!(published.view().module().entry.is_some());
        let typed = source.source();
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "entry")
            .unwrap()
            .symbol;
        let callee = typed
            .machines()
            .iter()
            .find(|machine| typed.symbols.display_path(machine.symbol, "::") == "First::pick")
            .unwrap();
        let state = typed.machine_states(callee).first().unwrap();
        let formal = typed
            .tables
            .state_parameters
            .span_or_empty(state.parameters)
            .first()
            .unwrap()
            .symbol;
        let mut declarations = Declarations::new(&source, source.module()).unwrap();
        let mut questions = SelectionQuestions::default();
        let description = declarations
            .symbol_reference(
                &mut questions,
                entry,
                formal,
                StaticArgumentRole::DeclarationDescription,
            )
            .unwrap();
        assert!(matches!(description, SelectionReference::Declaration(_)));
        let error = declarations
            .symbol_reference(
                &mut questions,
                entry,
                formal,
                StaticArgumentRole::ValueDependency,
            )
            .unwrap_err();
        assert!(
            error.message.contains("outside its actual lexical context"),
            "{error:?}"
        );
    }

    #[test]
    fn callable_alias_capture_keeps_distinct_concrete_applications() {
        let source = accepted_source(
            "public data KeyHash<K> = machine(key: @K) -> u64 reaches none crashes none;
             machine read_small(value: @u32) -> u64 reaches none crashes none { 5u64 }
             machine read_large(value: @u64) -> u64 reaches none crashes none { $value }
             machine small(f: KeyHash<u32>, value: @u32) -> u64 reaches none crashes none { f(value) }
             machine large(f: KeyHash<u64>, value: @u64) -> u64 reaches none crashes none { f(value) }
             machine entry() -> u64 reaches none crashes none {
                 let a: u32 = 11u32;
                 let b: u64 = 47u64;
                 small(read_small, @a) | large(read_large, @b)
             }",
            &[],
        );
        let prepared = prepare(&source, source.module(), None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            source.module(),
            &prepared.questions,
        )
        .unwrap();
        let meanings = prepared
            .questions
            .declarations
            .iter()
            .filter_map(|(_, declaration)| {
                (declaration.name == "KeyHash")
                    .then_some(declaration.callable_identity.as_deref())
                    .flatten()
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            meanings.len() >= 2,
            "the two selected callable applications must survive publication: {meanings:?}"
        );
    }

    #[test]
    fn callable_alias_capture_can_select_only_its_enclosing_type_binder() {
        let source = accepted_source(
            "public data KeyHash<K> = machine(key: @K) -> u64;
             public data Sibling<K> = machine(value: @K) -> bool;
             machine entry() {}",
            &[],
        );
        let typed = source.source();
        let alias = |name| {
            typed
                .type_aliases()
                .iter()
                .find(|row| row.name.as_str() == name)
                .unwrap()
        };
        let owner = alias("KeyHash");
        let sibling = alias("Sibling");
        let binder = |row: &crate::source_state::type_aliases::TypeAliasDefinition| {
            typed
                .data_type_parameters
                .span_or_empty(row.type_parameters)[0]
                .symbol
        };
        let crate::source_state::types::TypeReferenceNode::Callable { contract } =
            typed.type_reference_table.type_reference(owner.expansion)
        else {
            panic!("KeyHash must retain its source callable");
        };
        let scope = typed.runtime_callable_contract(*contract).signature.symbol;
        let mut declarations = Declarations::new(&source, source.module()).unwrap();
        let mut questions = SelectionQuestions::default();
        declarations
            .local_binding(&mut questions, scope, binder(owner), owner.symbol)
            .unwrap();
        let error = declarations
            .local_binding(&mut questions, scope, binder(sibling), sibling.symbol)
            .unwrap_err();
        assert!(
            error.message.contains("outside its actual lexical context"),
            "{error:?}"
        );
    }

    #[test]
    fn ordinary_and_theorem_applied_statements_retain_their_exact_source_calls() {
        use crate::source_state::{SourceNode, statement::StatementNode};
        use semantic_vocabulary::source_selections::SelectionCallTarget;

        let text = include_str!(
            "../../06_lowered-program-to-checked-program/tests/fixtures/proofs/positive/named_implication/main.omg"
        )
        .replace("    use_step::<", "    only_seven(7u8);\n    use_step::<");
        let source = accepted_source(&text, &[]);
        let module = source.module();
        let [citation] = module.proofs.theorem_citations.as_slice() else {
            panic!("the entry supplies one named theorem");
        };
        let caller = module.machine(citation.machine).unwrap();
        let calls: Vec<_> = caller.module_invocations().collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].site, citation.site);

        let trees = source.source();
        let entry = trees
            .machines()
            .iter()
            .find(|m| m.name.as_str() == "entry")
            .unwrap();
        let statements: Vec<_> = trees
            .machine_states(entry)
            .iter()
            .flat_map(|state| state.statement_nodes.handles())
            .filter(|&statement| {
                matches!(
                    trees.statement_table.statement(statement),
                    StatementNode::Call(_)
                )
            })
            .collect();
        assert_eq!(statements.len(), 2);
        let mut questions = Questions::new(&source, module).unwrap();
        for (call, statement) in calls.iter().zip(statements) {
            assert!(matches!(call.site, InvocationSite::Operation(_)));
            let origin = questions.typed_origin(caller.id, call.site).unwrap();
            assert_eq!(origin.source_member, caller.id);
            assert_eq!(origin.owner, entry.symbol);
            assert_eq!(origin.source, SourceNode::Statement(statement));
            questions
                .invocation_reference(
                    origin.owner,
                    origin.source,
                    SelectionCallTarget::Declared(TargetMachineCallable::Machine(
                        call.inputs.callee,
                    )),
                )
                .expect("the emitted callee is selected by that exact statement");
        }
    }

    #[test]
    fn a_hosted_boundary_call_keeps_its_actual_machine_entry_selection() {
        use semantic_vocabulary::source_selections::SelectionCallTarget;

        let source = accepted_source_with_core(
            "use omega\\language\\core\\ProcessExit; use omega\\language\\core\\ProcessExitPrimitive; machine entry() reaches ProcessExit crashes none terminates { ProcessExitPrimitive(7); }",
            &[],
            &bundled_declarations::sealed_intrinsics::hosted_process_exit_sources(),
        );
        let module = source.module();
        let primitive = module
            .interfaces.boundary_machines
            .iter()
            .find(|boundary| {
                matches!(
                    boundary
                        .declaration
                        .as_ref()
                        .map(|declaration| declaration.execution),
                    Some(terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. })
                )
            })
            .expect("the real primitive is published as a boundary machine");
        let requirement = primitive.external_completion_requirement();
        assert_ne!(primitive.id, requirement);
        let (caller, site) = module
            .machines
            .iter()
            .find_map(|machine| {
                machine.blocks.iter().find_map(|block| {
                    let (site, call) = block.terminator.declared_invocation()?;
                    (call.callee == TargetMachineCallable::Boundary(primitive.id))
                        .then_some((machine.id, site))
                })
            })
            .expect("the actual terminal call retains its primitive boundary");
        let mut questions = Questions::new(&source, module).unwrap();
        let origin = questions.typed_origin(caller, site).unwrap();
        let selected = questions
            .source_target(TargetMachineCallable::Boundary(primitive.id))
            .unwrap();
        let machine = source.source().machine_by_symbol(selected).unwrap();
        let entry = source
            .source()
            .machine_states(machine)
            .first()
            .unwrap()
            .symbol;
        assert_ne!(entry, selected);
        let crate::source_state::SourceNode::Statement(statement) = origin.source else {
            panic!("the primitive call is an authored statement");
        };
        let crate::source_state::statement::StatementNode::Call(call) =
            source.source().statement_table.statement(statement)
        else {
            panic!("the source occurrence remains a call");
        };
        assert_eq!(call.target_symbol, entry);
        questions
            .invocation_reference(
                origin.owner,
                origin.source,
                SelectionCallTarget::Declared(TargetMachineCallable::Boundary(primitive.id)),
            )
            .expect("the exact entry and emitted boundary select one declaration");
        let error = questions
            .invocation_reference(
                origin.owner,
                origin.source,
                SelectionCallTarget::Declared(TargetMachineCallable::Boundary(requirement)),
            )
            .unwrap_err();
        assert!(
            error
                .message
                .contains("differs from its exact source selection")
        );

        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
    }

    #[test]
    fn copied_record_calls_keep_the_exact_operand_supplier_and_schema() {
        use semantic_vocabulary::source_selections::SelectionCallTarget;

        let text = include_str!(
            "../../../../../../../tests/fixtures/calls/run/record_callback_loop/main.omg"
        )
        .replace("machine main()", "machine entry()");
        let source = accepted_source(&text, &[]);
        let module = source.module();
        let prepared = prepare(&source, module, None)
            .expect("copied calls retain the enclosing record argument supplier");
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
        let (caller, operation, header, target) = module
            .machines
            .iter()
            .find_map(|machine| {
                machine.operations().find_map(|operation| {
                    let call = operation.declared_invocation()?;
                    Some((
                        machine.id,
                        operation.id,
                        call.required_contract?,
                        call.callee,
                    ))
                })
            })
            .expect("the loop calls the selected operation through its public header");
        let TargetMachineCallable::Machine(body) = target else {
            panic!("the selected record operation has a checked machine body");
        };
        let origin = Questions::new(&source, module)
            .unwrap()
            .typed_origin(caller, InvocationSite::Operation(operation))
            .unwrap();
        let key = (origin.source_member, header, body);
        let retained = source
            .closure()
            .elaboration()
            .selections
            .iter()
            .find(|selection| {
                (
                    selection.caller,
                    selection.required_contract,
                    selection.implementation,
                ) == (key.0, Some(key.1), key.2)
            })
            .unwrap();
        assert!(
            retained
                .source_operand
                .as_ref()
                .unwrap()
                .static_record
                .is_some()
        );
        for changed_field in 0..4 {
            let mut changed = retained.clone();
            let operand = changed.source_operand.as_mut().unwrap();
            match changed_field {
                0 => operand.supplier = origin.owner,
                1 => operand.identity.push_str("changed operand"),
                2 => operand.static_record.as_mut().unwrap().parameter = SymbolHandle::invalid(),
                3 => operand
                    .static_record
                    .as_mut()
                    .unwrap()
                    .application
                    .push_str("changed schema"),
                _ => unreachable!(),
            }
            let mut questions = Questions::new(&source, module).unwrap();
            questions.requirements.insert(key, &changed);
            let error = questions
                .retain_invocation(
                    caller,
                    InvocationSite::Operation(operation),
                    SelectionCallTarget::Declared(target),
                    Some(header),
                )
                .unwrap_err();
            let expected = if changed_field < 2 {
                "changes its retained implementation operand or supplier"
            } else {
                "changes its retained record parameter or schema"
            };
            assert!(
                error.message.contains(expected),
                "{changed_field}: {error:?}"
            );
        }
    }

    #[test]
    fn named_calls_keep_their_public_header_and_exact_body() {
        use semantic_vocabulary::source_selections::SelectionCallTarget;

        let source = named_calls();
        let module = source.module();
        let prepared = prepare(&source, module, None).unwrap();
        psi_authorization::selection_questions::validate_selection_questions(
            module,
            &prepared.questions,
        )
        .unwrap();
        let calls = module
            .machines
            .iter()
            .flat_map(|machine| {
                machine.operations().filter_map(move |operation| {
                    let call = operation.declared_invocation()?;
                    Some((
                        machine.id,
                        operation.id,
                        call.required_contract?,
                        call.callee,
                    ))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        let mut bodies = Vec::new();
        for &(machine, operation, _, target) in &calls {
            let question = prepared
                .questions
                .joins
                .iter()
                .find_map(|join| match join {
                    SelectionJoin::Invocation {
                        question,
                        machine: caller,
                        site: CallableInvocationSite::Invocation(InvocationSite::Operation(site)),
                        target: SelectionCallTarget::Declared(body),
                        ..
                    } if *caller == machine && *site == operation && *body == target => {
                        Some(*question)
                    }
                    _ => None,
                })
                .unwrap();
            let SelectionReference::Declaration(selected) =
                prepared.questions.questions.get(question).unwrap().selected
            else {
                panic!("a named call must retain its exact source body");
            };
            bodies.push(
                prepared
                    .questions
                    .declarations
                    .get(selected)
                    .unwrap()
                    .name
                    .as_str(),
            );
        }
        bodies.sort_unstable();
        assert_eq!(bodies, ["First::pick", "First::pick", "Second::pick"]);

        let (caller, operation, header, target) = calls[0];
        let TargetMachineCallable::Machine(body) = target else {
            panic!("the named call must have a machine body");
        };
        let origin = Questions::new(&source, module)
            .unwrap()
            .typed_origin(caller, InvocationSite::Operation(operation))
            .unwrap();
        let key = (origin.source_member, header, body);
        let retained = source
            .closure()
            .elaboration()
            .selections
            .iter()
            .find(|selection| {
                (
                    selection.caller,
                    selection.required_contract,
                    selection.implementation,
                ) == (key.0, Some(key.1), key.2)
            })
            .unwrap();
        for change_supplier in [false, true] {
            let mut changed = retained.clone();
            let operand = changed.source_operand.as_mut().unwrap();
            if change_supplier {
                operand.supplier = *source.closure().elaboration().machines.get(body).unwrap();
            } else {
                operand.identity.push_str("different operand");
            }
            let mut questions = Questions::new(&source, module).unwrap();
            questions.requirements.insert(key, &changed);
            let error = questions
                .retain_invocation(
                    caller,
                    InvocationSite::Operation(operation),
                    SelectionCallTarget::Declared(target),
                    Some(header),
                )
                .unwrap_err();
            assert!(
                error
                    .message
                    .contains("changes its retained implementation operand or supplier"),
                "{error:?}"
            );
        }

        let (_, _, other_header, other_target) = calls
            .iter()
            .copied()
            .find(|call| call.3 != calls[0].3)
            .unwrap();
        for change_body in [false, true] {
            let mut changed = module.clone();
            let operation = changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == caller)
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|candidate| candidate.id == operation)
                .unwrap();
            if change_body {
                let OperationKind::Call { callee, .. } = &mut operation.kind else {
                    panic!("the scalar fixture must retain a declared scalar call");
                };
                let TargetMachineCallable::Machine(other) = other_target else {
                    panic!("the other named call must have a machine body");
                };
                *callee = other;
            } else {
                operation.required_contract = Some(other_header);
            }
            let error = prepare(&source, &changed, None).err().unwrap();
            assert!(
                error
                    .message
                    .contains("exact caller, public header and body association"),
                "{error:?}"
            );
        }
    }
}

#[cfg(test)]
mod core_catalog_tests {
    use super::*;
    use numerics::intrinsics::{CoreIntegerOperand, ScalarBinaryOperation};

    #[test]
    fn core_operator_declarations_keep_their_actual_applied_signatures() {
        let typed = TypedTrees::default();
        let mut declarations = Declarations {
            callable_declarations: HashMap::new(),
            typed: &typed,
            build_entry_source: None,
            ids: SymbolRows::new(0),
            identities: SymbolRows::new(0),
            contracts: SymbolRows::new(0),
            sources: Vec::new(),
            core_operators: BTreeMap::new(),
        };
        let mut output = SelectionQuestions::default();
        let carrier = numerics::integer_carrier::FixedInteger::signed(64);
        let plain = CoreIntegerOperand {
            carrier,
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
        };
        let qualified = CoreIntegerOperand {
            policy: numerics::arithmetic::ArithmeticDomain::Wrapping,
            ..plain
        };
        let selected = CoreIntegerOperatorSignature::select(
            ScalarBinaryOperation::GreaterOrEqual,
            [qualified, plain],
        )
        .unwrap();
        let reversed = CoreIntegerOperatorSignature::select(
            ScalarBinaryOperation::GreaterOrEqual,
            [plain, qualified],
        )
        .unwrap();
        let first = declarations
            .core_integer_operator(&mut output, selected)
            .unwrap();
        assert_eq!(
            declarations
                .core_integer_operator(&mut output, selected)
                .unwrap(),
            first
        );
        let second = declarations
            .core_integer_operator(&mut output, reversed)
            .unwrap();
        assert_ne!(first, second);
        let row = output.declarations.get(first).unwrap();
        assert_eq!(row.custody, DeclarationCustody::Toolchain);
        assert!(row.identity.starts_with("core-integer-policy-operator-v1:"));
        assert_eq!(row.callable_identity.as_ref(), Some(&row.identity));
        assert_ne!(
            row.identity,
            output.declarations.get(second).unwrap().identity
        );
    }
}
