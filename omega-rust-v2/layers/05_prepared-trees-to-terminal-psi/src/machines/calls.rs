use crate::lowering::applied_tree::LoweringTree;
use omega_resolved::DeclarationId;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, PauseEffects, TypedSignature, ValueType};

use crate::{LoweringError, MachineId, site_at};

pub(crate) type LocalCallees<'a> = std::collections::HashMap<&'a DeclarationId, std::collections::HashMap<&'a [ValueType], Callee<'a>>>;

#[derive(Default)]
pub(crate) struct Callees<'a> {
    pub local: LocalCallees<'a>,
    pub machine_pause: Vec<PauseEffects>,
    /// The published `requires` count of each runtime callable contract, by
    /// `CallableContractId` index. A call through the contract owes each clause.
    pub runtime_requirements: Vec<usize>,
    pub members: std::collections::HashMap<crate::declarations::members::Key, (usize, &'a TypedSignature, usize, PauseEffects)>,
    /// Local logical theorems, which a body cites by erased application.
    pub theorems: std::collections::HashMap<&'a DeclarationId, super::theorem_citations::TheoremCallee<'a>>,
}

impl Callees<'_> {
    pub(crate) fn realized_pause(
        &self,
        id: MachineId,
    ) -> PauseEffects {
        self.machine_pause[(id.get() - 1) as usize]
    }
    pub(super) fn may_park(
        &self,
        target: crate::CallTarget,
    ) -> bool {
        matches!(target, crate::CallTarget::Machine(id) if self.realized_pause(id).suspend)
    }
}

pub(crate) struct Callee<'a> {
    pub machine: MachineId,
    pub signature: &'a TypedSignature,
    pub callbacks: &'a [crate::lowering::callbacks::Callback<'a>],
    pub restricted_arguments: Vec<(omega_typed::RequirementSlot, DeclarationId)>,
    pub requirement_count: usize,
}

pub(crate) fn record_acknowledgement(
    body: &mut crate::MachineBody,
    call: crate::CallSite,
    effects: PauseEffects,
) {
    if effects != PauseEffects::default() {
        body.call_acknowledgements
            .push(crate::CallAcknowledgement { call, effects });
    }
}

/// The clauses a call owes and, for a closed callback invocation, the binder
/// whose interface owns them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OwedClauses {
    pub count: usize,
    pub binder: Option<usize>,
}

impl OwedClauses {
    pub(crate) fn callee(count: usize) -> Self {
        Self { count, binder: None }
    }
}

/// Publish the call's obligations at its final site. A callback invocation is
/// listed so its clause positions refer to the binder, not to the callee.
pub(crate) fn owe(
    body: &mut crate::MachineBody,
    call: crate::CallSite,
    owed: OwedClauses,
) -> Vec<crate::CallRequirement> {
    if let Some(binder) = owed.binder {
        body.callback_invocations.push(crate::CallbackInvocation { call, binder });
    }
    crate::declarations::contracts::call_requirements(call, owed.count)
}

/// Once selection is complete, ordinary and terminal calls use the same
/// operand lowering for static targets and runtime descriptors.
pub(super) struct SelectedCall<'a> {
    pub acknowledgements: PauseEffects,
    pub callee: crate::CallTarget,
    pub operands: Vec<(usize, &'a ValueType)>,
    pub owed: OwedClauses,
}

pub(super) fn select_call<'a>(
    expressions: &super::expressions::Expressions<'a>,
    node: usize,
    expected: &ValueType,
    body: &crate::MachineBody,
) -> Result<SelectedCall<'a>, LoweringError> {
    if let Some(call) = super::runtime_calls::select_runtime_call(expressions, node, expected, body)? {
        return Ok(call);
    }
    select_direct_call(expressions.tree, expressions.signature, node, expected, expressions.types, expressions.callees)
}

fn select_direct_call<'a>(
    tree: &'a LoweringTree<'_>,
    caller: &TypedSignature,
    node: usize,
    expected: &ValueType,
    types: &crate::LoweringTypes,
    callees: &Callees<'a>,
) -> Result<SelectedCall<'a>, LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let error = |message| LoweringError::new(&site, message);
    let mut selected = None;
    let mut typed_result = false;
    let mut precondition = None;
    let mut refinements = Vec::new();
    let mut evidence = Vec::new();
    let mut crash_coverage = Vec::new();
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Call(call) if call.use_site == CallUse::Execution => {
                if selected.replace(call).is_some_and(|previous| previous != call) {
                    return Err(error("call has multiple executable selections"));
                }
            }
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() && &expression.ty == expected => {
                typed_result = true;
            }
            NodeSemantics::Obligation(obligation) => match &obligation.kind {
                ObligationKind::RefineEvidence { binder, implementation } if obligation.node == file.node_id(node) && obligation.site == site => {
                    evidence.push((binder, implementation));
                }
                ObligationKind::RefineCallable {
                    formal: Some((owner, slot)),
                    contract,
                    selection,
                } if obligation.node == file.node_id(node) && obligation.site == site => refinements.push((owner, *slot, contract.as_ref(), selection)),
                ObligationKind::ReturnBorrow { machine }
                    if crate::lowering::types::is_reference_descriptor(expected)
                        && machine == &caller.machine
                        && obligation.node == file.node_id(node)
                        && obligation.site == site => {}
                ObligationKind::CrashCoverage { machine, cause }
                    if machine == &caller.machine && obligation.node == file.node_id(node) && obligation.site == site =>
                {
                    crash_coverage.push(*cause);
                }
                ObligationKind::CallPrecondition(machine) if precondition.is_none() && obligation.node == file.node_id(node) && obligation.site == site => {
                    precondition = Some(machine);
                }
                _ => {
                    return Err(LoweringError::new(
                        &obligation.site,
                        format!("unsupported call obligation: {:?}", obligation.kind),
                    ));
                }
            },
            NodeSemantics::IndirectCall(_) => return Err(error("indirect calls require callable descriptor lowering")),
            _ => return Err(error("unsupported typed call record")),
        }
    }
    let call = selected.ok_or_else(|| error("call has no executable direct selection"))?;
    if call.node != file.node_id(node) || call.site != site || call.caller.as_ref() != Some(&caller.machine) {
        return Err(error("selected call disagrees with its owning occurrence"));
    }
    if call.layout_query.is_some() {
        return Err(error("selected layout query requires target lowering"));
    }

    let (target, signature, requirement_count, callbacks, possible) = if let Some(key) = crate::declarations::members::Key::from_call(call) {
        let &(index, signature, count, pause) = callees
            .members
            .get(&key)
            .ok_or_else(|| error("selected member has no packaged applied requirement"))?;
        if !evidence.is_empty() {
            return Err(error("unexpected evidence refinement at member dispatch"));
        }
        if pause != PauseEffects::default() {
            return Err(error("member effect requires its selected suspension contract"));
        }
        (crate::CallTarget::Member(index), signature, count, &[][..], PauseEffects::default())
    } else {
        if call.evidence.is_some() || !call.enclosing_arguments.is_empty() {
            return Err(error("call lacks an exact applied requirement owner"));
        }
        let callee = callees
            .local
            .get(&call.machine)
            .and_then(|entries| entries.get(call.type_arguments.as_slice()))
            .ok_or_else(|| error("selected callee has no local machine identity"))?;
        // The machine ID on the call leads to the ordered static arguments in Psi.
        // Match the obligations to those exact selected pairs, including multiplicity.
        for (bound, selected) in &callee.restricted_arguments {
            let Some(index) = evidence.iter().position(|&pair| pair == (bound, selected)) else {
                return Err(error("call lacks its selected evidence refinement"));
            };
            evidence.swap_remove(index);
        }
        if !evidence.is_empty() {
            return Err(error("evidence refinement disagrees with the selected application"));
        }
        (
            crate::CallTarget::Machine(callee.machine),
            callee.signature,
            callee.requirement_count,
            callee.callbacks,
            callees.realized_pause(callee.machine),
        )
    };
    if refinements.len() != callbacks.len()
        || callbacks.iter().any(|binding| {
            !refinements.iter().any(|(owner, slot, contract, selected)| {
                *owner == &call.machine
                    && *slot == binding.slot
                    && *contract == binding.bound
                    && selected.machine.as_ref() == &binding.selection.machine
                    && selected.arguments.is_empty()
                    && selected.enclosing_arguments.is_empty()
            })
        })
    {
        return Err(error("static call lacks the exact callable refinement for its selected instance"));
    }
    // L3 marks explicit preconditions. Psi also gives each call the predicates
    // on its callee's qualified formals. An invocation of a closed static binder
    // owes the binder's clauses instead: typing records no direct precondition
    // there, and the selection's refinement covers the callee's own premises.
    let owed = if let Some((binder, count)) = tree.callback_invocation(node) {
        if precondition.is_some() || !matches!(target, crate::CallTarget::Machine(_)) {
            return Err(error("selected call precondition records disagree with its callee contract"));
        }
        OwedClauses { count, binder: Some(binder) }
    } else {
        if precondition != signature.requires.then_some(&call.machine) {
            return Err(error("selected call precondition records disagree with its callee contract"));
        }
        OwedClauses::callee(requirement_count)
    };
    // L3's occurrence obligations describe the invoked interface. For a
    // closed routed binder that interface deferred crashes to the selection;
    // the independently joined guarded-call rows supply its dependency.
    let declared = tree.callback_routes.iter()
        .find(|(invocation, _)| invocation.node == call.node)
        .map_or(signature.crash_ceiling.as_deref(), |(invocation, _)| invocation.contract.crash_ceiling.as_deref());
    crate::declarations::crash_routes::check_call_coverage(&site, declared, &crash_coverage)?;
    if signature.machine != call.machine || signature.result != call.result || !typed_result || &call.result != expected {
        return Err(error("selected call result disagrees with its normal continuation"));
    }
    if signature.result != ValueType::NoResult
        && !(types.can_forward_value(&signature.result)
            || crate::lowering::types::contains_nominal_value(&signature.result)
            || (matches!(target, crate::CallTarget::Machine(_)) && crate::lowering::types::place_reference(&signature.result).is_some()))
    {
        return Err(error("call result requires unsupported carrier lowering"));
    }

    // The selected call supplies operands in evaluation order for both token and named syntax.
    if call.operands.len() != signature.parameters.len() {
        return Err(error("selected callee and operand list disagree"));
    }
    let mut operands = Vec::with_capacity(call.operands.len());
    for (applied, (span, formal)) in call.operands.iter().zip(&signature.parameters) {
        let argument = applied.node.node;
        if argument >= file.nodes.len()
            || applied.node != file.node_id(argument)
            || applied.site != site_at(tree, argument)
            || applied.parameter.package != signature.site.package
            || applied.parameter.file != signature.site.file
            || applied.parameter.span != *span
        {
            return Err(error("selected operand disagrees with its argument occurrence or formal"));
        }
        if matches!(formal, ValueType::Callable(_)) && types.id_for(formal).is_ok() {
            super::runtime_calls::check_callable_operand(tree, caller, applied)?;
        } else if applied.callable.is_some()
            || !(types.can_forward_value(formal) || crate::lowering::types::contains_nominal_value(formal) || crate::lowering::types::place_reference(formal).is_some())
        {
            return Err(LoweringError::new(&applied.site, "call operand requires unsupported carrier lowering"));
        }
        operands.push((argument, formal));
    }

    if (possible.block && !call.acknowledgements.block) || (possible.suspend && !call.acknowledgements.suspend) {
        return Err(error("call lacks the acknowledgement for its selected operational effect"));
    }
    if possible.block {
        return Err(error("selected blocking call requires effect lowering"));
    }
    Ok(SelectedCall {
        acknowledgements: call.acknowledgements,
        callee: target,
        operands,
        owed,
    })
}
