use omega_register_model::RegisterOperandAccess;
use omega_target::{Architecture, ObjectFormat};
use sha2::{Digest, Sha256};

use crate::{
    TerminalArchitecturalUnitActionKind, TerminalLiveRangeIdentity, TerminalLiveRangePlan,
    TerminalVirtualFixedConstraintSite,
};

pub fn terminal_live_range_identity(plan: &TerminalLiveRangePlan) -> TerminalLiveRangeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-live-range-fragments.v1\0");
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.liveness.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.target.architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 1,
    });
    bytes.push(match plan.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(plan.target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(plan.target.pointer_alignment as u64).to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.block_domains.len());
        for block in &function.block_domains {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
            bytes.extend_from_slice(&block.start.0.to_le_bytes());
            bytes.extend_from_slice(&block.end.0.to_le_bytes());
        }
        encode_len(&mut bytes, function.virtual_registers.len());
        for register in &function.virtual_registers {
            bytes.extend_from_slice(&register.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&register.class.0.to_le_bytes());
            encode_len(&mut bytes, register.occurrences.len());
            for occurrence in &register.occurrences {
                bytes.extend_from_slice(&occurrence.position.0.to_le_bytes());
                bytes.extend_from_slice(&occurrence.point.0.to_le_bytes());
                bytes.extend_from_slice(&occurrence.instruction.0.to_le_bytes());
                bytes.extend_from_slice(&occurrence.operand.to_le_bytes());
                bytes.push(access_tag(occurrence.access));
            }
            encode_len(&mut bytes, register.fixed_constraints.len());
            for constraint in &register.fixed_constraints {
                match constraint.site {
                    TerminalVirtualFixedConstraintSite::Entry => bytes.push(0),
                    TerminalVirtualFixedConstraintSite::Operand {
                        position,
                        point,
                        instruction,
                        operand,
                        access,
                    } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&position.0.to_le_bytes());
                        bytes.extend_from_slice(&point.0.to_le_bytes());
                        bytes.extend_from_slice(&instruction.0.to_le_bytes());
                        bytes.extend_from_slice(&operand.to_le_bytes());
                        bytes.push(access_tag(access));
                    }
                }
                bytes.extend_from_slice(&constraint.view.0.to_le_bytes());
            }
            encode_fragments(&mut bytes, &register.fragments);
            encode_connectors(&mut bytes, &register.edge_connectors);
        }
        encode_len(&mut bytes, function.architectural_units.len());
        for unit in &function.architectural_units {
            bytes.extend_from_slice(&unit.unit.0.to_le_bytes());
            encode_len(&mut bytes, unit.actions.len());
            for action in &unit.actions {
                bytes.extend_from_slice(&action.block.0.to_le_bytes());
                bytes.extend_from_slice(&action.position.0.to_le_bytes());
                bytes.extend_from_slice(&action.point.0.to_le_bytes());
                bytes.extend_from_slice(&action.instruction.0.to_le_bytes());
                bytes.push(match action.kind {
                    TerminalArchitecturalUnitActionKind::Use => 0,
                    TerminalArchitecturalUnitActionKind::Def => 1,
                    TerminalArchitecturalUnitActionKind::Clobber => 2,
                });
            }
            encode_fragments(&mut bytes, &unit.fragments);
            encode_connectors(&mut bytes, &unit.edge_connectors);
        }
        encode_len(&mut bytes, function.interference.len());
        for pair in &function.interference {
            bytes.extend_from_slice(&pair.lower.0.to_le_bytes());
            bytes.extend_from_slice(&pair.higher.0.to_le_bytes());
        }
    }
    TerminalLiveRangeIdentity(Sha256::digest(bytes).into())
}

fn encode_fragments(bytes: &mut Vec<u8>, fragments: &[crate::TerminalLiveRangeFragment]) {
    encode_len(bytes, fragments.len());
    for fragment in fragments {
        bytes.extend_from_slice(&fragment.block.0.to_le_bytes());
        bytes.extend_from_slice(&fragment.start.0.to_le_bytes());
        bytes.extend_from_slice(&fragment.end.0.to_le_bytes());
    }
}

fn encode_connectors(bytes: &mut Vec<u8>, connectors: &[crate::TerminalLiveRangeEdgeConnector]) {
    encode_len(bytes, connectors.len());
    for connector in connectors {
        bytes.extend_from_slice(&connector.source.0.to_le_bytes());
        bytes.extend_from_slice(&connector.terminator.0.to_le_bytes());
        bytes.push(connector.polarity_ordinal);
        bytes.extend_from_slice(&connector.psi_edge.get().to_le_bytes());
        bytes.extend_from_slice(&connector.target.0.to_le_bytes());
    }
}

fn access_tag(access: RegisterOperandAccess) -> u8 {
    match access {
        RegisterOperandAccess::Use => 0,
        RegisterOperandAccess::Def => 1,
        RegisterOperandAccess::UseDef => 2,
    }
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("canonical live-range collection length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::OptimizationUnitIdentity;
    use omega_register_model::{
        RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
    };
    use omega_target::NativeTarget;
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId,
        TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};

    use super::terminal_live_range_identity;
    use crate::{
        TerminalArchitecturalUnitAction, TerminalArchitecturalUnitActionKind,
        TerminalArchitecturalUnitLiveRange, TerminalBlockPointDomain, TerminalFunctionLiveRanges,
        TerminalLiveRangeEdgeConnector, TerminalLiveRangeFragment, TerminalLiveRangePlan,
        TerminalLiveRangePoint, TerminalLivenessIdentity, TerminalLivenessPosition,
        TerminalVirtualFixedConstraint, TerminalVirtualFixedConstraintSite,
        TerminalVirtualInterference, TerminalVirtualLiveRange, TerminalVirtualOccurrence,
    };

    fn plan() -> TerminalLiveRangePlan {
        let connector = TerminalLiveRangeEdgeConnector {
            source: TerminalSelectedBlockId(0),
            terminator: TerminalSelectedInstructionId(1),
            polarity_ordinal: 0,
            psi_edge: EdgeId::new(1).unwrap(),
            target: TerminalSelectedBlockId(1),
        };
        let fragment = TerminalLiveRangeFragment {
            block: TerminalSelectedBlockId(0),
            start: TerminalLiveRangePoint(0),
            end: TerminalLiveRangePoint(2),
        };
        TerminalLiveRangePlan {
            selected: TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            liveness: TerminalLivenessIdentity([7; 32]),
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            functions: vec![TerminalFunctionLiveRanges {
                machine: MachineId::new(1).unwrap(),
                block_domains: vec![TerminalBlockPointDomain {
                    block: TerminalSelectedBlockId(0),
                    source_block: BlockId::new(1).unwrap(),
                    start: TerminalLiveRangePoint(0),
                    end: TerminalLiveRangePoint(4),
                }],
                virtual_registers: vec![TerminalVirtualLiveRange {
                    virtual_register: TerminalVirtualRegisterId(0),
                    class: RegisterClassId(1),
                    occurrences: vec![TerminalVirtualOccurrence {
                        position: TerminalLivenessPosition(0),
                        point: TerminalLiveRangePoint(0),
                        instruction: TerminalSelectedInstructionId(0),
                        operand: 0,
                        access: RegisterOperandAccess::Use,
                    }],
                    fixed_constraints: vec![TerminalVirtualFixedConstraint {
                        site: TerminalVirtualFixedConstraintSite::Entry,
                        view: RegisterViewId(1),
                    }],
                    fragments: vec![fragment],
                    edge_connectors: vec![connector],
                }],
                architectural_units: vec![TerminalArchitecturalUnitLiveRange {
                    unit: RegisterUnitId(1),
                    actions: vec![TerminalArchitecturalUnitAction {
                        block: TerminalSelectedBlockId(0),
                        position: TerminalLivenessPosition(0),
                        point: TerminalLiveRangePoint(0),
                        instruction: TerminalSelectedInstructionId(0),
                        kind: TerminalArchitecturalUnitActionKind::Use,
                    }],
                    fragments: vec![fragment],
                    edge_connectors: vec![connector],
                }],
                interference: vec![TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(0),
                    higher: TerminalVirtualRegisterId(1),
                }],
            }],
        }
    }

    #[test]
    fn identity_commits_to_every_live_range_domain() {
        let original = plan();
        let identity = terminal_live_range_identity(&original);
        let mut mutations = Vec::new();

        let mut changed = original.clone();
        changed.selected =
            TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"other-selected");
        mutations.push(changed);
        let mut changed = original.clone();
        changed.liveness = TerminalLivenessIdentity([8; 32]);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.optimization_unit = OptimizationUnitIdentity::from_canonical_bytes(b"other-unit");
        mutations.push(changed);
        let mut changed = original.clone();
        changed.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.target = NativeTarget::linux_arm64();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].machine = MachineId::new(2).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].block_domains[0].block.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].block_domains[0].source_block = BlockId::new(2).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].block_domains[0].start.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].block_domains[0].end.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].virtual_register.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].occurrences[0]
            .position
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].occurrences[0]
            .point
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].occurrences[0]
            .instruction
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].occurrences[0].operand += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].occurrences[0].access =
            RegisterOperandAccess::Def;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].fixed_constraints[0].site =
            TerminalVirtualFixedConstraintSite::Operand {
                position: TerminalLivenessPosition(0),
                point: TerminalLiveRangePoint(0),
                instruction: TerminalSelectedInstructionId(0),
                operand: 0,
                access: RegisterOperandAccess::Use,
            };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].fixed_constraints[0]
            .view
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].fragments[0].end.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].edge_connectors[0]
            .target
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].edge_connectors[0]
            .source
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].edge_connectors[0]
            .terminator
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].edge_connectors[0].psi_edge =
            EdgeId::new(2).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].unit.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].actions[0].kind =
            TerminalArchitecturalUnitActionKind::Def;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].actions[0]
            .block
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].actions[0]
            .position
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].actions[0]
            .point
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].actions[0]
            .instruction
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].fragments[0]
            .start
            .0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].architectural_units[0].edge_connectors[0].polarity_ordinal = 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].interference[0].higher.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].interference[0].lower.0 += 1;
        mutations.push(changed);

        for mutation in mutations {
            assert_ne!(terminal_live_range_identity(&mutation), identity);
        }
    }
}
