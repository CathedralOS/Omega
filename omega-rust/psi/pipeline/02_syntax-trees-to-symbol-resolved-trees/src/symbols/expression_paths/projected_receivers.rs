//! Projected method candidates follow exact indexed and case-payload declarations.
//! Bounds, case reachability, effects, and access remain later-stage obligations.

use arena::Arena;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableCallExpression,
};
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::scope::MachineScope;

#[cfg(test)]
mod tests;

pub(in crate::symbols) fn needs_declared_projection(
    table: &ExpressionTable,
    mut expression: ExpressionHandle,
) -> bool {
    loop {
        expression = match table.expression(expression) {
            ExpressionNode::Indexed(_) => return true,
            ExpressionNode::Member(member) if member.case_variant.is_some() => return true,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => return false,
        };
    }
}

enum ReceiverType<'program> {
    Declared(&'program TypeReference),
    Data(SymbolHandle),
}

// Every projected member-call repeats the same linear scans: the prior
// statements for a local receiver type, the parameter roster for a
// parameter receiver, the data-definition roster for each projection hop,
// and the attached-machine roster for the call target. The positions are
// stable within a program, so each roster is bucketed once and reused;
// the statement index only grows its watermark since a state's stamped
// prefix strictly appends.
fn statement_sample(statement: &Statement) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(statement).hash(&mut hasher);
    if let Statement::LocalData(local) = statement {
        local.symbol.arena_index().hash(&mut hasher);
    }
    hasher.finish() as usize
}

// Every projected member-call repeats the same linear scans: the prior
// statements for a local receiver type, the parameter roster for a
// parameter receiver, the data-definition roster for each projection hop,
// and the attached-machine roster for the call target. The positions are
// stable within a program, so each roster is bucketed once and reused;
// the statement index only grows its watermark since a state's stamped
// prefix strictly appends. A fresh program can recycle the same slice
// address with different contents, so each slot carries content samples
// of the indexed prefix alongside the pointer.
thread_local! {
    static LOCAL_STATEMENT_POSITIONS: std::cell::RefCell<
        Option<(
            *const Statement,
            usize,
            [usize; 3],
            std::collections::HashMap<SymbolHandle, u32>,
        )>,
    > = const { std::cell::RefCell::new(None) };
    static PARAMETER_POSITIONS: std::cell::RefCell<
        Option<(
            *const StateParameter,
            usize,
            usize,
            std::collections::HashMap<SymbolHandle, u32>,
        )>,
    > = const { std::cell::RefCell::new(None) };
    static DATA_DEFINITION_POSITIONS: std::cell::RefCell<
        Option<(
            *const arena::OrderedRootArena<symbol_resolved_trees::data::DataDefinition>,
            usize,
            usize,
            std::collections::HashMap<SymbolHandle, u32>,
        )>,
    > = const { std::cell::RefCell::new(None) };
    static ATTACHED_MACHINES_BY_OWNER: std::cell::RefCell<
        Option<(
            *const super::super::scope::AttachedMachine,
            usize,
            usize,
            std::collections::HashMap<SymbolHandle, Vec<usize>>,
        )>,
    > = const { std::cell::RefCell::new(None) };
}

// Returns the first `Statement::LocalData` position naming `symbol`
// within `statements[..len]`; positions below the watermark are
// already indexed and stay valid for any shorter prefix of the same
// backing slice.
pub(in crate::symbols) fn local_statement_position(
    statements: &[Statement],
    symbol: SymbolHandle,
) -> Option<u32> {
    fn samples(statements: &[Statement], watermark: usize) -> [usize; 3] {
        [
            statement_sample(&statements[0]),
            statement_sample(&statements[watermark / 2]),
            statement_sample(&statements[watermark - 1]),
        ]
    }
    LOCAL_STATEMENT_POSITIONS.with(|cell| {
        let mut slot = cell.borrow_mut();
        let fresh = match &*slot {
            Some((ptr, watermark, stored_samples, _)) => {
                std::ptr::eq(*ptr, statements.as_ptr())
                    && *watermark <= statements.len()
                    && (*watermark == 0 || samples(statements, *watermark) == *stored_samples)
            }
            None => false,
        };
        if !fresh {
            *slot = Some((
                statements.as_ptr(),
                0,
                [0; 3],
                std::collections::HashMap::new(),
            ));
        }
        let (_, watermark, stored_samples, positions) =
            slot.as_mut().expect("index slot is populated");
        for (position, statement) in statements.iter().enumerate().skip(*watermark) {
            if let Statement::LocalData(local) = statement {
                positions.entry(local.symbol).or_insert(position as u32);
            }
        }
        *watermark = statements.len();
        *stored_samples = if *watermark == 0 {
            [0; 3]
        } else {
            samples(statements, *watermark)
        };
        positions.get(&symbol).copied()
    })
}

pub(in crate::symbols) fn parameter_position(
    parameters: &[StateParameter],
    symbol: SymbolHandle,
) -> Option<u32> {
    PARAMETER_POSITIONS.with(|cell| {
        let mut slot = cell.borrow_mut();
        let len = parameters.len();
        let sample = |index: usize| -> usize {
            if index < len {
                parameters[index].symbol.arena_index() as usize
            } else {
                0
            }
        };
        let fingerprint = sample(0).rotate_left(11)
            ^ sample(len / 2).rotate_left(23)
            ^ sample(len.saturating_sub(1));
        let fresh = matches!(&*slot, Some((ptr, count, seen, _))
            if std::ptr::eq(*ptr, parameters.as_ptr())
                && *count == len
                && *seen == fingerprint);
        if !fresh {
            let mut positions = std::collections::HashMap::new();
            for (position, parameter) in parameters.iter().enumerate() {
                positions.entry(parameter.symbol).or_insert(position as u32);
            }
            *slot = Some((parameters.as_ptr(), len, fingerprint, positions));
        }
        slot.as_ref()
            .expect("index slot is populated")
            .3
            .get(&symbol)
            .copied()
    })
}

pub(in crate::symbols) fn data_definition_position(
    definitions: &arena::OrderedRootArena<symbol_resolved_trees::data::DataDefinition>,
    symbol: SymbolHandle,
) -> Option<u32> {
    DATA_DEFINITION_POSITIONS.with(|cell| {
        let mut slot = cell.borrow_mut();
        let len = definitions.len();
        let sample = |index: usize| -> usize {
            if index < len {
                let definition = &definitions[index];
                definition.symbol.arena_index() as usize
                    ^ definition.name.as_str().as_ptr() as usize
            } else {
                0
            }
        };
        let fingerprint = (definitions as *const _) as usize
            ^ len.rotate_left(17)
            ^ sample(0).rotate_left(31)
            ^ sample(len / 2).rotate_left(45)
            ^ sample(len.saturating_sub(1));
        let fresh = matches!(&*slot, Some((ptr, count, seen, _))
            if std::ptr::eq(*ptr, definitions as *const _)
                && *count == len
                && *seen == fingerprint);
        if !fresh {
            let mut positions = std::collections::HashMap::new();
            for (position, definition) in definitions.iter().enumerate() {
                positions
                    .entry(definition.symbol)
                    .or_insert(position as u32);
            }
            *slot = Some((definitions as *const _, len, fingerprint, positions));
        }
        slot.as_ref()
            .expect("index slot is populated")
            .3
            .get(&symbol)
            .copied()
    })
}

pub(in crate::symbols) fn attached_machines_for_owner(
    attached: &[super::super::scope::AttachedMachine],
    owner: SymbolHandle,
) -> Vec<usize> {
    ATTACHED_MACHINES_BY_OWNER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let len = attached.len();
        let sample = |index: usize| -> usize {
            if index < len {
                attached[index].owner.arena_index() as usize
                    ^ (attached[index].machine.arena_index() as usize).rotate_left(9)
            } else {
                0
            }
        };
        let fingerprint = sample(0).rotate_left(11)
            ^ sample(len / 2).rotate_left(23)
            ^ sample(len.saturating_sub(1));
        let fresh = matches!(&*slot, Some((ptr, count, seen, _))
            if std::ptr::eq(*ptr, attached.as_ptr())
                && *count == len
                && *seen == fingerprint);
        if !fresh {
            let mut by_owner = std::collections::HashMap::new();
            for (index, entry) in attached.iter().enumerate() {
                by_owner
                    .entry(entry.owner)
                    .or_insert_with(Vec::new)
                    .push(index);
            }
            *slot = Some((attached.as_ptr(), len, fingerprint, by_owner));
        }
        slot.as_ref()
            .expect("index slot is populated")
            .3
            .get(&owner)
            .cloned()
            .unwrap_or_default()
    })
}

fn peel<'program>(
    mut reference: &'program TypeReference,
    children: &'program Arena<TypeReference>,
) -> &'program TypeReference {
    loop {
        reference = match reference {
            TypeReference::Reference(reference) => children.get(reference.referee),
            TypeReference::Constrained(reference) => children.get(reference.base_type),
            _ => return reference,
        };
    }
}

impl ReceiverType<'_> {
    fn nominal(&self, children: &Arena<TypeReference>) -> Option<SymbolHandle> {
        match self {
            Self::Data(symbol) => Some(*symbol),
            Self::Declared(reference) => match peel(reference, children) {
                TypeReference::Named { symbol, .. } => Some(*symbol),
                _ => None,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call_target(
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    call: &TableCallExpression,
    table: &ExpressionTable,
    children: &Arena<TypeReference>,
    symbols: &SymbolTable,
) -> SymbolHandle {
    let resolve = || {
        let mut projections = Vec::new();
        let mut expression = call.receiver;
        let name = loop {
            expression = match table.expression(expression) {
                node @ ExpressionNode::Indexed(indexed) => {
                    projections.push(node);
                    indexed.collection
                }
                node @ ExpressionNode::Member(member) => {
                    projections.push(node);
                    member.receiver
                }
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Name(name) => break name,
                _ => return None,
            };
        };
        let [spelling] = table.name_path_members(name.members) else {
            return None;
        };
        let declaration = symbols.get(name.symbol);
        if !name.symbol.is_valid() || name.head_symbol != name.symbol {
            return None;
        }
        let mut receiver = if spelling.is_self_receiver()
            && name.symbol == machine.symbol
            && declaration.kind == SymbolKind::Machine
        {
            ReceiverType::Data(machine.attached_data_symbol)
        } else {
            if declaration.parent != state_symbol || symbols.name(name.symbol) != spelling.as_str()
            {
                return None;
            }
            let reference = match declaration.kind {
                SymbolKind::Parameter => {
                    &parameters
                        .get(parameter_position(parameters, name.symbol)? as usize)?
                        .type_reference
                }
                SymbolKind::Local => {
                    let position = local_statement_position(machine.prior_statements, name.symbol)?;
                    let Statement::LocalData(local) = &machine.prior_statements[position as usize]
                    else {
                        return None;
                    };
                    &local.type_reference
                }
                _ => return None,
            };
            ReceiverType::Declared(reference)
        };
        for projection in projections.into_iter().rev() {
            receiver = match projection {
                ExpressionNode::Indexed(_) => {
                    let ReceiverType::Declared(reference) = receiver else {
                        return None;
                    };
                    let element = match peel(reference, children) {
                        TypeReference::FixedArray(array) => array.element_type,
                        TypeReference::Slice(slice) => slice.element_type,
                        _ => return None,
                    };
                    if !element.is_valid() {
                        return None;
                    }
                    ReceiverType::Declared(children.get(element))
                }
                ExpressionNode::Member(member) => {
                    let owner = receiver.nominal(children)?;
                    if !owner.is_valid() {
                        return None;
                    }
                    let definition = &machine.data_definitions
                        [data_definition_position(machine.data_definitions, owner)? as usize];
                    let members = machine
                        .data_members
                        .span_or_empty(definition.storage.members);
                    let payload = if let Some(case) = &member.case_variant {
                        let mut variants = members.iter().filter_map(|node| match node {
                            DataMember::Variant(variant) if variant.name == *case => Some(variant),
                            _ => None,
                        });
                        let variant = variants.next()?;
                        if variants.next().is_some()
                            || !variant.symbol.is_valid()
                            || symbols.get(variant.symbol).parent != owner
                        {
                            return None;
                        }
                        Some((
                            variant.symbol,
                            machine.data_payload_fields.span_or_empty(variant.payload),
                        ))
                    } else {
                        None
                    };
                    let mut fields = members
                        .iter()
                        .filter_map(|node| match node {
                            DataMember::Field(field) if payload.is_none() => Some(field),
                            _ => None,
                        })
                        .chain(payload.into_iter().flat_map(|(_, fields)| fields))
                        .filter(|field| field.name == member.member);
                    let field = fields.next()?;
                    let field_owner = payload.map_or(owner, |(variant, _)| variant);
                    if fields.next().is_some()
                        || !field.symbol.is_valid()
                        || symbols.get(field.symbol).parent != field_owner
                        || (payload.is_some()
                            && member.member_symbol.is_valid()
                            && member.member_symbol != field.symbol)
                    {
                        return None;
                    }
                    ReceiverType::Declared(&field.type_reference)
                }
                _ => return None,
            };
        }
        let owner = receiver.nominal(children)?;
        if !owner.is_valid() || symbols.get(owner).kind != SymbolKind::Data {
            return None;
        }
        let target = machine.attached_call_target(symbols, owner, &call.target);
        target.is_valid().then_some(target)
    };
    resolve().unwrap_or_else(SymbolHandle::invalid)
}
