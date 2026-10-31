//! Target-neutral nominal coordinates for boundary parameter telescopes.
//! Compact fingerprints discriminate retained rows; they never grant authority.
//! Consumers still retain exact declarations and reject differing rows behind
//! an equal report coordinate.

macro_rules! nominal_declaration_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);
        impl $name {
            pub const fn new(value: u64) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

nominal_declaration_id!(StaticMachineBinderId);
nominal_declaration_id!(NativeParameterId);
nominal_declaration_id!(CallbackRequirementId);

/// Authored origin, independent of physical placement or equal-sized values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterOrigin {
    SemanticFormal {
        formal_ordinal: u32,
    },
    PrivateCallback {
        binder: StaticMachineBinderId,
        requirement: CallbackRequirementId,
    },
}

pub fn callback_nominal_report_fingerprint(domain: &[u8], parts: &[&[u8]]) -> u64 {
    let mut hasher = crate::Fnv1a::new();
    for bytes in std::iter::once(domain).chain(parts.iter().copied()) {
        hasher.bytes_framed(bytes);
    }
    match hasher.finish() {
        0 => 1,
        fingerprint => fingerprint,
    }
}

pub fn callback_requirement_id(canonical_requirement: &str) -> CallbackRequirementId {
    CallbackRequirementId(callback_nominal_report_fingerprint(
        b"omega.callback-requirement.v1",
        &[canonical_requirement.as_bytes()],
    ))
}

/// Names fix identity; telescope order is separate and cannot rename a formal.
pub fn nominal_callback_native_parameter_id(
    canonical_requirement: &str,
    canonical_parameter_name: &str,
) -> NativeParameterId {
    NativeParameterId(callback_nominal_report_fingerprint(
        b"omega.callback-native-parameter.v2",
        &[
            canonical_requirement.as_bytes(),
            canonical_parameter_name.as_bytes(),
        ],
    ))
}

pub fn nominal_callback_binder_id(owner: &str, ordinal: u32, name: &str) -> StaticMachineBinderId {
    StaticMachineBinderId(callback_nominal_report_fingerprint(
        b"omega.callback-binder.v1",
        &[owner.as_bytes(), &ordinal.to_le_bytes(), name.as_bytes()],
    ))
}
