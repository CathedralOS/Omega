use crate::typed_trees::data::DataMember;
use crate::typed_trees::typed_trees::TypedTrees;

/// Compact report fingerprint for one exact typed data definition.
///
/// Numbered member names are presentation-only; unnumbered members retain
/// their declaration position and source name. Runtime case ordinals are not
/// stable schema identities and are deliberately absent. This FNV value is not
/// authority: typed consumers retain and structurally replay the exact schema.
pub fn normalized_schema_report_fingerprint(
    typed: &TypedTrees,
    data: &crate::typed_trees::data::DataDefinition,
) -> u64 {
    fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    fn member_name(hash: &mut u64, identity: Option<u64>, name: &str, position: usize) {
        match identity {
            Some(identity) => {
                byte(hash, 1);
                uint(hash, identity);
            }
            None => {
                byte(hash, 0);
                uint(hash, position as u64);
                text(hash, name);
            }
        }
    }

    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, b"omega.schema.v2");
    let members = typed.data_members(data);
    let mut fields = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let mut cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if fields.iter().all(|field| field.identity.is_some()) {
        fields.sort_by_key(|field| field.identity);
    }
    if cases.iter().all(|case| case.identity.is_some()) {
        cases.sort_by_key(|case| case.identity);
    }
    uint(&mut hash, fields.len() as u64);
    for (position, field) in fields.iter().enumerate() {
        member_name(&mut hash, field.identity, field.name.as_str(), position);
        byte(
            &mut hash,
            match field.relevance {
                language_core::BindingRelevance::Relevant => 0,
                language_core::BindingRelevance::Erased => 1,
            },
        );
        text(
            &mut hash,
            typed.display_type_reference(field.type_reference).as_str(),
        );
    }
    uint(&mut hash, cases.len() as u64);
    for (position, case) in cases.iter().enumerate() {
        member_name(&mut hash, case.identity, case.name.as_str(), position);
        let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
        if payload.iter().all(|field| field.identity.is_some()) {
            payload.sort_by_key(|field| field.identity);
        }
        uint(&mut hash, payload.len() as u64);
        for (payload_position, field) in payload.iter().enumerate() {
            member_name(
                &mut hash,
                field.identity,
                field.name.as_str(),
                payload_position,
            );
            byte(
                &mut hash,
                match field.relevance {
                    language_core::BindingRelevance::Relevant => 0,
                    language_core::BindingRelevance::Erased => 1,
                },
            );
            text(
                &mut hash,
                typed.display_type_reference(field.type_reference).as_str(),
            );
        }
        let mut retired = case.retired_payload_identities.clone();
        retired.sort_unstable();
        uint(&mut hash, retired.len() as u64);
        for identity in retired {
            uint(&mut hash, identity);
        }
    }
    let mut retired = data.retired_identities.clone();
    retired.sort_unstable();
    uint(&mut hash, retired.len() as u64);
    for identity in retired {
        uint(&mut hash, identity);
    }
    if hash == 0 { 1 } else { hash }
}
