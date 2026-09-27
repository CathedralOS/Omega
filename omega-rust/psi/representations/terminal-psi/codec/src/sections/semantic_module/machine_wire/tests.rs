use super::{decode_ranked_scc, encode_ranked_scc};
use crate::CodecError;
use crate::sections::semantic_module::wire::{Reader, Writer};
use semantic_vocabulary::{BlockId, EdgeId, IntegerSign, IntegerType, ValueId};
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
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
