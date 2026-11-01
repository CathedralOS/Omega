//! Normalized inputs to the historical Source closed-conformance identity.
//!
//! Source resolution owns normalization. This syntax-free description owns the
//! v4 byte protocol so a retained layout selection can use the same encoder
//! without keeping Source or confusing it with Terminal's distinct v7 identity.
//! A description is an identity claim, not evidence of resolved role custody.

use optimization::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::Fnv1a;

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
    /// Encode the existing Source v4 protocol, including its intentionally
    /// uncounted machine-selection and row lanes. Changing these bytes is an
    /// identity migration, not a harmless codec cleanup.
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

    /// Historical report fingerprint and domain-separated Source commitment.
    /// Neither replaces the selected declaration's hermetic identity or its
    /// independently retained semantic/layout/requirement role correspondence.
    pub fn fingerprint_and_commitment(&self) -> (u64, [u8; 32]) {
        let bytes = self.encoded_bytes();
        let mut report = Fnv1a::new();
        report.bytes(&bytes);
        let mut commitment =
            DomainDigestEncoder::new(b"omega.psi.closed-conformance-application.v4\0");
        commitment.raw(&bytes);
        (report.finish(), commitment.finish())
    }
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
mod tests;
