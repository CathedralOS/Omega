use ::wire::digest_encoder::DomainDigestEncoder;
use wire::fnv1a::Fnv1a;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConformanceIdentityDescription<'identity> {
    pub declaration: &'identity str,
    pub trait_name: &'identity str,
    pub lifetime_arguments: &'identity [String],
    pub type_arguments: &'identity [String],
    pub trait_lifetime_arguments: &'identity [String],
    pub trait_arguments: &'identity [String],
    pub const_arguments: Vec<ConformanceConstIdentity<'identity>>,
    pub machine_arguments: Vec<String>,
    pub machine_operands: Vec<MachineOperandIdentity>,
    pub rows: Vec<ConformanceRowIdentity<'identity>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceConstIdentity<'identity> {
    Evaluated {
        parameter_carrier: String,
        declared_carrier: String,
        value_type: &'identity str,
        value_encoding: &'identity str,
    },
    CallerBinder {
        parameter_carrier: String,
        binder_carrier: String,
        binder_identity: String,
    },
    CallerExpression {
        parameter_carrier: String,
        expression_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineOperandIdentity {
    pub selection: MachineOperandSelectionIdentity,
    pub application: Option<MachineOperandApplicationIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineOperandSelectionIdentity {
    Const(String),
    Type(String),
    Conformance(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineOperandApplicationIdentity {
    pub lifetime_arguments: Vec<String>,
    pub arguments: Vec<MachineOperandIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRowIdentity<'identity> {
    pub declaring_trait: &'identity str,
    pub requirement: &'identity str,
    pub realization: &'identity str,
    pub requirement_signature: Option<Vec<u8>>,
}

impl ConformanceIdentityDescription<'_> {
    pub fn encoded_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for singleton in [self.declaration, self.trait_name] {
            bytes.extend(1u64.to_le_bytes());
            append_text(&mut bytes, singleton);
            bytes.push(0xff);
        }
        for lane in [
            self.lifetime_arguments,
            self.type_arguments,
            self.trait_lifetime_arguments,
            self.trait_arguments,
        ] {
            bytes.extend((lane.len() as u64).to_le_bytes());
            for item in lane {
                append_text(&mut bytes, item);
            }
            bytes.push(0xff);
        }
        bytes.extend((self.const_arguments.len() as u64).to_le_bytes());
        for argument in &self.const_arguments {
            match argument {
                ConformanceConstIdentity::Evaluated {
                    parameter_carrier,
                    declared_carrier,
                    value_type,
                    value_encoding,
                } => {
                    bytes.push(0);
                    for identity in [
                        parameter_carrier.as_str(),
                        declared_carrier.as_str(),
                        value_type,
                        value_encoding,
                    ] {
                        append_text(&mut bytes, identity);
                    }
                }
                ConformanceConstIdentity::CallerBinder {
                    parameter_carrier,
                    binder_carrier,
                    binder_identity,
                } => {
                    bytes.push(1);
                    for identity in [parameter_carrier, binder_carrier, binder_identity] {
                        append_text(&mut bytes, identity);
                    }
                }
                ConformanceConstIdentity::CallerExpression {
                    parameter_carrier,
                    expression_identity,
                } => {
                    bytes.push(2);
                    append_text(&mut bytes, parameter_carrier);
                    append_text(&mut bytes, expression_identity);
                }
            }
            bytes.push(0xfd);
        }
        for machine in &self.machine_arguments {
            append_text(&mut bytes, machine);
        }
        bytes.extend((self.machine_operands.len() as u64).to_le_bytes());
        for operand in &self.machine_operands {
            operand.encode(&mut bytes);
        }
        for row in &self.rows {
            for identity in [row.declaring_trait, row.requirement, row.realization] {
                append_text(&mut bytes, identity);
            }
            match &row.requirement_signature {
                Some(signature) => {
                    bytes.push(1);
                    bytes.extend((signature.len() as u64).to_le_bytes());
                    bytes.extend(signature);
                }
                None => bytes.push(0),
            }
            bytes.push(0xfe);
        }
        bytes
    }

    pub fn fingerprint_and_commitment(&self) -> (u64, [u8; 32]) {
        let bytes = self.encoded_bytes();
        source_identity_commitments(&bytes)
    }
}

/// A generated body's source publisher and receiver calculate its identity
/// from the same complete arguments. Value arguments retain the checked
/// binder's carrier as well as their canonical static value.
pub struct MachineApplicationCommitment(DomainDigestEncoder);

impl MachineApplicationCommitment {
    pub fn new(callable: &str) -> Result<Self, &'static str> {
        use std::hash::Hash;
        if callable.is_empty() {
            return Err("machine application has no exact template callable identity");
        }
        let mut commitment = Self(DomainDigestEncoder::new(
            b"omega.lowered-machine-static-application.v1\0",
        ));
        callable.hash(&mut commitment);
        Ok(commitment)
    }

    pub fn type_argument(&mut self, identity: &str) -> Result<(), &'static str> {
        use std::hash::Hash;
        if identity.is_empty() {
            return Err("machine application has an empty type argument");
        }
        identity.hash(self);
        Ok(())
    }

    pub fn const_argument(
        &mut self,
        value: &crate::const_value::CanonicalConstValue,
    ) -> Result<(), &'static str> {
        use std::hash::Hash;
        if !value.is_canonical_static_value() {
            return Err("machine application has no checked static carrier and value");
        }
        (
            "canonical-value.v2",
            value.type_name.as_str(),
            value.encoding.as_str(),
        )
            .hash(self);
        Ok(())
    }

    pub fn conformance_argument(&mut self, identity: &str) -> Result<(), &'static str> {
        use std::hash::Hash;
        if identity.is_empty() {
            return Err("machine application has an empty implementation identity");
        }
        ("implementation", identity).hash(self);
        Ok(())
    }

    pub fn machine_argument(&mut self, identity: &str) -> Result<(), &'static str> {
        use std::hash::Hash;
        if identity.is_empty() {
            return Err("machine application has an empty callable argument identity");
        }
        ("machine", identity).hash(self);
        Ok(())
    }

    pub fn finish(self) -> [u8; 32] {
        self.0.finish()
    }
}

impl std::hash::Hasher for MachineApplicationCommitment {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

/// The source publisher and artifact receiver must identify the same
/// applied public member. Preserve the existing type/value hash sequence;
/// another operand category needs its complete contract before use here.
pub fn requirement_application_commitment(
    callable: &str,
    arguments: &[MachineOperandSelectionIdentity],
) -> Result<[u8; 32], &'static str> {
    use std::hash::Hash;
    let mut commitment = MachineApplicationCommitment::new(callable)?;
    for argument in arguments {
        match argument {
            MachineOperandSelectionIdentity::Type(identity) if !identity.is_empty() => {
                commitment.type_argument(identity)?;
            }
            MachineOperandSelectionIdentity::Const(identity) if !identity.is_empty() => {
                ("value", identity.as_str()).hash(&mut commitment);
            }
            _ => {
                return Err("applied requirement needs complete type or value argument identity");
            }
        }
    }
    Ok(commitment.finish())
}

pub fn source_identity_commitments(bytes: &[u8]) -> (u64, [u8; 32]) {
    let mut report = Fnv1a::new();
    report.bytes(bytes);
    let mut commitment = DomainDigestEncoder::new(b"omega.psi.closed-conformance-application.v4\0");
    commitment.raw(bytes);
    (report.finish(), commitment.finish())
}

impl MachineOperandIdentity {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let (tag, identity) = match &self.selection {
            MachineOperandSelectionIdentity::Const(identity) => (1, identity),
            MachineOperandSelectionIdentity::Type(identity) => (2, identity),
            MachineOperandSelectionIdentity::Conformance(identity) => (3, identity),
        };
        bytes.push(tag);
        append_text(bytes, identity);
        match &self.application {
            Some(application) => {
                bytes.push(1);
                bytes.extend((application.lifetime_arguments.len() as u64).to_le_bytes());
                for lifetime in &application.lifetime_arguments {
                    append_text(bytes, lifetime);
                }
                bytes.extend((application.arguments.len() as u64).to_le_bytes());
                for argument in &application.arguments {
                    argument.encode(bytes);
                }
            }
            None => bytes.push(0),
        }
    }
}

fn append_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend((text.len() as u64).to_le_bytes());
    bytes.extend(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_const_commitments_preserve_scalars_and_exact_callable_records() {
        use crate::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use std::hash::Hash;
        let scalar = CanonicalConstValue::boolean(true);
        let mut current = MachineApplicationCommitment::new("template").unwrap();
        current.const_argument(&scalar).unwrap();
        let mut previous = MachineApplicationCommitment::new("template").unwrap();
        (
            "canonical-value.v2",
            scalar.type_name.as_str(),
            scalar.encoding.as_str(),
        )
            .hash(&mut previous);
        assert_eq!(current.finish(), previous.finish());
        let callable = |declaration: &str, requirement: &str| DecodedCanonicalConstValue::Record {
            type_name: "Operations".into(),
            fields: vec![(
                "write".into(),
                DecodedCanonicalConstValue::StaticCallable {
                    declaration: declaration.into(),
                    application: Some("selected-application".into()),
                    requirement: Some(requirement.into()),
                },
            )],
        };
        let commitment = |value| {
            let value = CanonicalConstValue::from_decoded(&value, "same display").unwrap();
            let mut commitment = MachineApplicationCommitment::new("template").unwrap();
            commitment.const_argument(&value).unwrap();
            commitment.finish()
        };
        let expected = commitment(callable("selected-write", "required-write"));
        assert_ne!(
            expected,
            commitment(callable("another-write", "required-write"))
        );
        assert_ne!(
            expected,
            commitment(callable("selected-write", "another-requirement"))
        );
        for value in [
            CanonicalConstValue::new("bool", "broken", "true"),
            CanonicalConstValue::new("u8", "integer2:u83:256", "256"),
        ] {
            assert!(
                MachineApplicationCommitment::new("template")
                    .unwrap()
                    .const_argument(&value)
                    .is_err()
            );
        }
    }

    fn description<'identity>(
        lanes: &'identity [Vec<String>; 4],
    ) -> ConformanceIdentityDescription<'identity> {
        ConformanceIdentityDescription {
            declaration: "Pick",
            trait_name: "Slot",
            lifetime_arguments: &lanes[0],
            type_arguments: &lanes[1],
            trait_lifetime_arguments: &lanes[2],
            trait_arguments: &lanes[3],
            const_arguments: vec![
                ConformanceConstIdentity::Evaluated {
                    parameter_carrier: "u64".into(),
                    declared_carrier: "u32".into(),
                    value_type: "u32",
                    value_encoding: "7",
                },
                ConformanceConstIdentity::CallerBinder {
                    parameter_carrier: "u64".into(),
                    binder_carrier: "u64".into(),
                    binder_identity: "Owner::N".into(),
                },
                ConformanceConstIdentity::CallerExpression {
                    parameter_carrier: "u64".into(),
                    expression_identity: "add(N,1)".into(),
                },
            ],
            machine_arguments: vec!["Handler::call".into(), "Other::entry".into()],
            machine_operands: vec![MachineOperandIdentity {
                selection: MachineOperandSelectionIdentity::Conformance("pkg::Dispatch".into()),
                application: Some(MachineOperandApplicationIdentity {
                    lifetime_arguments: vec!["'a".into()],
                    arguments: vec![
                        MachineOperandIdentity {
                            selection: MachineOperandSelectionIdentity::Type("u8".into()),
                            application: None,
                        },
                        MachineOperandIdentity {
                            selection: MachineOperandSelectionIdentity::Const("u64:7".into()),
                            application: None,
                        },
                        MachineOperandIdentity {
                            selection: MachineOperandSelectionIdentity::Conformance(
                                "pkg::Inner".into(),
                            ),
                            application: Some(MachineOperandApplicationIdentity {
                                lifetime_arguments: Vec::new(),
                                arguments: Vec::new(),
                            }),
                        },
                    ],
                }),
            }],
            rows: vec![
                ConformanceRowIdentity {
                    declaring_trait: "Slot",
                    requirement: "invoke",
                    realization: "handler",
                    requirement_signature: Some(vec![0, 255, 254]),
                },
                ConformanceRowIdentity {
                    declaring_trait: "Slot",
                    requirement: "empty",
                    realization: "none",
                    requirement_signature: None,
                },
            ],
        }
    }

    fn lanes() -> [Vec<String>; 4] {
        [
            vec!["'a".into(), "'b".into()],
            vec!["Pair<u8>".into()],
            vec!["'b".into()],
            vec!["Pair<u8>".into(), "Layout".into(), "Callback".into()],
        ]
    }

    #[test]
    fn source_v4_snapshot_keeps_all_lanes_and_historical_framing() {
        let lanes = lanes();
        let description = description(&lanes);
        assert_eq!(description.encoded_bytes().len(), 545);
        assert_eq!(
            description.fingerprint_and_commitment(),
            (
                0xb83bfa9b72729933,
                [
                    0x93, 0x93, 0x25, 0xb5, 0xfa, 0xe8, 0x93, 0x96, 0xcc, 0x8e, 0xe3, 0x13, 0x43,
                    0x71, 0xf2, 0xd0, 0xd5, 0x14, 0xc6, 0x63, 0x1a, 0x00, 0x69, 0x52, 0x19, 0xb7,
                    0x23, 0x62, 0x3f, 0x30, 0x7d, 0x88,
                ]
            )
        );
    }

    #[test]
    fn role_carriers_recursive_selection_and_application_presence_are_distinct() {
        let lanes = lanes();
        let original = description(&lanes);
        let expected = original.fingerprint_and_commitment();
        let changed = |candidate: ConformanceIdentityDescription<'_>| {
            let observed = candidate.fingerprint_and_commitment();
            assert_ne!(observed.0, expected.0);
            assert_ne!(observed.1, expected.1);
        };
        let mut candidate = original.clone();
        let ConformanceConstIdentity::Evaluated {
            declared_carrier, ..
        } = &mut candidate.const_arguments[0]
        else {
            panic!("fixture must contain an evaluated const");
        };
        *declared_carrier = "u64".into();
        changed(candidate);
        let mut candidate = original.clone();
        let application = candidate.machine_operands[0].application.as_mut().unwrap();
        application.arguments[0].selection = MachineOperandSelectionIdentity::Const("u8".into());
        changed(candidate);
        let mut candidate = original.clone();
        candidate.machine_operands[0]
            .application
            .as_mut()
            .unwrap()
            .arguments[2]
            .application = None;
        changed(candidate);
        let mut candidate = original.clone();
        candidate.rows[1].requirement_signature = Some(Vec::new());
        changed(candidate);
        let mut candidate = original.clone();
        candidate.machine_arguments.reverse();
        changed(candidate);

        let mut terminal_domain =
            DomainDigestEncoder::new(b"omega.psi.terminal.closed-conformance-application.v7\0");
        terminal_domain.raw(&original.encoded_bytes());
        assert_ne!(expected.1, terminal_domain.finish());
    }

    #[test]
    fn selected_slot_record_preserves_roles_and_source_preimage() {
        use crate::callback_declarations::PrivateCallbackSlotSelection;
        let lanes = lanes();
        let description = description(&lanes);
        let application_encoding: std::sync::Arc<[u8]> = description.encoded_bytes().into();
        let (application_report_fingerprint, application_commitment) =
            source_identity_commitments(&application_encoding);
        let selection = PrivateCallbackSlotSelection {
            declaration_identity: "package::Pick".into(),
            trait_identity: "core::layout::PrivateCallbackSlot".into(),
            semantic_identity: "package::Pair<u8>".into(),
            layout_identity: "package::Layout".into(),
            requirement_identity: "package::Callback::call".into(),
            application_report_fingerprint,
            application_commitment,
            application_encoding: application_encoding.clone(),
        };
        selection.validate_identity_encoding().unwrap();
        let bytes = record_storage::encode(&selection, Default::default()).unwrap();
        let restored: PrivateCallbackSlotSelection =
            record_storage::decode(&bytes, Default::default()).unwrap();
        assert_eq!(restored, selection);
        restored.validate_identity_encoding().unwrap();
        let cloned = selection.clone();
        assert!(std::sync::Arc::ptr_eq(
            &cloned.application_encoding,
            &application_encoding
        ));
        let mut changed = selection.clone();
        std::sync::Arc::make_mut(&mut changed.application_encoding)[0] ^= 1;
        assert!(
            changed
                .validate_identity_encoding()
                .unwrap_err()
                .contains("Source-v4 identity preimage")
        );
        let mut changed = selection;
        changed.requirement_identity.clear();
        assert!(
            changed
                .validate_identity_encoding()
                .unwrap_err()
                .contains("exact selected application identity")
        );
    }
}
