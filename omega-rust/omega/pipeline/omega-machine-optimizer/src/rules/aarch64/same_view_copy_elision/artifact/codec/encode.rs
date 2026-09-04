use super::super::Aarch64SameViewCopyElisionPlan;

pub(super) const MAGIC: &[u8; 8] = b"OMGICE\0\0";
pub(super) const VERSION: u32 = 4;

pub(super) fn encode(plan: &Aarch64SameViewCopyElisionPlan) -> Vec<u8> {
    let content = super::super::identity::encode_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}
