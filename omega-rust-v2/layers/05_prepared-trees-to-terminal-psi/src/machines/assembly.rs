//! Lower one admitted assembly region into ordinary control-flow blocks.
//! Local declarations share the entry's working registers. Explicit transitions
//! retain their physical adjacency constraint on the corresponding successor.
use super::{
    body::next_id,
    branches::{BlockFlow, emit_return_block, successor},
    expressions::{EmitOperation, Expressions, OperationOutput},
};
use crate::{
    AssemblyEntry, AssemblyInstruction, AssemblyOperand, AssemblyRegister, BlockId, BlockTerminator, InstructionExit, LoweringError, MachineBody, OperationId,
    OperationKind, OperationResult, TypeId, ValueId, site_at,
};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, TypedInstructionOperand, ValueType};
use std::collections::{HashMap, HashSet};

type Blocks<'a> = HashMap<&'a DeclarationId, BlockId>;

fn entry(
    expr: &Expressions<'_>,
    node: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<OperationId, LoweringError> {
    let tree = expr.tree;
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let region = tree
        .assembly_region(node)
        .ok_or_else(|| LoweringError::new(&site, "missing typed assembly region"))?;
    let mut bindings = Vec::with_capacity(region.bindings.len());
    let mut assigned = HashSet::new();
    for (rhs, name, ty) in &region.bindings {
        if rhs.file != file.ordinal() || rhs.node >= file.nodes.len() {
            return Err(LoweringError::new(&site, "assembly register initializer belongs to a different tree"));
        }
        let site = site_at(tree, rhs.node);
        if !assigned.insert(name) {
            return Err(LoweringError::new(&site, "register initialized more than once"));
        }
        crate::lowering::types::scalar_carrier(ty).map_err(|_| LoweringError::new(&site, "selected register requires unqualified scalar storage"))?;
        let OperationResult::Scalar(input) = expr.lower(rhs.node, ty, body, flow)? else {
            return Err(LoweringError::new(&site, "register initialization needs a scalar"));
        };
        let register_type = expr
            .types
            .id_for(ty)
            .map_err(|err| LoweringError::new(&site, err.to_string()))?;
        if body.values[input.index()].ty != register_type {
            return Err(LoweringError::new(&site, "register initializer requires its selected scalar conversion"));
        }
        bindings.push((
            AssemblyRegister {
                name: name.clone(),
                ty: register_type,
            },
            input,
        ));
    }
    let id = OperationId(next_id(body.operations.len()));
    body.push_operation(
        OperationOutput::Unit,
        OperationKind::BeginAssembly(Box::new(AssemblyEntry {
            target: region.target.clone(),
            bindings,
            clobbers: region.clobbers.clone(),
        })),
        site,
    );
    Ok(id)
}

enum Transfer {
    Continue,
    Return,
    Jump(BlockId),
    Branch { taken: BlockId, condition: ValueId },
}

fn instruction(
    expr: &Expressions<'_>,
    node: usize,
    entry: OperationId,
    locals: &Blocks<'_>,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<Transfer, LoweringError> {
    let tree = expr.tree;
    let site = site_at(tree, node);
    let mut records = tree.semantics_at(node);
    let Some(NodeSemantics::Instruction(i)) = records.next() else {
        return Err(LoweringError::new(&site, "missing selected instruction contract"));
    };
    if records.next().is_some() || i.node != tree.resolved().file.node_id(node) {
        return Err(LoweringError::new(&site, "additional instruction obligations require lowering"));
    }
    let mut operands = Vec::new();
    let file = tree.resolved().file;
    if file.nodes[node].children.len() != i.operands.len() {
        return Err(LoweringError::new(&site, "instruction operand roster disagrees with its contract"));
    }
    let id_for = |ty: &ValueType| {
        crate::lowering::types::scalar_carrier(ty).map_err(|_| LoweringError::new(&site, "selected instruction requires unqualified scalar storage"))?;
        expr.types
            .id_for(ty)
            .map_err(|err| LoweringError::new(&site, err.to_string()))
    };
    for (operand, &n) in i.operands.iter().zip(&file.nodes[node].children) {
        let (id, value) = match operand {
            TypedInstructionOperand::Block { node, declaration } => (
                *node,
                AssemblyOperand::LocalBlock(
                    *locals
                        .get(declaration)
                        .ok_or_else(|| LoweringError::new(&site, "instruction target lies outside this assembly region"))?,
                ),
            ),
            TypedInstructionOperand::Register { node, name, ty } => (
                *node,
                AssemblyOperand::Register(AssemblyRegister {
                    name: name.clone(),
                    ty: id_for(ty)?,
                }),
            ),
            TypedInstructionOperand::Value { node, ty } => {
                let operand_type = id_for(ty)?;
                let OperationResult::Scalar(value) = expr.lower(n, ty, body, flow)? else {
                    return Err(LoweringError::new(&site, "instruction input requires a scalar value"));
                };
                if body.values[value.index()].ty != operand_type {
                    return Err(LoweringError::new(&site, "instruction input requires its selected scalar conversion"));
                }
                (*node, AssemblyOperand::Value(value))
            }
        };
        if id != file.node_id(n) {
            return Err(LoweringError::new(&site, "instruction operand differs from its selected occurrence"));
        }
        operands.push(value);
    }
    let target_index = match &i.semantics.exit {
        InstructionExit::Jump { target } | InstructionExit::Branch { target, .. } => Some(*target),
        _ => None,
    };
    let target = target_index
        .map(|index| match operands.get(index) {
            Some(AssemblyOperand::LocalBlock(block)) => Ok(*block),
            _ => Err(LoweringError::new(&site, "selected branch operand is not a local block")),
        })
        .transpose()?;
    let output = if matches!(i.semantics.exit, InstructionExit::Branch { .. }) {
        OperationOutput::Scalar(id_for(&ValueType::Bool)?)
    } else {
        OperationOutput::Unit
    };
    let implicit_registers = i
        .implicit_registers
        .iter()
        .map(|(name, ty)| {
            Ok(AssemblyRegister {
                name: name.clone(),
                ty: id_for(ty)?,
            })
        })
        .collect::<Result<_, LoweringError>>()?;
    let operation = AssemblyInstruction {
        entry,
        target: i.target.clone(),
        mnemonic: i.mnemonic.clone(),
        operands,
        implicit_registers,
        semantics: i.semantics.clone(),
    };
    let result = body.push_operation(output, OperationKind::AssemblyInstruction(Box::new(operation)), site);
    Ok(match (&i.semantics.exit, target, result) {
        (InstructionExit::Continue, None, OperationResult::Unit) => Transfer::Continue,
        (InstructionExit::Return, None, OperationResult::Unit) => Transfer::Return,
        (InstructionExit::Jump { .. }, Some(block), OperationResult::Unit) => Transfer::Jump(block),
        (InstructionExit::Branch { .. }, Some(taken), OperationResult::Scalar(condition)) => Transfer::Branch { taken, condition },
        _ => unreachable!("validated instruction exit"),
    })
}

pub(super) fn inline(
    expr: &Expressions<'_>,
    node: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<(), LoweringError> {
    let tree = expr.tree;
    let file = tree.resolved().file;
    let start = entry(expr, node, body, flow)?;
    for &n in &file.nodes[node].children {
        match file.nodes[n].kind {
            NodeKind::Instruction => {
                if !matches!(instruction(expr, n, start, &Blocks::new(), body, flow)?, Transfer::Continue) {
                    return Err(LoweringError::new(
                        &site_at(tree, n),
                        "inline control transfer requires its enclosing continuation",
                    ));
                }
            }
            NodeKind::Registers | NodeKind::Clobbers => {}
            _ => return Err(LoweringError::new(&site_at(tree, n), "inline assembly clause needs its checked block contract")),
        }
    }
    Ok(())
}

pub(super) fn machine(
    expr: &Expressions<'_>,
    root: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    result_type: Option<TypeId>,
) -> Result<(), LoweringError> {
    let tree = expr.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    if result_type.is_some() {
        return Err(LoweringError::new(
            &site_at(tree, root),
            "assembly return requires its selected result calling plan",
        ));
    }
    let mut local = Blocks::new();
    let mut nodes = vec![(root, BlockId(next_id(0)))];
    let mut index = 0;
    while let Some(&(n, _)) = nodes.get(index) {
        for &child in &file.nodes[n].children {
            if file.nodes[child].kind == NodeKind::AsmMachine {
                let declaration = resolved
                    .declaration_at(child)
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "local assembly block lacks its declaration"))?;
                let id = flow.reserve();
                local.insert(declaration, id);
                nodes.push((child, id));
            }
        }
        index += 1;
    }
    let start = entry(expr, root, body, flow)?;
    let mut neighbors = vec![Vec::new(); nodes.len()];
    for (node, id) in nodes {
        if node != root {
            flow.open(body, id, None);
        }
        let mut lines = Vec::new();
        for &n in &file.nodes[node].children {
            match file.nodes[n].kind {
                NodeKind::Instruction | NodeKind::AsmTransition => lines.push(n),
                NodeKind::Path | NodeKind::AsmMachine => {}
                NodeKind::Params if node == root || file.nodes[n].children.is_empty() => {}
                NodeKind::Clobbers
                | NodeKind::Registers
                | NodeKind::Result
                | NodeKind::Requires
                | NodeKind::Ensures
                | NodeKind::Terminates
                | NodeKind::Reaches
                | NodeKind::Crashes
                | NodeKind::Blocks
                | NodeKind::Suspends
                    if node == root => {}
                _ => {
                    return Err(LoweringError::new(
                        &site_at(tree, n),
                        "local assembly block requires additional contracts or entry state",
                    ));
                }
            }
        }
        let mut lines = lines.into_iter();
        let mut terminated = false;
        while let Some(n) = lines.next() {
            let site = site_at(tree, n);
            if terminated {
                return Err(LoweringError::new(&site, "instruction follows a terminating assembly transfer"));
            }
            let adjacent = |n| {
                resolved
                    .symbol(n)
                    .and_then(|s| match s {
                        Symbol::Declaration(id) => local.get(id).copied(),
                        _ => None,
                    })
                    .ok_or_else(|| LoweringError::new(&site_at(tree, n), "transition requires a local assembly continuation"))
            };
            let transition = file.nodes[n].kind == NodeKind::AsmTransition;
            let transfer = if transition {
                Transfer::Jump(adjacent(n)?)
            } else {
                instruction(expr, n, start, &local, body, flow)?
            };
            match transfer {
                Transfer::Continue => continue,
                Transfer::Return => emit_return_block(body, flow, None, None, site)?,
                Transfer::Jump(target) => {
                    let mut edge = successor(body, target, vec![], site);
                    edge.requires_adjacency = transition;
                    neighbors[id.get() - 1].push(target);
                    flow.close(body, BlockTerminator::Jump(edge));
                }
                Transfer::Branch { taken, condition } => {
                    let next = lines
                        .next()
                        .filter(|&adj| file.nodes[adj].kind == NodeKind::AsmTransition)
                        .ok_or_else(|| LoweringError::new(&site, "conditional assembly instruction needs an explicit adjacent transition"))?;
                    let fallthrough = adjacent(next)?;
                    let when_true = successor(body, taken, vec![], site);
                    let mut when_false = successor(body, fallthrough, vec![], site_at(tree, next));
                    when_false.requires_adjacency = true;
                    neighbors[id.get() - 1].extend([taken, fallthrough]);
                    flow.close(
                        body,
                        BlockTerminator::Conditional {
                            condition,
                            when_true,
                            when_false,
                        },
                    );
                }
            }
            terminated = true;
        }
        if !terminated {
            return Err(LoweringError::new(&site_at(tree, node), "assembly block requires an explicit checked exit"));
        }
    }
    let mut reached = vec![false; neighbors.len()];
    let mut pending = vec![BlockId(next_id(0))];
    while let Some(id) = pending.pop() {
        if !std::mem::replace(&mut reached[id.get() - 1], true) {
            pending.extend(&neighbors[id.get() - 1]);
        }
    }
    if reached.contains(&false) {
        return Err(LoweringError::new(&site_at(tree, root), "local assembly block has no path from entry"));
    }
    Ok(())
}
