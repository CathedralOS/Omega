//! Normalize the exact selections of every accepted source form. The output
//! is a question packet; only receiving execution admission grants permission.

use std::collections::HashMap;
use std::fmt::Write as _;

use compiler_storage::Record;
use diagnostics::Diagnostic;
use semantic_vocabulary::source_selections::{
    BuildCallSite, SelectionBindingKind, SelectionDeclaration, SelectionForm, SelectionJoin,
    SelectionNode, SelectionOccurrence, SelectionProductDesignation, SelectionPurpose,
    SelectionQuestion, SelectionQuestions, SelectionReference, SelectionRole,
};
use semantic_vocabulary::{
    CallableInvocationSite, InvocationSite, MachineId, SelectionDeclarationId, SelectionQuestionId,
    TargetMachineCallable,
};
use structure_elaboration::selection_questions::{
    RawDependencyKind, RawDesignationInput, RawSelectionQuestion, RawSelectionQuestions,
    RawSelectionRole, RawSelectionTarget,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_psi::{OperationKind, TerminalModule};
use typed_trees::TypedTrees;
use typed_trees::callable_values::RuntimeCallableContractHandle;

use crate::AcceptedTerminalSource;

type SymbolMap<Value> = HashMap<SymbolHandle, Value, symbols::BuildSymbolHasher>;

fn refused(detail: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!("source declaration-selection admission: {detail}"))
}

pub(crate) fn prepare(
    source: &AcceptedTerminalSource,
    module: &TerminalModule,
) -> Result<SelectionQuestions, Diagnostic> {
    let mut prepared = Questions {
        source,
        module,
        declarations: Declarations::new(source, module)?,
        output: SelectionQuestions::default(),
        build_sites: HashMap::default(),
    };
    prepared.retain_build_sites()?;
    for (ordinal, form) in source.selection_forms().iter().enumerate() {
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
        prepared.retain_form(form, raw)?;
    }
    prepared.retain_physical_joins()?;
    Ok(prepared.output)
}

struct Questions<'source> {
    source: &'source AcceptedTerminalSource,
    module: &'source TerminalModule,
    declarations: Declarations<'source>,
    output: SelectionQuestions,
    build_sites: HashMap<
        (SymbolHandle, typed_trees::expression::ExpressionHandle),
        BuildCallSite,
        symbols::BuildSymbolHasher,
    >,
}

impl Questions<'_> {
    fn typed_origin(
        &self,
        machine: MachineId,
        site: InvocationSite,
    ) -> Result<(SymbolHandle, typed_trees::SourceNode), Diagnostic> {
        let sources = self
            .source
            .closure()
            .elaboration
            .sources
            .get(machine)
            .ok_or_else(|| refused("invocation has no retained source machine"))?;
        let origin = match site {
            InvocationSite::Operation(operation) => sources.operations.get(operation),
            InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge) => {
                sources.edges.get(edge)
            }
        }
        .ok_or_else(|| refused("invocation has no exact source site"))?;
        origin.node().ok_or_else(|| {
            refused("source invocation lost its typed occurrence or original lexical owner")
        })
    }

    fn source_target(&self, target: TargetMachineCallable) -> Result<SymbolHandle, Diagnostic> {
        let elaboration = &self.source.closure().elaboration;
        match target {
            TargetMachineCallable::Machine(machine) => elaboration.machines.get(machine),
            TargetMachineCallable::Boundary(boundary) => elaboration.boundaries.get(boundary),
        }
        .copied()
        .ok_or_else(|| refused("invoked declaration has no exact source correspondence"))
    }

    fn node(node: typed_trees::SourceNode) -> SelectionNode {
        match node {
            typed_trees::SourceNode::Expression(expression) => SelectionNode::Expression {
                index: expression.arena_index(),
                generation: expression.generation(),
            },
            typed_trees::SourceNode::Statement(statement) => SelectionNode::Statement {
                index: statement.arena_index(),
                generation: statement.generation(),
            },
        }
    }

    fn execution_question(
        &mut self,
        owner: SymbolHandle,
        node: typed_trees::SourceNode,
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
        node: typed_trees::SourceNode,
        target: semantic_vocabulary::source_selections::SelectionCallTarget,
    ) -> Result<SelectionReference, Diagnostic> {
        let typed = self.source.source();
        let (runtime, parameter, dispatch, source_target) = match node {
            typed_trees::SourceNode::Expression(expression) => {
                if !typed.expression_table.expression_is_valid(expression) {
                    return Err(refused("invocation source expression is stale"));
                }
                let typed_trees::expression::ExpressionNode::Call(call) =
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
            typed_trees::SourceNode::Statement(statement) => {
                if !typed.statement_table.statement_is_valid(statement) {
                    return Err(refused("invocation source statement is stale"));
                }
                let call = typed
                    .statement_table
                    .statement(statement)
                    .as_call()
                    .ok_or_else(|| refused("emitted invocation source is no call statement"))?;
                (
                    call.runtime_call,
                    call.static_machine_parameter,
                    call.static_requirement_dispatch.as_ref(),
                    call.target_symbol,
                )
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
                let retained = self.module.callables.contract(contract).ok_or_else(|| {
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
                    return self
                        .declarations
                        .symbol_reference(&mut self.output, owner, parameter);
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
                        );
                    }
                } else if source_target != selected {
                    return Err(refused(
                        "declared invocation differs from its exact source selection",
                    ));
                }
                self.declarations
                    .symbol_reference(&mut self.output, owner, selected)
            }
        }
    }

    fn retain_invocation(
        &mut self,
        machine: MachineId,
        site: InvocationSite,
        target: semantic_vocabulary::source_selections::SelectionCallTarget,
    ) -> Result<(), Diagnostic> {
        let (owner, node) = self.typed_origin(machine, site)?;
        let selected = self.invocation_reference(owner, node, target)?;
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

    fn retain_physical_joins(&mut self) -> Result<(), Diagnostic> {
        use semantic_vocabulary::source_selections::SelectionCallTarget;
        let module = self.module;
        let elaboration = &self.source.closure().elaboration;
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
                    if let Some(call) = operation.kind.declared_invocation() {
                        self.retain_invocation(
                            machine.id,
                            site,
                            SelectionCallTarget::Declared(call.callee),
                        )?;
                    } else if let Some(call) = operation.kind.callable_invocation() {
                        self.retain_invocation(
                            machine.id,
                            site,
                            SelectionCallTarget::Selected {
                                contract: call.contract,
                                selector: call.callee,
                            },
                        )?;
                    }
                    if let OperationKind::SelectMachine { descriptor } = operation.kind {
                        let (owner, node) = self.typed_origin(machine.id, site)?;
                        let typed_trees::SourceNode::Expression(expression) = node else {
                            return Err(refused(
                                "machine selection has no exact expression occurrence",
                            ));
                        };
                        if !matches!(
                            self.source.source().expression_table.expression(expression),
                            typed_trees::expression::ExpressionNode::MachineValue(_)
                        ) {
                            return Err(refused(
                                "descriptor constructor differs from its source expression",
                            ));
                        }
                        let selected = module
                            .callables
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
                if let Some((site, call)) = block.terminator.declared_invocation() {
                    self.retain_invocation(
                        machine.id,
                        site,
                        SelectionCallTarget::Declared(call.callee),
                    )?;
                }
            }
            let mut successors = machine
                .blocks
                .iter()
                .flat_map(|block| block.terminator.successor_edges())
                .collect::<Vec<_>>();
            successors.sort_unstable_by_key(|(edge, _)| *edge);
            for (edge, origin) in sources.edges.iter() {
                let Some((owner, node)) = origin.node() else {
                    continue;
                };
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
        for record in &elaboration.source_records {
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
        let elaboration = &self.source.closure().elaboration;
        for machine in &self.module.machines {
            let sources = elaboration
                .sources
                .get(machine.id)
                .ok_or_else(|| refused("emitted machine lost its exact source sites"))?;
            for call in machine.boundary_invocations() {
                let InvocationSite::Operation(operation) = call.site else {
                    continue;
                };
                let Some((owner, typed_trees::SourceNode::Expression(expression))) = sources
                    .operations
                    .get(operation)
                    .and_then(|origin| origin.node())
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
                .declaration(&mut self.output, declaration)?;
        }
        for question in &raw.questions {
            self.authored_question(form, question)?;
        }
        for dependency in &raw.dependencies {
            if dependency.role == RawSelectionRole::ProductDesignation {
                return Err(refused(
                    "Product dependency has no exact retained input commitment",
                ));
            }
            let owner = self
                .declarations
                .declaration(&mut self.output, dependency.lexical_owner)?;
            let selected = self.declarations.symbol_reference(
                &mut self.output,
                dependency.lexical_owner,
                dependency.target,
            )?;
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
                if form != SelectionForm::Published {
                    return Err(refused(
                        "generic Product designation needs its own retained semantic input site",
                    ));
                }
                Some(self.product_designation(raw, input)?)
            }
            _ => {
                return Err(refused(
                    "selection role disagrees with its retained Product input",
                ));
            }
        };
        let selected = match raw.target {
            RawSelectionTarget::Declaration(symbol) => {
                self.declarations
                    .symbol_reference(&mut self.output, raw.lexical_owner, symbol)?
            }
            RawSelectionTarget::LocalBinding { symbol, owner } => self.declarations.local_binding(
                &mut self.output,
                raw.lexical_owner,
                symbol,
                owner,
            )?,
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
        raw: &RawSelectionQuestion,
        input: RawDesignationInput,
    ) -> Result<SelectionProductDesignation, Diagnostic> {
        use optimization::digest_encoder::DomainDigestEncoder;
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
        let site = *self
            .build_sites
            .get(&(raw.lexical_owner, input.expression))
            .ok_or_else(|| refused("Product input has no exact emitted Build site"))?;
        let encoded = compiler_storage::encode(selected, Default::default()).map_err(refused)?;
        let owner = self.declarations.identity(raw.lexical_owner)?;
        let request = typed
            .expression_table
            .build_selection_request(input.request)
            .ok_or_else(|| refused("Product input lost its complete request"))?;
        let request_bytes =
            compiler_storage::encode(request, Default::default()).map_err(refused)?;
        let mut digest = DomainDigestEncoder::new(b"omega.source-product-input.v1\0");
        digest.bytes(owner.as_bytes());
        digest.bytes(&encoded);
        digest.bytes(&request_bytes);
        let input_selection = digest.finish();
        let typed_trees::expression::ExpressionNode::Call(call) =
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
                compiler_storage::encode(application, Default::default()).map_err(refused)?;
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
    let encoded = compiler_storage::encode(value, Default::default())
        .map_err(|error| refused(format!("intrinsic identity cannot be retained: {error}")))?;
    let mut identity = String::with_capacity(domain.len() + encoded.len() * 2);
    identity.push_str(domain);
    for byte in encoded {
        write!(identity, "{byte:02x}").expect("writing an owned string cannot fail");
    }
    Ok(identity)
}

struct Declarations<'source> {
    typed: &'source TypedTrees,
    ids: SymbolMap<SelectionDeclarationId>,
    identities: SymbolMap<String>,
    contracts: SymbolMap<Vec<RuntimeCallableContractHandle>>,
}

impl<'source> Declarations<'source> {
    fn new(
        source: &'source AcceptedTerminalSource,
        module: &TerminalModule,
    ) -> Result<Self, Diagnostic> {
        let mut identities = SymbolMap::default();
        let elaboration = &source.closure().elaboration;
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
            identities.insert(symbol, identity.to_owned());
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
            identities.insert(*symbol, identity.to_owned());
        }
        // Prepare this association once. Only an admitted contract's meaning
        // is normalized, so unrelated generic contracts never become work.
        let mut contracts: SymbolMap<Vec<RuntimeCallableContractHandle>> = SymbolMap::default();
        for (handle, contract) in source.source().runtime_callable_contracts.iter() {
            contracts
                .entry(contract.signature.symbol)
                .or_default()
                .push(handle);
        }
        Ok(Self {
            typed: source.source(),
            ids: SymbolMap::default(),
            identities,
            contracts,
        })
    }

    fn valid(&self, symbol: SymbolHandle) -> Result<(), Diagnostic> {
        if !symbol.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(refused(
                "source selection has a stale declaration coordinate",
            ));
        }
        Ok(())
    }

    fn purpose(&self, symbol: SymbolHandle) -> Result<SelectionPurpose, Diagnostic> {
        self.valid(symbol)?;
        let source = self
            .typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| self.typed.symbols.source_file(span))
            .ok_or_else(|| refused("source declaration has no retained source purpose"))?;
        Ok(match source.dependency_scope {
            source::DependencyScope::Build => SelectionPurpose::Build,
            source::DependencyScope::Product => SelectionPurpose::Product,
        })
    }

    fn identity(&mut self, symbol: SymbolHandle) -> Result<String, Diagnostic> {
        if let Some(identity) = self.identities.get(&symbol) {
            return Ok(identity.clone());
        }
        let identity = self
            .typed
            .normalized_hermetic_symbol_identity(symbol)
            .map_err(refused)?;
        self.identities.insert(symbol, identity.clone());
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

    fn declaration(
        &mut self,
        output: &mut SelectionQuestions,
        symbol: SymbolHandle,
    ) -> Result<SelectionDeclarationId, Diagnostic> {
        self.valid(symbol)?;
        if let Some(id) = self.ids.get(&symbol) {
            return Ok(*id);
        }
        let identity = self.identity(symbol)?;
        let callable_identity = self.callable_identity(symbol)?;
        let custody =
            structure_elaboration::selection_questions::declaration_custody(self.typed, symbol)
                .map_err(refused)?;
        let purpose = self.purpose(symbol)?;
        let id = output.declarations.push(SelectionDeclaration {
            identity,
            name: self.typed.symbols.display_path(symbol, "::"),
            custody,
            purpose,
            callable_identity,
        });
        self.ids.insert(symbol, id);
        Ok(id)
    }

    fn local_binding(
        &mut self,
        output: &mut SelectionQuestions,
        lexical_owner: SymbolHandle,
        symbol: SymbolHandle,
        declared_owner: SymbolHandle,
    ) -> Result<SelectionReference, Diagnostic> {
        self.valid(symbol)?;
        self.valid(lexical_owner)?;
        self.valid(declared_owner)?;
        if self.typed.symbols.get(symbol).parent != declared_owner {
            return Err(refused(
                "local selection changed its actual declaration owner",
            ));
        }
        // A formal belongs to its entry State, while the body is traversed in
        // its machine's lexical context. Other binders must be in that actual
        // context or one of its retained enclosing declaration scopes.
        let mut scope = lexical_owner;
        let mut visible = false;
        let mut scopes = Vec::new();
        while scope.is_valid() {
            self.valid(scope)?;
            if scopes.contains(&scope) {
                return Err(refused("local selection has a cyclic lexical scope"));
            }
            scopes.push(scope);
            if scope == declared_owner {
                visible = true;
                break;
            }
            if self.typed.machine_by_symbol(scope).is_some_and(|machine| {
                self.typed
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == declared_owner)
            }) {
                visible = true;
                break;
            }
            let parent = self.typed.symbols.get(scope).parent;
            scope = parent;
        }
        if !visible {
            return Err(refused(
                "selected local binding is outside its actual lexical context",
            ));
        }
        let kind = match self.typed.symbols.get(symbol).kind {
            SymbolKind::Parameter | SymbolKind::Local => SelectionBindingKind::Value,
            SymbolKind::TypeParameter => SelectionBindingKind::Type,
            SymbolKind::MachineParameter => SelectionBindingKind::Machine,
            SymbolKind::ConformanceParameter => SelectionBindingKind::Conformance,
            SymbolKind::PropositionParameter => SelectionBindingKind::Proof,
            SymbolKind::PropositionMachineParameter => SelectionBindingKind::PropositionMachine,
            _ => return Err(refused("resolved local selection names no actual binding")),
        };
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
    ) -> Result<SelectionReference, Diagnostic> {
        self.valid(symbol)?;
        if let Some(atom) = self.typed.symbols.builtin_type_atom(symbol) {
            return Ok(SelectionReference::Intrinsic {
                identity: intrinsic_identity(&atom, "builtin-type-v1:")?,
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
        match self.typed.symbols.get(symbol).kind {
            SymbolKind::Parameter
            | SymbolKind::Local
            | SymbolKind::TypeParameter
            | SymbolKind::MachineParameter
            | SymbolKind::ConformanceParameter
            | SymbolKind::PropositionParameter
            | SymbolKind::PropositionMachineParameter => {
                self.local_binding(output, owner, symbol, self.typed.symbols.get(symbol).parent)
            }
            _ => Ok(SelectionReference::Declaration(
                self.declaration(output, symbol)?,
            )),
        }
    }
}
