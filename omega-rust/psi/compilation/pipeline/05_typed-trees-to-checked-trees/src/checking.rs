/*
    Immutable typed-tree checking: validate contracts and ownership, assemble
    execution plans, then publish checked trees. Preparation has already closed
    the executable bodies and call identities; this stage never repairs them.

    `lower_typed_trees` is the one entrance. `TypedPreparation` carries the
    producer's admission checkpoint and source judgments; `CheckingRequest`
    carries package declaration authority. Checking cannot change the prepared
    input's checkpoint. Every production request enforces crash admission.

    Whole-program reuse has one gate: the source facade compares the original
    program, complete preparation admission and entries callable from outside the
    program against a [`RetainedCheck`] from
    this invocation. A miss prepares and checks afresh. Equal prepared trees
    and Copy receipts do not establish equal checking inputs: selected opaque
    carriers travel separately and determine structural plans. A second partial
    match here would silently keep the preceding Build's carrier.

    Retention keeps that original input beside the shared checked result, not
    another prepared-program copy. Matching dependency-body entailments remain
    independently reusable during fresh checking; pending checks supply none.
    Reuse conveys no authority: preparation admits wire demands, completeness
    and asm, and the source facade rejoins declaration and access authority
    for each request. No process-global memo supplies an implicit judgment.
*/

pub(crate) mod call_acknowledgements {
    use diagnostics::Diagnostic;
    use flow_effects::{CallOperational, OperationalPlan};
    use symbols::JoinedPath;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    /*
        Validate the source acknowledgement set against the call envelope already
        normalized by `flow-effects`, and reject suspension in a nested expression
        before continuation planning sees it.
    */
    pub(crate) fn validate_call_acknowledgements(
        program: &dyn ::validation::CheckingInput,
        operational: &OperationalPlan,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let Some(operational_state) = operational
                    .machines()
                    .iter()
                    .flat_map(|machine| operational.states.span_or_empty(machine.states))
                    .find(|summary| summary.symbol == state.symbol)
                else {
                    continue;
                };
                let operational_calls = operational.calls.span_or_empty(operational_state.calls);
                let statements = program.statement_table.statements(state.statement_nodes);

                for (statement_index, statement) in statements.iter().enumerate() {
                    let mut call_ordinal = 0usize;
                    validate_statement(
                        program,
                        statement,
                        statement_index,
                        operational_calls,
                        &mut call_ordinal,
                        &mut diagnostics,
                    );
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn validate_statement(
        program: &dyn ::validation::CheckingInput,
        statement: &StatementNode,
        statement_index: usize,
        operational_calls: &[CallOperational],
        call_ordinal: &mut usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                validate_child(
                    program,
                    assignment.target,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
                validate_child(
                    program,
                    assignment.value,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
            StatementNode::Call(call) => {
                validate_call(
                    call.target.as_str(),
                    call.source_span,
                    statement_index,
                    true,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
                for argument in program.statement_table.expression_handles(call.arguments) {
                    validate_child(
                        program,
                        *argument,
                        statement_index,
                        operational_calls,
                        call_ordinal,
                        diagnostics,
                    );
                }
            }
            StatementNode::Expression(expression) => validate_expression(
                program,
                *expression,
                statement_index,
                true,
                operational_calls,
                call_ordinal,
                diagnostics,
            ),
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                validate_expression(
                    program,
                    local.initial_value,
                    statement_index,
                    true,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                )
            }
            StatementNode::LocalData(_) => {}
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    validate_expression(
                        program,
                        guard,
                        statement_index,
                        true,
                        operational_calls,
                        call_ordinal,
                        diagnostics,
                    );
                }
                for target in transition.targets() {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path,
                            arguments,
                            source_span,
                            ..
                        } => {
                            let target_name = program
                                .statement_table
                                .name_path_members(path.members)
                                .joined_path();
                            validate_call(
                                &target_name,
                                *source_span,
                                statement_index,
                                true,
                                operational_calls,
                                call_ordinal,
                                diagnostics,
                            );
                            for argument in program.statement_table.expression_handles(*arguments) {
                                validate_child(
                                    program,
                                    *argument,
                                    statement_index,
                                    operational_calls,
                                    call_ordinal,
                                    diagnostics,
                                );
                            }
                        }
                        TransitionTargetNode::Value(expression) => validate_expression(
                            program,
                            *expression,
                            statement_index,
                            true,
                            operational_calls,
                            call_ordinal,
                            diagnostics,
                        ),
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
        }
    }

    /* Every recursive child walk spells non-direct position identically. */
    #[allow(clippy::too_many_arguments)]
    fn validate_child(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        statement_index: usize,
        operational_calls: &[CallOperational],
        call_ordinal: &mut usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_expression(
            program,
            expression,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        );
    }

    fn validate_expression(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        statement_index: usize,
        direct_position: bool,
        operational_calls: &[CallOperational],
        call_ordinal: &mut usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression.is_valid() {
            return;
        }
        macro_rules! visit_child {
            ($child:expr) => {
                validate_child(
                    program,
                    $child,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                )
            };
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                visit_child!(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        visit_child!(pattern);
                    }
                    visit_child!(arm.value);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                let inputs = validation::atomic_execution_children(program, atomic);
                for input in inputs {
                    visit_child!(input);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    visit_child!(*value);
                }
            }
            ExpressionNode::Binary(binary) => {
                visit_child!(binary.left);
                visit_child!(binary.right);
            }
            ExpressionNode::Cast(cast) => visit_child!(cast.value),
            ExpressionNode::Call(call) => {
                validate_call(
                    call.target.as_str(),
                    program.expression_table.source_span(expression),
                    statement_index,
                    direct_position,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
                visit_child!(call.receiver);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    visit_child!(*argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                visit_child!(indexed.collection);
                visit_child!(indexed.index);
            }
            ExpressionNode::Member(member) => visit_child!(member.receiver),
            ExpressionNode::Borrow(inner) => visit_child!(inner.target),
            ExpressionNode::Range(range) => {
                visit_child!(range.start);
                visit_child!(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    visit_child!(field.value);
                }
            }
            ExpressionNode::Unary(unary) => visit_child!(unary.operand),
            ExpressionNode::TypeExpression(_) | typed_trees::literal_expression_pattern!() => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_call(
        target: &str,
        source_span: source::SourceSpan,
        statement_index: usize,
        direct_position: bool,
        operational_calls: &[CallOperational],
        call_ordinal: &mut usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let ordinal = *call_ordinal;
        *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
        let Some(call) = operational_calls
            .iter()
            .find(|call| call.statement_index == statement_index && call.call_ordinal == ordinal)
        else {
            return;
        };

        let at_call = |message| {
            let diagnostic = Diagnostic::error(message);
            if source_span.span.start < source_span.span.end {
                diagnostic.with_source_span(source_span)
            } else {
                diagnostic
            }
        };
        let acknowledgement = call.acknowledgement;
        let may_suspend = call.transitive_may_suspend;
        let may_block = call.transitive_may_block;
        if acknowledgement.acknowledges_suspend != may_suspend
            || acknowledgement.acknowledges_block != may_block
        {
            diagnostics.push(at_call(format!(
                "call to `{target}` has operational envelope {} but acknowledges {}; call acknowledgements must match exactly",
                envelope_label(may_suspend, may_block),
                envelope_label(
                    acknowledgement.acknowledges_suspend,
                    acknowledgement.acknowledges_block,
                ),
            )).with_source_span(source_span));
        }

        if may_suspend && !direct_position {
            diagnostics.push(at_call(format!(
                "suspending call to `{target}` is nested inside a partially evaluated expression; bind it as a complete statement, simple `let` right-hand side, transition subject, or terminal expression before continuation planning",
            )).with_source_span(source_span));
        }
    }

    fn envelope_label(may_suspend: bool, may_block: bool) -> &'static str {
        match (may_suspend, may_block) {
            (false, false) => "neither suspension nor blocking",
            (true, false) => "`suspend`",
            (false, true) => "`block`",
            (true, true) => "`suspend block`",
        }
    }
}
pub(crate) mod program_validation {
    use diagnostics::Diagnostic;
    use flow_effects::{OperationalPlan, ServiceReachInferencePlan};
    use proof::obligations::ProofPlan;
    use typed_trees::data::DataMember;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct PreparedTypedProgram<'program> {
        pub(crate) deferred_entailment: validation::DeferredEntailment,
        pub(crate) frozen_dependency_bodies: Vec<validation::FrozenDependencyBody>,
        pub(crate) integer_assignments: Vec<proof::checker::CheckedIntegerAssignment>,
        /*
            Comparisons the proof pass established against retained constant
            recipes, for the target closure to discharge.
        */
        pub(crate) constant_recipe_obligations: Vec<checked_trees::ConstantRecipeObligation>,
        pub(crate) proof_plan: ProofPlan<'program>,
        pub(crate) operational: OperationalPlan,
        pub(crate) service_reaches: ServiceReachInferencePlan,
        pub(crate) validation_facts: validation::ProgramValidationFacts,
        pub(crate) deferred_overflow: Vec<Diagnostic>,
    }

    pub(crate) fn prepare_typed_program<'program>(
        program: &'program dyn ::validation::CheckingInput,
        proof_plan: ProofPlan<'program>,
        opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
        allow_pending_opaque_copy: bool,
        prior_dependency_bodies: &[validation::DependencyBodyJudgment],
        prior_frozen_bodies: &[validation::FrozenDependencyBody],
        record_dependency_bodies: bool,
    ) -> Result<PreparedTypedProgram<'program>, Vec<Diagnostic>> {
        let opaque_properties = if allow_pending_opaque_copy {
            validation::OpaquePropertyValidation::PendingBuildSelection
        } else {
            validation::OpaquePropertyValidation::Required(opaque_property_receipts)
        };
        let mut validated = validation::prepare_specialized_program(
            program,
            opaque_properties,
            prior_dependency_bodies,
            prior_frozen_bodies,
            record_dependency_bodies,
        )?;

        let decisions =
            proof::checker::check_proof_plan_with_decisions(&proof_plan).and_then(|decisions| {
                validate_no_bare_boundary_trait_values(program).map(|()| decisions)
            });
        let proof::checker::ProofDecisions {
            integer_assignments,
            constant_recipe_obligations,
        } = match decisions {
            Ok(decisions) => decisions,
            Err(errors) => {
                let mut diagnostics = std::mem::take(&mut validated.deferred_overflow);
                diagnostics.extend(entailment_errors_first(
                    program,
                    validated.deferred_entailment,
                    errors,
                ));
                return Err(diagnostics);
            }
        };

        let operational = validated.operational;

        let frozen_dependency_bodies = validated.frozen_dependency_bodies;
        Ok(PreparedTypedProgram {
            deferred_entailment: validated.deferred_entailment,
            frozen_dependency_bodies,
            integer_assignments,
            constant_recipe_obligations,
            proof_plan,
            operational,
            validation_facts: validated.facts,
            service_reaches: validated.service_reaches,
            deferred_overflow: validated.deferred_overflow,
        })
    }

    /*
        Report the failure that prevented storage tracking from completing. Rechecking
        contracts without that history can blame valid code for losing facts after
        unrelated writes. With storage history available, show contract errors first
        while retaining the original failure that stopped checking.
    */
    pub(crate) fn entailment_errors_first(
        program: &dyn ::validation::CheckingInput,
        deferred: validation::DeferredEntailment,
        errors: Vec<Diagnostic>,
    ) -> Vec<Diagnostic> {
        if program.storage_graph().is_none() {
            return errors;
        }
        match validation::judge_contract_entailment(program, deferred) {
            Ok(_) => errors,
            Err(mut entailment) => {
                entailment.extend(errors);
                entailment
            }
        }
    }

    /*
        A boundary trait is a static provider slot reached through its requirement
        calls, never a value: fields, parameters, signature slots and returns cannot
        name it bare. Borrows (`&T`, `&mut T`) and container members are other
        positions and stay out of this gate; only the outermost
        `Named`/`DynamicTrait`/`Generic` head (through `Constrained` shells) is
        inspected.
    */
    fn validate_no_bare_boundary_trait_values(
        program: &dyn ::validation::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut check = |type_reference: TypeReferenceHandle, position: &dyn Fn() -> String| {
            if let Some(trait_name) = bare_boundary_trait_name(program, type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "{} names bare boundary trait `{trait_name}` in value position; a boundary trait is a static provider slot, not a value type",
                    position()
                )));
            }
        };
        for definition in program.data_definitions() {
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => check(field.type_reference, &|| {
                        format!(
                            "field `{}` on data `{}`",
                            field.name.as_str(),
                            definition.name.as_str()
                        )
                    }),
                    DataMember::Variant(variant) => {
                        for payload in program.data_payload_fields(variant) {
                            check(payload.type_reference, &|| {
                                format!(
                                    "payload field `{}` on case `{}` of data `{}`",
                                    payload.name.as_str(),
                                    variant.name.as_str(),
                                    definition.name.as_str()
                                )
                            });
                        }
                    }
                }
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    check(parameter.type_reference, &|| {
                        format!(
                            "parameter `{}` on `{}.{}`",
                            parameter.name.as_str(),
                            machine.name.as_str(),
                            state.name.as_str()
                        )
                    });
                }
                check(state.return_type, &|| {
                    format!(
                        "return type of `{}.{}`",
                        machine.name.as_str(),
                        state.name.as_str()
                    )
                });
            }
        }
        for trait_definition in program.traits() {
            for signature in program.trait_machine_signatures(trait_definition) {
                for parameter in program.state_signature_parameters(signature) {
                    check(parameter.type_reference, &|| {
                        format!(
                            "parameter `{}` on signature `{}.{}`",
                            parameter.name.as_str(),
                            trait_definition.name.as_str(),
                            signature.name.as_str()
                        )
                    });
                }
                check(signature.return_type, &|| {
                    format!(
                        "return type of signature `{}.{}`",
                        trait_definition.name.as_str(),
                        signature.name.as_str()
                    )
                });
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn bare_boundary_trait_name(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<String> {
        loop {
            if !program
                .type_reference_table
                .contains_type_reference(type_reference)
            {
                return None;
            }
            let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => {
                    type_reference = *base_type;
                    continue;
                }
                TypeReferenceNode::Named { symbol, name }
                | TypeReferenceNode::DynamicTrait { symbol, name, .. } => (*symbol, name.as_str()),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => (*base_symbol, base_name.as_str()),
                _ => return None,
            };
            return program
                .trait_definition_by_symbol(symbol)
                .is_some_and(|definition| definition.is_boundary)
                .then(|| name.to_owned());
        }
    }
}
mod target_families {
    /*
        Portable references name a family contract, never an authored member.

        Resolution gives each product family a separate bodyless Requirement whose
        fresh telescope conjoins member premises. Typing checks every retained
        member's complete callable shape; checking publishes their joined operational
        and crash ceilings before reaching this gate. That Requirement has no private
        body or return-range guarantee to import. Exact member references remain
        confined to target-scoped bodies, including references in proofs and types.
        Execution selection is a later consumer obligation, not source authority.
    */
    use diagnostics::Diagnostic;
    use std::collections::HashSet;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::references::{StaticArgumentCall, StaticArgumentRole, TypedReferences};

    pub(super) fn validate_portable_references(
        checked: &checked_trees::CheckedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let program = &checked.typed;
        if program.target_machine_families.is_empty() {
            return Ok(());
        }
        let mut references = PortableReferences {
            program,
            owner: SymbolHandle::invalid(),
            expressions: HashSet::new(),
            types: HashSet::new(),
            refused: Vec::new(),
        };
        typed_trees::references::visit_declarations(program, &mut references);
        for machine in program.machines() {
            if !references.declaration(machine.symbol) {
                continue;
            }
            typed_trees::references::visit_machine(program, &mut references, machine);
        }
        for dependency in checked.facts.flow.semantic_dependencies.iter() {
            if references.declaration(dependency.consumer_machine) {
                references.extend([dependency.dependency]);
            }
        }
        for (_, state) in checked.facts.flow.control.states.iter() {
            if references.declaration(state.machine_symbol) {
                references.extend(
                    crate::flow::call_flows(&checked.facts.flow.control, state.calls)
                        .iter()
                        .map(|call| call.target_symbol),
                );
            }
        }
        if references.refused.is_empty() {
            return Ok(());
        }
        Err(references
            .refused
            .into_iter()
            .map(|(owner, member)| {
                Diagnostic::error(format!(
                    "unimplemented: portable target-family contract for `{}` referenced by `{}`; \
                 source checking cannot use a selected target body's guarantees",
                    program.symbols.display_path(member, "::"),
                    program.symbols.display_path(owner, "::"),
                ))
            })
            .collect())
    }

    struct PortableReferences<'program> {
        program: &'program TypedTrees,
        owner: SymbolHandle,
        expressions: HashSet<typed_trees::expression::ExpressionHandle>,
        types: HashSet<typed_trees::types::TypeReferenceHandle>,
        refused: Vec<(SymbolHandle, SymbolHandle)>,
    }

    impl Extend<SymbolHandle> for PortableReferences<'_> {
        fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: References) {
            for symbol in symbols {
                let machine = self
                    .program
                    .machine_by_symbol(symbol)
                    .or_else(|| self.program.machine_holding_state(symbol));
                let Some(machine) = machine else { continue };
                if self.program.machine_authored_target(machine).is_some()
                    && !self.refused.contains(&(self.owner, machine.symbol))
                {
                    self.refused.push((self.owner, machine.symbol));
                }
            }
        }
    }

    impl TypedReferences for PortableReferences<'_> {
        fn layout_realization_dependency(&mut self, _: SymbolHandle) {}

        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            if self
                .program
                .machine_by_symbol(symbol)
                .is_some_and(|machine| {
                    machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
                        || self.program.source_target_context(machine).is_some()
                })
                || self.program.symbols.get(symbol).kind == symbols::SymbolKind::Conformance
                || self
                    .program
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| self.program.symbols.source_file(span))
                    .is_some_and(|source| source.dependency_scope == source::DependencyScope::Build)
            {
                return false;
            }
            self.owner = symbol;
            self.expressions.clear();
            self.types.clear();
            true
        }

        fn expression(&mut self, handle: typed_trees::expression::ExpressionHandle) -> bool {
            self.expressions.insert(handle)
        }

        fn type_reference(&mut self, handle: typed_trees::types::TypeReferenceHandle) -> bool {
            self.types.insert(handle)
        }

        fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
            validation::compiler_operations::static_argument_role(program, call)
                == StaticArgumentRole::ValueDependency
        }
    }
}

use crate::checking::program_validation::prepare_typed_program;
use crate::checks;
use crate::published::facts::build_check_facts;
use checked_trees::CheckedTrees;
use representation_selections::OpaqueRepresentationSelection;
use typed_trees_to_typed_trees::TypedPreparation;

/*
    Invocation-owned package declaration authority. The preparation producer
    carries its admission checkpoint in TypedPreparation; checking cannot choose
    a different checkpoint over that producer's result.
*/
#[derive(Clone, Copy, Default)]
pub struct CheckingRequest<'request> {
    declaration_authority: Option<&'request dyn validation::DeclarationSelectionAuthority>,
    external_roots: &'request [symbols::SymbolHandle],
}

impl<'request> CheckingRequest<'request> {
    pub const fn new() -> Self {
        Self {
            declaration_authority: None,
            external_roots: &[],
        }
    }

    /* Admit checked selections against this invocation's reconciled package authority. */
    pub const fn with_declaration_authority(
        mut self,
        authority: Option<&'request dyn validation::DeclarationSelectionAuthority>,
    ) -> Self {
        self.declaration_authority = authority;
        self
    }

    /*
        Ordinary callers cannot establish input compatibility for an external
        entry. Check the selected entries on every request, including reuse.
    */
    pub const fn with_external_roots(mut self, roots: &'request [symbols::SymbolHandle]) -> Self {
        self.external_roots = roots;
        self
    }
}

/*
    Check the prepared typed input under this invocation's declaration authority. Toolchain-owned
    selections may remain late-bound until build-time evaluation; ordinary
    authored selections remain strict in both modes.
*/
pub fn lower_typed_trees(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_reusing(program, preparation, None, request, &[], &[], false).map(|outcome| {
        std::sync::Arc::try_unwrap(outcome.checked).unwrap_or_else(|shared| (*shared).clone())
    })
}

/*
    One language judgment kept with the exact input it judged. Only this module
    constructs one, from a check it ran, so a judgment taken from it is a
    judgment this function computed for an equal input.
*/
#[derive(Debug, Clone)]
pub struct RetainedCheck {
    /*
        The typed program before preparation and the admission inputs it was
        prepared under. Preparation is a pure function of that pair, so an
        equal request can take this judgment without preparing or checking.
    */
    preparation_input: std::sync::Arc<typed_trees::TypedTrees>,
    preparation_admission: typed_trees_to_typed_trees::PreparationAdmission,
    external_roots: Vec<symbols::SymbolHandle>,
    admits_pending_inputs: bool,
    checked: std::sync::Arc<CheckedTrees>,
    /*
        Dependency bodies whose declaration/state/expression correspondence
        matched this strict check. Later consumers read those frozen bodies.
    */
    frozen_dependency_bodies: Vec<validation::FrozenDependencyBody>,
    /*
        Entailment judgments for non-instance machines in this strict check.
        A later dependent in the same invocation may reuse one of them when
        the body's correspondence still matches. Pending checks record none.
    */
    dependency_bodies: Vec<validation::DependencyBodyJudgment>,
}

impl RetainedCheck {
    /*
        Identity of an already-issued language judgment, not semantic equality
        or permission to skip admission. Invocation owners use it only to avoid
        retaining the same immutable result twice.
    */
    pub fn same_judgment(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.checked, &other.checked)
    }

    /*
        The frozen typed trees this judgment accepted. A request that takes
        this judgment runs no preparation of its own, so a caller comparing
        another checker on the same input reads the trees here.
    */
    pub fn judged_typed_trees(&self) -> &typed_trees::FrozenTypedTrees {
        &self.checked.typed
    }

    /*
        Whether this judgment was computed for the complete source request.
        Preparation depends on the program and its admission; checking also
        depends on which entries can be invoked independently. Only an equal
        request can take this judgment without preparing or checking again.
    */
    pub fn judges_preparation_input(
        &self,
        program: &typed_trees::TypedTrees,
        admission: &typed_trees_to_typed_trees::PreparationAdmission,
        external_roots: &[symbols::SymbolHandle],
    ) -> bool {
        self.judges_preparation_admission(admission, external_roots)
            && *self.preparation_input == *program
    }

    /*
        Whether preparation and independent entries match this request. An
        equal typed program then completes [`Self::judges_preparation_input`].
    */
    pub fn judges_preparation_admission(
        &self,
        admission: &typed_trees_to_typed_trees::PreparationAdmission,
        external_roots: &[symbols::SymbolHandle],
    ) -> bool {
        self.preparation_admission == *admission && self.external_roots == external_roots
    }

    /*
        Dependency-body entailment judgments a strict consumer may reuse.
        A pending admission does not supply a judgment to anyone.
    */
    pub fn strict_dependency_bodies(&self) -> &[validation::DependencyBodyJudgment] {
        if self.admits_pending_inputs {
            &[]
        } else {
            &self.dependency_bodies
        }
    }

    /* Frozen bodies a strict consumer may read through the same correspondence. */
    pub fn strict_frozen_dependency_bodies(&self) -> &[validation::FrozenDependencyBody] {
        if self.admits_pending_inputs {
            &[]
        } else {
            &self.frozen_dependency_bodies
        }
    }
}

/*
    What [`lower_typed_trees_reusing`] returns: the checked program, this
    judgment when retention was asked for, and whether an equal prior supplied
    it instead of a check.
*/
pub struct CheckingOutcome {
    pub checked: std::sync::Arc<CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub took_prior: bool,
}

/*
    Take `prior`'s judgment for a preparation input it already judged. The
    caller has compared the input and admission and found them equal
    ([`RetainedCheck::judges_preparation_input`]); preparation and checking are
    both pure over that pair, so nothing recomputes. Invocation-owned
    declaration authority still runs.
*/
pub fn reuse_retained_check(
    prior: &RetainedCheck,
    request: &CheckingRequest<'_>,
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    if prior.external_roots != request.external_roots {
        return Err(vec![diagnostics::Diagnostic::error(
            "retained source judgment has a different external invocation roster",
        )]);
    }
    let checked = prior.checked.clone();
    let retained = retain.then(|| prior.clone());
    admit_request(&checked, request)?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior: true,
    })
}

/*
    Check this prepared program, reusing only matching dependency-body entailments.
    Whole-program reuse belongs to the source facade's complete preparation-input
    match. Retain that original input beside the shared checked result when the
    caller will ask again; no second prepared-program snapshot or partial
    admission key is needed.
*/
pub fn lower_typed_trees_reusing(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    retained_input: Option<(
        std::sync::Arc<typed_trees::TypedTrees>,
        typed_trees_to_typed_trees::PreparationAdmission,
    )>,
    request: &CheckingRequest<'_>,
    prior_dependency_bodies: &[validation::DependencyBodyJudgment],
    prior_frozen_bodies: &[validation::FrozenDependencyBody],
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    let (
        admits_pending_inputs,
        deferred_layout_applications,
        mathematics,
        opaque_property_receipts,
        opaque_representation_selections,
    ) = preparation.into_checking_judgments();
    let (checked, dependency_bodies, frozen_dependency_bodies) = check_program(
        program,
        admits_pending_inputs,
        &deferred_layout_applications,
        mathematics,
        opaque_property_receipts,
        opaque_representation_selections,
        prior_dependency_bodies,
        prior_frozen_bodies,
        request.external_roots,
        retain && !admits_pending_inputs && deferred_layout_applications.is_empty(),
    )?;
    let checked = std::sync::Arc::new(checked);
    let retained = retained_input.map(|(preparation_input, preparation_admission)| RetainedCheck {
        preparation_input,
        preparation_admission,
        admits_pending_inputs: admits_pending_inputs || !deferred_layout_applications.is_empty(),
        checked: checked.clone(),
        external_roots: request.external_roots.to_vec(),
        frozen_dependency_bodies,
        dependency_bodies,
    });
    admit_request(&checked, request)?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior: false,
    })
}

/*
    Package authority and selected entry admission belong to this request.
    Neither can be supplied by a preceding request's ordinary source judgment.
*/
fn admit_request(
    checked: &CheckedTrees,
    request: &CheckingRequest<'_>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if let Some(authority) = request.declaration_authority {
        validation::validate_authored_declaration_selections(checked, authority)?;
    }
    checks::check_external_root_borrows(checked, request.external_roots)?;
    Ok(())
}

fn merge_overflow(
    overflow: &mut Vec<diagnostics::Diagnostic>,
    errors: Vec<diagnostics::Diagnostic>,
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostics = std::mem::take(overflow);
    diagnostics.extend(errors);
    diagnostics
}

/* The check itself, over a program `lower_typed_trees` has found complete. */
fn check_program(
    program: typed_trees::FrozenTypedTrees,
    admits_pending_inputs: bool,
    deferred_layout_applications: &[typed_trees::types::TypeReferenceHandle],
    mathematics: proof::mathematical_declarations::CheckedMathematics,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    prior_dependency_bodies: &[validation::DependencyBodyJudgment],
    prior_frozen_bodies: &[validation::FrozenDependencyBody],
    external_roots: &[symbols::SymbolHandle],
    record_dependency_bodies: bool,
) -> Result<
    (
        CheckedTrees,
        Vec<validation::DependencyBodyJudgment>,
        Vec<validation::FrozenDependencyBody>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let typed_program = program;
    let program = validation::CheckingContext::new(&typed_program)
        .with_external_invocation_roots(external_roots)
        .with_pending_layout_geometry(
            admits_pending_inputs || !deferred_layout_applications.is_empty(),
        )
        .with_opaque_representation_selections(&opaque_representation_selections)
        .with_prepared_mathematics(mathematics.prepared_view());
    validation::validate_selected_attached_method_bounds(&program)?;
    let declared_content = checks::prepare_retained_content_custody(
        &program,
        crate::published::facts::build_content_facts(&program),
    )?;
    let program = program
        .with_retained_borrow_custodies(declared_content.retained_borrow_custodies.clone().into());
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let mut validated = prepare_typed_program(
        &program,
        proof_plan,
        &opaque_property_receipts,
        admits_pending_inputs,
        prior_dependency_bodies,
        prior_frozen_bodies,
        record_dependency_bodies,
    )?;
    let deferred_entailment = validated.deferred_entailment;
    let mut deferred_overflow = std::mem::take(&mut validated.deferred_overflow);
    let frozen_dependency_bodies = std::mem::take(&mut validated.frozen_dependency_bodies);
    let mutation_summaries = crate::flow::StateMutationSummaryCache::default();
    let incoming_guard_index = checks::IncomingGuardIndexCache::default();
    let program = program.clone_with_expression_intervals(std::sync::Arc::from(std::mem::take(
        &mut validated.validation_facts.expression_intervals,
    )));
    let proof_recipe_obligations = std::mem::take(&mut validated.constant_recipe_obligations);
    let integer_range_obligations =
        std::mem::take(&mut validated.validation_facts.integer_range_obligations);
    let built = build_check_facts(
        &program,
        &validated.proof_plan,
        declared_content,
        &validated.integer_assignments,
        validated.operational,
        validated.service_reaches,
        validated.validation_facts,
        &mutation_summaries,
    );
    let (mut facts, prepared_effects, program, formation_overflow) = match built {
        Ok(built) => built,
        Err(failure) => {
            return Err(merge_overflow(
                &mut deferred_overflow,
                crate::checking::program_validation::entailment_errors_first(
                    failure.program.as_deref().unwrap_or(&program),
                    deferred_entailment,
                    failure.diagnostics,
                ),
            ));
        }
    };
    deferred_overflow.extend(formation_overflow);
    let validation::ContractEntailment {
        proven_machine_contracts,
        dependency_body_judgments,
    } = match validation::judge_contract_entailment(&program, deferred_entailment) {
        Ok(entailment) => entailment,
        Err(errors) => return Err(merge_overflow(&mut deferred_overflow, errors)),
    };
    let program = program.with_prepared_effect_plans(&prepared_effects);
    facts.deferred_layout_applications = deferred_layout_applications.to_vec();
    facts.integer_range_obligations = integer_range_obligations;
    let quotient_operations =
        match validation::admit_checked_quotient_requests(&program, &|machine| {
            facts
                .termination
                .for_machine(machine)
                .map(|plan| plan.checked_summary.clone())
        }) {
            Ok(operations) => operations,
            Err(errors) => return Err(merge_overflow(&mut deferred_overflow, errors)),
        };
    facts
        .quotient_operations
        .extend(quotient_operations.into_iter().map(|operation| {
            checked_trees::CheckedQuotientOperation {
                request: operation.request,
                public_machine: operation.public_machine,
                public_state: operation.public_state,
                representative_state: operation.representative_state,
                correspondence: operation.correspondence,
            }
        }));
    if let Err(errors) = checks::initialize_checked_direct_borrow_resources(
        &program,
        &mut facts,
        &mutation_summaries,
    ) {
        return Err(merge_overflow(&mut deferred_overflow, errors));
    }
    let specialization_identities =
        match validation::contract_identities::prepare_specialization_contract_identities(
            &program,
            &facts.contract_plans,
            &prepared_effects,
        ) {
            Ok(identities) => identities,
            Err(errors) => return Err(merge_overflow(&mut deferred_overflow, errors)),
        };

    if let Err(errors) = checks::check_checked_facts_recording(
        &program,
        &mut facts,
        &mutation_summaries,
        &incoming_guard_index,
        &proven_machine_contracts,
    ) {
        return Err(merge_overflow(&mut deferred_overflow, errors));
    }
    if !deferred_overflow.is_empty() {
        return Err(merge_overflow(&mut deferred_overflow, Vec::new()));
    }
    let (quotients, (cleanup, citations)) = validation::checking_thread_pool().install(|| {
        rayon::join(
            || crate::products::quotient_eligibility::check(&program, &facts),
            || {
                rayon::join(
                    || checks::validate_selected_cleanup_guarantees(&program, &facts),
                    || {
                        validation::validate_runtime_proof_citations(&program, &|machine| {
                            facts
                                .termination
                                .for_machine(machine)
                                .map(|plan| plan.checked_summary.clone())
                        })
                    },
                )
            },
        )
    });
    quotients?;
    cleanup?;
    citations?;
    facts
        .constant_recipe_obligations
        .extend(proof_recipe_obligations);
    crate::published::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = crate::execution::finalize_execution::finalize_execution(
        &program,
        facts,
        typed_program.boundary_signatures(),
    )?;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    facts.specialization_identities = specialization_identities;
    facts.accepted_template_identities = {
        checked_trees::CheckedAcceptedTemplateIdentities::with_rows(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                    })
                    .map(|machine| checked_trees::CheckedAcceptedTemplateClassification {
                        machine: machine.symbol,
                        identity: validation::contract_identities::generic_machine_template_report_fingerprint(
                            &program,
                            machine.symbol,
                        )
                        .zip(validation::contract_identities::generic_machine_template_commitment(
                            &program,
                            machine.symbol,
                        ))
                        .map(|(report_fingerprint, commitment)| {
                            checked_trees::CheckedAcceptedTemplateIdentity {
                                report_fingerprint,
                                commitment,
                            }
                        }),
                    })
                    .collect(),
            )
    };
    facts.finalized_selections =
        crate::shared::authored_selections::finalize_checked_authored_selections(
            &program, &mut facts,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    let finalized = program.with_authored_declaration_selections(&facts.finalized_selections);
    let mut selections = None;
    let mut reference_custody = None;
    let mut content_classes = None;
    let mut resolved_types = None;
    let mut state_locals = None;
    let mut provider_closed_requirements = None;
    let mut executable_bindings = None;
    {
        let finalized = &finalized;
        validation::checking_thread_pool().scope(|scope| {
            scope.spawn(|_| {
                selections = Some((|| -> Result<(), Vec<diagnostics::Diagnostic>> {
                        validation::validate_reserved_cleanup_selections(finalized)?;
                        validation::validate_declaration_visibility(finalized)?;
                        Ok(())
                    })());
            });
            scope.spawn(|_| {
                reference_custody = Some(validation::reference_result_custody::classify_program_reference_custody(
                            finalized,
                        ));
            });
            scope.spawn(|_| {
                content_classes = Some(validation::classify_program_content_classes(finalized));
            });
            scope.spawn(|_| {
                resolved_types = Some(crate::published::facts::resolved_types::resolve_program_types(finalized));
            });
            scope.spawn(|_| {
                state_locals = Some(crate::published::facts::state_locals::record_state_locals(finalized));
            });
            scope.spawn(|_| {
                provider_closed_requirements = Some(crate::published::facts::provider_closed_requirements::record_provider_closed_requirements(
                            finalized,
                        ));
            });
            scope.spawn(|_| {
                executable_bindings = Some(crate::published::facts::executable_bindings::record(finalized));
            });
        });
    }
    let filled = "every selection and classification pass ran";
    selections.expect(filled)?;
    facts.reference_custody = reference_custody.expect(filled);
    facts.content_classes = content_classes.expect(filled);
    facts.resolved_types = resolved_types.expect(filled);
    facts.state_locals = state_locals.expect(filled);
    facts.provider_closed_requirements = provider_closed_requirements.expect(filled);
    facts.executable_bindings = executable_bindings.expect(filled);
    crate::published::facts::executable_bindings::freeze_contract_subjects(
        &finalized,
        &mut facts.proof,
    );
    crate::published::facts::executable_bindings::freeze_declaration_arguments(
        &finalized,
        &mut facts.termination.rank_arrivals,
    );
    drop(finalized);

    facts.proof.mathematical_declarations = mathematics.declarations;
    if mathematics.signature.is_some() {
        for (_, application) in mathematics.applications.applications.iter() {
            for value in mathematics
                .applications
                .identity_values
                .span_or_empty(application.identity_values)
            {
                facts
                    .proof
                    .identity_formations
                    .push(checked_trees::CheckedIdentityFormation {
                        level_arity: application.level_arity,
                        expression: application.expression,
                        owner: application.owner,
                        type_identity: value.type_identity,
                        term: value.term,
                        ty: value.ty,
                        domains: application.domains.clone(),
                        captures: mathematics
                            .applications
                            .captures
                            .span_or_empty(application.captures)
                            .iter()
                            .map(|capture| {
                                (
                                    capture.expression,
                                    typed_program.normalized_type_identity(capture.source_type),
                                )
                            })
                            .collect(),
                    });
            }
        }
    }

    facts.proof.source_evidence_carriers = mathematics
        .applications
        .source_carriers
        .iter()
        .map(|(_, carrier)| checked_trees::CheckedSourceEvidenceCarrier {
            level_arity: carrier.level_arity,
            type_identity: carrier.type_identity,
            ty: carrier.ty,
            domains: carrier.domains.clone(),
        })
        .collect();
    for (_, theorem) in mathematics.applications.indexed_theorems.iter() {
        let lost = || {
            vec![diagnostics::Diagnostic::error(
                "indexed theorem lost its exact authored contract association",
            )]
        };
        let machine = typed_program.machine(theorem.machine).ok_or_else(lost)?;
        let exact_clause = typed_program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .flat_map(|contract| {
                typed_program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
            })
            .filter_map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .nth(theorem.ensure_ordinal as usize);
        if exact_clause != Some(theorem.expression) {
            return Err(lost());
        }
        let entry = typed_program
            .machine_states(machine)
            .first()
            .ok_or_else(lost)?;
        let (position, formal) = typed_program
            .state_parameters(entry)
            .iter()
            .enumerate()
            .find(|(_, formal)| formal.symbol == theorem.evidence_formal)
            .ok_or_else(lost)?;
        let type_identity = typed_program.normalized_type_identity(theorem.source_type);
        if !formal.relevance.is_erased()
            || typed_program.normalized_type_identity(formal.type_reference) != type_identity
        {
            return Err(lost());
        }
        facts
            .proof
            .indexed_theorems
            .push(checked_trees::CheckedIndexedTheorem {
                level_arity: theorem.level_arity,
                machine: theorem.machine,
                expression: theorem.expression,
                ensure_ordinal: theorem.ensure_ordinal,
                evidence_source_position: u32::try_from(position).map_err(|_| lost())?,
                evidence_formal: theorem.evidence_formal,
                type_identity,
                term: theorem.theorem,
                ty: theorem.ty,
                assumption_closure: theorem.assumption_closure.clone(),
            });
    }
    facts.proof.mathematical_safety_assumptions = mathematics.applications.safety_assumptions;
    facts.proof.identity_formation_plan = crate::published::facts::identity_formations::record(
        &typed_program,
        &facts.proof.identity_formations,
        mathematics.signature.as_ref(),
    )?;
    facts.proof.mathematical_signature = mathematics.signature;
    let mut checked = CheckedTrees::with_roots(typed_program, facts);
    (|| -> Result<(), Vec<diagnostics::Diagnostic>> {
        validation::validate_boundary_calls(&checked)?;
        target_families::validate_portable_references(&checked)?;
        crate::shared::operators::retain_boundary_body_candidates(&mut checked)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
        crate::shared::operators::retain_provider_requirements(&mut checked);
        Ok(())
    })()?;
    crate::published::facts::declaration_descriptions::retain_declaration_descriptions(
        &mut checked,
    );
    crate::published::facts::provider_instance_publication::retain(&mut checked);
    checked.facts.proof_applications =
        crate::published::facts::proof_applications::retain(&checked);
    Ok((checked, dependency_body_judgments, frozen_dependency_bodies))
}
