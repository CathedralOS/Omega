//! Raw custody persistence preserves every proof/frontier field without admission.

use abstract_operations::{
    RankedU32CountdownCustody, decode_ranked_u32_countdown_custody,
    encode_ranked_u32_countdown_custody, ranked_u32_countdown_custody_identity,
};

pub(super) fn check(custody: &RankedU32CountdownCustody) {
    let encoded = encode_ranked_u32_countdown_custody(custody).unwrap();
    let decoded = decode_ranked_u32_countdown_custody(&encoded).unwrap();
    assert_eq!(&decoded, custody);
    assert_eq!(
        encode_ranked_u32_countdown_custody(&decoded).unwrap(),
        encoded
    );
    assert_eq!(
        ranked_u32_countdown_custody_identity(&decoded).unwrap(),
        ranked_u32_countdown_custody_identity(custody).unwrap()
    );
    let domain = b"omega.ranked-custody.v1\0";
    assert!(encoded.starts_with(domain));
    for length in [
        0,
        domain.len() - 1,
        domain.len(),
        encoded.len() / 2,
        encoded.len() - 1,
    ] {
        assert!(decode_ranked_u32_countdown_custody(&encoded[..length]).is_err());
    }
    for version in *b"02" {
        let mut changed = encoded.clone();
        changed[domain.len() - 2] = version;
        assert!(decode_ranked_u32_countdown_custody(&changed).is_err());
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_ranked_u32_countdown_custody(&trailing).is_err());

    let mut changed = custody.clone();
    changed.structural_frontiers.backedge = custody.graph.return_edge;
    assert_ne!(
        ranked_u32_countdown_custody_identity(&changed).unwrap(),
        ranked_u32_countdown_custody_identity(custody).unwrap()
    );
    // Raw decoding keeps the changed field. The publication tests independently
    // reject this same substitution against their retained checked source.
    let changed_bytes = encode_ranked_u32_countdown_custody(&changed).unwrap();
    assert_eq!(
        decode_ranked_u32_countdown_custody(&changed_bytes).unwrap(),
        changed
    );
}
