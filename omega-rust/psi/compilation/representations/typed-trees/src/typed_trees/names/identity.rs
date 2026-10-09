use crate::data::DataMember;
use crate::typed_trees::TypedTrees;

/// Compact report fingerprint for one exact typed data definition.
///
/// Numbered member names are presentation-only; unnumbered members retain
/// their declaration position and source name. Runtime case ordinals are not
/// stable schema identities and are deliberately absent. This FNV value is not
/// authority: typed consumers retain and structurally replay the exact schema.
pub fn normalized_schema_report_fingerprint(
    typed: &TypedTrees,
    data: &crate::data::DataDefinition,
) -> u64 {
    fn member_name(
        hash: &mut semantic_vocabulary::Fnv1a,
        identity: Option<u64>,
        name: &str,
        position: usize,
    ) {
        match identity {
            Some(identity) => {
                hash.byte(1);
                hash.u64(identity);
            }
            None => {
                hash.byte(0);
                hash.u64(position as u64);
                hash.string_framed(name);
            }
        }
    }

    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.schema.v2");
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
    hash.u64(fields.len() as u64);
    for (position, field) in fields.iter().enumerate() {
        member_name(&mut hash, field.identity, field.name.as_str(), position);
        hash.byte(match field.relevance {
            language_core::BindingRelevance::Relevant => 0,
            language_core::BindingRelevance::Erased => 1,
        });
        hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
    }
    hash.u64(cases.len() as u64);
    for (position, case) in cases.iter().enumerate() {
        member_name(&mut hash, case.identity, case.name.as_str(), position);
        let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
        if payload.iter().all(|field| field.identity.is_some()) {
            payload.sort_by_key(|field| field.identity);
        }
        hash.u64(payload.len() as u64);
        for (payload_position, field) in payload.iter().enumerate() {
            member_name(
                &mut hash,
                field.identity,
                field.name.as_str(),
                payload_position,
            );
            hash.byte(match field.relevance {
                language_core::BindingRelevance::Relevant => 0,
                language_core::BindingRelevance::Erased => 1,
            });
            hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
        }
        let mut retired = case.retired_payload_identities.clone();
        retired.sort_unstable();
        hash.u64(retired.len() as u64);
        for identity in retired {
            hash.u64(identity);
        }
    }
    let mut retired = data.retired_identities.clone();
    retired.sort_unstable();
    hash.u64(retired.len() as u64);
    for identity in retired {
        hash.u64(identity);
    }
    hash.finish().max(1)
}
