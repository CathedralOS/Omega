// Calling admission must follow the code and declarations in the shared
// module after source is released. Retain that correspondence here, before
// judgment; publishing it does not establish an input premise or a provider.

use semantic_vocabulary::MachineId;
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
};
use semantic_vocabulary::proof_applications::{
    ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{
    OperationKind, ProofApplicationScope, ProofCallableAssociation, ProofCallableHeader,
    ProofDeclarationId, ProofScopeDeclaration, TargetMachineCallable, TerminalAffineCleanupAction,
    Terminator,
};
use typed_trees::TypedTrees;

use crate::{Elaboration, Unserved, refuse};

pub(crate) fn declare(
    typed: &TypedTrees,
    elaboration: &mut Elaboration,
    selected_calls: &[crate::compilation::SelectedSourceCall<'_>],
) -> Checked<()> {
    let selection_requests = selection_requests(typed, elaboration, selected_calls)?;
    let mut scope = ProofApplicationScope::default();
    let mut declarations = Vec::with_capacity(elaboration.module.machines.len());
    for machine in &elaboration.module.machines {
        let id = u32::try_from(declarations.len())
            .ok()
            .and_then(|position| position.checked_add(1))
            .map(ProofDeclarationId)
            .ok_or_else(|| {
                refuse(
                    machine.id,
                    Construct::Machine,
                    Unserved::Declaration("proof declaration coordinate overflow"),
                )
            })?;
        let declaration = machine.declaration.clone().ok_or_else(|| {
            let reason = elaboration
                .undeclared
                .iter()
                .find_map(|(owner, reason)| (*owner == machine.id).then_some(*reason))
                .unwrap_or("proof scope without an exact machine declaration");
            refuse(
                machine.id,
                Construct::Machine,
                Unserved::Declaration(reason),
            )
        })?;
        scope
            .declarations
            .insert(ProofScopeDeclaration::Callable(declaration));
        declarations.push((machine.id, id));
    }
    declarations.sort_unstable_by_key(|(machine, _)| *machine);
    let declaration_for = |machine: MachineId| -> Result<ProofDeclarationId, String> {
        let position = declarations
            .binary_search_by_key(&machine, |(machine, _)| *machine)
            .map_err(|_| "proof call has no exact emitted declaration")?;
        Ok(declarations[position].1)
    };
    for machine in &elaboration.module.machines {
        let id = declaration_for(machine.id).map_err(|_| {
            refuse(
                machine.id,
                Construct::Machine,
                Unserved::Declaration("proof machine lost its declaration"),
            )
        })?;
        let symbol = *terminal_check_tables::row(
            &elaboration.machines,
            machine.id,
            machine.id,
            Construct::Machine,
        )?;
        let source = typed.machine_by_symbol(symbol).ok_or_else(|| {
            refuse(
                machine.id,
                Construct::Machine,
                Unserved::Declaration("proof machine lost its exact typed owner"),
            )
        })?;
        let saved = machine
            .declaration
            .as_ref()
            .expect("declarations were checked above");
        let mut calls = Vec::new();
        let mut cleanup = Vec::new();
        let mut unavailable = None;
        for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(callee) = operation.kind.direct_callee() {
                    match declaration_for(callee) {
                        Ok(callee) => calls.push(ProofApplicationCall {
                            callee,
                            ..ProofApplicationCall::default()
                        }),
                        Err(reason) => unavailable = Some(reason),
                    }
                }
                if matches!(
                    operation.kind,
                    OperationKind::CallDynamic { .. } | OperationKind::BoundaryCall { .. }
                ) {
                    unavailable = Some(
                        "proof scope needs the actual selected boundary correspondence".into(),
                    );
                }
            }
            match &block.terminator {
                Terminator::InvokeNoNormal { callee, .. } => match declaration_for(*callee) {
                    Ok(callee) => calls.push(ProofApplicationCall {
                        callee,
                        ..ProofApplicationCall::default()
                    }),
                    Err(reason) => unavailable = Some(reason),
                },
                Terminator::ExternalCompletion { .. } => {
                    unavailable = Some(
                        "proof scope needs the actual external completion correspondence".into(),
                    )
                }
                _ => {}
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let TerminalAffineCleanupAction::InvokeNominal(action) = action {
                        match declaration_for(action.cleanup_machine) {
                            Ok(target) => cleanup.push(target),
                            Err(reason) => unavailable = Some(reason),
                        }
                    }
                }
            }
        }
        if source.target.is_some()
            || source.authored_target.is_some()
            || source.target_family_interface
        {
            unavailable = Some(
                "proof scope needs the actual target-family declaration correspondence".into(),
            );
        }
        let package = typed.symbols.symbol_package_identity(symbol);
        let custody = declaration_custody(typed, symbol);
        let evaluation = (|| {
            if let Some(reason) = unavailable {
                return Err(reason);
            }
            if machine
                .declared_service_reach
                .as_ref()
                .is_some_and(|reach| !reach.is_empty())
                || !machine.published_service_ceiling.is_empty()
                || machine.closed_reach_application.is_some()
            {
                return Err("proof scope needs the complete service reach correspondence".into());
            }
            let local_terminates =
                terminal_semantics::control_graph::control_cycle_members(machine)
                    .map_err(|reason| {
                        format!("proof scope has an invalid control graph: {reason:?}")
                    })?
                    .is_empty();
            let mut linear_carrier = None;
            for identity in machine
                .structural_parameters
                .iter()
                .map(|parameter| parameter.structural_type)
                .chain(
                    machine
                        .structural_places
                        .iter()
                        .filter_map(|place| place.kind.produced_structural_type()),
                )
                .chain(
                    machine
                        .result
                        .structural()
                        .map(|result| result.structural_type),
                )
            {
                let position = usize::try_from(identity.get() - 1)
                    .map_err(|_| "proof storage type coordinate is out of range")?;
                let row = elaboration
                    .module
                    .structural_types
                    .get(position)
                    .filter(|row| row.id == identity)
                    .ok_or("proof storage has no exact structural type declaration")?;
                if !row.no_code_affine_cleanup || !row.descriptor_cleanup || row.owns_linear_claim {
                    linear_carrier = Some(
                        "compiler evaluation retains a runtime carrier or cleanup duty".to_owned(),
                    );
                }
            }
            let name = elaboration
                .module
                .identities
                .text(saved.callable)
                .filter(|identity| !identity.is_empty())
                .ok_or("proof machine has no exact callable identity")?
                .to_owned();
            Ok(CompilerEvaluationDeclaration {
                name,
                custody,
                effects: CompilerEvaluationEffects {
                    services: Vec::new(),
                    may_suspend: saved.published_may_suspend,
                    may_block: saved.published_may_block,
                },
                local_terminates,
                entry_requires: !machine.contract.requires.is_empty()
                    || !machine.contract.source_checked_requires.is_empty(),
                linear_carrier,
                static_application: None,
                selections: selection_requests
                    .get(machine.id)
                    .ok_or("proof selection requests lost their emitted caller")?
                    .clone(),
            })
        })();
        scope.callable_headers.insert(ProofCallableHeader {
            declaration: id,
            callable: Ok(saved.callable),
            package,
            source_target: source
                .target
                .as_ref()
                .map(|target| target.as_str().to_owned()),
            product_eligible: true,
            // The declaration has been read, not accepted yet. The accepted
            // closure marks only its exact discharged body as accepted.
            supplies_checked_body: false,
            type_parameter_count: typed.machine_type_parameters(source).len(),
            compiler_evaluation: evaluation,
            // A machine with no `invokes` clause declares the empty list. A
            // written clause names requirement identities, which this header
            // has no complete declaration for, so it is unavailable here.
            synchronous_invocations: if source.invokes.is_empty() {
                Ok(Vec::new())
            } else {
                Err("proof scope needs complete synchronous invocation declarations".into())
            },
        });
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration: id,
            callable: TargetMachineCallable::Machine(machine.id),
        });
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: id,
            product_selected: true,
            ..ProofApplicationMachine::default()
        });
        let calls = scope.calls.calls.insert_many(calls);
        let cleanup = scope.calls.cleanup.insert_many(cleanup);
        scope.calls.insert_machine(ProofCallMachine {
            machine: id,
            calls,
            cleanup,
        });
    }
    scope.calls.index_machines();
    scope.plan.declarations = scope
        .plan
        .symbols
        .insert_many(declarations.iter().map(|(_, declaration)| *declaration));
    scope.plan.obligation_owners = scope
        .plan
        .symbols
        .insert_many(declarations.iter().filter_map(|(machine, declaration)| {
            elaboration
                .tables
                .machines
                .get(*machine)
                .filter(|table| !table.obligations.is_empty())
                .map(|_| *declaration)
        }));
    scope.plan.has_obligations = !scope.plan.obligation_owners.is_empty();
    if elaboration.module.scalar_applications.proof_scope != ProofApplicationScope::default() {
        let machine = elaboration
            .module
            .machines
            .first()
            .map(|machine| machine.id)
            .unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
        return Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Declaration("competing proof-scope publication"),
        ));
    }
    elaboration.module.scalar_applications.proof_scope = scope;
    Ok(())
}

fn declaration_custody(typed: &TypedTrees, symbol: symbols::SymbolHandle) -> DeclarationCustody {
    if !typed.symbols.symbols().nodes().is_valid(symbol) {
        return DeclarationCustody::Missing;
    }
    match (
        typed.symbols.symbol_package_identity(symbol),
        typed.symbols.symbol_source_origin(symbol),
    ) {
        (Some(package), _) => DeclarationCustody::Package(package),
        (_, Some(source::SourceOrigin::Toolchain)) => DeclarationCustody::Toolchain,
        (_, Some(source::SourceOrigin::ToolchainContract)) => DeclarationCustody::ToolchainContract,
        (_, Some(source::SourceOrigin::User)) => DeclarationCustody::UnownedUser,
        _ if typed.symbols.builtin_type_atom(symbol).is_some() => DeclarationCustody::Toolchain,
        _ => DeclarationCustody::Missing,
    }
}

fn selection_requests(
    typed: &TypedTrees,
    elaboration: &Elaboration,
    calls: &[crate::compilation::SelectedSourceCall<'_>],
) -> Checked<
    semantic_vocabulary::IdTable<
        MachineId,
        Result<Vec<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>, String>,
    >,
> {
    use semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest;
    let mut requests = semantic_vocabulary::IdTable::default();
    for _ in &elaboration.module.machines {
        requests.push(Ok(Vec::<DeclarationAuthorityRequest>::new()));
    }
    for selected in calls {
        let result = (|| {
            let (member, required, body) = selected.key;
            let owner = *elaboration
                .machines
                .get(member)
                .ok_or("selected proof call lost its source member")?;
            let header = elaboration
                .module
                .invocation_machine_contract(body, Some(required))
                .ok_or("selected proof call lost its exact public contract and implementation")?;
            if header.selected_application.is_none() {
                return Err("selected proof call has no complete closed application".to_owned());
            }
            let (operand, supplier) = typed.source_call_selected_operand(owner, selected.call)?;
            let conformance = typed
                .conformance_by_symbol(operand.symbol)
                .ok_or("selected proof call lost its implementation declaration")?;
            let mut references = SelectionReferences::new(typed, supplier);
            // The caller supplied this operand. The implementation declaration,
            // separately, selected its trait and that trait's explicit arguments.
            // Its source scope cannot replace the original caller's package edge.
            typed_trees::references::visit_static_argument(typed, &mut references, &operand);
            references.finish()?;
            let mut declared = SelectionReferences::new(typed, conformance.symbol);
            declared.extend([conformance.trait_symbol]);
            for argument in typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
            {
                typed_trees::references::visit_type_reference(typed, &mut declared, *argument);
            }
            let mut method = SelectionReferences::new(typed, {
                let actual = selected.call.lexical_owner(typed);
                if actual.is_valid() { actual } else { owner }
            });
            for argument in selected.call.arguments(typed) {
                typed_trees::references::visit_static_argument(typed, &mut method, argument);
            }
            let mut all = references.requests;
            declared.finish()?;
            method.finish()?;
            all.extend(declared.requests);
            all.extend(method.requests);
            Ok(all)
        })();
        let target = requests.get_mut(selected.emitted_caller).ok_or_else(|| {
            refuse(
                selected.emitted_caller,
                Construct::Machine,
                Unserved::Declaration("selected call has no emitted proof-scope caller"),
            )
        })?;
        match (target, result) {
            (Ok(known), Ok(incoming)) => known.extend(incoming),
            (target @ Ok(_), Err(reason)) => *target = Err(reason),
            (Err(_), _) => {}
        }
    }
    for machine in &elaboration.module.machines {
        let result = requests.get_mut(machine.id).ok_or_else(|| {
            refuse(
                machine.id,
                Construct::Machine,
                Unserved::Declaration("proof selection requests lost their emitted caller"),
            )
        })?;
        if let Ok(requests) = result {
            requests.sort_unstable_by_key(|request| (request.requester, request.owner));
            requests.dedup();
        }
    }
    Ok(requests)
}

/// Use the existing source-reference walker for the operand's declaration
/// choices. An alias retains its authored selection and arguments; its
/// expanded carrier is a semantic dependency, not a new caller selection.
struct SelectionReferences<'source> {
    typed: &'source TypedTrees,
    supplier: symbols::SymbolHandle,
    requests: Vec<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>,
    visited: Vec<(u32, u32)>,
    failed: Option<String>,
}

impl<'source> SelectionReferences<'source> {
    fn new(typed: &'source TypedTrees, supplier: symbols::SymbolHandle) -> Self {
        Self {
            typed,
            supplier,
            requests: Vec::new(),
            visited: Vec::new(),
            failed: None,
        }
    }
    fn finish(&self) -> Result<(), String> {
        self.failed
            .as_ref()
            .map_or(Ok(()), |reason| Err(reason.clone()))
    }
}

impl Extend<symbols::SymbolHandle> for SelectionReferences<'_> {
    fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
        use semantic_vocabulary::compiler_evaluation::declaration_authority_request;
        for symbol in symbols {
            if self.failed.is_some() {
                return;
            }
            match declaration_authority_request(
                declaration_custody(self.typed, self.supplier),
                declaration_custody(self.typed, symbol),
            ) {
                Ok(Some(request)) => self.requests.push(request),
                Ok(None) => {}
                Err(reason) => self.failed = Some(reason.to_owned()),
            }
        }
    }
}

impl typed_trees::references::TypedReferences for SelectionReferences<'_> {
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        if self.failed.is_some() {
            return false;
        }
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.failed = Some("selected application has a stale argument type".to_owned());
            return false;
        }
        let key = (reference.arena_index(), reference.generation());
        match self.visited.binary_search(&key) {
            Ok(_) => return false,
            Err(position) => self.visited.insert(position, key),
        }
        if self
            .typed
            .static_const_literal_from_type_reference(reference)
            .is_some()
        {
            return false;
        }
        let aliases = self
            .typed
            .type_reference_table
            .type_alias_applications(reference)
            .cloned()
            .collect::<Vec<_>>();
        if !aliases.is_empty() {
            for alias in aliases {
                self.extend([alias.declaration]);
                for argument in self
                    .typed
                    .type_reference_table
                    .type_reference_handles(alias.arguments)
                {
                    typed_trees::references::visit_type_reference(self.typed, self, *argument);
                }
            }
            return false;
        }
        true
    }
    fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        if self.typed.expression_table.expression_is_valid(expression) {
            return self.failed.is_none();
        }
        self.failed = Some("selected application has a stale argument expression".to_owned());
        false
    }
    fn declaration_description_dependency(&mut self, symbol: symbols::SymbolHandle) {
        self.extend([symbol]);
    }
}
