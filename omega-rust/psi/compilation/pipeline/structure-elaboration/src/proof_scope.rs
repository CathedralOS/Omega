// Calling admission must follow the code and declarations in the shared
// module after source is released. Retain that correspondence here, before
// judgment; publishing it does not establish an input premise or a provider.

use semantic_vocabulary::MachineId;
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects,
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

pub(crate) fn declare(typed: &TypedTrees, elaboration: &mut Elaboration) -> Checked<()> {
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
        // A generated static selection needs its complete applied declaration
        // authority, not just the direct body the lowered call reaches.
        if elaboration
            .selections
            .iter()
            .any(|selected| selected.caller == machine.id)
        {
            unavailable = Some(
                "proof scope needs the complete static selection authority correspondence".into(),
            );
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
        let custody =
            crate::selection_questions::declaration_custody(typed, symbol).map_err(|reason| {
                terminal_check_tables::Refusal {
                    reason: terminal_check_tables::RefusalReason::Unsupported,
                    machine: machine.id,
                    concerns: Construct::Machine,
                    detail: reason.to_string(),
                }
            })?;
        let evaluation = (|| {
            if let Some(reason) = unavailable {
                return Err(reason);
            }
            if !machine.declared_service_reach.is_empty()
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
                selections: Ok(Vec::new()),
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
