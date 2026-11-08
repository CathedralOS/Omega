/*
  Durable selected-instruction live-range facts (`LiveRangePlan`,
  `FunctionLiveRanges`, `VirtualLiveRange`, `ArchitecturalUnitLiveRange`),
  compressed virtual interference tables (`VirtualInterferences`), and their
  canonical SHA-256 identity (`LiveRangeIdentity`).
*/

mod identity {
    use crate::digest_encoder::DomainDigestEncoder;
    use crate::{
        ArchitecturalUnitActionKind, LiveRangeIdentity, LiveRangePlan, VirtualFixedConstraintSite,
    };

    pub fn live_range_identity_from_digests(
        plan: &LiveRangePlan,
        digests: &[[u8; 32]],
    ) -> LiveRangeIdentity {
        assert_eq!(
            digests.len(),
            plan.functions.len(),
            "one digest per live-range function"
        );
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega.terminal-live-range-fragments.v12\0");
        bytes.extend_from_slice(&plan.selected.bytes());
        bytes.extend_from_slice(&plan.liveness.bytes());
        bytes.extend_from_slice(&plan.optimization_unit.bytes());
        crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
        bytes.extend_from_slice(&plan.target.identity_bytes());
        crate::codec_bytes::encode_len(&mut bytes, digests.len());
        for digest in digests {
            bytes.extend_from_slice(digest);
        }
        LiveRangeIdentity::from_bytes(crate::digest_encoder::content_identity(&bytes))
    }

    pub fn live_range_function_digest(function: &crate::FunctionLiveRanges) -> [u8; 32] {
        let mut digest = DomainDigestEncoder::new(b"");
        digest.raw(b"omega.terminal-live-range-function.v1\0");
        digest.raw(&function.machine.get().to_le_bytes());
        digest.len(function.block_domains.len());
        for block in &function.block_domains {
            digest.raw(&block.block.0.to_le_bytes());
            digest.raw(&block.source_block.get().to_le_bytes());
            digest.raw(&block.start.0.to_le_bytes());
            digest.raw(&block.end.0.to_le_bytes());
        }
        digest.len(function.virtual_registers.len());
        for register in &function.virtual_registers {
            digest.raw(&register.virtual_register.0.to_le_bytes());
            digest.raw(&register.class.0.to_le_bytes());
            digest.len(register.occurrences.len());
            for occurrence in &register.occurrences {
                digest.raw(&occurrence.position.0.to_le_bytes());
                digest.raw(&occurrence.point.0.to_le_bytes());
                digest.raw(&occurrence.instruction.0.to_le_bytes());
                digest.raw(&occurrence.operand.to_le_bytes());
                digest.byte(occurrence.access.access_tag());
            }
            digest.len(register.fixed_constraints.len());
            for constraint in &register.fixed_constraints {
                match constraint.site {
                    VirtualFixedConstraintSite::Entry => digest.byte(0),
                    VirtualFixedConstraintSite::Operand {
                        position,
                        point,
                        instruction,
                        operand,
                        access,
                    } => {
                        digest.byte(1);
                        digest.raw(&position.0.to_le_bytes());
                        digest.raw(&point.0.to_le_bytes());
                        digest.raw(&instruction.0.to_le_bytes());
                        digest.raw(&operand.to_le_bytes());
                        digest.byte(access.access_tag());
                    }
                }
                digest.raw(&constraint.view.0.to_le_bytes());
            }
            encode_fragments(&mut digest, &register.fragments);
            encode_connectors(&mut digest, &register.edge_connectors);
        }
        digest.len(function.tied_pairs.len());
        digest.len(function.edge_transfers.len());
        for edge in &function.edge_transfers {
            digest.raw(&edge.source.0.to_le_bytes());
            digest.raw(&edge.target.0.to_le_bytes());
            digest.raw(&edge.psi_edge.get().to_le_bytes());
            digest.raw(&edge.argument.0.to_le_bytes());
            digest.raw(&edge.parameter.0.to_le_bytes());
            digest.raw(&edge.class.0.to_le_bytes());
        }
        digest.len(function.copy_affinities.len());
        for affinity in &function.copy_affinities {
            digest.raw(&affinity.block.0.to_le_bytes());
            digest.raw(&affinity.instruction.0.to_le_bytes());
            digest.raw(&affinity.source.0.to_le_bytes());
            digest.raw(&affinity.destination.0.to_le_bytes());
        }
        for tie in &function.tied_pairs {
            digest.raw(&tie.block.0.to_le_bytes());
            digest.raw(&tie.position.0.to_le_bytes());
            digest.raw(&tie.instruction.0.to_le_bytes());
            digest.raw(&tie.use_operand.to_le_bytes());
            digest.raw(&tie.use_virtual_register.0.to_le_bytes());
            digest.raw(&tie.use_point.0.to_le_bytes());
            digest.raw(&tie.def_operand.to_le_bytes());
            digest.raw(&tie.def_virtual_register.0.to_le_bytes());
            digest.raw(&tie.def_point.0.to_le_bytes());
            digest.raw(&tie.class.0.to_le_bytes());
        }
        digest.len(function.early_clobbers.len());
        for early in &function.early_clobbers {
            digest.raw(&early.block.0.to_le_bytes());
            digest.raw(&early.position.0.to_le_bytes());
            digest.raw(&early.instruction.0.to_le_bytes());
            digest.raw(&early.early_point.0.to_le_bytes());
            digest.raw(&early.def_operand.to_le_bytes());
            digest.raw(&early.def_virtual_register.0.to_le_bytes());
            digest.raw(&early.def_class.0.to_le_bytes());
            digest.raw(&early.def_point.0.to_le_bytes());
            digest.len(early.uses.len());
            for used in &early.uses {
                digest.raw(&used.operand.to_le_bytes());
                digest.raw(&used.virtual_register.0.to_le_bytes());
                digest.raw(&used.class.0.to_le_bytes());
            }
        }
        digest.len(function.architectural_units.len());
        for unit in &function.architectural_units {
            digest.raw(&unit.unit.0.to_le_bytes());
            digest.len(unit.actions.len());
            for action in &unit.actions {
                digest.raw(&action.block.0.to_le_bytes());
                digest.raw(&action.position.0.to_le_bytes());
                digest.raw(&action.point.0.to_le_bytes());
                digest.raw(&action.instruction.0.to_le_bytes());
                digest.byte(match action.kind {
                    ArchitecturalUnitActionKind::Use => 0,
                    ArchitecturalUnitActionKind::Def => 1,
                    ArchitecturalUnitActionKind::Clobber => 2,
                });
            }
            encode_fragments(&mut digest, &unit.fragments);
            encode_connectors(&mut digest, &unit.edge_connectors);
        }
        digest.len(function.interference.len());
        for pair in &function.interference {
            digest.raw(&pair.lower.0.to_le_bytes());
            digest.raw(&pair.higher.0.to_le_bytes());
        }
        digest.finish()
    }

    fn encode_fragments(digest: &mut DomainDigestEncoder, fragments: &[crate::LiveRangeFragment]) {
        digest.len(fragments.len());
        for fragment in fragments {
            digest.raw(&fragment.block.0.to_le_bytes());
            digest.raw(&fragment.start.0.to_le_bytes());
            digest.raw(&fragment.end.0.to_le_bytes());
        }
    }

    fn encode_connectors(
        digest: &mut DomainDigestEncoder,
        connectors: &[crate::LiveRangeEdgeConnector],
    ) {
        digest.len(connectors.len());
        for connector in connectors {
            digest.raw(&connector.source.0.to_le_bytes());
            digest.raw(&connector.terminator.0.to_le_bytes());
            digest.byte(connector.polarity_ordinal);
            digest.raw(
                &connector
                    .psi_edge
                    .map_or(0, |edge| edge.get())
                    .to_le_bytes(),
            );
            digest.raw(&connector.target.0.to_le_bytes());
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::*;
        use register_model::{
            RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
        };
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
                }]
                .into(),
            }
        }

        #[test]
        fn streamed_function_digest_matches_original_bytes_for_all_row_kinds_and_boundaries() {
            let populated = fixture();
            assert_eq!(
                live_range_function_digest(&populated),
                buffered_original_digest(&populated)
            );
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
            for raw in [
                vec![(u32::MAX, 0), (0, u32::MAX), (u32::MAX, u32::MAX)],
                vec![(3, 9), (3, 9), (1, 4), (3, 0)],
            ] {
                let mut function = populated.clone();
                function.interference = raw
                    .into_iter()
                    .map(|(lower, higher)| VirtualInterference {
                        lower: VirtualRegisterId(lower),
                        higher: VirtualRegisterId(higher),
                    })
                    .collect();
                assert_eq!(
                    live_range_function_digest(&function),
                    buffered_original_digest(&function)
                );
            }
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
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    block.source_block.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, block.start.0.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, block.end.0.to_le_bytes());
            }
            crate::codec_bytes::encode_len(&mut bytes, function.virtual_registers.len());
            for register in &function.virtual_registers {
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    register.virtual_register.0.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, register.class.0.to_le_bytes());
                crate::codec_bytes::encode_len(&mut bytes, register.occurrences.len());
                for occurrence in &register.occurrences {
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        occurrence.position.0.to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(&mut bytes, occurrence.point.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        occurrence.instruction.0.to_le_bytes(),
                    );
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
                            crate::codec_bytes::push_encoded(
                                &mut bytes,
                                instruction.0.to_le_bytes(),
                            );
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
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    tie.use_virtual_register.0.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, tie.use_point.0.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, tie.def_operand.to_le_bytes());
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    tie.def_virtual_register.0.to_le_bytes(),
                );
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
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    early.def_virtual_register.0.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, early.def_class.0.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, early.def_point.0.to_le_bytes());
                crate::codec_bytes::encode_len(&mut bytes, early.uses.len());
                for used in &early.uses {
                    crate::codec_bytes::push_encoded(&mut bytes, used.operand.to_le_bytes());
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        used.virtual_register.0.to_le_bytes(),
                    );
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
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        action.instruction.0.to_le_bytes(),
                    );
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
            crate::digest_encoder::content_identity(&bytes)
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
    }
}
mod interference {

    use std::cmp::Ordering;
    use std::fmt;

    use super::VirtualInterference;
    use crate::VirtualRegisterId;

    #[derive(Clone, Default)]
    pub struct VirtualInterferences {
        lower_ids: Vec<VirtualRegisterId>,
        run_ends: Vec<usize>,
        higher_ids: Vec<VirtualRegisterId>,
    }

    impl VirtualInterferences {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn len(&self) -> usize {
            self.higher_ids.len()
        }

        pub fn is_empty(&self) -> bool {
            self.higher_ids.is_empty()
        }

        pub fn clear(&mut self) {
            self.lower_ids.clear();
            self.run_ends.clear();
            self.higher_ids.clear();
        }

        pub fn push(&mut self, pair: VirtualInterference) {
            self.higher_ids.push(pair.higher);
            let end = self.len();
            if self.lower_ids.last() == Some(&pair.lower) {
                *self.run_ends.last_mut().expect("lower run has an end") = end;
            } else {
                self.lower_ids.push(pair.lower);
                self.run_ends.push(end);
            }
        }

        pub fn get(&self, index: usize) -> Option<VirtualInterference> {
            let higher = *self.higher_ids.get(index)?;
            let run = self.run_ends.partition_point(|end| *end <= index);
            Some(VirtualInterference {
                lower: self.lower_ids[run],
                higher,
            })
        }

        pub fn iter(&self) -> Interferences<'_> {
            Interferences {
                rows: self,
                index: 0,
                run: 0,
            }
        }

        pub fn binary_search(&self, pair: &VirtualInterference) -> Result<usize, usize> {
            let mut size = self.len();
            if size == 0 {
                return Err(0);
            }
            let mut base = 0;
            while size > 1 {
                let half = size / 2;
                let mid = base + half;
                let cmp = self
                    .get(mid)
                    .expect("search position is in range")
                    .cmp(pair);
                base = if cmp == Ordering::Greater { base } else { mid };
                size -= half;
            }
            let cmp = self
                .get(base)
                .expect("search position is in range")
                .cmp(pair);
            if cmp == Ordering::Equal {
                Ok(base)
            } else {
                Err(base + usize::from(cmp == Ordering::Less))
            }
        }

        pub fn sort_lower_tail(&mut self, start: usize) {
            assert!(start <= self.len());
            if start == self.len() {
                return;
            }
            let run = self.run_ends.partition_point(|end| *end <= start);
            assert_eq!(run + 1, self.lower_ids.len(), "tail must have one lower ID");
            self.higher_ids[start..].sort_unstable();
        }

        pub fn sort_unstable_and_dedup(&mut self) {
            let mut pairs: Vec<_> = self.iter().collect();
            pairs.sort_unstable();
            pairs.dedup();
            *self = pairs.into_iter().collect();
        }
    }

    impl FromIterator<VirtualInterference> for VirtualInterferences {
        fn from_iter<T: IntoIterator<Item = VirtualInterference>>(rows: T) -> Self {
            let mut result = Self::new();
            for row in rows {
                result.push(row);
            }
            result
        }
    }

    impl From<Vec<VirtualInterference>> for VirtualInterferences {
        fn from(rows: Vec<VirtualInterference>) -> Self {
            rows.into_iter().collect()
        }
    }

    impl PartialEq for VirtualInterferences {
        fn eq(&self, other: &Self) -> bool {
            self.len() == other.len() && self.iter().eq(other.iter())
        }
    }

    impl Eq for VirtualInterferences {}

    impl fmt::Debug for VirtualInterferences {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_list().entries(self.iter()).finish()
        }
    }

    pub struct Interferences<'a> {
        rows: &'a VirtualInterferences,
        index: usize,
        run: usize,
    }

    impl Iterator for Interferences<'_> {
        type Item = VirtualInterference;

        fn next(&mut self) -> Option<Self::Item> {
            let higher = *self.rows.higher_ids.get(self.index)?;
            while self.index >= self.rows.run_ends[self.run] {
                self.run += 1;
            }
            let lower = self.rows.lower_ids[self.run];
            self.index += 1;
            Some(VirtualInterference { lower, higher })
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.rows.len() - self.index;
            (remaining, Some(remaining))
        }
    }

    impl ExactSizeIterator for Interferences<'_> {}
    impl std::iter::FusedIterator for Interferences<'_> {}

    impl<'a> IntoIterator for &'a VirtualInterferences {
        type Item = VirtualInterference;
        type IntoIter = Interferences<'a>;

        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn pair(lower: u32, higher: u32) -> VirtualInterference {
            VirtualInterference {
                lower: VirtualRegisterId(lower),
                higher: VirtualRegisterId(higher),
            }
        }

        fn original_bytes(rows: &[VirtualInterference]) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(rows.len() as u64).to_le_bytes());
            for row in rows {
                bytes.extend_from_slice(&row.lower.0.to_le_bytes());
                bytes.extend_from_slice(&row.higher.0.to_le_bytes());
            }
            bytes
        }

        fn check(rows: &[VirtualInterference]) {
            let compact: VirtualInterferences = rows.iter().copied().collect();
            assert_eq!(compact.len(), rows.len());
            assert_eq!(compact.is_empty(), rows.is_empty());
            assert_eq!(compact.iter().collect::<Vec<_>>(), rows);
            assert_eq!(format!("{compact:?}"), format!("{rows:?}"));
            assert_eq!(format!("{compact:#?}"), format!("{rows:#?}"));
            assert_eq!(compact, compact.clone());
            assert_eq!(compact, VirtualInterferences::from(rows.to_vec()));
            for index in 0..=rows.len() {
                assert_eq!(compact.get(index), rows.get(index).copied());
            }
            let mut encoded = Vec::new();
            encoded.extend_from_slice(&(compact.len() as u64).to_le_bytes());
            for row in &compact {
                encoded.extend_from_slice(&row.lower.0.to_le_bytes());
                encoded.extend_from_slice(&row.higher.0.to_le_bytes());
            }
            assert_eq!(encoded, original_bytes(rows));
            for lower in [0, 1, 2, u32::MAX] {
                for higher in [0, 1, 2, u32::MAX] {
                    let target = pair(lower, higher);
                    assert_eq!(compact.binary_search(&target), rows.binary_search(&target));
                }
            }
            let mut changed = compact.clone();
            changed.push(pair(7, 8));
            assert_ne!(changed, compact);
            changed.clear();
            assert!(changed.is_empty());
            changed.push(pair(u32::MAX, 0));
            assert_eq!(changed.iter().collect::<Vec<_>>(), vec![pair(u32::MAX, 0)]);
            let mut iter = compact.iter();
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(iter.len(), rows.len() - index);
                assert_eq!(iter.next(), Some(*row));
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.len(), 0);
        }

        #[test]
        fn compact_edges_match_original_rows_search_debug_identity_and_construction() {
            let alphabet = [pair(0, 0), pair(0, 1), pair(1, 0), pair(1, 1)];
            for len in 0..=5 {
                for mut code in 0..4usize.pow(len) {
                    let rows = (0..len)
                        .map(|_| {
                            let row = alphabet[code % alphabet.len()];
                            code /= alphabet.len();
                            row
                        })
                        .collect::<Vec<_>>();
                    check(&rows);
                }
            }
            check(&[
                pair(u32::MAX, 0),
                pair(0, u32::MAX),
                pair(u32::MAX, u32::MAX),
            ]);
            check(&[pair(4, 3), pair(4, 3), pair(2, 8), pair(4, 9), pair(4, 0)]);

            let split = VirtualInterferences {
                lower_ids: vec![VirtualRegisterId(4), VirtualRegisterId(4)],
                run_ends: vec![1, 3],
                higher_ids: vec![
                    VirtualRegisterId(9),
                    VirtualRegisterId(2),
                    VirtualRegisterId(2),
                ],
            };
            let merged: VirtualInterferences = vec![pair(4, 9), pair(4, 2), pair(4, 2)].into();
            assert_eq!(split, merged);
            assert_eq!(format!("{split:?}"), format!("{merged:?}"));

            let mut old = Vec::new();
            let mut compact = VirtualInterferences::new();
            for (lower, higher) in [
                (4, vec![9, 1, 1]),
                (4, vec![8, 2]),
                (0, vec![]),
                (2, vec![3, 0]),
            ] {
                let first = old.len();
                for higher in higher {
                    let row = pair(lower, higher);
                    old.push(row);
                    compact.push(row);
                }
                old[first..].sort_unstable();
                compact.sort_lower_tail(first);
                assert_eq!(compact.iter().collect::<Vec<_>>(), old);
            }
            old.sort_unstable();
            old.dedup();
            compact.sort_unstable_and_dedup();
            assert_eq!(compact.iter().collect::<Vec<_>>(), old);

            let dense: VirtualInterferences = (0..128)
                .flat_map(|lower| (lower + 1..128).map(move |higher| pair(lower, higher)))
                .collect();
            assert_eq!(dense.len(), 128 * 127 / 2);
            assert_eq!(dense.lower_ids.len(), 127);
            assert_eq!(std::mem::size_of::<VirtualInterference>(), 8);
            let payload = dense.higher_ids.len() * std::mem::size_of::<VirtualRegisterId>()
                + dense.lower_ids.len() * std::mem::size_of::<VirtualRegisterId>()
                + dense.run_ends.len() * std::mem::size_of::<usize>();
            assert!(payload < dense.len() * std::mem::size_of::<VirtualInterference>());
        }
    }
}
pub use identity::{live_range_function_digest, live_range_identity_from_digests};
pub use interference::VirtualInterferences;

use crate::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;

use crate::{LivenessIdentity, LivenessPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LiveRangePoint(pub u32);

crate::identity::sha256_identity!(LiveRangeIdentity);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRangePlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<std::sync::Arc<FunctionLiveRanges>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveRanges {
    pub machine: MachineId,
    pub block_domains: Vec<BlockPointDomain>,
    pub virtual_registers: Vec<VirtualLiveRange>,
    pub tied_pairs: Vec<DistinctUseDefTie>,
    pub edge_transfers: Vec<EdgeRegisterTransfer>,
    pub copy_affinities: Vec<CopyAffinity>,
    pub early_clobbers: Vec<EarlyClobberConstraint>,
    pub architectural_units: Vec<ArchitecturalUnitLiveRange>,
    pub interference: VirtualInterferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRegisterTransfer {
    pub source: SelectedBlockId,
    pub target: SelectedBlockId,
    pub psi_edge: EdgeId,
    pub argument: VirtualRegisterId,
    pub parameter: VirtualRegisterId,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CopyAffinity {
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub source: VirtualRegisterId,
    pub destination: VirtualRegisterId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberConstraint {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub early_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_class: RegisterClassId,
    pub def_point: LiveRangePoint,
    pub uses: Vec<EarlyClobberUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberUse {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DistinctUseDefTie {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub use_operand: u16,
    pub use_virtual_register: VirtualRegisterId,
    pub use_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_point: LiveRangePoint,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPointDomain {
    pub block: SelectedBlockId,
    pub source_block: BlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLiveRange {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub occurrences: Vec<VirtualOccurrence>,
    pub fixed_constraints: Vec<VirtualFixedConstraint>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualOccurrence {
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub access: RegisterOperandAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFixedConstraintSite {
    Entry,
    Operand {
        position: LivenessPosition,
        point: LiveRangePoint,
        instruction: SelectedInstructionId,
        operand: u16,
        access: RegisterOperandAccess,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualFixedConstraint {
    pub site: VirtualFixedConstraintSite,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeFragment {
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeEdgeConnector {
    pub source: SelectedBlockId,
    pub terminator: SelectedInstructionId,
    pub polarity_ordinal: u8,
    pub psi_edge: Option<EdgeId>,
    pub target: SelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitecturalUnitLiveRange {
    pub unit: RegisterUnitId,
    pub actions: Vec<ArchitecturalUnitAction>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchitecturalUnitAction {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub kind: ArchitecturalUnitActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchitecturalUnitActionKind {
    Use,
    Def,
    Clobber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualInterference {
    pub lower: VirtualRegisterId,
    pub higher: VirtualRegisterId,
}
