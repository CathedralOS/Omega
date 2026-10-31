//! The literal predecessor encoder is the byte-protocol oracle. These are raw
//! representation rows; neither encoder confers analysis admission.
use super::*;
use crate::*;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

fn fixture() -> FunctionLiveRanges {
    let block = SelectedBlockId(u32::MAX);
    let position = LivenessPosition(0x10203040);
    let point = LiveRangePoint(0x50607080);
    let instruction = SelectedInstructionId(0x90a0b0c0);
    let register = VirtualRegisterId(0xf0e0d0c0);
    let class = RegisterClassId(0x1234);
    let fragment = LiveRangeFragment {
        block,
        start: point,
        end: LiveRangePoint(u32::MAX),
    };
    let connectors = vec![
        LiveRangeEdgeConnector {
            source: block,
            terminator: instruction,
            polarity_ordinal: u8::MAX,
            psi_edge: None,
            target: SelectedBlockId(0),
        },
        LiveRangeEdgeConnector {
            source: SelectedBlockId(0),
            terminator: instruction,
            polarity_ordinal: 1,
            psi_edge: Some(EdgeId::new(u64::MAX).unwrap()),
            target: block,
        },
    ];
    FunctionLiveRanges {
        machine: MachineId::new(u64::MAX).unwrap(),
        block_domains: vec![BlockPointDomain {
            block,
            source_block: BlockId::new(0x0102030405060708).unwrap(),
            start: point,
            end: LiveRangePoint(u32::MAX),
        }],
        virtual_registers: vec![VirtualLiveRange {
            virtual_register: register,
            class,
            occurrences: [
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
                RegisterOperandAccess::UseDef,
            ]
            .into_iter()
            .map(|access| VirtualOccurrence {
                position,
                point,
                instruction,
                operand: u16::MAX,
                access,
            })
            .collect(),
            fixed_constraints: vec![
                VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Entry,
                    view: RegisterViewId(0x4321),
                },
                VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Operand {
                        position,
                        point,
                        instruction,
                        operand: 0x1122,
                        access: RegisterOperandAccess::UseDef,
                    },
                    view: RegisterViewId(u16::MAX),
                },
            ],
            fragments: vec![fragment],
            edge_connectors: connectors.clone(),
        }],
        tied_pairs: vec![DistinctUseDefTie {
            block,
            position,
            instruction,
            use_operand: 1,
            use_virtual_register: register,
            use_point: point,
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(7),
            def_point: LiveRangePoint(8),
            class,
        }],
        edge_transfers: vec![EdgeRegisterTransfer {
            source: block,
            target: SelectedBlockId(9),
            psi_edge: EdgeId::new(10).unwrap(),
            argument: register,
            parameter: VirtualRegisterId(11),
            class,
        }],
        copy_affinities: vec![CopyAffinity {
            block,
            instruction,
            source: register,
            destination: VirtualRegisterId(12),
        }],
        early_clobbers: vec![EarlyClobberConstraint {
            block,
            position,
            instruction,
            early_point: point,
            def_operand: 13,
            def_virtual_register: register,
            def_class: class,
            def_point: LiveRangePoint(14),
            uses: vec![EarlyClobberUse {
                operand: 15,
                virtual_register: VirtualRegisterId(16),
                class: RegisterClassId(17),
            }],
        }],
        architectural_units: vec![ArchitecturalUnitLiveRange {
            unit: RegisterUnitId(0x5678),
            actions: [
                ArchitecturalUnitActionKind::Use,
                ArchitecturalUnitActionKind::Def,
                ArchitecturalUnitActionKind::Clobber,
            ]
            .into_iter()
            .map(|kind| ArchitecturalUnitAction {
                block,
                position,
                point,
                instruction,
                kind,
            })
            .collect(),
            fragments: vec![fragment],
            edge_connectors: connectors,
        }],
        interference: vec![VirtualInterference {
            lower: register,
            higher: VirtualRegisterId(u32::MAX),
        }],
    }
}

#[test]
fn streamed_function_digest_matches_original_bytes_for_all_row_kinds_and_boundaries() {
    let populated = fixture();
    assert_eq!(
        live_range_function_digest(&populated),
        buffered_original_digest(&populated)
    );
    // Empty collections and nonempty framing remain distinct, including the old
    // ordering of tie count, edge transfers, affinities, then actual tie rows.
    let mut empty = populated.clone();
    empty.block_domains.clear();
    empty.virtual_registers.clear();
    empty.tied_pairs.clear();
    empty.edge_transfers.clear();
    empty.copy_affinities.clear();
    empty.early_clobbers.clear();
    empty.architectural_units.clear();
    empty.interference.clear();
    assert_eq!(
        live_range_function_digest(&empty),
        buffered_original_digest(&empty)
    );
    assert_ne!(
        live_range_function_digest(&empty),
        live_range_function_digest(&populated)
    );
    // Exercise incremental SHA block boundaries and length framing without a
    // synthetic hash implementation or an admitted-plan shortcut.
    for count in [
        0, 1, 2, 7, 8, 15, 16, 31, 32, 55, 56, 63, 64, 65, 127, 128, 129, 4097,
    ] {
        let mut function = populated.clone();
        function.interference = (0..count)
            .map(|index| VirtualInterference {
                lower: VirtualRegisterId(index),
                higher: VirtualRegisterId(index.wrapping_add(9)),
            })
            .collect();
        function.virtual_registers[0]
            .fragments
            .resize(count as usize, populated.virtual_registers[0].fragments[0]);
        assert_eq!(
            live_range_function_digest(&function),
            buffered_original_digest(&function),
            "row count {count}"
        );
    }
    // Byte chunking is independent of the persisted schema and of row values.
    for byte in 0..=u8::MAX {
        let mut function = populated.clone();
        function.virtual_registers[0].edge_connectors[0].polarity_ordinal = byte;
        function.virtual_registers[0].fixed_constraints[1].site =
            VirtualFixedConstraintSite::Operand {
                position: LivenessPosition(u32::from(byte)),
                point: LiveRangePoint(u32::MAX),
                instruction: SelectedInstructionId(u32::from(byte) << 24),
                operand: u16::from(byte) << 8,
                access: RegisterOperandAccess::Def,
            };
        assert_eq!(
            live_range_function_digest(&function),
            buffered_original_digest(&function),
            "byte {byte}"
        );
    }
}

fn buffered_original_digest(function: &crate::FunctionLiveRanges) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        256 + function.virtual_registers.len() * 48 + function.interference.len() * 8,
    );
    bytes.extend_from_slice(b"omega.terminal-live-range-function.v1\0");
    crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
    crate::codec_bytes::encode_len(&mut bytes, function.block_domains.len());
    for block in &function.block_domains {
        crate::codec_bytes::push_encoded(&mut bytes, block.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.source_block.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.start.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, block.end.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        crate::codec_bytes::push_encoded(&mut bytes, register.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, register.class.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, register.occurrences.len());
        for occurrence in &register.occurrences {
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, occurrence.operand.to_le_bytes());
            bytes.push(occurrence.access.access_tag());
        }
        crate::codec_bytes::encode_len(&mut bytes, register.fixed_constraints.len());
        for constraint in &register.fixed_constraints {
            match constraint.site {
                VirtualFixedConstraintSite::Entry => bytes.push(0),
                VirtualFixedConstraintSite::Operand {
                    position,
                    point,
                    instruction,
                    operand,
                    access,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(&mut bytes, position.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, point.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, instruction.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, operand.to_le_bytes());
                    bytes.push(access.access_tag());
                }
            }
            crate::codec_bytes::push_encoded(&mut bytes, constraint.view.0.to_le_bytes());
        }
        buffered_fragments(&mut bytes, &register.fragments);
        buffered_connectors(&mut bytes, &register.edge_connectors);
    }
    crate::codec_bytes::encode_len(&mut bytes, function.tied_pairs.len());
    crate::codec_bytes::encode_len(&mut bytes, function.edge_transfers.len());
    for edge in &function.edge_transfers {
        crate::codec_bytes::push_encoded(&mut bytes, edge.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.target.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.psi_edge.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.argument.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.parameter.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, edge.class.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.copy_affinities.len());
    for affinity in &function.copy_affinities {
        crate::codec_bytes::push_encoded(&mut bytes, affinity.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, affinity.destination.0.to_le_bytes());
    }
    for tie in &function.tied_pairs {
        crate::codec_bytes::push_encoded(&mut bytes, tie.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.position.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.use_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.def_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, tie.class.0.to_le_bytes());
    }
    crate::codec_bytes::encode_len(&mut bytes, function.early_clobbers.len());
    for early in &function.early_clobbers {
        crate::codec_bytes::push_encoded(&mut bytes, early.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.position.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.early_point.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_operand.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_class.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, early.def_point.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, early.uses.len());
        for used in &early.uses {
            crate::codec_bytes::push_encoded(&mut bytes, used.operand.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, used.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, used.class.0.to_le_bytes());
        }
    }
    crate::codec_bytes::encode_len(&mut bytes, function.architectural_units.len());
    for unit in &function.architectural_units {
        crate::codec_bytes::push_encoded(&mut bytes, unit.unit.0.to_le_bytes());
        crate::codec_bytes::encode_len(&mut bytes, unit.actions.len());
        for action in &unit.actions {
            crate::codec_bytes::push_encoded(&mut bytes, action.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, action.instruction.0.to_le_bytes());
            bytes.push(match action.kind {
                ArchitecturalUnitActionKind::Use => 0,
                ArchitecturalUnitActionKind::Def => 1,
                ArchitecturalUnitActionKind::Clobber => 2,
            });
        }
        buffered_fragments(&mut bytes, &unit.fragments);
        buffered_connectors(&mut bytes, &unit.edge_connectors);
    }
    crate::codec_bytes::encode_len(&mut bytes, function.interference.len());
    for pair in &function.interference {
        crate::codec_bytes::push_encoded(&mut bytes, pair.lower.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, pair.higher.0.to_le_bytes());
    }
    crate::digest_encoder::content_identity(&bytes).into()
}

fn buffered_fragments(bytes: &mut Vec<u8>, fragments: &[crate::LiveRangeFragment]) {
    crate::codec_bytes::encode_len(bytes, fragments.len());
    for fragment in fragments {
        crate::codec_bytes::push_encoded(bytes, fragment.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, fragment.start.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, fragment.end.0.to_le_bytes());
    }
}

fn buffered_connectors(bytes: &mut Vec<u8>, connectors: &[crate::LiveRangeEdgeConnector]) {
    crate::codec_bytes::encode_len(bytes, connectors.len());
    for connector in connectors {
        crate::codec_bytes::push_encoded(bytes, connector.source.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, connector.terminator.0.to_le_bytes());
        bytes.push(connector.polarity_ordinal);
        crate::codec_bytes::push_encoded(
            bytes,
            connector
                .psi_edge
                .map_or(0, |edge| edge.get())
                .to_le_bytes(),
        );
        crate::codec_bytes::push_encoded(bytes, connector.target.0.to_le_bytes());
    }
}
