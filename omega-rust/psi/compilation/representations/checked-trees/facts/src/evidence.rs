/*
We define `FactTable`, `Fact`, `FactPayload`, qualification correspondence
records, and float predicates (`evidence`) in `FactPlan`.
*/

use crate::{FactHandle, FactPlace, PlaceHandle, ProgramPoint};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::name::Identifier;
use typed_trees::types::TypeConstraintNode;

pub(crate) mod float_predicate {
    use numerics::bignum::ExactFloat;
    use numerics::literals::FloatFormat;
    use std::cmp::Ordering;
    use typed_trees::expression::BinaryOperator;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct FloatPredicate {
        format: FloatFormat,
        comparison: BinaryOperator,
        bound_bits: Option<u64>,
    }

    impl FloatPredicate {
        pub fn non_nan(format: FloatFormat) -> Self {
            Self {
                format,
                comparison: BinaryOperator::Equal,
                bound_bits: None,
            }
        }

        pub fn comparison(
            format: FloatFormat,
            comparison: BinaryOperator,
            bound: &ExactFloat,
        ) -> Option<Self> {
            use BinaryOperator::{Equal, Greater, GreaterOrEqual, Less, LessOrEqual};
            if !matches!(
                comparison,
                Equal | Greater | GreaterOrEqual | Less | LessOrEqual
            ) || bound.partial_cmp_value(bound).is_none()
            {
                return None;
            }
            let bound_bits = match format {
                FloatFormat::F32 => u64::from(bound.to_f32().to_bits()),
                FloatFormat::F64 => bound.to_f64().to_bits(),
            };
            Some(Self {
                format,
                comparison,
                bound_bits: Some(bound_bits),
            })
        }

        pub fn format(self) -> FloatFormat {
            self.format
        }

        pub fn equal_bound_spelling(self) -> Option<String> {
            if self.comparison != BinaryOperator::Equal {
                return None;
            }
            let bits = self.bound_bits?;
            let value = match self.format {
                FloatFormat::F32 => f64::from(f32::from_bits(bits as u32)),
                FloatFormat::F64 => f64::from_bits(bits),
            };
            Some(
                numerics::literals::FloatLiteral::from_f64(value)
                    .text()
                    .to_owned(),
            )
        }

        pub fn common_consequence(self, other: Self) -> Option<Self> {
            if self.implies(other) {
                Some(other)
            } else if other.implies(self) {
                Some(self)
            } else if self.format == other.format {
                Some(Self::non_nan(self.format))
            } else {
                None
            }
        }

        fn bound(self) -> Option<ExactFloat> {
            let bits = self.bound_bits?;
            Some(match self.format {
                FloatFormat::F32 => ExactFloat::from_f32(f32::from_bits(bits as u32)),
                FloatFormat::F64 => ExactFloat::from_f64(f64::from_bits(bits)),
            })
        }

        pub fn implies(self, required: Self) -> bool {
            use BinaryOperator::{Equal, Greater, GreaterOrEqual, Less, LessOrEqual};
            if self.format != required.format {
                return false;
            }
            let Some(required_bound) = required.bound() else {
                return true;
            };
            let Some(bound) = self.bound() else {
                return false;
            };
            let Some(ordering) = bound.partial_cmp_value(&required_bound) else {
                return false;
            };
            match required.comparison {
                Equal => self.comparison == Equal && ordering == Ordering::Equal,
                Greater | GreaterOrEqual
                    if matches!(self.comparison, Equal | Greater | GreaterOrEqual) =>
                {
                    ordering == Ordering::Greater
                        || (ordering == Ordering::Equal
                            && (required.comparison == GreaterOrEqual
                                || self.comparison == Greater))
                }
                Less | LessOrEqual if matches!(self.comparison, Equal | Less | LessOrEqual) => {
                    ordering == Ordering::Less
                        || (ordering == Ordering::Equal
                            && (required.comparison == LessOrEqual || self.comparison == Less))
                }
                _ => false,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ieee_bound_implication_keeps_strictness_format_and_nan() {
            let predicate = |format, operation, value| {
                FloatPredicate::comparison(format, operation, &ExactFloat::from_f64(value)).unwrap()
            };
            let lower = predicate(FloatFormat::F64, BinaryOperator::GreaterOrEqual, 0.0);
            assert!(lower.implies(FloatPredicate::non_nan(FloatFormat::F64)));
            assert!(lower.implies(predicate(FloatFormat::F64, BinaryOperator::Greater, -1.0)));
            assert!(!lower.implies(predicate(FloatFormat::F64, BinaryOperator::Greater, 0.0)));
            assert!(!lower.implies(FloatPredicate::non_nan(FloatFormat::F32)));
            assert!(
                !predicate(FloatFormat::F32, BinaryOperator::GreaterOrEqual, 16777217.0).implies(
                    predicate(FloatFormat::F32, BinaryOperator::Greater, 16777216.0)
                )
            );
            assert!(
                FloatPredicate::comparison(
                    FloatFormat::F64,
                    BinaryOperator::Equal,
                    &ExactFloat::from_f64(f64::NAN)
                )
                .is_none()
            );
            assert!(
                FloatPredicate::comparison(
                    FloatFormat::F64,
                    BinaryOperator::NotEqual,
                    &ExactFloat::from_f64(0.0)
                )
                .is_none()
            );
        }
    }
}
#[cfg(test)]
pub(crate) mod qualification_tests {
    use super::{QualificationEvidence, QualificationProvenance};
    use symbols::SymbolHandle;

    fn receipt() -> QualificationEvidence {
        QualificationEvidence::from_admitted_requirement(
            SymbolHandle::from_parts(10, 1),
            SymbolHandle::from_parts(11, 1),
            arena::Handle::from_parts(12, 1),
        )
    }

    fn input(generation: u32) -> QualificationEvidence {
        receipt().at_invocation_input(
            SymbolHandle::from_parts(20, generation),
            SymbolHandle::from_parts(21, 1),
        )
    }

    #[test]
    fn an_arrival_preserves_authority_without_independent_receipt_credit() {
        let local = receipt();
        let input = input(1);
        assert!(local.same_authority(input));
        assert_eq!(local.issuer, input.issuer);
        assert_eq!(local.source_symbol, input.source_symbol);
        assert_eq!(local.requirement_symbol, input.requirement_symbol);
        assert_eq!(local.origin, input.origin);
        assert!(!input.is_independent_receipt());
        assert_eq!(local.common_consequence(input), Some(input));
        assert_eq!(input.common_consequence(local), Some(input));
    }

    #[test]
    fn an_available_independent_proof_survives_input_proofs() {
        let local = receipt();
        let input = input(1);
        assert_eq!(local.available_alternative(input), Some(local));
        assert_eq!(input.available_alternative(local), Some(local));
        assert!(local.is_independent_receipt());
    }

    #[test]
    fn distinct_input_generations_and_unknown_provenance_fail_closed() {
        let first = input(1);
        let second = input(2);
        assert_ne!(first, second);
        let joined = first.common_consequence(second).unwrap();
        assert_eq!(joined.provenance, QualificationProvenance::MixedInputs);
        assert!(!joined.is_independent_receipt());
        assert_eq!(first.common_consequence(first), Some(first));
        let mut unknown = receipt();
        unknown.provenance = QualificationProvenance::Unknown;
        assert_eq!(first.common_consequence(unknown), Some(unknown));
        assert!(!QualificationEvidence::default().is_independent_receipt());
        assert_eq!(
            receipt().at_invocation_input(SymbolHandle::invalid(), SymbolHandle::invalid()),
            unknown
        );
    }

    #[test]
    fn branch_consequences_preserve_input_dependency_and_available_local_proof() {
        let local = receipt();
        let input = input(1);
        let branch = local.common_consequence(input).unwrap();
        assert!(!branch.is_independent_receipt());
        assert_eq!(branch.available_alternative(local), Some(local));
        let mut other_authority = local;
        other_authority.issuer = arena::Handle::from_parts(12, 2);
        assert!(!local.same_authority(other_authority));
        assert_eq!(local.common_consequence(other_authority), None);
        assert_eq!(local.available_alternative(other_authority), None);
    }
}
pub(crate) mod rows {
    use arena::{Handle, OwnedIndex, PagedArenaSet};
    use std::collections::HashMap;
    use std::ops::DerefMut;
    use std::sync::OnceLock;

    use crate::{Fact, ProgramPoint};

    struct Points {
        rows: HashMap<ProgramPoint, Vec<Handle<Fact>>, symbols::BuildSymbolHasher>,
        handles_round_trip: bool,
    }

    #[derive(Default)]
    pub struct FactTable {
        arena: PagedArenaSet<Fact>,
        points: OwnedIndex<Points>,
        revision: OnceLock<u64>,
    }

    impl FactTable {
        pub fn at_point(&self, point: ProgramPoint) -> Vec<&Fact> {
            self.with_points(|points| {
                points
                    .rows
                    .get(&point)
                    .into_iter()
                    .flatten()
                    .map(|handle| self.arena.get(*handle))
                    .collect()
            })
        }

        pub fn rows_at_point(&self, point: ProgramPoint) -> Vec<(Handle<Fact>, &Fact)> {
            self.with_points(|points| {
                if points.handles_round_trip {
                    points
                        .rows
                        .get(&point)
                        .into_iter()
                        .flatten()
                        .map(|handle| (*handle, self.arena.get(*handle)))
                        .collect()
                } else {
                    self.arena
                        .iter()
                        .filter(|(_, row)| row.point == point)
                        .collect()
                }
            })
        }

        fn with_points<Result>(&self, read: impl FnOnce(&Points) -> Result) -> Result {
            self.points.with(
                *self.revision.get_or_init(|| self.arena.revision()),
                || {
                    let mut points = Points {
                        rows: HashMap::default(),
                        handles_round_trip: true,
                    };
                    for (handle, row) in self.arena.iter() {
                        points.rows.entry(row.point).or_default().push(handle);
                        if !self.arena.is_valid(handle)
                            || !std::ptr::eq(self.arena.get(handle), row)
                        {
                            points.handles_round_trip = false;
                        }
                    }
                    points
                },
                read,
            )
        }

        pub fn take_additional_pages(&mut self) -> Self {
            self.deref_mut().take_additional_pages().into()
        }

        pub fn adopt_additional_pages(&mut self, other: Self) {
            self.deref_mut().adopt_additional_pages(other.arena);
        }
    }

    impl From<PagedArenaSet<Fact>> for FactTable {
        fn from(arena: PagedArenaSet<Fact>) -> Self {
            Self {
                arena,
                ..Self::default()
            }
        }
    }

    impl std::ops::Deref for FactTable {
        type Target = PagedArenaSet<Fact>;

        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    impl DerefMut for FactTable {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.points.clear();
            self.revision.take();
            &mut self.arena
        }
    }

    impl Clone for FactTable {
        fn clone(&self) -> Self {
            self.arena.clone().into()
        }

        fn clone_from(&mut self, source: &Self) {
            self.deref_mut().clone_from(&source.arena);
        }
    }

    impl std::fmt::Debug for FactTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.arena.fmt(formatter)
        }
    }

    impl PartialEq for FactTable {
        fn eq(&self, other: &Self) -> bool {
            self.arena == other.arena
        }
    }

    impl Eq for FactTable {}

    impl compiler_storage::Record for FactTable {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            compiler_storage::Record::write(&self.arena, output)
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            <PagedArenaSet<Fact> as compiler_storage::Record>::read(input).map(Self::from)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn matches_scan(rows: &FactTable, points: &[ProgramPoint]) {
            for point in points {
                let expected = rows
                    .iter()
                    .filter_map(|(_, row)| (row.point == *point).then_some(row))
                    .collect::<Vec<_>>();
                assert_eq!(
                    rows.rows_at_point(*point)
                        .into_iter()
                        .map(|(handle, _)| handle)
                        .collect::<Vec<_>>(),
                    rows.iter()
                        .filter_map(|(handle, row)| (row.point == *point).then_some(handle))
                        .collect::<Vec<_>>()
                );
                assert!(
                    rows.rows_at_point(*point)
                        .iter()
                        .map(|(_, row)| *row)
                        .zip(expected.iter().copied())
                        .all(|(actual, expected)| std::ptr::eq(actual, expected))
                );
                let actual = rows.at_point(*point);
                assert_eq!(actual.len(), expected.len());
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| std::ptr::eq(*actual, expected))
                );
            }
        }

        #[test]
        fn point_rows_preserve_arena_order_and_follow_mutation_and_replacement() {
            let points = [
                ProgramPoint::Global,
                ProgramPoint::Definition {
                    symbol: symbols::SymbolHandle::from_parts(3, 9),
                },
                ProgramPoint::Definition {
                    symbol: symbols::SymbolHandle::from_parts(3, 10),
                },
                ProgramPoint::Definition {
                    symbol: symbols::SymbolHandle::invalid(),
                },
            ];
            let mut rows = FactTable::default();
            let first = rows.append(Fact {
                point: points[0],
                ..Default::default()
            });
            let second = rows.append(Fact {
                point: points[1],
                ..Default::default()
            });
            rows.append(Fact {
                point: points[0],
                ..Default::default()
            });
            for point in &points[2..] {
                rows.append(Fact {
                    point: *point,
                    ..Default::default()
                });
            }
            matches_scan(&rows, &points);
            rows.get_mut(first).point = points[1];
            matches_scan(&rows, &points);
            rows.free(second);
            matches_scan(&rows, &points);
            rows.begin_page(12);
            rows.append(Fact {
                point: points[0],
                ..Default::default()
            });
            matches_scan(&rows, &points);
            let mut baseline = rows.clone();
            let pages = rows.take_additional_pages();
            matches_scan(&rows, &points);
            rows.adopt_additional_pages(pages);
            matches_scan(&rows, &points);
            baseline.clone_from(&rows);
            matches_scan(&baseline, &points);

            let mut left = PagedArenaSet::default();
            left.append(Fact {
                point: points[0],
                ..Default::default()
            });
            let mut right = PagedArenaSet::default();
            right.append(Fact {
                point: points[1],
                ..Default::default()
            });
            assert_eq!(left.revision(), right.revision());
            let mut replaced = FactTable::from(left);
            matches_scan(&replaced, &points);
            *replaced = right;
            matches_scan(&replaced, &points);
        }

        struct RawFactPages {
            first: arena::Arena<Fact>,
            other: Option<arena::Arena<Fact>>,
        }

        impl compiler_storage::Record for RawFactPages {
            fn write(
                &self,
                out: &mut compiler_storage::RecordWriter,
            ) -> Result<(), compiler_storage::RecordError> {
                out.record(|out| {
                    1u32.write(out)?;
                    false.write(out)?;
                    self.first.write(out)?;
                    out.record(|out| {
                        out.sequence(usize::from(self.other.is_some()))?;
                        if let Some(page) = &self.other {
                            out.record(|out| {
                                2u32.write(out)?;
                                false.write(out)?;
                                page.write(out)
                            })?;
                        }
                        Ok(())
                    })?;
                    (if self.other.is_some() { 2u32 } else { 1u32 }).write(out)?;
                    (self.first.len() + self.other.as_ref().map_or(0, arena::Arena::len))
                        .write(out)?;
                    Fact::default().write(out)
                })
            }

            fn read<Context>(
                input: &mut compiler_storage::RecordReader<'_, Context>,
            ) -> Result<Self, compiler_storage::RecordError> {
                Err(input.error("raw fact page fixture is write-only"))
            }
        }

        #[test]
        fn point_lookup_keeps_the_exact_arena_encoding_and_decodes_cold() {
            let mut rows = FactTable::default();
            rows.append(Fact::default());
            let limits = compiler_storage::RecordLimits::default();
            let original = compiler_storage::encode(&*rows, limits).unwrap();
            matches_scan(&rows, &[ProgramPoint::Global]);
            assert_eq!(compiler_storage::encode(&rows, limits).unwrap(), original);
            let decoded: FactTable = compiler_storage::decode(&original, limits).unwrap();
            assert!(decoded.revision.get().is_none());
            matches_scan(&decoded, &[ProgramPoint::Global]);
            assert_eq!(decoded, rows);

            let symbol = symbols::SymbolHandle::from_parts(3, 9);
            let point = ProgramPoint::Definition { symbol };
            let fact = Fact {
                point,
                ..Default::default()
            };
            for alias_page in [false, true] {
                let first = arena::Arena::from_storage(
                    Fact::default(),
                    vec![fact, fact],
                    Some(arena::ArenaSlotStorage {
                        generations: vec![1, 2],
                        occupied: vec![true, true],
                        free_indices: Vec::new(),
                    }),
                    1,
                    false,
                )
                .unwrap();
                let other = alias_page.then(|| {
                    arena::Arena::from_storage(
                        Fact::default(),
                        vec![Fact::default(); 2],
                        None,
                        2,
                        true,
                    )
                    .unwrap()
                });
                let bytes =
                    compiler_storage::encode(&RawFactPages { first, other }, limits).unwrap();
                let mut decoded: FactTable = compiler_storage::decode(&bytes, limits).unwrap();
                let raw: Vec<_> = decoded
                    .iter()
                    .filter(|(_, row)| row.point == point)
                    .collect();
                assert_eq!(raw.len(), 2);
                let displaced = raw[1].0;
                if alias_page {
                    assert!(decoded.is_valid(displaced));
                    assert!(!std::ptr::eq(decoded.get(displaced), raw[1].1));
                } else {
                    assert!(!decoded.is_valid(displaced));
                }
                for selected in [
                    point,
                    ProgramPoint::Global,
                    ProgramPoint::Definition {
                        symbol: symbols::SymbolHandle::from_parts(3, 10),
                    },
                ] {
                    let original: Vec<_> = decoded
                        .iter()
                        .filter(|(_, row)| row.point == selected)
                        .collect();
                    let actual = decoded.rows_at_point(selected);
                    assert_eq!(actual.len(), original.len());
                    assert!(
                        actual
                            .iter()
                            .zip(&original)
                            .all(|((a, ar), (b, br))| a == b && std::ptr::eq(*ar, *br))
                    );
                    let old: Vec<_> = original
                        .iter()
                        .map(|(handle, _)| decoded.get(*handle))
                        .collect();
                    assert!(
                        decoded
                            .at_point(selected)
                            .iter()
                            .zip(old)
                            .all(|(a, b)| std::ptr::eq(*a, b))
                    );
                }
                assert_eq!(compiler_storage::encode(&decoded, limits).unwrap(), bytes);
                let first = raw[0].0;
                drop(raw);
                decoded.get_mut(first).point = ProgramPoint::Global;
                let remaining = decoded.rows_at_point(point);
                assert_eq!(remaining.len(), 1);
                assert_eq!(remaining[0].0, displaced);
                assert!(std::ptr::eq(
                    remaining[0].1,
                    decoded
                        .iter()
                        .find(|(handle, _)| *handle == displaced)
                        .unwrap()
                        .1
                ));
            }
        }
    }
}
pub use float_predicate::FloatPredicate;
pub use rows::FactTable;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DomainDefinitionFactRecord {
    pub domain_symbol: SymbolHandle,
    pub fact: Handle<ProofFact>,
    pub semantic_fact: FactHandle,
    pub dependencies: Vec<DomainDefinitionFactDependency>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DomainDefinitionFactDependency {
    pub expression: ExpressionHandle,
    pub place: PlaceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DataDefinitionFactRecord {
    pub data_symbol: SymbolHandle,
    pub fact: Handle<ProofFact>,
    pub semantic_fact: FactHandle,
    pub dependencies: Vec<DataDefinitionFactDependency>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DataDefinitionFactDependency {
    pub expression: ExpressionHandle,
    pub place: PlaceHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ContractMemberProjection {
    pub origin: FactOrigin,
    pub fact: Handle<ProofFact>,
    pub expression: ExpressionHandle,
    pub receiver_type: typed_trees::types::TypeReferenceHandle,
    pub authored_field: SymbolHandle,
    pub applied_field: SymbolHandle,
    pub applied_case: SymbolHandle,
    pub authored_case: SymbolHandle,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FactOrigin {
    #[default]
    Unknown,
    DomainDefinition {
        domain_symbol: SymbolHandle,
    },
    DataDefinition {
        data_symbol: SymbolHandle,
    },
    TypeReference,
    ProofObligation,
    MachineContract {
        machine_symbol: SymbolHandle,
    },
    StateContract {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    ContextualCleanup {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateParameterDomain {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    LocalCasePayloadDomain {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignatureContract {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallRequires,
    CallEnsures,
    TransitionGuard,
    OperatorRequires {
        operator_symbol: SymbolHandle,
    },
    OperatorEnsures {
        operator_symbol: SymbolHandle,
    },
    StatementTransfer,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct QualificationEvidence {
    pub origin: language_semantics::QualificationEvidenceOrigin,
    pub source_symbol: SymbolHandle,
    pub requirement_symbol: SymbolHandle,
    pub issuer: Handle<BoundaryQualificationIssuerRecord>,
    pub provenance: QualificationProvenance,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum QualificationProvenance {
    #[default]
    Unknown,
    Independent,
    InvocationInput {
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    MixedInputs,
}

impl QualificationProvenance {
    fn common(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Independent, other) | (other, Self::Independent) => other,
            _ => Self::MixedInputs,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct BoundaryQualificationIssuerRecord {
    pub source_symbol: SymbolHandle,
    pub requirement_symbol: SymbolHandle,
    pub issuer: language_semantics::BoundaryQualificationIssuer,
}

impl QualificationEvidence {
    pub const fn from_origin(
        origin: language_semantics::QualificationEvidenceOrigin,
        source_symbol: SymbolHandle,
    ) -> Self {
        Self {
            origin,
            source_symbol,
            requirement_symbol: SymbolHandle::invalid(),
            issuer: Handle::invalid(),
            provenance: QualificationProvenance::Unknown,
        }
    }

    pub const fn from_admitted_requirement(
        source_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        issuer: Handle<BoundaryQualificationIssuerRecord>,
    ) -> Self {
        Self {
            origin: language_semantics::QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol,
            requirement_symbol,
            issuer,
            provenance: QualificationProvenance::Independent,
        }
    }

    pub fn at_invocation_input(
        mut self,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    ) -> Self {
        self.provenance = if state_symbol.is_valid() && parameter_symbol.is_valid() {
            QualificationProvenance::InvocationInput {
                state_symbol,
                parameter_symbol,
            }
        } else {
            QualificationProvenance::Unknown
        };
        self
    }

    pub fn same_authority(self, other: Self) -> bool {
        self.origin == other.origin
            && self.source_symbol == other.source_symbol
            && self.requirement_symbol == other.requirement_symbol
            && self.issuer == other.issuer
    }

    pub fn common_consequence(mut self, other: Self) -> Option<Self> {
        if !self.same_authority(other) {
            return None;
        }
        self.provenance = self.provenance.common(other.provenance);
        Some(self)
    }

    pub fn available_alternative(self, other: Self) -> Option<Self> {
        if !self.same_authority(other) {
            return None;
        }
        if self.provenance == QualificationProvenance::Independent {
            Some(self)
        } else if other.provenance == QualificationProvenance::Independent {
            Some(other)
        } else {
            self.common_consequence(other)
        }
    }

    pub fn is_independent_receipt(self) -> bool {
        self.origin == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
            && self.provenance == QualificationProvenance::Independent
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ContractFactKind {
    #[default]
    Requires,
    Ensures,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct InstantiatedExpression {
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProofObligationKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum InstructionEffectOperation {
    Register(language_core::inline_assembly::AsmRegisterOperation),
    AtomicReleaseStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum FactPayload {
    InstructionEffect {
        operation: InstructionEffectOperation,
    },
    CallCompletion {
        target: SymbolHandle,
    },

    BooleanPredicate {
        predicate: Handle<crate::BooleanPredicate>,
    },
    CaseValue {
        variant: SymbolHandle,
        value: bool,
    },
    /*
        Zero formation of an owned fixed array, stated once instead of once per
        element. The fact's place names the array, and for every index in
        `first..end` the place `array[index]` followed by `element` has `variant`
        selected, exactly as a `CaseValue { variant, value: true }` there would say.
        The zero representation keeps the first declared case, so one formation
        covers every element alike; a write that can replace one element's tag
        narrows the range around that element and leaves the others standing.
    */
    ZeroCaseElements {
        first: usize,
        end: usize,
        element: HandleSpan<crate::PlaceSegment>,
        variant: SymbolHandle,
    },
    AssignedIntegerBounds {
        bounds: Handle<IntegerRange>,
    },
    FloatPredicate {
        predicate: FloatPredicate,
    },
    AssignedScalarValue {
        value: Handle<ScalarValue>,
    },
    AssignedValueCapture {
        source: PlaceHandle,
        point: ProgramPoint,
    },
    AssignedValue {
        value: ExpressionHandle,
    },
    BytePredicate {
        predicate: language_semantics::byte_predicates::ByteSequencePredicate,
    },
    BooleanExpression(ExpressionHandle),
    StorageDependency {
        dependent: FactHandle,
    },
    BooleanValue {
        expression: ExpressionHandle,
        value: bool,
    },
    MatchPattern {
        expression: ExpressionHandle,
        arm: Handle<typed_trees::expression::TableMatchArm>,
        matched: bool,
    },
    DomainMembership {
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    },
    PropositionApplication {
        fact: Handle<ProofFact>,
        proposition: SymbolHandle,
    },
    CarryPermission {
        value: ExpressionHandle,
        permission: language_semantics::CarryPermission,
    },
    CarryOrigin {
        value: ExpressionHandle,
    },
    TypeConstraint {
        constraint: Handle<TypeConstraintNode>,
    },
    ProofObligation {
        kind: ProofObligationKind,
    },
    Contract {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
    },
    ContractBooleanExpression {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        expression: ExpressionHandle,
        instantiated: Handle<InstantiatedExpression>,
    },
    ContractDomainMembership {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    },
    ContractPropositionApplication {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        proposition: SymbolHandle,
        instantiated: Handle<InstantiatedExpression>,
    },
    ContractCarryPermission {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        value: ExpressionHandle,
        permission: language_semantics::CarryPermission,
    },
}

impl FactPayload {
    pub const fn domain_membership(
        self,
    ) -> Option<(
        ExpressionHandle,
        HandleSpan<Identifier>,
        SymbolHandle,
        language_semantics::SemanticDomainId,
    )> {
        match self {
            Self::DomainMembership {
                value,
                domain,
                domain_symbol,
                semantic_domain,
            }
            | Self::ContractDomainMembership {
                value,
                domain,
                domain_symbol,
                semantic_domain,
                ..
            } => Some((value, domain, domain_symbol, semantic_domain)),
            _ => None,
        }
    }

    pub const fn boolean_expression(self) -> Option<ExpressionHandle> {
        match self {
            Self::BooleanExpression(expression)
            | Self::ContractBooleanExpression { expression, .. } => Some(expression),
            _ => None,
        }
    }

    pub const fn is_carry_claim(self) -> bool {
        matches!(
            self,
            Self::CarryPermission { .. }
                | Self::ContractCarryPermission { .. }
                | Self::CarryOrigin { .. }
        )
    }

    pub const fn carry_permission(self) -> Option<language_semantics::CarryPermission> {
        match self {
            Self::CarryPermission { permission, .. }
            | Self::ContractCarryPermission { permission, .. } => Some(permission),
            _ => None,
        }
    }

    pub fn match_pattern_comparison(
        self,
        program: &typed_trees::TypedTrees,
    ) -> Option<(ExpressionHandle, ExpressionHandle, bool)> {
        let Self::MatchPattern {
            expression,
            arm,
            matched,
        } = self
        else {
            return None;
        };
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let typed_trees::expression::ExpressionNode::Match(dispatch) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        if arm.generation() != dispatch.arms.start().generation() {
            return None;
        }
        let ordinal = arm
            .arena_index()
            .checked_sub(dispatch.arms.start().arena_index())?;
        let source = program
            .expression_table
            .match_arms(dispatch.arms)
            .get(ordinal as usize)?;
        let typed_trees::expression::MatchPattern::Value(pattern) = source.pattern else {
            return None;
        };
        (program
            .expression_table
            .expression_is_valid(dispatch.subject)
            && program.expression_table.expression_is_valid(pattern))
        .then_some((dispatch.subject, pattern, matched))
    }
}

impl Default for FactPayload {
    fn default() -> Self {
        Self::BooleanExpression(ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum QualificationPayloadIdentity {
    DomainMembership {
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    },
    CarryPermission {
        permission: language_semantics::CarryPermission,
    },
    #[default]
    CarryOrigin,
}

impl QualificationPayloadIdentity {
    pub const fn from_fact_payload(payload: FactPayload) -> Option<Self> {
        if let Some((_, domain, domain_symbol, semantic_domain)) = payload.domain_membership() {
            return Some(Self::DomainMembership {
                domain,
                domain_symbol,
                semantic_domain,
            });
        }
        if let Some(permission) = payload.carry_permission() {
            return Some(Self::CarryPermission { permission });
        }
        if matches!(payload, FactPayload::CarryOrigin { .. }) {
            return Some(Self::CarryOrigin);
        }
        None
    }
}

pub use numerics::scalar_predicates::ScalarValue;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct IntegerRange {
    pub minimum: numerics::bignum::BigInt,
    pub maximum: numerics::bignum::BigInt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct QualificationCorrespondence {
    pub source_fact: FactHandle,
    pub destination_fact: FactHandle,
    pub source_occurrence_place: PlaceHandle,
    pub source_place: PlaceHandle,
    pub destination_place: PlaceHandle,
    pub formation: ProgramPoint,
    pub payload: QualificationPayloadIdentity,
    pub evidence: QualificationEvidence,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct Fact {
    pub place: FactPlace,
    pub point: ProgramPoint,
    pub origin: FactOrigin,
    pub evidence: QualificationEvidence,
    pub payload: FactPayload,
}

#[macro_export]
macro_rules! domain_membership_pattern {
    ($($field:ident),+ $(,)?) => {
        $crate::FactPayload::DomainMembership { $($field,)+ .. }
            | $crate::FactPayload::ContractDomainMembership { $($field,)+ .. }
    };
}

#[macro_export]
macro_rules! carry_permission_pair_pattern {
    ($($fields:tt)*) => {
        $crate::FactPayload::CarryPermission { $($fields)* }
            | $crate::FactPayload::ContractCarryPermission { $($fields)* }
    };
}
