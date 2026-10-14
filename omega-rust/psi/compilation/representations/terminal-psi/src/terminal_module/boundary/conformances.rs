use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTable};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplication {
    pub owner: MachineId,
    pub declaration_identity: TerminalIdentityIndex,
    pub telescope: Vec<ClosedConformanceParameterBinding>,
    pub trait_identity: TerminalIdentityIndex,
    pub trait_lifetime_arguments: Vec<String>,
    pub trait_arguments: Vec<String>,
    /// Ordered standalone replay registry derived from the exact checked
    /// source-machine closure. Rows name entries by canonical callable
    /// identity without duplicating the artifact-local machine coordinate.
    pub realization_callables: Vec<ClosedConformanceRealizationCallable>,
    /// Complete authored inventory, including uncalled open generic methods.
    /// These schemas supply neither executable coverage nor authority.
    pub schemas: Vec<ClosedConformanceSchema>,
    /// Exact static applications are independent of finite dynamic rosters.
    pub static_applications: Vec<ClosedConformanceStaticApplication>,
    pub rows: Vec<ClosedConformanceRow>,
    /// Historical compact report/index coordinate. It cannot authorize a
    /// dispatch or replay without the adjacent strong commitment.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the exact source-free
    /// application structure.
    pub commitment: ClosedConformanceApplicationCommitment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceSchemaKind {
    Nongeneric,
    Finite,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceSchema {
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub public_requirement_identity: TerminalIdentityIndex,
    pub requirement_identity: TerminalIdentityIndex,
    pub realization_identity: TerminalIdentityIndex,
    /// Canonical original declaration, including its independent telescope.
    pub definition: Vec<u8>,
    pub kind: ClosedConformanceSchemaKind,
    pub lifetime_parameter_count: u32,
    pub parameter_kinds: Vec<u8>,
    /// Complete source-checked explicit roster, never observed call tuples.
    pub finite_tuples: Option<Vec<Vec<TerminalIdentityIndex>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceStaticApplication {
    pub public_requirement_identity: TerminalIdentityIndex,
    pub realization_identity: TerminalIdentityIndex,
    pub source_callable_identity: TerminalIdentityIndex,
    pub machine: MachineId,
    /// Checked method application, including its independently checked capture
    /// commitment when this invocation's template was capture-applied.
    pub application_commitment: Option<[u8; 32]>,
    /// The independently retained method tuple, excluding map captures.
    pub method_arguments: Vec<crate::ClosedReachArgument>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplicationCommitment([u8; 32]);

impl ClosedConformanceApplicationCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceParameterKind {
    Lifetime,
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceParameterBinding {
    pub parameter: String,
    pub kind: ClosedConformanceParameterKind,
    pub argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRow {
    pub declaring_trait_identity: TerminalIdentityIndex,
    /// Canonical normalized overload identity of the public requirement.
    pub public_requirement_identity: TerminalIdentityIndex,
    /// Canonical value tuple of the finite generic method family row this
    /// table row supplies: one canonical const identity per requirement value
    /// binder, in the requirement's binder declaration order, the same
    /// strings a typed `MachineSpecialization` retains in
    /// `const_argument_identities`. Together with the two identities above it
    /// names the exact `(declaring trait, complete requirement overload,
    /// canonical value tuple)` row a dispatch rejoins. A nongeneric requirement carries
    /// the empty tuple; readable spellings never appear here.
    pub family_tuple: Vec<crate::TerminalIdentityIndex>,
    /// Declaration path retained separately for exact row-map replay.
    pub requirement_identity: TerminalIdentityIndex,
    pub realization_identity: TerminalIdentityIndex,
    /// Reference into the owning application's standalone callable registry.
    /// Rows outside the bounded static named-witness lane remain map-free.
    pub realization_callable_identity: Option<TerminalIdentityIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRealizationCallable {
    /// Canonical checked callable identity, not a declaration display path.
    pub source_callable_identity: TerminalIdentityIndex,
    /// Artifact-local Terminal machine emitted for that exact callable.
    pub machine: MachineId,
    /// Source-derived matched requirement/realization result class for the
    /// bounded static requirement cohort.
    /// Callable identity intentionally excludes return type, so this separately
    /// committed value prevents coordinated scalar-result retargeting.
    pub result: ClosedConformanceCallableResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceCallableResult {
    Unit,
    Scalar(semantic_vocabulary::ScalarType),
    Structural(semantic_vocabulary::StructuralTypeId),
}

pub fn closed_conformance_application_report_fingerprint(
    identities: &TerminalIdentityTable,
    application: &ClosedConformanceApplication,
) -> u64 {
    fn push(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value.as_bytes());
    }

    let mut bytes = Vec::new();
    push(
        &mut bytes,
        identities
            .text(application.declaration_identity)
            .unwrap_or_default(),
    );
    push(
        &mut bytes,
        identities
            .text(application.trait_identity)
            .unwrap_or_default(),
    );
    bytes.extend((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_lifetime_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.telescope.len() as u64).to_le_bytes());
    for binding in &application.telescope {
        push(&mut bytes, &binding.parameter);
        bytes.push(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        push(&mut bytes, &binding.argument);
    }
    bytes.extend((application.trait_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.realization_callables.len() as u64).to_le_bytes());
    for callable in &application.realization_callables {
        push(
            &mut bytes,
            identities
                .text(callable.source_callable_identity)
                .unwrap_or_default(),
        );
        bytes.extend(callable.machine.get().to_le_bytes());
        bytes.extend(callable_result_bytes(callable.result));
    }
    bytes.extend(schema_application_bytes(identities, application));
    bytes.extend((application.rows.len() as u64).to_le_bytes());
    for row in &application.rows {
        push(
            &mut bytes,
            identities
                .text(row.declaring_trait_identity)
                .unwrap_or_default(),
        );
        push(
            &mut bytes,
            identities
                .text(row.public_requirement_identity)
                .unwrap_or_default(),
        );
        bytes.extend((row.family_tuple.len() as u64).to_le_bytes());
        for identity in &row.family_tuple {
            push(&mut bytes, identities.text(*identity).unwrap_or_default());
        }
        push(
            &mut bytes,
            identities
                .text(row.requirement_identity)
                .unwrap_or_default(),
        );
        push(
            &mut bytes,
            identities
                .text(row.realization_identity)
                .unwrap_or_default(),
        );
        bytes.push(u8::from(row.realization_callable_identity.is_some()));
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut bytes, identities.text(*identity).unwrap_or_default());
        }
    }
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(&bytes);
    hash.finish()
}

// Both report and authority commitments include the complete original schema
// and the separate exact static applications. Interned indices never hash as
// semantic identity; portable declaration identities supply their text.
fn callable_result_bytes(result: ClosedConformanceCallableResult) -> Vec<u8> {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};
    match result {
        ClosedConformanceCallableResult::Unit => vec![1],
        ClosedConformanceCallableResult::Structural(identity) => {
            let mut bytes = vec![3];
            bytes.extend(identity.get().to_le_bytes());
            bytes
        }
        ClosedConformanceCallableResult::Scalar(ScalarType::Boolean) => vec![2, 1],
        ClosedConformanceCallableResult::Scalar(ScalarType::Integer(integer)) => {
            let mut bytes = vec![
                2,
                2,
                match (integer.carrier(), integer.sign()) {
                    (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
                    (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
                    (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
                    (IntegerCarrier::Address, IntegerSign::Signed) => {
                        unreachable!("address carriers are unsigned")
                    }
                },
            ];
            bytes.extend(integer.bits().to_le_bytes());
            bytes
        }
        ClosedConformanceCallableResult::Scalar(ScalarType::IeeeFloat(format)) => vec![
            2,
            3,
            match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            },
        ],
    }
}

fn schema_application_bytes(
    identities: &TerminalIdentityTable,
    application: &ClosedConformanceApplication,
) -> Vec<u8> {
    fn text(
        bytes: &mut Vec<u8>,
        identities: &TerminalIdentityTable,
        identity: TerminalIdentityIndex,
    ) {
        let value = identities.text(identity).unwrap_or_default().as_bytes();
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value);
    }
    let mut bytes = Vec::new();
    bytes.extend((application.schemas.len() as u64).to_le_bytes());
    for schema in &application.schemas {
        for identity in [
            schema.declaring_trait_identity,
            schema.public_requirement_identity,
            schema.requirement_identity,
            schema.realization_identity,
        ] {
            text(&mut bytes, identities, identity);
        }
        bytes.push(match schema.kind {
            ClosedConformanceSchemaKind::Nongeneric => 1,
            ClosedConformanceSchemaKind::Finite => 2,
            ClosedConformanceSchemaKind::Open => 3,
        });
        bytes.extend((schema.definition.len() as u64).to_le_bytes());
        bytes.extend(&schema.definition);
        bytes.extend(schema.lifetime_parameter_count.to_le_bytes());
        bytes.extend((schema.parameter_kinds.len() as u64).to_le_bytes());
        bytes.extend(&schema.parameter_kinds);
        bytes.push(u8::from(schema.finite_tuples.is_some()));
        if let Some(tuples) = &schema.finite_tuples {
            bytes.extend((tuples.len() as u64).to_le_bytes());
            for tuple in tuples {
                bytes.extend((tuple.len() as u64).to_le_bytes());
                for member in tuple {
                    text(&mut bytes, identities, *member);
                }
            }
        }
    }
    bytes.extend((application.static_applications.len() as u64).to_le_bytes());
    for applied in &application.static_applications {
        for identity in [
            applied.public_requirement_identity,
            applied.realization_identity,
            applied.source_callable_identity,
        ] {
            text(&mut bytes, identities, identity);
        }
        bytes.extend(applied.machine.get().to_le_bytes());
        bytes.push(u8::from(applied.application_commitment.is_some()));
        if let Some(commitment) = applied.application_commitment {
            bytes.extend(commitment);
        }
        bytes.extend((applied.method_arguments.len() as u64).to_le_bytes());
        for argument in &applied.method_arguments {
            match argument {
                crate::ClosedReachArgument::Type(identity) => {
                    bytes.push(1);
                    text(&mut bytes, identities, *identity);
                }
                crate::ClosedReachArgument::Const(identity) => {
                    bytes.push(2);
                    text(&mut bytes, identities, *identity);
                }
                crate::ClosedReachArgument::Proposition(identity) => {
                    bytes.push(3);
                    text(&mut bytes, identities, *identity);
                }
                crate::ClosedReachArgument::Machine {
                    identity,
                    contract_commitment,
                } => {
                    bytes.push(4);
                    text(&mut bytes, identities, *identity);
                    bytes.extend(contract_commitment);
                }
            }
        }
    }
    bytes
}

/// Authority-bearing identity for a closed conformance application.
///
/// The owner is deliberately outside this commitment: ownership is an exact
/// independent join, while this value commits to the reusable semantic
/// application structure itself.
pub fn closed_conformance_application_commitment(
    identities: &TerminalIdentityTable,
    application: &ClosedConformanceApplication,
) -> ClosedConformanceApplicationCommitment {
    fn push(digest: &mut crate::digest_encoder::DomainDigestEncoder, value: &str) {
        digest.u64(value.len() as u64);
        digest.raw(value.as_bytes());
    }

    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.psi.terminal.closed-conformance-application.v6\0",
    );
    push(
        &mut digest,
        identities
            .text(application.declaration_identity)
            .unwrap_or_default(),
    );
    push(
        &mut digest,
        identities
            .text(application.trait_identity)
            .unwrap_or_default(),
    );
    digest.u64(application.trait_lifetime_arguments.len() as u64);
    for argument in &application.trait_lifetime_arguments {
        push(&mut digest, argument);
    }
    digest.u64(application.telescope.len() as u64);
    for binding in &application.telescope {
        push(&mut digest, &binding.parameter);
        digest.byte(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        push(&mut digest, &binding.argument);
    }
    digest.u64(application.trait_arguments.len() as u64);
    for argument in &application.trait_arguments {
        push(&mut digest, argument);
    }
    digest.u64(application.realization_callables.len() as u64);
    for callable in &application.realization_callables {
        push(
            &mut digest,
            identities
                .text(callable.source_callable_identity)
                .unwrap_or_default(),
        );
        digest.raw(&callable.machine.get().to_le_bytes());
        digest.raw(&callable_result_bytes(callable.result));
    }
    digest.raw(&schema_application_bytes(identities, application));
    digest.u64(application.rows.len() as u64);
    for row in &application.rows {
        push(
            &mut digest,
            identities
                .text(row.declaring_trait_identity)
                .unwrap_or_default(),
        );
        push(
            &mut digest,
            identities
                .text(row.public_requirement_identity)
                .unwrap_or_default(),
        );
        digest.u64(row.family_tuple.len() as u64);
        for identity in &row.family_tuple {
            push(&mut digest, identities.text(*identity).unwrap_or_default());
        }
        push(
            &mut digest,
            identities
                .text(row.requirement_identity)
                .unwrap_or_default(),
        );
        push(
            &mut digest,
            identities
                .text(row.realization_identity)
                .unwrap_or_default(),
        );
        digest.byte(u8::from(row.realization_callable_identity.is_some()));
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut digest, identities.text(*identity).unwrap_or_default());
        }
    }
    ClosedConformanceApplicationCommitment::from_digest(digest.finish())
}

#[cfg(test)]
mod tests {
    use super::{
        ClosedConformanceApplication, ClosedConformanceRow,
        closed_conformance_application_commitment,
        closed_conformance_application_report_fingerprint,
    };

    fn application(
        identities: &mut crate::TerminalIdentityTableBuilder,
        family_tuple: Vec<crate::TerminalIdentityIndex>,
    ) -> ClosedConformanceApplication {
        ClosedConformanceApplication {
            owner: semantic_vocabulary::MachineId::new(1).unwrap(),
            declaration_identity: identities.intern("package::CarrierScanner"),
            telescope: Vec::new(),
            trait_identity: identities.intern("package::Scanner"),
            trait_lifetime_arguments: Vec::new(),
            trait_arguments: vec!["package::Carrier".into()],
            realization_callables: Vec::new(),
            schemas: Vec::new(),
            static_applications: Vec::new(),
            rows: vec![ClosedConformanceRow {
                declaring_trait_identity: identities.intern("package::Scanner"),
                public_requirement_identity: identities.intern("package::Scanner::scan()"),
                family_tuple,
                requirement_identity: identities.intern("package::Scanner::scan"),
                realization_identity: identities.intern("package::Carrier::scan"),
                realization_callable_identity: None,
            }],
            report_fingerprint: 0,
            commitment: Default::default(),
        }
    }

    fn width_lanes_tuple(
        identities: &mut crate::TerminalIdentityTableBuilder,
    ) -> Vec<crate::TerminalIdentityIndex> {
        vec![
            identities.intern("named(integer-const(16))"),
            identities.intern("named(integer-const(4))"),
        ]
    }

    /// The tuple is a row coordinate: rows equal in every identity but the
    /// tuple commit differently, and tuple order is not normalized away.
    #[test]
    fn family_tuple_enters_row_commitment_and_report_fingerprint() {
        let mut builder = crate::TerminalIdentityTableBuilder::default();
        let nongeneric = application(&mut builder, Vec::new());
        let tuple = width_lanes_tuple(&mut builder);
        let width_lanes = application(&mut builder, tuple);
        let mut reversed = width_lanes_tuple(&mut builder);
        reversed.reverse();
        let lanes_width = application(&mut builder, reversed);
        let identities = builder.snapshot();

        let commitments = [
            closed_conformance_application_commitment(&identities, &nongeneric),
            closed_conformance_application_commitment(&identities, &width_lanes),
            closed_conformance_application_commitment(&identities, &lanes_width),
        ];
        let fingerprints = [
            closed_conformance_application_report_fingerprint(&identities, &nongeneric),
            closed_conformance_application_report_fingerprint(&identities, &width_lanes),
            closed_conformance_application_report_fingerprint(&identities, &lanes_width),
        ];
        assert!(commitments.iter().all(|commitment| !commitment.is_zero()));
        assert_ne!(commitments[0], commitments[1]);
        assert_ne!(commitments[1], commitments[2]);
        assert_ne!(commitments[0], commitments[2]);
        assert_ne!(fingerprints[0], fingerprints[1]);
        assert_ne!(fingerprints[1], fingerprints[2]);
        assert_ne!(fingerprints[0], fingerprints[2]);
        assert_eq!(
            closed_conformance_application_commitment(&identities, &{
                let tuple = width_lanes_tuple(&mut builder);
                application(&mut builder, tuple)
            }),
            commitments[1]
        );
    }
}
