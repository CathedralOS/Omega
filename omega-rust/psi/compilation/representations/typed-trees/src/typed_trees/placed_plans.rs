//! Placed view, field and accessor plans and the plan-laid layouts.

mod sealed_accessors;
pub use sealed_accessors::{PlacedScalarAccessorDeclaration, PlacedScalarAccessorOperation};

use crate::typed_trees::ClosedConformanceApplication;

impl crate::TypedTrees {
    /// Operation classification follows exact declaration/specialization
    /// identities, independently of call syntax or destination type.
    pub fn placed_borrow_factory(
        &self,
        mut selected: symbols::SymbolHandle,
    ) -> Option<&crate::expression::PlacedBorrowOperationRequest> {
        let mut visited = Vec::new();
        loop {
            if !selected.is_valid() || visited.contains(&selected) {
                return None;
            }
            visited.push(selected);
            let mut factories = self
                .placed_borrow_factories
                .iter()
                .filter(|row| row.factory == selected);
            if let Some(factory) = factories.next() {
                return factories.next().is_none().then_some(factory);
            }
            let mut applications = self
                .machine_specializations
                .iter()
                .filter(|application| application.instance == selected);
            let (Some(application), None) = (applications.next(), applications.next()) else {
                return None;
            };
            selected = application.template;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewPlan {
    /// Exact sealed source family whose closed P/T application this realizes.
    /// Invalid only for source-free derivation with no canonical interface.
    pub template_symbol: symbols::SymbolHandle,
    pub data_name: String,
    /// Exact synthesized placed-view data identity. `data_name` is diagnostic
    /// presentation and must not be used as the semantic join key.
    pub data_symbol: symbols::SymbolHandle,
    pub policy_name: String,
    /// Exact nominal placement-policy data identity.
    pub policy_symbol: symbols::SymbolHandle,
    /// Exact build-time `Policy::plan` machine that produced `placement`.
    pub policy_plan_machine_symbol: symbols::SymbolHandle,
    pub schema_name: String,
    /// Exact source schema identity whose fields the plan interprets.
    pub schema_symbol: symbols::SymbolHandle,
    pub placement: access_plans::ValidatedPlacementPlan,
    pub fields: Vec<PlacedFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFieldPlan {
    /// Exact normalizer-issued field key; later retention never selects by spelling.
    pub access_key: access_plans::AccessFieldKey,
    /// Exact generated view member, independently of the schema member.
    pub view_field_symbol: symbols::SymbolHandle,
    pub field_name: String,
    /// Stable numbered identity when the source schema numbers this field.
    /// The spelling remains diagnostic presentation only in that case.
    pub member_identity: Option<u64>,
    pub field_symbol: symbols::SymbolHandle,
    pub accessor_name: String,
    /// Exact synthesized accessor type reference. Shell-aware typed lookup
    /// rejoins through this handle rather than `accessor_name`.
    pub accessor_type: crate::types::TypeReferenceHandle,
    /// Exact generated accessor data definition. `accessor_name` is retained
    /// for diagnostics and source-oriented artifact presentation only.
    pub accessor_data_symbol: symbols::SymbolHandle,
    /// Exact generated operation targets for non-atomic placed accessors.
    /// Atomic operations retain their separate typed carrier and therefore
    /// have no cloned `PlacedField` operation target rows here.
    pub accessor_targets: Vec<PlacedAccessorTarget>,
    pub value_type: crate::types::TypeReferenceHandle,
    pub access: access_plans::FieldAccess,
    /// Checked, non-authorizing resident/result-shape evidence retained only
    /// when an Atomic placed field admits an observing compare-exchange axis.
    /// Try-only fields remain `None`; this does not carry a resident value, an
    /// operation attempt, selected encoding law, or target-lowering authority.
    /// Current source-formable Atomic scalar residents are unrestricted because
    /// aggregate fields remain behind the Inaccessible fence, but formation
    /// still consumes the exact normalized multiplicity rather than assuming it.
    pub atomic_resident: Option<PlacedAtomicResidentContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAtomicResidentContract {
    /// Exact source field whose resident type is described.
    pub field_symbol: symbols::SymbolHandle,
    /// Exact semantic resident type retained by the source schema field.
    pub resident_type: crate::types::TypeReferenceHandle,
    /// Normalized source multiplicity used to admit observing failure arms.
    pub multiplicity: language_semantics::Multiplicity,
    /// Exact normalized Atomic transfer width.
    pub transfer_width_bits: u16,
    /// Exact independently admitted observing permission axes.
    pub compare_exchange: bool,
    pub compare_exchange_once: bool,
    /// Canonical decisive-then-single-attempt result-shape rows.
    pub observing_results: Vec<PlacedAtomicObservingResultContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedAtomicObservingResultContract {
    pub operation: language_core::atomic::AtomicObservingCompareExchangeOperation,
    pub result_shape: language_core::atomic::AtomicObservingCompareExchangeResultShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAccessorTarget {
    pub operation: String,
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
}

pub(crate) fn named_type_reference_through_shells(
    table: &crate::types::TypeReferenceTable,
    handle: crate::types::TypeReferenceHandle,
) -> Option<crate::types::TypeReferenceHandle> {
    match table.type_reference(handle) {
        crate::types::TypeReferenceNode::Named { .. } => Some(handle),
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_through_shells(table, *referee)
        }
        crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_through_shells(table, *base_type)
        }
        _ => None,
    }
}

/// One exact policy/schema application. Every retained geometry is validated;
/// target-family alternatives remain guarded until target closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidLayout {
    /// Name of the synthesized data definition (e.g. `CLayout<GdtEntryish>`).
    /// Diagnostic/source-oriented presentation only.
    pub data_name: String,
    /// Exact synthesized data definition whose physical layout this plan owns.
    pub data_symbol: symbols::SymbolHandle,
    /// Exact runtime field identities in the same order as `offsets`.
    pub field_symbols: Vec<symbols::SymbolHandle>,
    /// Exact source schema and runtime field identities reflected into the
    /// synthesized value type.
    pub schema_symbol: symbols::SymbolHandle,
    pub schema_field_symbols: Vec<symbols::SymbolHandle>,
    /// Exact nominal layout policy and plan machine, or the canonical family
    /// interface for guarded alternatives. These grant no runtime authority.
    pub policy_symbol: symbols::SymbolHandle,
    pub policy_plan_machine_symbol: symbols::SymbolHandle,
    /// Exact compiler-materialized Schema input for guarded family folds.
    /// Only invariant applications may omit this retained argument.
    pub schema_argument: Option<PlanLaidSchemaArgument>,
    pub realizations: PlanLaidRealizations,
}

/// Versioned active rows of the compiler-owned Schema ABI. Version 1 has a
/// record shape, no cases or retired identities, and fixed unused-row padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidSchemaArgument {
    pub abi_version: u16,
    pub fields: arena::HandleSpan<PlanLaidSchemaField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidSchemaField {
    pub schema_field: symbols::SymbolHandle,
    pub key: u64,
    pub identity: Option<u64>,
    pub byte_size: u64,
    pub byte_alignment: u64,
    pub kind: PlanLaidSchemaFieldKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlanLaidSchemaFieldKind {
    #[default]
    Scalar,
    Nested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLaidRealizations {
    Invariant(PlanLaidGeometry),
    TargetFamily(arena::HandleSpan<PlanLaidTargetLayout>),
}

/// Exact member identity accompanies its geometry even for literal-only policies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidTargetLayout {
    pub target: crate::name::Identifier,
    pub machine: symbols::SymbolHandle,
    pub geometry: PlanLaidGeometry,
}

impl PlanLaidLayout {
    /// Only an invariant application has geometry before target closure.
    pub fn invariant_geometry(&self) -> Option<&PlanLaidGeometry> {
        match &self.realizations {
            PlanLaidRealizations::Invariant(geometry) => Some(geometry),
            PlanLaidRealizations::TargetFamily(_) => None,
        }
    }

    /// Query one exact target within a checked target context. Absence never
    /// selects a family representative; unknown targets and lost spans refuse.
    pub fn geometry_for_target<'a>(
        &'a self,
        typed: &'a crate::TypedTrees,
        target: Option<&str>,
    ) -> Option<&'a PlanLaidGeometry> {
        match &self.realizations {
            PlanLaidRealizations::Invariant(geometry) => Some(geometry),
            PlanLaidRealizations::TargetFamily(span) => {
                let target = target?;
                typed
                    .plan_laid_target_layouts
                    .span(*span)?
                    .iter()
                    .find(|row| row.target.as_str() == target)
                    .map(|row| &row.geometry)
            }
        }
    }

    /// Enumerate every alternative; no member is a representative for a family.
    pub fn geometries<'a>(
        &'a self,
        typed: &'a crate::TypedTrees,
    ) -> impl Iterator<Item = &'a PlanLaidGeometry> {
        let invariant = match &self.realizations {
            PlanLaidRealizations::Invariant(geometry) => Some(geometry),
            PlanLaidRealizations::TargetFamily(_) => None,
        };
        invariant
            .into_iter()
            .chain(self.target_layouts(typed).iter().map(|row| &row.geometry))
    }

    /// Exact guarded alternatives after application validation. Validators must
    /// check the arena span and complete nonempty family roster before use.
    pub fn target_layouts<'a>(&self, typed: &'a crate::TypedTrees) -> &'a [PlanLaidTargetLayout] {
        match self.realizations {
            PlanLaidRealizations::Invariant(_) => &[],
            PlanLaidRealizations::TargetFamily(span) => {
                typed.plan_laid_target_layouts.span_or_empty(span)
            }
        }
    }
}

/// One fully static geometry, derived only from a validated policy result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidGeometry {
    /// Exact validated target-neutral geometry from which the host-sized
    /// consumer projections below were derived.
    pub validated_layout: layout_plans::LayoutPlanReport,
    /// Source-authored semantic-field-free callback destinations. These rows
    /// intentionally stop at exact declaration identity plus offset; the
    /// selected target calling plan supplies pointer extent and completes
    /// bounds/non-overlap validation before materialization.
    pub private_callback_demands:
        Vec<layout_plans::PrivateCallbackLayoutDemandReport<ClosedConformanceApplication>>,
    /// Byte offset of each field, in declaration order.
    pub offsets: Vec<usize>,
    /// Fragmented scalar fields keyed by declaration-order field index.
    /// Empty for ordinary byte-aligned plans. The compiler's layout
    /// validator has already proved complete source tiling, non-overlapping
    /// destinations, and in-bounds containers.
    pub bit_fields: Vec<PlanLaidBitField>,
    /// Fixed-width integer fields whose physical encoding is narrower than
    /// their semantic carrier. The validated plan has already proved that
    /// every stored bit pattern decodes into the carrier.
    pub integer_fields: Vec<PlanLaidIntegerField>,
    /// Outer fixed-array fields whose validated plan places one complete
    /// compiler-sized element at each destination in one constant-stride
    /// sequence. The ordinary field offset remains the first destination;
    /// consumers use this row only while indexing that outer array.
    pub repeated_fields: Vec<PlanLaidRepeatedField>,
    /// Total value size (fixed by the value-type gate).
    pub size: usize,
    pub align: usize,
}

impl PlanLaidGeometry {
    /// Project one validated field placement into the source-free storage
    /// vocabulary. The caller supplies the exact declaration-order position
    /// and semantic scalar classification; this operation never chooses a target.
    pub fn authored_field_storage(
        &self,
        field_index: usize,
        scalar_field: bool,
    ) -> Option<semantic_vocabulary::AuthoredFieldStorage> {
        use semantic_vocabulary::{
            AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, IntegerSign,
            IntegerType, ScalarType,
        };
        if let Some(field) = self
            .bit_fields
            .iter()
            .find(|field| field.field_index == field_index)
        {
            let fragments = field
                .fragments
                .iter()
                .map(|fragment| {
                    Some(AuthoredBitFragment {
                        container_byte_offset: u64::try_from(fragment.container_byte_offset)
                            .ok()?,
                        container_width_bits: u64::from(fragment.container_width_bits),
                        destination_lsb: u64::from(fragment.destination_lsb),
                        source_lsb: u64::from(fragment.source_lsb),
                        width: u64::from(fragment.width),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            return Some(AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(
                fragments,
            )));
        }
        let byte_offset = u64::try_from(*self.offsets.get(field_index)?).ok()?;
        if let Some(field) = self
            .repeated_fields
            .iter()
            .find(|field| field.field_index == field_index)
        {
            return Some(AuthoredFieldStorage::Repeated {
                byte_offset,
                element_stride: u64::try_from(field.element_stride).ok()?,
            });
        }
        if let Some(field) = self
            .integer_fields
            .iter()
            .find(|field| field.field_index == field_index)
        {
            let sign = match field.interpretation {
                layout_plans::IntegerInterpretation::Signed => IntegerSign::Signed,
                layout_plans::IntegerInterpretation::Unsigned => IntegerSign::Unsigned,
            };
            return Some(AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar: Some(ScalarType::Integer(
                    IntegerType::new(sign, field.stored_width_bits).ok()?,
                )),
            });
        }
        if scalar_field {
            Some(AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar: None,
            })
        } else {
            Some(AuthoredFieldStorage::Direct { byte_offset })
        }
    }
}

impl Default for PlanLaidGeometry {
    /// Arena zero storage is absence, never a validated application.
    fn default() -> Self {
        Self {
            validated_layout: layout_plans::LayoutPlanReport {
                schema_report_fingerprint: 0,
                entries: Vec::new(),
                offsets: None,
                size: None,
                align: 0,
            },
            private_callback_demands: Vec::new(),
            offsets: Vec::new(),
            bit_fields: Vec::new(),
            integer_fields: Vec::new(),
            repeated_fields: Vec::new(),
            size: 0,
            align: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidRepeatedField {
    pub field_index: usize,
    pub element_stride: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidBitField {
    pub field_index: usize,
    pub fragments: Vec<PlanLaidBitFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidIntegerField {
    pub field_index: usize,
    pub stored_width_bits: u16,
    pub interpretation: layout_plans::IntegerInterpretation,
    /// Every value admitted by the semantic field type has an encoding at the
    /// stored width. Mutation may truncate only when this validated fact holds.
    pub write_is_total: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanLaidBitFragment {
    pub container_byte_offset: usize,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}
