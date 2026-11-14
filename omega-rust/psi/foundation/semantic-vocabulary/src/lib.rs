/*
We define the target-neutral semantic identities (`identity`), dense ID tables
(`id_table`), mathematical propositions and scalar terms (`proposition`,
`scalar_terms`), proof-application schemas (`proof_applications`), storage
places and content projections (`storage`), and boundary calling/locator
vocabulary shared across Checked and Terminal Psi.
*/

/*
Terminal Psi artifacts and independent stage validators must agree on the exact
identities of machines, blocks, edges, values, places, obligations, and
propositions without importing target-specific Omega representations. Keeping
dense semantic IDs, bounded integer intervals, scalar refinements, content
conservation algebras, and FNV-1a fingerprinting in one foundational vocabulary
crate guarantees that Psi verification and Omega lowering speak a single
canonical language.
*/

/*
`identity.rs` defines the `semantic_id!` family of dense typed IDs and `IdTable`
indexes flat slices by those IDs. `proposition.rs` and `scalar_terms.rs` own
integer, float, byte-sequence, and proof terms plus `Proposition` contexts;
`storage.rs` owns authored bit fragments, structural places, and content
projections; `proof_applications.rs` owns proof-application verification.
*/

#![forbid(unsafe_code)]

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
pub use source_review::{IntegerBinaryKind, SemanticDependencyKind};

pub(crate) use optimization::codec_bytes;

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

    impl compiler_storage::Record for TerminalAuthorityClass {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            compiler_storage::Record::write(&self.canonical_tag(), output)
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            let tag = <u8 as compiler_storage::Record>::read(input)?;
            Self::ALL
                .into_iter()
                .find(|class| class.canonical_tag() == tag)
                .ok_or_else(|| input.error("unknown retained terminal authority class"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::TerminalAuthorityClass;
        use compiler_storage::{RecordLimits, decode, encode};

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
                    compiler_storage::Record,
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
        #[derive(Debug, Clone, PartialEq, Eq)]
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

        #[derive(Debug, Clone, PartialEq, Eq)]
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
        pub enum ProgramEntryVisibleParameters {
            None,
            ImageAndInitialStorage,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ProgramEntrySourceContract {
            pub owner: &'static str,
            pub slot_name: &'static str,
            pub schema: ProgramEntrySchema,
            pub semantic_arrival_requirement: &'static str,
            pub visible_parameters: ProgramEntryVisibleParameters,
        }

        pub trait ProgramEntrySourceCatalog: Sync {
            fn source_contract(&self, slot: &str) -> Result<ProgramEntrySourceContract, String>;
        }
    }
}
mod scalar_condition_role {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ScalarConditionRole {
        Prerequisite,
        Result,
    }
}
mod scalars {
    pub(crate) mod bounded_integer_type {
        use crate::{IntegerCarrier, IntegerType, IntegerValue, PropositionError};

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
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
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
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
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
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

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
pub use boundary::callback_identities::{
    BoundaryNativeParameterOrigin, CallbackRequirementId, NativeParameterId, StaticMachineBinderId,
    callback_nominal_report_fingerprint, callback_requirement_id, nominal_callback_binder_id,
    nominal_callback_native_parameter_id,
};
pub mod byte_predicates {
    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ByteSequencePredicate {
        ValidUtf8,
        NoNul,
        AsciiOnly,
        NonEmpty,
    }

    impl ByteSequencePredicate {
        pub const ALL: [Self; 4] = [
            Self::ValidUtf8,
            Self::NoNul,
            Self::AsciiOnly,
            Self::NonEmpty,
        ];

        pub fn from_name(name: &str) -> Option<Self> {
            match name {
                "valid_utf8" => Some(Self::ValidUtf8),
                "no_nul" => Some(Self::NoNul),
                "ascii_only" => Some(Self::AsciiOnly),
                "non_empty" => Some(Self::NonEmpty),
                _ => None,
            }
        }

        pub fn holds_for(self, bytes: &[u8]) -> bool {
            match self {
                Self::ValidUtf8 => std::str::from_utf8(bytes).is_ok(),
                Self::NoNul => !bytes.contains(&0),
                Self::AsciiOnly => bytes.iter().all(|byte| *byte < 128),
                Self::NonEmpty => !bytes.is_empty(),
            }
        }

        pub fn is_concat_preserving(self) -> bool {
            match self {
                Self::ValidUtf8 | Self::NoNul | Self::AsciiOnly | Self::NonEmpty => true,
            }
        }

        pub fn is_subslice_preserving(self) -> bool {
            match self {
                Self::NoNul | Self::AsciiOnly => true,
                Self::ValidUtf8 | Self::NonEmpty => false,
            }
        }

        pub fn implies(self, other: Self) -> bool {
            self == other || (self == Self::AsciiOnly && other == Self::ValidUtf8)
        }

        pub fn mask_bit(self) -> u8 {
            match self {
                Self::ValidUtf8 => 1 << 0,
                Self::NoNul => 1 << 1,
                Self::AsciiOnly => 1 << 2,
                Self::NonEmpty => 1 << 3,
            }
        }

        pub fn in_mask(mask: u8) -> impl Iterator<Item = Self> {
            Self::ALL
                .into_iter()
                .filter(move |predicate| mask & predicate.mask_bit() != 0)
        }
    }

    #[derive(
        Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
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

    pub trait DeclarationSelectionAuthority: Send + Sync {
        fn allows_declaration_selection(
            &self,
            requester: PackageKeyIdentity,
            owner: PackageKeyIdentity,
        ) -> bool;

        fn package_label(&self, identity: PackageKeyIdentity) -> String;

        fn declaration_selection_scope(
            &self,
        ) -> Option<std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>>
        {
            None
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum DeclarationCustody {
        Package(PackageKeyIdentity),
        Toolchain,
        UnownedUser,
        Missing,
        // Issued contract closure, never an ambient package-selection exemption.
        ToolchainContract,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CompilerEvaluationEffects {
        pub services: Vec<String>,
        pub may_suspend: bool,
        pub may_block: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CompilerEvaluationDeclaration {
        pub name: String,
        pub custody: DeclarationCustody,
        pub effects: CompilerEvaluationEffects,
        pub local_terminates: bool,
        pub entry_requires: bool,
        pub linear_carrier: Option<String>,
        pub static_application: Option<String>,
        pub selections: Result<Vec<DeclarationAuthorityRequest>, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DeclarationAuthorityRequest {
        pub requester: crate::PackageKeyIdentity,
        pub owner: crate::PackageKeyIdentity,
    }

    pub fn require_compiler_evaluation_floor(
        machine: &str,
        effects: &CompilerEvaluationEffects,
        closure_violation: Option<&str>,
        selection_violation: Option<&str>,
    ) -> Result<(), String> {
        let mut violations = Vec::new();
        if !effects.services.is_empty() {
            violations.push(format!("service reach [{}]", effects.services.join(", ")));
        }
        if effects.may_suspend {
            violations.push("may suspend".to_owned());
        }
        if effects.may_block {
            violations.push("may block".to_owned());
        }
        violations.extend(closure_violation.map(str::to_owned));
        violations.extend(selection_violation.map(str::to_owned));
        if violations.is_empty() {
            return Ok(());
        }
        Err(format!(
            "machine `{machine}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
            violations.join("; ")
        ))
    }

    pub fn require_compiler_evaluation_failures(
        machine: &str,
        failures: Result<(), &str>,
    ) -> Result<(), String> {
        failures.map_err(|reason| format!(
            "machine `{machine}` is not build-time admissible: {reason}; constant evaluation requires proof that no crash route can occur before execution"
        ))
    }
}
pub mod fnv1a {
    #[derive(Clone, Copy)]
    pub struct Fnv1a(u64);

    impl Fnv1a {
        pub const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        pub const PRIME: u64 = 0x100000001b3;

        pub const fn new() -> Self {
            Self(Self::OFFSET_BASIS)
        }

        pub const fn resume(state: u64) -> Self {
            Self(state)
        }

        pub fn byte(&mut self, byte: u8) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }

        pub fn flag(&mut self, value: bool) {
            self.byte(u8::from(value));
        }

        pub fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.byte(*byte);
            }
        }

        pub fn u16(&mut self, value: u16) {
            self.bytes(&value.to_le_bytes());
        }

        pub fn u32(&mut self, value: u32) {
            self.bytes(&value.to_le_bytes());
        }

        pub fn u64(&mut self, value: u64) {
            self.bytes(&value.to_le_bytes());
        }

        pub fn u64_mix(&mut self, value: u64) {
            self.0 ^= value;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }

        pub fn bytes_terminated(&mut self, bytes: &[u8], terminator: u8) {
            self.bytes(bytes);
            self.byte(terminator);
        }

        pub fn bytes_framed(&mut self, bytes: &[u8]) {
            self.u64(bytes.len() as u64);
            self.bytes(bytes);
        }

        pub fn string_terminated(&mut self, value: &str) {
            self.bytes(value.as_bytes());
            self.byte(0);
        }

        pub fn string_framed(&mut self, value: &str) {
            self.u64(value.len() as u64);
            self.bytes(value.as_bytes());
        }

        pub fn usize(&mut self, value: usize) {
            self.u64(value as u64);
        }

        pub const fn finish(self) -> u64 {
            self.0
        }

        pub const fn finish_or(self, fallback: u64) -> u64 {
            if self.0 == 0 { fallback } else { self.0 }
        }
    }

    impl Default for Fnv1a {
        fn default() -> Self {
            Self::new()
        }
    }
}
mod id_table {
    use std::marker::PhantomData;

    use crate::PsiSemanticId;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct IdTable<Id, T> {
        first: u64,
        values: Vec<T>,
        identity: PhantomData<Id>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NonDenseIdentities {
        pub expected: u64,
        pub found: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IdTableBuildError {
        RepeatedOrUnordered { previous: u64, found: u64 },
        SpanOverflow,
        SpanLimit { span: u64, maximum: usize },
        AllocationFailed,
    }

    impl<Id: PsiSemanticId, T> IdTable<Id, T> {
        pub fn from_rows(
            rows: impl IntoIterator<Item = (Id, T)>,
        ) -> Result<Self, NonDenseIdentities> {
            let mut rows = rows.into_iter();
            let Some((first, value)) = rows.next() else {
                return Ok(Self::default());
            };
            let first = first.get();
            let mut values = Vec::with_capacity(rows.size_hint().0 + 1);
            values.push(value);
            for (identity, value) in rows {
                let expected = first.checked_add(values.len() as u64);
                if expected != Some(identity.get()) {
                    return Err(NonDenseIdentities {
                        expected: expected.unwrap_or(u64::MAX),
                        found: identity.get(),
                    });
                }
                values.push(value);
            }
            Ok(Self {
                first,
                values,
                identity: PhantomData,
            })
        }

        pub fn from_rows_with_gaps(
            rows: impl IntoIterator<Item = (Id, T)>,
            maximum_span: usize,
        ) -> Result<Self, IdTableBuildError>
        where
            T: Default,
        {
            let mut rows = rows.into_iter();
            let Some((first, value)) = rows.next() else {
                return Ok(Self::default());
            };
            let first = first.get();
            if maximum_span == 0 {
                return Err(IdTableBuildError::SpanLimit {
                    span: 1,
                    maximum: maximum_span,
                });
            }
            let mut values = Vec::new();
            values
                .try_reserve(1)
                .map_err(|_| IdTableBuildError::AllocationFailed)?;
            values.push(value);
            let mut previous = first;
            for (identity, value) in rows {
                let raw = identity.get();
                if raw <= previous {
                    return Err(IdTableBuildError::RepeatedOrUnordered {
                        previous,
                        found: raw,
                    });
                }
                let span = raw
                    .checked_sub(first)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or(IdTableBuildError::SpanOverflow)?;
                if span > maximum_span as u64 {
                    return Err(IdTableBuildError::SpanLimit {
                        span,
                        maximum: maximum_span,
                    });
                }
                let offset = raw
                    .checked_sub(first)
                    .and_then(|offset| usize::try_from(offset).ok())
                    .ok_or(IdTableBuildError::SpanOverflow)?;
                let target_len = offset
                    .checked_add(1)
                    .ok_or(IdTableBuildError::SpanOverflow)?;
                values
                    .try_reserve(target_len - values.len())
                    .map_err(|_| IdTableBuildError::AllocationFailed)?;
                values.resize_with(offset, T::default);
                values.push(value);
                previous = raw;
            }
            Ok(Self {
                first,
                values,
                identity: PhantomData,
            })
        }

        /// Appends one row and mints its identity: the next in this table's run,
        /// starting at 1 when empty, so a table built by pushing is dense by
        /// construction.
        pub fn push(&mut self, value: T) -> Id {
            if self.values.is_empty() {
                self.first = 1;
            }
            let identity = Id::new(self.first + self.values.len() as u64)
                .expect("a pushed identity is nonzero");
            self.values.push(value);
            identity
        }

        fn position(&self, identity: Id) -> Option<usize> {
            let offset = identity.get().checked_sub(self.first)?;
            usize::try_from(offset)
                .ok()
                .filter(|position| *position < self.values.len())
        }

        pub fn get(&self, identity: Id) -> Option<&T> {
            self.position(identity)
                .map(|position| &self.values[position])
        }

        pub fn get_mut(&mut self, identity: Id) -> Option<&mut T> {
            self.position(identity)
                .map(|position| &mut self.values[position])
        }

        pub fn len(&self) -> usize {
            self.values.len()
        }

        pub fn is_empty(&self) -> bool {
            self.values.is_empty()
        }

        pub fn values(&self) -> std::slice::Iter<'_, T> {
            self.values.iter()
        }

        pub fn keys(&self) -> impl DoubleEndedIterator<Item = Id> + ExactSizeIterator + '_ {
            (0..self.values.len()).map(|offset| {
                Id::new(self.first + offset as u64).expect("table identities are nonzero")
            })
        }

        pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Id, &T)> + ExactSizeIterator + '_ {
            self.keys().zip(self.values.iter())
        }

        pub fn map<U>(&self, mut derive: impl FnMut(Id, &T) -> U) -> IdTable<Id, U> {
            IdTable {
                first: self.first,
                values: self
                    .iter()
                    .map(|(identity, value)| derive(identity, value))
                    .collect(),
                identity: PhantomData,
            }
        }
    }

    impl<Id, T> Default for IdTable<Id, T> {
        fn default() -> Self {
            Self {
                first: 1,
                values: Vec::new(),
                identity: PhantomData,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IdSet<Id> {
        words: Vec<u64>,
        identity: PhantomData<Id>,
    }

    impl<Id: PsiSemanticId> IdSet<Id> {
        pub fn new() -> Self {
            Self::default()
        }

        fn coordinate(identity: Id) -> (usize, u64) {
            let bit = identity.get() - 1;
            (
                usize::try_from(bit / 64).expect("identity exceeds the address space"),
                1 << (bit % 64),
            )
        }

        pub fn contains(&self, identity: Id) -> bool {
            let (word, mask) = Self::coordinate(identity);
            self.words.get(word).is_some_and(|bits| bits & mask != 0)
        }

        pub fn insert(&mut self, identity: Id) -> bool {
            let (word, mask) = Self::coordinate(identity);
            if word >= self.words.len() {
                self.words.resize(word + 1, 0);
            }
            let absent = self.words[word] & mask == 0;
            self.words[word] |= mask;
            absent
        }

        /// Adds every member of `other` using the existing dense bit storage.
        pub fn union_with(&mut self, other: &Self) {
            self.words
                .resize(self.words.len().max(other.words.len()), 0);
            for (word, incoming) in self.words.iter_mut().zip(&other.words) {
                *word |= incoming;
            }
        }

        pub fn remove(&mut self, identity: Id) -> bool {
            let (word, mask) = Self::coordinate(identity);
            let Some(bits) = self.words.get_mut(word) else {
                return false;
            };
            let present = *bits & mask != 0;
            *bits &= !mask;
            present
        }
    }

    impl<Id> Default for IdSet<Id> {
        fn default() -> Self {
            Self {
                words: Vec::new(),
                identity: PhantomData,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::StructuralTypeId;

        fn id(raw: u64) -> StructuralTypeId {
            StructuralTypeId::new(raw).unwrap()
        }

        #[test]
        fn dense_set_union_preserves_both_sides_across_word_boundaries() {
            let mut left = IdSet::new();
            left.insert(id(2));
            let mut right = IdSet::new();
            right.insert(id(65));
            left.union_with(&right);
            assert!(left.contains(id(2)));
            assert!(left.contains(id(65)));
            assert!(!left.contains(id(64)));
            assert!(!right.contains(id(2)));
            left.union_with(&IdSet::new());
            assert!(left.contains(id(65)));
        }

        #[test]
        fn table_holds_one_consecutive_run_in_identity_order() {
            let table = IdTable::from_rows([(id(4), 'a'), (id(5), 'b'), (id(6), 'c')]).unwrap();
            assert_eq!(table.get(id(3)), None);
            assert_eq!(table.get(id(5)), Some(&'b'));
            assert_eq!(table.get(id(7)), None);
            assert_eq!(table.keys().collect::<Vec<_>>(), [id(4), id(5), id(6)]);
            assert_eq!(
                table.map(|_, value| value.to_ascii_uppercase()).get(id(6)),
                Some(&'C')
            );
            assert_eq!(
                IdTable::from_rows([(id(1), ()), (id(3), ())]),
                Err(NonDenseIdentities {
                    expected: 2,
                    found: 3
                })
            );
            assert!(IdTable::from_rows([(id(2), ()), (id(1), ())]).is_err());
            assert!(IdTable::from_rows([(id(2), ()), (id(2), ())]).is_err());
        }

        #[test]
        fn subset_rows_keep_coordinates_and_refuse_invalid_or_excessive_spans() {
            let rows =
                IdTable::from_rows_with_gaps([(id(4), Some('a')), (id(7), Some('b'))], 4).unwrap();
            assert_eq!(rows.get(id(4)), Some(&Some('a')));
            assert_eq!(rows.get(id(5)), Some(&None));
            assert_eq!(rows.get(id(7)), Some(&Some('b')));
            assert_eq!(rows.get(id(8)), None);
            assert_eq!(
                rows.keys().collect::<Vec<_>>(),
                [id(4), id(5), id(6), id(7)]
            );
            let high = IdTable::from_rows_with_gaps([(id(u64::MAX), Some('z'))], 1).unwrap();
            assert_eq!(high.get(id(u64::MAX)), Some(&Some('z')));
            assert!(matches!(
                IdTable::from_rows_with_gaps([(id(4), Some('a'))], 0),
                Err(IdTableBuildError::SpanLimit {
                    span: 1,
                    maximum: 0
                })
            ));
            assert!(IdTable::from_rows([(id(4), ()), (id(7), ())]).is_err());
            assert!(
                IdTable::from_rows_with_gaps([(id(7), Some('b')), (id(7), Some('c'))], 4).is_err()
            );
            assert!(
                IdTable::from_rows_with_gaps([(id(7), Some('b')), (id(4), Some('c'))], 4).is_err()
            );
            assert!(matches!(
                IdTable::from_rows_with_gaps([(id(1), Some('a')), (id(u64::MAX), Some('b'))], 16),
                Err(IdTableBuildError::SpanLimit { .. }) | Err(IdTableBuildError::SpanOverflow)
            ));
        }

        #[test]
        fn set_tracks_membership_across_words() {
            let mut set = IdSet::new();
            assert!(set.insert(id(70)));
            assert!(set.insert(id(1)));
            assert!(!set.insert(id(70)));
            assert!(set.remove(id(70)) && !set.remove(id(70)) && !set.remove(id(900)));
        }
    }
}
mod identity;
pub mod proof_applications;
mod proposition;
pub use boundary::program_entry::{
    ProgramEntrySchema, ProgramEntrySourceCatalog, ProgramEntrySourceContract,
    ProgramEntryVisibleParameters,
};
pub use storage::atomic_cell::AtomicCellType;
pub use storage::target_carrier_layout::TargetCarrierLayout;

pub use scalars::ieee_float_comparison_operation::IeeeFloatComparisonOperation;
pub use scalars::qualified_scalar_type::QualifiedScalarType;
pub use scalars::scalar_refinement::{ScalarRefinement, ScalarRefinementError};
pub use storage::reference_origins::ReferenceOriginExtent;

pub use byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};
pub use fnv1a::Fnv1a;
pub use id_table::{IdSet, IdTable, IdTableBuildError, NonDenseIdentities};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryCompletion, BoundaryMachineId, BoundaryRequirementKind,
    CallingAbiIdentity, CallingDeclarationOwner, CallingGenericFormal, CallingOperandKind,
    CallingSignatureId, CallingStaticOperand, ClaimId, ContentDomainId, ContractId,
    CycleComponentId, DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId,
    ExternalCompletionKind, InvocationSite, MachineId, ObligationId, OperationId,
    PackageKeyIdentity, PlaceId, ProfileDecisionId, PropositionId, PsiSemanticId,
    RankingRelationId, RecursiveComponentId, ScalarApplicationId, ScalarDomainId,
    ScalarQualificationSetId, ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId, SuspensionCrossingId, ValueId, WorkScheduleIdentity,
};
pub use proposition::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, IntegerCarrier, IntegerMathLiteral,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, MathematicalArgument,
    ProofFormalScope, ProofTerm, ProofTermField, Proposition, PropositionContext, PropositionError,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralCaseSubject, StructuralIdentitySubject,
};
pub use scalars::bounded_integer_type::BoundedIntegerType;
pub use storage::authored_storage::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, AuthoredRecordStorage,
};
pub use storage::content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    content_conservation_report_fingerprint, encode_fingerprint_string,
};
pub use storage::structural_places::{AffineConstructionElement, StructuralPlaceKind};

pub use boundary::foreign_locator::{ForeignLocatorCandidate, NativeBindingLocator};

/// The termination promise a callable publishes. A local body proof is kept
/// separately; absence of a promise never asserts that execution will finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    Terminates,
    NoGuarantee,
}
