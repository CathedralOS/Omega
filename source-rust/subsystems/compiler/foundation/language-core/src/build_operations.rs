//! Authored Build operations and the declaration references each one selects in the Product.

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum BuildOperation {    ProviderReadiness,
    ProviderSelection,
    AssumedBindingContract,
    RepresentationSelection,
    ServiceExclusion,
    ServiceAudit,
    OptimizationSelection,
    OptimizationReportRequest,
    BoundaryAcceptance,
    WireCompatibilityRequest,
    IncludedSourceHandoff,
    LogWriteLine,
    NativeBinding,
    // `builder.tests.group<Requirement>()` (wiki/spec/build/testing.md): one
    // exact trait machine requirement registered as a test group.
    TestGroupRegistration,
    ParameterDescription,
    FieldDescription,
    ServiceDescription,
}

impl BuildOperation {
    pub const fn uses_tests_facet(self) -> bool {
        matches!(self, Self::TestGroupRegistration)
    }
    pub const fn has_designated_operands(self) -> bool {
        self.selects_product_declarations()
            || matches!(self, Self::ProviderReadiness | Self::ServiceDescription)
    }
    pub const fn selects_product_declarations(self) -> bool {
        matches!(
            self,
            Self::ProviderSelection
                | Self::NativeBinding
                | Self::ServiceExclusion
                | Self::ServiceAudit
                | Self::BoundaryAcceptance
                | Self::WireCompatibilityRequest
                | Self::RepresentationSelection
                | Self::AssumedBindingContract
                | Self::TestGroupRegistration
        )
    }

    pub const ALL: [Self; 15] = [
        Self::ProviderReadiness,
        Self::ServiceDescription,
        Self::ProviderSelection,
        Self::AssumedBindingContract,
        Self::RepresentationSelection,
        Self::ServiceExclusion,
        Self::ServiceAudit,
        Self::OptimizationSelection,
        Self::OptimizationReportRequest,
        Self::BoundaryAcceptance,
        Self::WireCompatibilityRequest,
        Self::IncludedSourceHandoff,
        Self::LogWriteLine,
        Self::NativeBinding,
        Self::TestGroupRegistration,
    ];

    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::ServiceDescription => "service",
            Self::ParameterDescription => "reflect::parameter",
            Self::FieldDescription => "reflect::field",
            Self::NativeBinding => "bind_native",
            Self::ProviderReadiness => "await_provider",
            Self::ProviderSelection => "select_provider",
            Self::AssumedBindingContract => "assume_binding_contract",
            Self::RepresentationSelection => "select_representation",
            Self::ServiceExclusion => "exclude_service",
            Self::ServiceAudit => "audit_service",
            Self::OptimizationSelection => "enable",
            Self::OptimizationReportRequest => "emit_report",
            Self::BoundaryAcceptance => "accept_boundary",
            Self::WireCompatibilityRequest => "require_wire_compatibility",
            Self::IncludedSourceHandoff => "include_source",
            Self::LogWriteLine => "write_line",
            Self::TestGroupRegistration => "group",
        }
    }

    pub fn from_call_target(target: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.authored_spelling() == target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum ProductOperandRole {
    BoundarySubject,
    RootGrantSubject,
    Provider,
    Requirement,
    // An ordinary (non-boundary) trait machine requirement: the exact
    // identity a test group registers (wiki/spec/build/testing.md).
    TestRequirement,
    Data,
    Representation,
    WireCompatibilityFact,
    Formal,
    Field,
    /// A domain naming a qualification inside a static Product type argument.
    Domain,
    CheckedMachine,
}

impl BuildOperation {
    pub const fn product_operand_role(self, position: usize) -> Option<ProductOperandRole> {
        use ProductOperandRole as Role;
        match (self, position) {
            (Self::ProviderSelection, 0) | (Self::ServiceExclusion | Self::ServiceAudit, 0) => {
                Some(Role::BoundarySubject)
            }
            (Self::ProviderSelection, 1) => Some(Role::Provider),
            (Self::BoundaryAcceptance, 0) => Some(Role::RootGrantSubject),
            (Self::ParameterDescription, 0) => Some(Role::Formal),
            (Self::FieldDescription, 0) => Some(Role::Field),
            (Self::AssumedBindingContract, 0 | 1) | (Self::NativeBinding, 0) => {
                Some(Role::Requirement)
            }
            (Self::TestGroupRegistration, 0) => Some(Role::TestRequirement),
            (Self::TestGroupRegistration, 1) => Some(Role::CheckedMachine),
            (Self::RepresentationSelection, 0) | (Self::WireCompatibilityRequest, 0..=3) => {
                Some(Role::Data)
            }
            (Self::RepresentationSelection, 1) => Some(Role::Representation),
            (Self::WireCompatibilityRequest, 4..) => Some(Role::WireCompatibilityFact),
            _ => None,
        }
    }

    pub const fn product_operand_count_valid(self, count: usize) -> bool {
        match self {
            Self::ServiceDescription | Self::NativeBinding | Self::ParameterDescription | Self::FieldDescription => count == 1,
            Self::ProviderSelection
            | Self::RepresentationSelection
            | Self::AssumedBindingContract => count == 2,
            Self::ServiceExclusion | Self::ServiceAudit | Self::BoundaryAcceptance => count == 1,
            Self::TestGroupRegistration => matches!(count, 1 | 2),
            Self::WireCompatibilityRequest => count >= 5,
            _ => false,
        }
    }

    // The checked recording call must agree with the authored request.
    // Keep its ordinary operands beside its name so preparation and
    // validation cannot admit a different Product operation's protocol.
    pub const fn product_recording_operation(
        self,
    ) -> Option<(crate::build_protocol::SelectionProtocolOperation, usize)> {
        use crate::build_protocol::SelectionProtocolOperation as Protocol;
        match self {
            Self::TestGroupRegistration => Some((Protocol::RegisterTestGroup, 1)),
            Self::NativeBinding => Some((Protocol::BindNative, 3)),
            Self::ParameterDescription => Some((Protocol::DescribeParameter, 0)),
            Self::FieldDescription => Some((Protocol::DescribeField, 0)),
            Self::ProviderSelection => Some((Protocol::SelectProvider, 3)),
            operation if operation.selects_product_declarations() => Some((Protocol::Select, 1)),
            _ => None,
        }
    }
}
