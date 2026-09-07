use super::{decode_ranked_scc, encode_ranked_scc};
use crate::CodecError;
use crate::wire::{Reader, Writer};
use semantic_vocabulary::{BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, ValueId};
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedGuard, TerminalRankedScc, TerminalRankedSccEdge,
    TerminalRankedSuccessorArgument, TerminalUnsignedCountdownScc,
};

fn natural_ranking() -> TerminalRankedScc {
    TerminalRankedScc::Natural(vec![
        TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            ranks: vec![
                TerminalBlockNaturalRank {
                    block: BlockId::new(11).unwrap(),
                    value: ValueId::new(21).unwrap(),
                },
                TerminalBlockNaturalRank {
                    block: BlockId::new(12).unwrap(),
                    value: ValueId::new(21).unwrap(),
                },
            ],
            edges: vec![
                TerminalNaturalRankEdge {
                    edge: EdgeId::new(31).unwrap(),
                    source: BlockId::new(11).unwrap(),
                    target: BlockId::new(12).unwrap(),
                    successor_rank: ValueId::new(21).unwrap(),
                    comparison: TerminalNaturalRankComparison::Preserving,
                },
                TerminalNaturalRankEdge {
                    edge: EdgeId::new(32).unwrap(),
                    source: BlockId::new(12).unwrap(),
                    target: BlockId::new(11).unwrap(),
                    successor_rank: ValueId::new(22).unwrap(),
                    comparison: TerminalNaturalRankComparison::Strict,
                },
            ],
        },
        TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            ranks: vec![TerminalBlockNaturalRank {
                block: BlockId::new(13).unwrap(),
                value: ValueId::new(23).unwrap(),
            }],
            edges: vec![TerminalNaturalRankEdge {
                edge: EdgeId::new(33).unwrap(),
                source: BlockId::new(13).unwrap(),
                target: BlockId::new(13).unwrap(),
                successor_rank: ValueId::new(24).unwrap(),
                comparison: TerminalNaturalRankComparison::Strict,
            }],
        },
    ])
}

fn encoded(ranking: &TerminalRankedScc) -> Vec<u8> {
    let mut writer = Writer::default();
    encode_ranked_scc(&mut writer, Some(ranking)).unwrap();
    writer.finish()
}

#[test]
fn natural_wire_retains_multiple_components_observations_and_comparisons() {
    let ranking = natural_ranking();
    let bytes = encoded(&ranking);
    assert_eq!(&bytes[..8], &[2, 2, 0, 0, 0, 2, 32, 0]);
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode_ranked_scc(&mut reader), Ok(Some(ranking)));
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn natural_wire_rejects_unknown_tags_zero_identities_and_truncation() {
    let bytes = encoded(&natural_ranking());
    for end in 0..bytes.len() {
        assert!(decode_ranked_scc(&mut Reader::new(&bytes[..end])).is_err());
    }
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() = 3;
    assert_eq!(
        decode_ranked_scc(&mut Reader::new(&changed)),
        Err(CodecError::InvalidTag("TerminalNaturalRankComparison", 3)),
    );
    let mut changed = bytes;
    // Tag, component count, integer type, rank count precede the first block.
    changed[12..20].fill(0);
    assert_eq!(
        decode_ranked_scc(&mut Reader::new(&changed)),
        Err(CodecError::ZeroIdentity("BlockId")),
    );
    assert_eq!(
        decode_ranked_scc(&mut Reader::new(&[3])),
        Err(CodecError::InvalidTag("TerminalRankedScc", 3)),
    );
}

#[test]
fn unsigned_countdown_tag_one_payload_is_unchanged() {
    let ranking = TerminalRankedScc::UnsignedCountdown(TerminalUnsignedCountdownScc {
        header: BlockId::new(11).unwrap(),
        rank_parameter: ValueId::new(21).unwrap(),
        rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        lower_bound: IntegerValue::Unsigned(0),
        upper_bound: IntegerValue::Unsigned(255),
        covered_cyclic_edges: vec![TerminalRankedSccEdge {
            edge: EdgeId::new(31).unwrap(),
            source: BlockId::new(12).unwrap(),
            target: BlockId::new(11).unwrap(),
            guard: TerminalRankedGuard::UnsignedParameterPositive {
                block: BlockId::new(11).unwrap(),
                edge: EdgeId::new(30).unwrap(),
                condition: ValueId::new(22).unwrap(),
                parameter: ValueId::new(21).unwrap(),
            },
            successor_argument: TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                argument_index: 0,
                argument: ValueId::new(23).unwrap(),
                source_parameter: ValueId::new(21).unwrap(),
                target_parameter: ValueId::new(21).unwrap(),
            },
        }],
    });
    let bytes = encoded(&ranking);
    let mut expected = vec![1];
    expected.extend_from_slice(&11_u64.to_le_bytes());
    expected.extend_from_slice(&21_u64.to_le_bytes());
    expected.extend_from_slice(&[2, 32, 0, 2]);
    expected.extend_from_slice(&0_u128.to_le_bytes());
    expected.push(2);
    expected.extend_from_slice(&255_u128.to_le_bytes());
    expected.extend_from_slice(&1_u32.to_le_bytes());
    for identity in [31_u64, 12, 11] {
        expected.extend_from_slice(&identity.to_le_bytes());
    }
    expected.push(1);
    for identity in [11_u64, 30, 22, 21] {
        expected.extend_from_slice(&identity.to_le_bytes());
    }
    expected.push(1);
    expected.extend_from_slice(&0_u32.to_le_bytes());
    for identity in [23_u64, 21, 21] {
        expected.extend_from_slice(&identity.to_le_bytes());
    }
    assert_eq!(bytes, expected);
    assert_eq!(
        decode_ranked_scc(&mut Reader::new(&expected)),
        Ok(Some(ranking)),
    );
}
