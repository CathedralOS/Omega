//! A program may use only declarations its package can select. Match retained
//! source reads to the calls and values in the published program before checking
//! the dependency permissions for each read's Build or Product purpose.

use semantic_vocabulary::compiler_evaluation::{DeclarationCustody, DeclarationSelectionAuthority};
use semantic_vocabulary::source_selections::*;
use semantic_vocabulary::{
    CallableInvocationSite, MachineId, SelectionDeclarationId, SelectionQuestionId,
    TargetMachineCallable,
};
use terminal_psi::{OperationKind, TerminalModule};

#[cfg(test)]
mod tests;

pub struct SelectionAuthorities<'a> {
    pub build: Option<&'a dyn DeclarationSelectionAuthority>,
    pub product: Option<&'a dyn DeclarationSelectionAuthority>,
}

impl SelectionAuthorities<'_> {
    fn for_purpose(&self, purpose: SelectionPurpose) -> Option<&dyn DeclarationSelectionAuthority> {
        match purpose {
            SelectionPurpose::Build => self.build,
            SelectionPurpose::Product => self.product,
        }
    }
}

fn declaration(
    packet: &SelectionQuestions,
    id: SelectionDeclarationId,
) -> Result<&SelectionDeclaration, String> {
    packet
        .declarations
        .get(id)
        .ok_or_else(|| format!("source selection names absent declaration {id}"))
}

fn question(
    packet: &SelectionQuestions,
    id: SelectionQuestionId,
) -> Result<&SelectionQuestion, String> {
    packet
        .questions
        .get(id)
        .ok_or_else(|| format!("source selection join names absent question {id}"))
}

fn published_question(
    packet: &SelectionQuestions,
    id: SelectionQuestionId,
) -> Result<&SelectionQuestion, String> {
    let row = question(packet, id)?;
    if row.form != SelectionForm::Published {
        return Err(format!(
            "semantic form question {id} was joined to the published module"
        ));
    }
    Ok(row)
}

fn actual_calls(
    module: &TerminalModule,
) -> Result<Vec<((MachineId, CallableInvocationSite), SelectionCallTarget)>, String> {
    let mut calls = Vec::new();
    for machine in &module.machines {
        for call in machine.module_invocations() {
            calls.push((
                (machine.id, CallableInvocationSite::Invocation(call.site)),
                SelectionCallTarget::Declared(TargetMachineCallable::Machine(call.inputs.callee)),
            ));
        }
        for call in machine.callable_invocations() {
            calls.push((
                (machine.id, CallableInvocationSite::Invocation(call.site)),
                SelectionCallTarget::Selected {
                    contract: call.inputs.contract,
                    selector: call.inputs.callee,
                },
            ));
        }
        for call in machine.boundary_invocations() {
            calls.push((
                (machine.id, CallableInvocationSite::Invocation(call.site)),
                SelectionCallTarget::Declared(TargetMachineCallable::Boundary(call.boundary)),
            ));
        }
        for block in &machine.blocks {
            for (edge, actions) in block.terminator.cleanup_rosters() {
                for (ordinal, action) in actions.iter().enumerate() {
                    if let Some(target) = action.invoked_machine() {
                        calls.push((
                            (
                                machine.id,
                                CallableInvocationSite::CleanupAction {
                                    edge,
                                    action_ordinal: u32::try_from(ordinal).map_err(
                                        |_| "cleanup ordinal exceeds its selection namespace",
                                    )?,
                                },
                            ),
                            SelectionCallTarget::Declared(TargetMachineCallable::Machine(target)),
                        ));
                    }
                }
            }
        }
    }
    calls.sort_unstable_by_key(|(site, _)| *site);
    if calls.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err("published selection has repeated invocation coordinates".into());
    }
    Ok(calls)
}

fn lookup<Key: Ord + Copy, Value>(rows: &[(Key, Value)], key: Key) -> Option<&Value> {
    rows.binary_search_by_key(&key, |(key, _)| *key)
        .ok()
        .map(|index| &rows[index].1)
}

fn cover<Key: Ord + Copy, Value>(
    rows: &[(Key, Value)],
    covered: &mut [bool],
    key: Key,
) -> Result<(), ()> {
    let index = rows
        .binary_search_by_key(&key, |(key, _)| *key)
        .map_err(|_| ())?;
    if std::mem::replace(&mut covered[index], true) {
        Err(())
    } else {
        Ok(())
    }
}

fn origin(
    rows: &[(
        (MachineId, SelectionDeclarationId),
        semantic_vocabulary::BlockId,
    )],
    machine: MachineId,
    source: SelectionDeclarationId,
) -> Option<semantic_vocabulary::BlockId> {
    lookup(rows, (machine, source)).copied()
}

/// The caller must supply the private source-issued companion, not decoded data
/// presented as authority. This function validates its data and physical coverage.
pub fn validate_selection_questions(
    module: &TerminalModule,
    packet: &SelectionQuestions,
) -> Result<(), String> {
    let mut identities = Vec::with_capacity(packet.declarations.len());
    for (_, row) in packet.declarations.iter() {
        if row.identity.is_empty() {
            return Err("source selection declaration has no canonical identity".into());
        }
        if row.callable_identity.as_deref() == Some("") {
            return Err("source selection declaration has an empty callable identity".into());
        }
        identities.push((&row.identity, row.purpose));
    }
    identities.sort_unstable();
    if identities.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err("source selection dictionary repeats a declaration in one purpose".into());
    }
    for (id, row) in packet.questions.iter() {
        declaration(packet, row.occurrence.lexical_owner)?;
        match &row.selected {
            SelectionReference::Declaration(selected) => {
                declaration(packet, *selected)?;
            }
            SelectionReference::RuntimeContract { identity } => {
                if identity.is_empty()
                    || !matches!(
                        row.role,
                        SelectionRole::Contract | SelectionRole::Executable
                    )
                {
                    return Err(format!(
                        "selection question {id} has no actual runtime callable contract"
                    ));
                }
            }
            SelectionReference::ProductInput { input_selection } => {
                if !matches!(&row.role, SelectionRole::ProductDesignation(designation)
                    if designation.input_selection == *input_selection)
                {
                    return Err(format!(
                        "selection question {id} has a deferred Product input outside its designated operand"
                    ));
                }
            }
            SelectionReference::Intrinsic { identity } if identity.is_empty() => {
                return Err(format!(
                    "selection question {id} has no exact intrinsic identity"
                ));
            }
            SelectionReference::Intrinsic { .. } => (),
            SelectionReference::LocalBinding {
                owner,
                index,
                generation,
                ..
            } => {
                declaration(packet, *owner)?;
                if *owner != row.occurrence.lexical_owner || *index == 0 || *generation == 0 {
                    return Err(format!(
                        "selection question {id} has no authenticated local binding context"
                    ));
                }
            }
        }
        let coordinate = match row.occurrence.node {
            SelectionNode::AuthoredSelection { .. } => None,
            SelectionNode::Declaration { index, generation }
            | SelectionNode::Expression { index, generation }
            | SelectionNode::Statement { index, generation }
            | SelectionNode::TypeReference { index, generation } => Some((index, generation)),
        };
        if coordinate.is_some_and(|(index, generation)| index == 0 || generation == 0) {
            return Err(format!(
                "selection question {id} has an invalid source node"
            ));
        }
        if let SelectionRole::ProductDesignation(designation) = &row.role {
            if row.purpose != SelectionPurpose::Product
                || designation.input_selection == [0; 32]
                || designation.static_application == Some([0; 32])
            {
                return Err(format!(
                    "selection question {id} has no authenticated Product input association"
                ));
            }
        } else if declaration(packet, row.occurrence.lexical_owner)?.purpose != row.purpose {
            return Err(format!(
                "selection question {id} changes its lexical source purpose"
            ));
        }
    }
    let calls = actual_calls(module)?;
    let mut origins = Vec::new();
    let mut boundary_origins = Vec::new();
    let mut constructed = Vec::new();
    for machine in &module.machines {
        for operation in machine.operations() {
            if let OperationKind::SelectMachine { descriptor } = operation.kind {
                constructed.push(((machine.id, operation.id), descriptor));
            }
        }
    }
    constructed.sort_unstable_by_key(|(site, _)| *site);
    if constructed.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err("published selection repeats a descriptor construction coordinate".into());
    }
    // Establish original declarations before checking calls into a folded body.
    for join in &packet.joins {
        match join {
            SelectionJoin::Machine {
                declaration: source,
                machine,
                head,
            } => {
                declaration(packet, *source)?;
                let body = module
                    .machine(*machine)
                    .ok_or("source selection origin has no emitted machine")?;
                if !body.blocks.iter().any(|block| block.id == *head) {
                    return Err("source selection origin has no actual machine head".into());
                }
                origins.push(((*machine, *source), *head));
            }
            SelectionJoin::Boundary {
                declaration: source,
                boundary,
            } => {
                let source_id = *source;
                let source = declaration(packet, source_id)?;
                let row = module
                    .boundary_machine(*boundary)
                    .and_then(|row| row.declaration.as_ref())
                    .ok_or("source selection boundary has no checked declaration")?;
                if module.identities.text(row.source.semantic_declaration)
                    != Some(source.identity.as_str())
                {
                    return Err(
                        "source selection boundary differs from its checked semantic declaration"
                            .into(),
                    );
                }
                boundary_origins.push((*boundary, source_id));
            }
            SelectionJoin::Structure {
                declaration: source,
                structural_type,
                storage_identity,
            } => {
                declaration(packet, *source)?;
                let row =
                    terminal_psi::find_structural_type(&module.structural_types, *structural_type)
                        .ok_or("source selection structure has no emitted carrier")?;
                if module.identities.text(row.identity) != Some(storage_identity.as_str()) {
                    return Err(
                        "source selection structure differs from its applied storage identity"
                            .into(),
                    );
                }
            }
            _ => (),
        }
    }
    origins.sort_unstable_by_key(|(key, _)| *key);
    if origins.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err("source selection repeats a machine origin".into());
    }
    boundary_origins.sort_unstable_by_key(|(key, _)| *key);
    if boundary_origins
        .windows(2)
        .any(|rows| rows[0].0 == rows[1].0)
    {
        return Err("source selection repeats a boundary origin".into());
    }
    for machine in &module.machines {
        let checked = machine
            .declaration
            .as_ref()
            .ok_or("source selection machine has no checked semantic declaration")?;
        let start = origins.partition_point(|((id, _), _)| *id < machine.id);
        let end = origins.partition_point(|((id, _), _)| *id <= machine.id);
        if !origins[start..end].iter().any(|((_, id), head)| {
            *head == machine.entry
                && packet.declarations.get(*id).is_some_and(|row| {
                    module.identities.text(checked.semantic_declaration)
                        == Some(row.identity.as_str())
                })
        }) {
            return Err("published machine origins omit its checked semantic declaration".into());
        }
    }
    if module
        .boundary_machines
        .iter()
        .any(|row| lookup(&boundary_origins, row.id).is_none())
    {
        return Err("published boundary has no source selection origin".into());
    }
    let mut covered_calls = vec![false; calls.len()];
    let mut covered_values = vec![false; constructed.len()];
    let mut joined_questions = Vec::new();
    let mut folded_edges = Vec::new();
    for join in &packet.joins {
        match join {
            SelectionJoin::Invocation {
                question: id,
                original_caller,
                machine,
                site,
                target,
            } => {
                let row = published_question(packet, *id)?;
                if row.occurrence.lexical_owner != *original_caller
                    || origin(&origins, *machine, *original_caller).is_none()
                {
                    return Err("invocation selection lost its original lexical caller".into());
                }
                if lookup(&calls, (*machine, *site)) != Some(target) {
                    return Err(
                        "invocation selection differs from its actual emitted target".into(),
                    );
                }
                if let SelectionReference::Declaration(selected) = &row.selected {
                    let same = match target {
                        SelectionCallTarget::Declared(TargetMachineCallable::Machine(target)) => {
                            origin(&origins, *target, *selected).is_some()
                        }
                        SelectionCallTarget::Declared(TargetMachineCallable::Boundary(target)) => {
                            lookup(&boundary_origins, *target) == Some(selected)
                        }
                        SelectionCallTarget::Selected { contract, .. } => module
                            .callables
                            .contract(*contract)
                            .is_some_and(|contract| {
                                packet
                                    .declarations
                                    .get(*selected)
                                    .and_then(|row| row.callable_identity.as_deref())
                                    .is_some_and(|identity| {
                                        module.identities.text(contract.identity) == Some(identity)
                                    })
                            }),
                    };
                    if !same {
                        return Err(
                            "invocation selection lost its exact selected source declaration"
                                .into(),
                        );
                    }
                }
                if let SelectionReference::RuntimeContract { identity } = &row.selected {
                    let same = match target {
                        SelectionCallTarget::Selected { contract, .. } => module
                            .callables
                            .contract(*contract)
                            .is_some_and(|contract| {
                                module.identities.text(contract.identity) == Some(identity.as_str())
                            }),
                        SelectionCallTarget::Declared(_) => false,
                    };
                    if !same {
                        return Err(
                            "runtime call selection differs from its actual callable contract"
                                .into(),
                        );
                    }
                }
                if matches!(
                    &row.selected,
                    SelectionReference::ProductInput { .. } | SelectionReference::Intrinsic { .. }
                ) {
                    return Err("invocation selection has no checked source callable".into());
                }
                if cover(&calls, &mut covered_calls, (*machine, *site)).is_err() {
                    return Err("invocation selection repeats an emitted call".into());
                }
                joined_questions.push(*id);
            }
            SelectionJoin::Descriptor {
                question: id,
                machine,
                operation,
                descriptor,
                selected,
            } => {
                let row = published_question(packet, *id)?;
                declaration(packet, *selected)?;
                if row.selected != SelectionReference::Declaration(*selected)
                    || lookup(&constructed, (*machine, *operation)) != Some(descriptor)
                    || origin(&origins, *machine, row.occurrence.lexical_owner).is_none()
                {
                    return Err("descriptor selection differs from its actual construction".into());
                }
                let value = module
                    .callables
                    .descriptor(*descriptor)
                    .ok_or("descriptor selection names no descriptor")?;
                if origin(&origins, value.machine, *selected).is_none() {
                    return Err(
                        "descriptor selection lost its original selected declaration".into(),
                    );
                }
                if cover(&constructed, &mut covered_values, (*machine, *operation)).is_err() {
                    return Err("descriptor selection repeats a construction".into());
                }
                joined_questions.push(*id);
            }
            SelectionJoin::FoldedCall {
                question: id,
                original_caller,
                original_target,
                machine,
                edge,
                head,
            } => {
                let row = published_question(packet, *id)?;
                let selected = match row.selected {
                    SelectionReference::Declaration(selected) => selected == *original_target,
                    SelectionReference::LocalBinding { .. } => true,
                    _ => false,
                };
                if row.occurrence.lexical_owner != *original_caller
                    || !selected
                    || origin(&origins, *machine, *original_caller).is_none()
                    || origin(&origins, *machine, *original_target) != Some(*head)
                {
                    return Err(
                        "folded selection lost its original caller, target or actual head".into(),
                    );
                }
                let body = module
                    .machine(*machine)
                    .ok_or("folded selection has no emitted body")?;
                if !body
                    .blocks
                    .iter()
                    .any(|block| block.terminator.successor_edges().contains(&(*edge, *head)))
                {
                    return Err("folded selection has no actual head edge".into());
                }
                folded_edges.push((*machine, *edge));
                joined_questions.push(*id);
            }
            _ => (),
        }
    }
    folded_edges.sort_unstable();
    if folded_edges.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err("folded selection repeats an emitted head edge".into());
    }
    if covered_calls.contains(&false) {
        return Err("published invocation lacks its exact source selection question".into());
    }
    if covered_values.contains(&false) {
        return Err(
            "published descriptor construction lacks its exact source selection question".into(),
        );
    }
    joined_questions.sort_unstable();
    for (id, row) in packet.questions.iter() {
        if row.form != SelectionForm::Published {
            continue;
        }
        if let SelectionRole::ProductDesignation(designation) = &row.role {
            let site = CallableInvocationSite::Invocation(
                semantic_vocabulary::InvocationSite::Operation(designation.site.operation),
            );
            if lookup(&calls, (designation.site.machine, site))
                != Some(&SelectionCallTarget::Declared(
                    TargetMachineCallable::Boundary(designation.site.boundary),
                ))
            {
                return Err("Product designation has no actual Build boundary occurrence".into());
            }
        } else if matches!(
            row.selected,
            SelectionReference::Declaration(_) | SelectionReference::RuntimeContract { .. }
        ) && matches!(
            row.role,
            SelectionRole::Executable | SelectionRole::MachineValue
        ) && joined_questions.binary_search(&id).is_err()
        {
            return Err(format!(
                "published executable question {id} has no actual emitted association"
            ));
        }
    }
    Ok(())
}

pub fn admit_selection_questions(
    module: &TerminalModule,
    packet: &SelectionQuestions,
    authorities: &SelectionAuthorities<'_>,
) -> Result<(), String> {
    validate_selection_questions(module, packet)?;
    for (id, row) in packet.questions.iter() {
        let requester = declaration(packet, row.occurrence.lexical_owner)?;
        // A local binder or deferred operand still belongs to an actual source
        // owner; neither can supply missing package custody.
        crate::declaration_selection::require_declaration_selection(
            requester.custody,
            requester.custody,
            None,
        )?;
        match &row.selected {
            // Product admission resolves this exact retained request later.
            // Build admission cannot grant permission to its unknown owner.
            SelectionReference::ProductInput { .. } => (),
            // The holder's declaration/type reads carry their own package
            // checks. Structural callable meaning selects no source owner.
            SelectionReference::RuntimeContract { .. } => (),
            SelectionReference::LocalBinding { .. } => (),
            SelectionReference::Intrinsic { .. } => {
                crate::declaration_selection::require_declaration_selection(
                    requester.custody,
                    DeclarationCustody::Toolchain,
                    authorities.for_purpose(row.purpose),
                )?
            }
            SelectionReference::Declaration(selected) => {
                let owner = declaration(packet, *selected)?;
                let same_package = matches!((requester.custody, owner.custody),
                    (DeclarationCustody::Package(a), DeclarationCustody::Package(b)) if a == b);
                let required_purpose = if same_package {
                    row.purpose
                } else {
                    SelectionPurpose::Product
                };
                if matches!(owner.custody, DeclarationCustody::Package(_))
                    && owner.purpose != required_purpose
                {
                    return Err(format!(
                        "selection question {id} selects a declaration from the wrong Build/Product source context"
                    ));
                }
                crate::declaration_selection::require_declaration_selection(
                    requester.custody,
                    owner.custody,
                    authorities.for_purpose(row.purpose),
                )?;
            }
        }
    }
    Ok(())
}
