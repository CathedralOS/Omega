/// Follow a conditional assembly edge to the operation that produced its Boolean,
/// then read the selected flag and the destinations recorded by the producer.
pub fn assembly_branch<'a>(
    body: &'a omega_psi::MachineBody,
    block: &'a omega_psi::BranchBlock,
) -> Option<(&'a str, omega_psi::BlockId, omega_psi::BlockId)> {
    use omega_psi::{AssemblyOperand, BlockTerminator, InstructionExit, InstructionInput, OperationKind, OperationResult};
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::ValueOrigin::Operation(id) = body.values.get(condition.index())?.origin else {
        return None;
    };
    let producer = body.operations.get(id.index())?;
    if producer.result != OperationResult::Scalar(*condition) {
        return None;
    }
    let OperationKind::AssemblyInstruction(selected) = &producer.kind else {
        return None;
    };
    let InstructionExit::Branch {
        condition: InstructionInput::Location(flag),
        target,
    } = &selected.semantics.exit
    else {
        return None;
    };
    let Some(AssemblyOperand::LocalBlock(taken)) = selected.operands.get(*target) else {
        return None;
    };
    if *taken != when_true.target || when_true.requires_adjacency || !when_false.requires_adjacency {
        return None;
    }
    Some((flag, *taken, when_false.target))
}

/// The published declaration of a location an instruction names, through a
/// register operand or directly in its selected semantics.
pub fn assembly_location<'a>(
    instruction: &'a omega_psi::AssemblyInstruction,
    name: &str,
) -> Option<&'a omega_psi::AssemblyLocation> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match operand {
            omega_psi::AssemblyOperand::Register(location) => Some(location),
            _ => None,
        })
        .chain(&instruction.implicit_locations)
        .find(|location| location.name == name)
}

/// Every location an instruction writes, with its published declaration, in contract order.
/// `None` when a destination is not a published location.
fn written_locations(instruction: &omega_psi::AssemblyInstruction) -> Option<Vec<&omega_psi::AssemblyLocation>> {
    use omega_psi::{AssemblyOperand, InstructionInput};
    instruction
        .semantics
        .writes
        .iter()
        .map(|write| match &write.destination {
            InstructionInput::Operand(index) => match instruction.operands.get(*index)? {
                AssemblyOperand::Register(location) => Some(location),
                _ => None,
            },
            InstructionInput::Location(name) => assembly_location(instruction, name),
        })
        .collect()
}

/// The registers an instruction changes, whatever their lifetime: explicit
/// destinations and implicit flag effects alike. These are the instruction's
/// contribution to the union an authored clobber set is compared with.
pub fn assembly_changed_registers(instruction: &omega_psi::AssemblyInstruction) -> Option<Vec<&str>> {
    Some(
        written_locations(instruction)?
            .into_iter()
            .filter(|location| location.role == omega_psi::LocationRole::Register)
            .map(|location| location.name.as_str())
            .collect(),
    )
}

/// The non-register target state an instruction writes. It is never a changed register.
pub fn assembly_state_writes(instruction: &omega_psi::AssemblyInstruction) -> Option<Vec<&str>> {
    Some(
        written_locations(instruction)?
            .into_iter()
            .filter(|location| location.role == omega_psi::LocationRole::State)
            .map(|location| location.name.as_str())
            .collect(),
    )
}

/// What supplies a location's value on one incoming path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocationSupply {
    /// The reader's own region entry binds it to this value.
    Initialized(omega_psi::ValueId),
    /// The latest earlier write on the path, in any region when the location is persistent.
    Written(omega_psi::OperationId),
    /// A region location that the reader's region neither binds nor writes before the read.
    RegionStart,
    /// A persistent location that nothing on the path writes since the machine began.
    MachineEntry,
}

/// Follow every incoming path back from the instruction at `operation` to whatever
/// supplies `name`. The walk uses only published facts: the location's lifetime
/// from the reader's own record, operation order within blocks, and block edges.
/// A region location stops at the reader's region entry. A persistent location
/// crosses region entries and blocks to the machine's first block. Each distinct
/// supply appears once, in discovery order.
pub fn assembly_location_supply(
    body: &omega_psi::MachineBody,
    operation: omega_psi::OperationId,
    name: &str,
) -> Option<Vec<LocationSupply>> {
    use omega_psi::{BlockTerminator, LocationLifetime, OperationKind};
    let index = operation.index();
    let OperationKind::AssemblyInstruction(reader) = &body.operations.get(index)?.kind else {
        return None;
    };
    let lifetime = assembly_location(reader, name)?.lifetime;
    let block_of = |op: usize| {
        body.blocks
            .iter()
            .position(|block| block.operations.contains(&op))
    };
    let position = |id: omega_psi::BlockId| body.blocks.iter().position(|block| block.id == id);
    let mut predecessors = vec![Vec::new(); body.blocks.len()];
    for (from, block) in body.blocks.iter().enumerate() {
        let targets = match &block.terminator {
            BlockTerminator::Jump(edge) => vec![edge.target],
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true.target, when_false.target],
            BlockTerminator::Return { .. } | BlockTerminator::TerminalCall { .. } | BlockTerminator::ExternalExit(_) => vec![],
        };
        for target in targets {
            predecessors[position(target)?].push(from);
        }
    }
    let mut supplies = Vec::new();
    let mut visited = vec![false; body.blocks.len()];
    // Each pending item scans one block backwards from an exclusive end.
    let start = block_of(index)?;
    let mut pending = vec![(start, index)];
    while let Some((block, end)) = pending.pop() {
        let mut found = None;
        for earlier in body.operations[body.blocks[block].operations.start..end]
            .iter()
            .rev()
        {
            match &earlier.kind {
                OperationKind::AssemblyInstruction(i) => {
                    let region_ok = lifetime == LocationLifetime::Persistent || i.entry == reader.entry;
                    if region_ok
                        && written_locations(i)?
                            .iter()
                            .any(|location| location.name == name)
                    {
                        found = Some(LocationSupply::Written(earlier.id));
                    }
                }
                OperationKind::BeginAssembly(entry) => {
                    let bound = entry
                        .bindings
                        .iter()
                        .find(|(location, _)| location.name == name);
                    if earlier.id == reader.entry || lifetime == LocationLifetime::Persistent {
                        if let Some((_, value)) = bound {
                            found = Some(LocationSupply::Initialized(*value));
                        } else if earlier.id == reader.entry && lifetime == LocationLifetime::Region {
                            found = Some(LocationSupply::RegionStart);
                        }
                    }
                }
                _ => {}
            }
            if found.is_some() {
                break;
            }
        }
        let supply = match found {
            Some(supply) => Some(supply),
            None if predecessors[block].is_empty() => Some(match lifetime {
                LocationLifetime::Persistent => LocationSupply::MachineEntry,
                LocationLifetime::Region => LocationSupply::RegionStart,
            }),
            None => {
                for &from in &predecessors[block] {
                    if !std::mem::replace(&mut visited[from], true) {
                        pending.push((from, body.blocks[from].operations.end));
                    }
                }
                None
            }
        };
        if let Some(supply) = supply
            && !supplies.contains(&supply)
        {
            supplies.push(supply);
        }
    }
    Some(supplies)
}
