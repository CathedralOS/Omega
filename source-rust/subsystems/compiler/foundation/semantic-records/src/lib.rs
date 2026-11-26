//! Leaf definitions shared by source forms, canonical Psi and independent
//! checking: typed coordinates, scalar and proposition records, proof schemas,
//! storage places and boundary contracts. Record codecs bind their exact tags.
//!
//! This owner has no executable program graph, source lookup, target producer,
//! or runtime authority. Generic ID tables and sets belong to arena; hashing and
//! byte encoding belong to wire. Consumers carry established values forward.

#![forbid(unsafe_code)]

mod declaration_contracts;
mod clause_identity;
pub use clause_identity::{ClauseCarrier, ClauseDeclaration, ClauseIdentity, ClauseOperand, ClauseValue};
pub use self::declaration_contracts::{
    BlockingInterface, BlockingPlan, BlockingSummary, ExternalBindingId, ExternalBindingIdentity,
    ExternalBindingMechanism, RANKING_VIEW_CORE_SOURCE, RankingViewDeclaration, RankingViewId,
    SemanticDomainId, ServiceReachId, ServiceReachInterface, ServiceReachPlan, ServiceReachRowId,
    ServiceReachSummary, SuspensionInterface, SuspensionPlan, SuspensionSummary,
    SynchronousInvocationInterface, SynchronousInvocationPlan,
};
mod declaration_descriptions;
pub use declaration_descriptions::{FieldInfoDescription, ParameterDescription};
pub mod dependency_reach;
pub mod provider_identity;

pub mod identity_encoding;
pub mod structural_path_encoding;

// Call obligations and runtime lowering use the same two declaration
// namespaces. Keeping this existing target identity in the shared vocabulary
// lets check tables name a boundary without depending on the artifact crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetMachineCallable {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
}

/// A known mismatch between two already-resolved callable signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableShapeMismatch {
    GenericArity { required: usize, actual: usize },
    ParameterArity { required: usize, actual: usize },
    ParameterMode { position: usize },
    ParameterType { position: usize },
    ReturnType,
}

impl From<MachineId> for TargetMachineCallable {
    fn from(machine: MachineId) -> Self {
        Self::Machine(machine)
    }
}

mod source_review;
pub mod source_selections;
pub use source_review::{IntegerBinaryKind, SemanticDependencyKind};

mod float_integer_conversions;
pub use float_integer_conversions::{
    FloatIntegerConversion, FloatIntegerConversionError, fixed_integer_type,
    float_integer_failure_condition, float_integer_preconditions,
};

mod float_meaning_clauses;
pub use float_meaning_clauses::{
    FloatMeaningClauseOperand, FloatMeaningClauseValue, FloatMeaningSlot,
};

pub(crate) use ::wire::codec_bytes;

mod behavior_policy {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CrashCause {
        Trap,
        Abort,
    }

    macro_rules! tagged_vocabulary {
        ($documentation:literal, $name:ident, $tag_method:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
            #[doc = $documentation]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub enum $name {
                $($variant,)+
            }

            impl $name {
                pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

                pub const CASE_NAMES: [&'static str; Self::ALL.len()] = [$(stringify!($variant)),+];

                pub const fn $tag_method(self) -> u8 {
                    match self {
                        $(Self::$variant => $tag,)+
                    }
                }
            }
        };
    }

    tagged_vocabulary!(
        "D45's closed physical terminal-authority vocabulary. Declaration order is the canonical encoded order; additions require a target-policy migration.",
        TerminalAuthorityClass,
        canonical_tag
        {
            FilesystemContentRead = 0,
            FilesystemContentWrite = 1,
            FilesystemMetadataQuery = 2,
            DirectoryEnumeration = 3,
            FilesystemNamespaceMutation = 4,
            FilesystemMetadataMutation = 5,
            ProcessOutput = 6,
            ProcessTermination = 7,
            MachineControl = 8,
            PortIo = 9,
            InterruptControl = 10,
            InterruptEntry = 11,
            RootMemoryAccess = 12,
            ProcessInput = 13,
        }
    );

    impl record_storage::Record for TerminalAuthorityClass {
        fn write(
            &self,
            output: &mut record_storage::RecordWriter,
        ) -> Result<(), record_storage::RecordError> {
            record_storage::Record::write(&self.canonical_tag(), output)
        }
        fn read<Context>(
            input: &mut record_storage::RecordReader<'_, Context>,
        ) -> Result<Self, record_storage::RecordError> {
            let tag = <u8 as record_storage::Record>::read(input)?;
            Self::ALL
                .into_iter()
                .find(|class| class.canonical_tag() == tag)
                .ok_or_else(|| input.error("unknown retained terminal authority class"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::TerminalAuthorityClass;
        use record_storage::{RecordLimits, decode, encode};

        #[test]
        fn physical_policy_tags_preserve_retained_encoding() {
            use TerminalAuthorityClass::*;
            let expected = [
                FilesystemContentRead,
                FilesystemContentWrite,
                FilesystemMetadataQuery,
                DirectoryEnumeration,
                FilesystemNamespaceMutation,
                FilesystemMetadataMutation,
                ProcessOutput,
                ProcessTermination,
                MachineControl,
                PortIo,
                InterruptControl,
                InterruptEntry,
                RootMemoryAccess,
                ProcessInput,
            ];
            assert_eq!(TerminalAuthorityClass::ALL, expected);
            for (tag, class) in expected.into_iter().enumerate() {
                let bytes = [u8::try_from(tag).expect("14 canonical tags")];
                assert_eq!(class.canonical_tag(), bytes[0]);
                assert_eq!(encode(&class, RecordLimits::default()).unwrap(), bytes);
                assert_eq!(
                    decode::<TerminalAuthorityClass>(&bytes, RecordLimits::default()).unwrap(),
                    class,
                );
            }
            for invalid_tag in [14, 255] {
                assert!(
                    decode::<TerminalAuthorityClass>(&[invalid_tag], RecordLimits::default(),)
                        .is_err()
                );
            }
        }
    }
}
mod boundary {
    pub(crate) mod callback_identities {
        macro_rules! nominal_declaration_id {
            ($name:ident) => {
                #[derive(
                    Debug,
                    Clone,
                    Copy,
                    PartialEq,
                    Eq,
                    PartialOrd,
                    Ord,
                    Hash,
                    record_storage::Record,
                )]
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
            let mut hasher = wire::fnv1a::Fnv1a::new();
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

        pub fn nominal_callback_binder_id(
            owner: &str,
            ordinal: u32,
            name: &str,
        ) -> StaticMachineBinderId {
            StaticMachineBinderId(callback_nominal_report_fingerprint(
                b"omega.callback-binder.v1",
                &[owner.as_bytes(), &ordinal.to_le_bytes(), name.as_bytes()],
            ))
        }
    }

    pub(crate) mod foreign_locator {
        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum ForeignLocatorCandidate {
            PeByName {
                library: Vec<u8>,
                export: Vec<u8>,
            },
            PeByOrdinal {
                library: Vec<u8>,
                ordinal: u16,
            },
            ElfVersioned {
                object: Vec<u8>,
                symbol: Vec<u8>,
                version: Vec<u8>,
            },
            MachODylibSymbol {
                install_name: Vec<u8>,
                symbol: Vec<u8>,
            },
        }

        impl ForeignLocatorCandidate {
            pub const fn case_name(&self) -> &'static str {
                match self {
                    Self::PeByName { .. } => "PeByName",
                    Self::PeByOrdinal { .. } => "PeByOrdinal",
                    Self::ElfVersioned { .. } => "ElfVersioned",
                    Self::MachODylibSymbol { .. } => "MachODylibSymbol",
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum NativeBindingLocator {
            Import(ForeignLocatorCandidate),
            Syscall { number: u64 },
        }
    }

    pub(crate) mod program_entry {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProgramEntrySchema {
            HostedApplication,
            ProgramStorageApplication,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProgramStorageEntryRootRole {
            Image,
            InitialStorage,
        }

        /// One source-visible argument supplied by the installed entry. The
        /// ordinal identifies its formal on the semantic arrival requirement.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ProgramEntryArgument {
            pub requirement_parameter: usize,
            pub role: ProgramStorageEntryRootRole,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ProgramEntrySourceContract {
            pub owner: &'static str,
            pub slot_name: &'static str,
            pub schema: ProgramEntrySchema,
            pub semantic_arrival_requirement: &'static str,
            pub visible_parameters: &'static [ProgramEntryArgument],
        }

        pub trait ProgramEntrySourceCatalog: Sync {
            fn source_contract(&self, slot: &str) -> Result<ProgramEntrySourceContract, String>;
        }
    }
}
mod scalar_condition_role {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum ScalarConditionRole {
        Prerequisite,
        Result,
    }
}
mod scalars {
    pub(crate) mod bounded_integer_type {
        use crate::{IntegerCarrier, IntegerType, IntegerValue, PropositionError};

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
        )]
        pub struct BoundedIntegerType {
            integer_type: IntegerType,
            minimum: IntegerValue,
            maximum: IntegerValue,
        }

        impl BoundedIntegerType {
            pub fn new(
                integer_type: IntegerType,
                minimum: IntegerValue,
                maximum: IntegerValue,
            ) -> Result<Self, PropositionError> {
                if integer_type.carrier() != IntegerCarrier::Fixed
                    || !integer_type.admits(minimum)
                    || !integer_type.admits(maximum)
                    || minimum > maximum
                {
                    return Err(PropositionError::InvalidIntegerRange {
                        integer_type,
                        minimum,
                        maximum,
                    });
                }
                Ok(Self {
                    integer_type,
                    minimum,
                    maximum,
                })
            }

            pub const fn integer_type(self) -> IntegerType {
                self.integer_type
            }
            pub const fn minimum(self) -> IntegerValue {
                self.minimum
            }
            pub const fn maximum(self) -> IntegerValue {
                self.maximum
            }

            pub fn contains(self, value: IntegerValue) -> bool {
                self.integer_type.admits(value) && self.minimum <= value && value <= self.maximum
            }
        }

        #[cfg(test)]
        mod tests {
            use super::{BoundedIntegerType, IntegerType, IntegerValue};
            use crate::IntegerSign;

            #[test]
            fn bounded_integer_type_rejects_invalid_endpoints_and_address_carriers() {
                let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
                for (minimum, maximum) in [
                    (IntegerValue::Signed(1), IntegerValue::Signed(0)),
                    (IntegerValue::Signed(-129), IntegerValue::Signed(0)),
                    (IntegerValue::Signed(0), IntegerValue::Signed(128)),
                    (IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
                ] {
                    assert!(BoundedIntegerType::new(integer, minimum, maximum).is_err());
                }
                assert!(
                    BoundedIntegerType::new(
                        IntegerType::address(64).unwrap(),
                        IntegerValue::Unsigned(0),
                        IntegerValue::Unsigned(1)
                    )
                    .is_err()
                );
                let singleton = BoundedIntegerType::new(
                    integer,
                    IntegerValue::Signed(-128),
                    IntegerValue::Signed(-128),
                )
                .unwrap();
                assert!(singleton.contains(IntegerValue::Signed(-128)));
                assert!(!singleton.contains(IntegerValue::Signed(-127)));
            }
        }
    }

    pub(crate) mod ieee_float_comparison_operation {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
        )]
        pub enum IeeeFloatComparisonOperation {
            Equal,
            NotEqual,
            Less,
            LessOrEqual,
            Greater,
            GreaterOrEqual,
        }

        impl IeeeFloatComparisonOperation {
            pub const ALL: [Self; 6] = [
                Self::Equal,
                Self::NotEqual,
                Self::Less,
                Self::LessOrEqual,
                Self::Greater,
                Self::GreaterOrEqual,
            ];

            pub const fn ordinal(self) -> u8 {
                match self {
                    Self::Equal => 0,
                    Self::NotEqual => 1,
                    Self::Less => 2,
                    Self::LessOrEqual => 3,
                    Self::Greater => 4,
                    Self::GreaterOrEqual => 5,
                }
            }
        }
    }

    pub(crate) mod qualified_scalar_type {
        use crate::{ScalarQualificationSetId, ScalarType};

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
        )]
        pub struct QualifiedScalarType {
            pub scalar_type: ScalarType,
            pub qualifications: ScalarQualificationSetId,
        }

        impl From<ScalarType> for QualifiedScalarType {
            fn from(scalar_type: ScalarType) -> Self {
                Self {
                    scalar_type,
                    qualifications: ScalarQualificationSetId::ZERO,
                }
            }
        }
    }

    pub(crate) mod scalar_refinement {
        use crate::{Proposition, PropositionContext, PropositionError, ScalarType, ValueId};

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
        pub struct ScalarRefinement {
            carrier: ScalarType,
            predicate: Proposition,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ScalarRefinementError {
            NotClosed,
            InvalidPredicate(PropositionError),
        }

        impl ScalarRefinement {
            pub const SUBJECT: ValueId = ValueId::FIRST;

            pub fn new(
                carrier: ScalarType,
                predicate: Proposition,
            ) -> Result<Self, ScalarRefinementError> {
                let mut remaining = 4096;
                let mut foreign_value = false;
                if predicate.visit_value_ids_bounded(&mut remaining, |value| {
                    foreign_value |= value != Self::SUBJECT;
                }) != Some(true)
                    || foreign_value
                {
                    return Err(ScalarRefinementError::NotClosed);
                }
                let context = PropositionContext::from_value_types([(Self::SUBJECT, carrier)])
                    .map_err(ScalarRefinementError::InvalidPredicate)?;
                context
                    .validate(&predicate)
                    .map_err(ScalarRefinementError::InvalidPredicate)?;
                Ok(Self { carrier, predicate })
            }

            pub fn carrier(&self) -> ScalarType {
                self.carrier
            }

            pub fn predicate(&self) -> &Proposition {
                &self.predicate
            }

            pub fn instantiate(&self, value: ValueId) -> Proposition {
                let mut predicate = self.predicate.clone();
                predicate.remap_value_ids(|_| value);
                predicate
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::{
                IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarTerm,
            };

            #[test]
            fn compound_mathematical_predicates_keep_their_operators_and_literals() {
                let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
                let subject = IntegerMathTerm::MathValue {
                    source_type: integer,
                    value: ScalarRefinement::SUBJECT,
                };
                let predicate = Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Multiply(Box::new(subject.clone()), Box::new(subject)),
                    IntegerMathTerm::literal(IntegerValue::Signed(100)),
                );
                let refinement =
                    ScalarRefinement::new(ScalarType::Integer(integer), predicate.clone()).unwrap();
                let value = ValueId::new(7).unwrap();
                let instantiated = refinement.instantiate(value);
                let mut expected = predicate;
                expected.remap_value_ids(|_| value);
                assert_eq!(instantiated, expected);
                let Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Multiply(left, right),
                    literal,
                ) = instantiated
                else {
                    panic!("substitution preserves mathematical multiplication and comparison");
                };
                assert_eq!(
                    *left,
                    IntegerMathTerm::MathValue {
                        source_type: integer,
                        value,
                    },
                );
                assert_eq!(left, right);
                assert_eq!(literal, IntegerMathTerm::literal(IntegerValue::Signed(100)));
            }

            #[test]
            fn nonzero_is_an_ordinary_disjunction_over_the_exact_snapshot() {
                let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
                let carrier = ScalarType::Integer(integer);
                let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
                let zero = ScalarTerm::integer(integer, crate::IntegerValue::Signed(0)).unwrap();
                let refinement = ScalarRefinement::new(
                    carrier,
                    Proposition::Disjunction(vec![
                        Proposition::LessThan(subject.clone(), zero.clone()),
                        Proposition::LessThan(zero, subject),
                    ]),
                )
                .unwrap();
                let value = ValueId::new(17).unwrap();
                let instantiated = refinement.instantiate(value);
                let mut values = Vec::new();
                assert!(instantiated.visit_value_ids(|value| values.push(value)));
                assert_eq!(values, vec![value, value]);
                PropositionContext::from_value_types([(value, carrier)])
                    .unwrap()
                    .validate(&instantiated)
                    .unwrap();
            }

            #[test]
            fn foreign_values_structural_observations_and_opaque_assumptions_reject() {
                for predicate in [
                    Proposition::Equal(
                        ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
                        ScalarTerm::boolean(true),
                    ),
                    Proposition::Equal(
                        ScalarTerm::boolean_field_path(PlaceId::new(1).unwrap(), Vec::new()),
                        ScalarTerm::boolean(true),
                    ),
                    Proposition::Atom(crate::PropositionId::new(1).unwrap()),
                ] {
                    assert_eq!(
                        ScalarRefinement::new(ScalarType::Boolean, predicate),
                        Err(ScalarRefinementError::NotClosed),
                    );
                }
            }

            #[test]
            fn a_subject_cannot_be_reinterpreted_at_another_carrier() {
                let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
                let subject =
                    ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Integer(integer));
                assert!(matches!(
                    ScalarRefinement::new(
                        ScalarType::Boolean,
                        Proposition::Equal(subject.clone(), subject)
                    ),
                    Err(ScalarRefinementError::InvalidPredicate(_)),
                ));
            }
        }
    }
}
pub use scalar_condition_role::ScalarConditionRole;
mod storage;
pub use behavior_policy::{CrashCause, TerminalAuthorityClass};
mod terminal_authority;
pub use terminal_authority::{PortableFilesystemAuthorityFacet, TerminalAuthorityDisposition};
pub use boundary::callback_identities::{
    BoundaryNativeParameterOrigin, CallbackRequirementId, NativeParameterId, StaticMachineBinderId,
    callback_nominal_report_fingerprint, callback_requirement_id, nominal_callback_binder_id,
    nominal_callback_native_parameter_id,
};
pub mod byte_predicates {
    pub use language_core::byte_predicates::ByteSequencePredicate;

    #[derive(
        Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
    )]
    pub struct ByteSequencePredicateSet(u8);

    impl ByteSequencePredicateSet {
        pub const EMPTY: Self = Self(0);

        pub fn from_bits(bits: u8) -> Option<Self> {
            (bits & !0x0f == 0).then_some(Self(bits))
        }

        pub const fn bits(self) -> u8 {
            self.0
        }
        pub const fn is_empty(self) -> bool {
            self.0 == 0
        }

        pub fn insert(&mut self, predicate: ByteSequencePredicate) {
            self.0 |= predicate.mask_bit();
        }

        pub fn predicates(self) -> impl Iterator<Item = ByteSequencePredicate> {
            ByteSequencePredicate::in_mask(self.0)
        }

        pub fn holds_for(self, bytes: &[u8]) -> bool {
            self.predicates()
                .all(|predicate| predicate.holds_for(bytes))
        }

        pub fn holding(bytes: &[u8]) -> Self {
            let mut result = Self::EMPTY;
            for predicate in ByteSequencePredicate::ALL {
                if predicate.holds_for(bytes) {
                    result.insert(predicate);
                }
            }
            result
        }

        pub fn implies(self, required: Self) -> bool {
            required
                .predicates()
                .all(|target| self.predicates().any(|source| source.implies(target)))
        }

        pub fn subslice(self) -> Self {
            let mut result = Self::EMPTY;
            for predicate in self.predicates() {
                if predicate.is_subslice_preserving() {
                    result.insert(predicate);
                }
            }
            result
        }

        pub fn concatenation(self, right: Self) -> Self {
            let mut result = Self::EMPTY;
            for predicate in ByteSequencePredicate::ALL {
                let mut required = Self::EMPTY;
                required.insert(predicate);
                let established = match predicate {
                    ByteSequencePredicate::NonEmpty => {
                        self.implies(required) || right.implies(required)
                    }
                    _ => {
                        predicate.is_concat_preserving()
                            && self.implies(required)
                            && right.implies(required)
                    }
                };
                if established {
                    result.insert(predicate);
                }
            }
            result
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn concatenation_predicates_hold_for_the_completed_bytes() {
            let samples = [b"".as_slice(), b"alpha", b"\0", &[0xff], "é".as_bytes()];
            let predicates_for = |bytes: &[u8]| {
                let mut predicates = ByteSequencePredicateSet::EMPTY;
                for predicate in ByteSequencePredicate::ALL {
                    if predicate.holds_for(bytes) {
                        predicates.insert(predicate);
                    }
                }
                predicates
            };
            for left in samples {
                for right in samples {
                    let completed = [left, right].concat();
                    assert!(
                        predicates_for(left)
                            .concatenation(predicates_for(right))
                            .holds_for(&completed)
                    );
                }
            }
        }

        #[test]
        fn concatenation_preserves_nonempty_from_either_operand() {
            let mut nonempty = ByteSequencePredicateSet::EMPTY;
            nonempty.insert(ByteSequencePredicate::NonEmpty);
            for (left, right, expected) in [
                (nonempty, ByteSequencePredicateSet::EMPTY, true),
                (ByteSequencePredicateSet::EMPTY, nonempty, true),
                (nonempty, nonempty, true),
                (
                    ByteSequencePredicateSet::EMPTY,
                    ByteSequencePredicateSet::EMPTY,
                    false,
                ),
            ] {
                assert_eq!(left.concatenation(right).implies(nonempty), expected);
            }
            for predicate in [
                ByteSequencePredicate::ValidUtf8,
                ByteSequencePredicate::NoNul,
                ByteSequencePredicate::AsciiOnly,
            ] {
                let mut required = ByteSequencePredicateSet::EMPTY;
                required.insert(predicate);
                assert!(!required.concatenation(nonempty).implies(required));
                assert!(!nonempty.concatenation(required).implies(required));
            }
        }

        #[test]
        fn subslice_predicates_hold_for_every_window_including_empty_and_split_utf8() {
            for bytes in [b"A\0Z".as_slice(), b"alpha", "é".as_bytes(), b"", &[0xff]] {
                let mut source = ByteSequencePredicateSet::EMPTY;
                for predicate in ByteSequencePredicate::ALL {
                    if predicate.holds_for(bytes) {
                        source.insert(predicate);
                    }
                }
                let result = source.subslice();
                assert!(!result.predicates().any(|predicate| matches!(
                    predicate,
                    ByteSequencePredicate::ValidUtf8 | ByteSequencePredicate::NonEmpty
                )));
                for start in 0..=bytes.len() {
                    for end in start..=bytes.len() {
                        assert!(result.holds_for(&bytes[start..end]));
                    }
                }
                for predicate in [
                    ByteSequencePredicate::NoNul,
                    ByteSequencePredicate::AsciiOnly,
                ] {
                    assert_eq!(
                        result.predicates().any(|candidate| candidate == predicate),
                        source.predicates().any(|candidate| candidate == predicate)
                    );
                }
            }
        }
    }
}
pub mod compiler_evaluation {
    use crate::PackageKeyIdentity;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum DeclarationCustody {
        Package(PackageKeyIdentity),
        Toolchain,
        UnownedUser,
        Missing,
        // Issued contract closure, never an ambient package-selection exemption.
        ToolchainContract,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct CompilerEvaluationEffects {
        pub services: Vec<String>,
        pub may_suspend: bool,
        pub may_block: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct CompilerEvaluationDeclaration {
        pub name: String,
        pub custody: DeclarationCustody,
        pub effects: CompilerEvaluationEffects,
        pub local_terminates: bool,
        pub entry_requires: bool,
        pub linear_carrier: Option<String>,
        pub invocation_refusal: Option<String>,
        pub selections: Result<Vec<DeclarationAuthorityRequest>, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct DeclarationAuthorityRequest {
        pub requester: crate::PackageKeyIdentity,
        pub owner: crate::PackageKeyIdentity,
    }

    /// Decide whether a declaration selection needs a package edge. The
    /// caller still checks any returned request against its actual authority;
    /// neither a source origin nor a recorded request proves that edge.
    pub fn declaration_authority_request(
        requester: DeclarationCustody,
        owner: DeclarationCustody,
    ) -> Result<Option<DeclarationAuthorityRequest>, &'static str> {
        match (requester, owner) {
            (DeclarationCustody::Missing, _) | (_, DeclarationCustody::Missing) => {
                Err("build-time invocation lacks compiler-owned source/package provenance")
            }
            (DeclarationCustody::UnownedUser, _) | (_, DeclarationCustody::UnownedUser) => {
                Err("build-time invocation has user source without reconciled package custody")
            }
            (DeclarationCustody::ToolchainContract, DeclarationCustody::Package(_))
            | (DeclarationCustody::Package(_), DeclarationCustody::ToolchainContract) => {
                Err("build-time invocation crosses closed-contract/package custody")
            }
            (DeclarationCustody::ToolchainContract, DeclarationCustody::ToolchainContract)
            | (DeclarationCustody::Toolchain, _)
            | (_, DeclarationCustody::Toolchain) => Ok(None),
            (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner)) => {
                Ok(Some(DeclarationAuthorityRequest { requester, owner }))
            }
        }
    }
}
mod closed_integers;
mod identity;
mod proposition;
pub use boundary::program_entry::{
    ProgramEntryArgument, ProgramEntrySchema, ProgramEntrySourceCatalog,
    ProgramEntrySourceContract, ProgramStorageEntryRootRole,
};
pub use closed_integers::{ClosedIntegerType, ClosedScalarType, IntegerWidthClosureError};
pub use storage::atomic_cell::AtomicCellType;
pub use storage::target_carrier_layout::TargetCarrierLayout;

pub use scalars::ieee_float_comparison_operation::IeeeFloatComparisonOperation;
pub use scalars::qualified_scalar_type::QualifiedScalarType;
pub use scalars::scalar_refinement::{ScalarRefinement, ScalarRefinementError};
pub use storage::reference_origins::ReferenceOriginExtent;

pub use byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryCompletion, BoundaryMachineId, BoundaryRequirementKind,
    CallableContractId, CallableInvocationSite, CallingAbiIdentity, CallingDeclarationOwner,
    CallingGenericFormal, CallingOperandKind, CallingSignatureId, CallingStaticOperand, ClaimId,
    ContentDomainId, ContractId, CycleComponentId, DomainSemanticId, EdgeId, EvidenceIdentity,
    EvidenceTermId, ExternalCompletionKind, InvocationSite, MachineDescriptorId, MachineId,
    ObligationId, OperationId, PackageKeyIdentity, PlaceId, ProfileDecisionId, PropositionId,
    RankingRelationId, RecursiveComponentId, ScalarApplicationId, ScalarDomainId,
    ScalarQualificationSetId, SelectionDeclarationId, SelectionQuestionId, ServiceId,
    StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    SuspensionCrossingId, ValueId, WorkScheduleIdentity,
};
pub use proposition::{
    CanonicalStructuralPathSegment, IeeeFloatComparisonKind, IeeeFloatFormat,
    IeeeFloatStructuralField, IeeeFloatValue, IntegerCarrier, IntegerMathLiteral, IntegerMathTerm,
    IntegerSign, IntegerType, IntegerValue, IntegerWidth, MathematicalArgument, ProofFormalScope,
    ProofTerm, ProofTermField, Proposition, PropositionContext, PropositionError,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralCaseSubject, WidthParameter,
};
pub use scalars::bounded_integer_type::BoundedIntegerType;
pub use storage::authored_storage::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, AuthoredRecordStorage,
};
pub use storage::content::ContentPlaceSegment as StructuralPlaceSegment;
pub use storage::content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, content_conservation_report_fingerprint,
    encode_fingerprint_string,
};
/// A content place orders case names before field names. Import this concrete
/// type and qualify its variants, for example `ContentPlaceSegment::Case`.
pub type ContentPlaceSegment = StructuralPlaceSegment<false>;
pub use storage::structural_places::{AffineConstructionElement, StructuralPlaceKind};

pub use boundary::foreign_locator::{ForeignLocatorCandidate, NativeBindingLocator};

/// The termination promise a callable publishes. A local body proof is kept
/// separately; absence of a promise never asserts that execution will finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    Terminates,
    NoGuarantee,
}

/// Whether a callable publishes its complete crash routes. An open static
/// requirement grants no closed runtime crash promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableCrashPolicy {
    Closed,
    Open,
}

/// The exact target's mathematical interpretation of address inputs. This
/// describes proof values; it supplies no native storage or access authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarAddressModel {
    pub target: String,
    pub carrier: ClosedIntegerType,
    pub semantics_version: u32,
    pub commitment: [u8; 32],
}

impl ScalarAddressModel {
    /// Whether the model fixes an address carrier, a semantics version and a
    /// commitment, so that an address input of the target has a mathematical
    /// meaning a proof can use.
    pub fn is_mathematical(&self) -> bool {
        self.carrier.is_address() && self.semantics_version != 0 && self.commitment != [0; 32]
    }
}
