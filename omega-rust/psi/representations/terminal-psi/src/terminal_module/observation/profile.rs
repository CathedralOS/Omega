//! Typed static observation profiles reconstructed from Terminal semantics,
//! and the checked construction of the runtime traces they observe.

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IeeeFloatValue, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId,
};

use crate::StructuralPathQualification;
use crate::{
    CrashCause, CrashRouteBucket, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    TerminalPsiIdentity, TrappingIntegerPrimitive,
};

/// The closed consumer-selected observation schema understood by this build.
///
/// `TerminalTraceV1` names the ordered, termination-sensitive trace domain.
/// Its profile row list is versioned separately: revision 2 appended the
/// operation-crash-site group for Trapping primitives, so a revision-1
/// decoder rejects the new shape instead of misreading a later group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalObservationSchema {
    TerminalTraceV1,
}

impl TerminalObservationSchema {
    pub const fn version(self) -> u16 {
        match self {
            Self::TerminalTraceV1 => 2,
        }
    }

    pub const fn from_version(version: u16) -> Option<Self> {
        match version {
            2 => Some(Self::TerminalTraceV1),
            _ => None,
        }
    }
}

/// Version-1 compares semantic values exactly; fingerprints and native bytes
/// are never substitute comparison relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceValueComparison {
    ExactSemanticValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceScalarSchema {
    pub scalar_type: ScalarType,
    pub comparison: TerminalTraceValueComparison,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceStructuralSchema {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: Vec<StructuralDomainId>,
    pub projected_qualifications: Vec<StructuralPathQualification>,
    pub comparison: TerminalTraceValueComparison,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceResultSchema {
    Unit,
    Scalar(TerminalTraceScalarSchema),
    Structural(TerminalTraceStructuralSchema),
}

/// The mandatory nonempty root row of every version-1 profile instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceRootRow {
    pub entry: MachineId,
    pub scalar_inputs: Vec<TerminalTraceScalarSchema>,
    pub structural_inputs: Vec<TerminalTraceStructuralSchema>,
    pub result: TerminalTraceResultSchema,
}

/// One exact semantic crash site. Site coordinates are correspondence
/// coordinates and are not themselves user-visible runtime trace values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceCrashSiteRow {
    pub machine: MachineId,
    pub block: BlockId,
    pub edge: EdgeId,
    pub cause: CrashCause,
}

/// One declared crash route of one exact `BoundaryCall` operation.
///
/// A boundary crash belongs to the invocation, not to a fabricated terminator
/// edge, so its site coordinate is the calling operation. `boundary` and its
/// canonical public `boundary_identity` bind the row to the invoked
/// declaration (the same convention ordinary event rows use); `route` carries
/// that declaration's exact cause bucket. Route guards speak the boundary's
/// scalar-formal telescope — simultaneous actual substitution at the call
/// decides them, never caller value identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceBoundaryCrashSiteRow {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub boundary_identity: String,
    pub route: CrashRouteBucket,
}

/// One Trapping primitive's own crash site.
///
/// A Trapping operation owns its crash: it has neither a terminator edge nor
/// a boundary identity, so its coordinate is the operation itself. `cause` is
/// fixed by the primitive (`Trap`); `primitive`, `result_type` and
/// `operand_type` retain the exact denotation whose trap predicate decides the
/// crash: `operand_type` is the right operand's type for binary arithmetic,
/// the count's type for a shift, and the source type for a conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceOperationCrashSiteRow {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub cause: CrashCause,
    pub primitive: TrappingIntegerPrimitive,
    pub result_type: IntegerType,
    pub operand_type: IntegerType,
}

/// The closed ordinary-event classification carried by TerminalTraceV1.
///
/// Module-local declaration IDs bind the event to its exact Terminal declaration;
/// the adjacent canonical public identity prevents a consumer from having to
/// reinterpret an artifact-local coordinate as the public event identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceOrdinaryEventKind {
    BoundaryCall {
        boundary: BoundaryMachineId,
        boundary_identity: String,
    },
    PortWrite {
        service: ServiceId,
        service_identity: String,
    },
}

/// One statically observable ordinary external-event site.
///
/// The schemas describe the runtime values that a maximal semantic trace will
/// carry. The site coordinates establish module correspondence and are not
/// themselves trace values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceOrdinaryEventRow {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub kind: TerminalTraceOrdinaryEventKind,
    pub scalar_arguments: Vec<TerminalTraceScalarSchema>,
    pub structural_arguments: Vec<TerminalTraceStructuralSchema>,
    pub result: TerminalTraceResultSchema,
}

/// Verifier-derived rows before the canonical codec binds the module identity.
/// This is not an independently reusable observation-profile instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceV1Rows {
    pub root: TerminalTraceRootRow,
    pub crash_sites: Vec<TerminalTraceCrashSiteRow>,
    pub boundary_crash_sites: Vec<TerminalTraceBoundaryCrashSiteRow>,
    pub operation_crash_sites: Vec<TerminalTraceOperationCrashSiteRow>,
    pub ordinary_events: Vec<TerminalTraceOrdinaryEventRow>,
}

/// First bounded `TerminalTraceV1` instance.
///
/// This bounded rung contains the root, the terminator-edge crash-site roster,
/// every declared boundary crash route at its exact call operation, every
/// Trapping primitive's operation-level crash site, and every ordinary
/// `BoundaryCall` and `PortWrite` event. Its canonical codec still
/// includes a zero terminal-external count. The interpreter may consume its
/// exact scalar schemas for the bounded semantic-value comparator and
/// [`Self::begin_trace`] for checked runtime trace construction; profile
/// refinement remains a separate later rung.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalTraceV1Profile {
    pub schema: TerminalObservationSchema,
    pub module_identity: TerminalPsiIdentity,
    pub root: TerminalTraceRootRow,
    pub crash_sites: Vec<TerminalTraceCrashSiteRow>,
    pub boundary_crash_sites: Vec<TerminalTraceBoundaryCrashSiteRow>,
    pub operation_crash_sites: Vec<TerminalTraceOperationCrashSiteRow>,
    pub ordinary_events: Vec<TerminalTraceOrdinaryEventRow>,
}

impl TerminalTraceV1Profile {
    /// Begin checked construction of one runtime trace under this profile.
    pub fn begin_trace(&self) -> TerminalTraceV1TraceBuilder<'_> {
        TerminalTraceV1TraceBuilder {
            profile: self,
            events: Vec::new(),
        }
    }

    fn ordinary_event_row(
        &self,
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    ) -> Option<&TerminalTraceOrdinaryEventRow> {
        self.ordinary_events
            .iter()
            .find(|row| row.machine == machine && row.block == block && row.operation == operation)
    }
}

/// The closed shape shared by a result schema and a result value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceResultKind {
    Unit,
    Scalar,
    Structural,
}

impl TerminalTraceResultSchema {
    pub const fn kind(&self) -> TerminalTraceResultKind {
        match self {
            Self::Unit => TerminalTraceResultKind::Unit,
            Self::Scalar(_) => TerminalTraceResultKind::Scalar,
            Self::Structural(_) => TerminalTraceResultKind::Structural,
        }
    }
}

/// One exact scalar runtime value carried by a `TerminalTraceV1` trace.
///
/// Values are semantic payloads, never fingerprints or native bytes: integers
/// retain their fixed-width carrier and IEEE values retain raw interchange
/// bits, so signed zero and NaN payloads stay distinct under the profile's
/// `ExactSemanticValue` comparison rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    IeeeFloat(IeeeFloatValue),
}

impl TerminalTraceScalarValue {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
        }
    }
}

/// One opaque structural runtime value carried by a `TerminalTraceV1` trace.
///
/// `opaque_identity` is the embedding host's chosen semantic identity,
/// preserved for exact value observation; it is never an address or a layout.
/// `qualifications` are canonical only when strictly increasing. `path` is
/// empty for a whole-root value: V1 schemas compare whole roots, so a nested
/// runtime value cannot yet be admitted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
    pub path: Vec<StructuralPathSegment>,
}

/// One exact runtime result value — the value-carrying counterpart of
/// [`TerminalTraceResultSchema`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceResultValue {
    Unit,
    Scalar(TerminalTraceScalarValue),
    Structural(TerminalTraceStructuralValue),
}

impl TerminalTraceResultValue {
    pub const fn kind(&self) -> TerminalTraceResultKind {
        match self {
            Self::Unit => TerminalTraceResultKind::Unit,
            Self::Scalar(_) => TerminalTraceResultKind::Scalar,
            Self::Structural(_) => TerminalTraceResultKind::Structural,
        }
    }
}

/// One observed ordinary external event of a `TerminalTraceV1` runtime trace.
///
/// The site coordinates establish correspondence with the profile's
/// `ordinary_events` row and are not themselves user-visible trace values.
/// Arguments and the result carry the invocation's exact semantic values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalTraceV1Event {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub scalar_arguments: Vec<TerminalTraceScalarValue>,
    pub structural_arguments: Vec<TerminalTraceStructuralValue>,
    pub result: TerminalTraceResultValue,
}

/// The resolved outcome ending a finite `TerminalTraceV1` runtime trace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceV1Outcome {
    /// Root return carrying the exact semantic value the root result schema
    /// describes; unit return is a value-free outcome distinct from every
    /// other.
    Return(TerminalTraceResultValue),
    /// Terminator-edge crash at a declared crash-site row.
    EdgeCrash {
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
        cause: CrashCause,
    },
    /// A Trapping primitive's trap at its own operation-level crash site.
    OperationCrash {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        cause: CrashCause,
    },
    /// One `BoundaryCall` invocation resolved to a single declared route.
    ///
    /// The call's actual arguments ride with the outcome because a crashing
    /// call produces no ordinary event row; the refinement join substitutes
    /// them into the route's guards.
    BoundaryCrash {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        route: CrashRouteBucket,
        scalar_arguments: Vec<TerminalTraceScalarValue>,
        structural_arguments: Vec<TerminalTraceStructuralValue>,
    },
}

/// One runtime trace bound to a [`TerminalTraceV1Profile`].
///
/// `events` preserve execution order. A `None` outcome is the finite
/// observable prefix of a continuing (possibly divergent) execution: an
/// infinite maximal execution is observed only through such prefixes. The
/// profile's empty terminal-external roster means a V1 trace cannot carry an
/// `ExternalTerminate` outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalTraceV1RuntimeTrace {
    pub schema: TerminalObservationSchema,
    pub module_identity: TerminalPsiIdentity,
    pub events: Vec<TerminalTraceV1Event>,
    pub outcome: Option<TerminalTraceV1Outcome>,
}

impl TerminalTraceV1RuntimeTrace {
    /// Whether a resolved outcome closes this trace; a `false` trace is a
    /// finite observable prefix of a continuing execution.
    pub const fn terminated(&self) -> bool {
        self.outcome.is_some()
    }
}

/// Rejection of one trace-construction step against the bound profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ConstructionError {
    /// No ordinary-event row exists at the event's site coordinates.
    UnknownOrdinaryEventSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
    /// Argument arity disagrees with the site row's schemas.
    ScalarArgumentCount {
        expected: usize,
        actual: usize,
    },
    StructuralArgumentCount {
        expected: usize,
        actual: usize,
    },
    /// An address-carrier schema is not an exact semantic value schema.
    UnsupportedScalarSchema {
        scalar_type: ScalarType,
    },
    ScalarTypeMismatch {
        schema: ScalarType,
        value: ScalarType,
    },
    InvalidIntegerValue {
        scalar_type: IntegerType,
    },
    /// A projected schema qualification cannot yet be validated against a
    /// runtime value.
    UnsupportedProjectedStructuralSchema,
    StructuralTypeMismatch {
        schema: StructuralTypeId,
        value: StructuralTypeId,
    },
    StructuralQualificationsNonCanonical,
    StructuralQualificationMissing {
        domain: StructuralDomainId,
    },
    /// A nested runtime value cannot yet be admitted by a whole-root schema.
    NestedStructuralValue,
    ResultKindMismatch {
        schema: TerminalTraceResultKind,
        value: TerminalTraceResultKind,
    },
    /// No crash-site row exists at the edge.
    UnknownCrashSite {
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
    },
    /// The declared crash site carries a different exact cause.
    CrashCauseMismatch {
        declared: CrashCause,
        actual: CrashCause,
    },
    /// No operation-crash row exists at the operation.
    UnknownOperationCrashSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
    /// No boundary-crash row exists at the call operation.
    NoBoundaryCrashSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
    /// The resolved route is not one of the call's declared routes.
    UndeclaredBoundaryCrashRoute {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
}

impl std::fmt::Display for TerminalTraceV1ConstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalTraceV1ConstructionError {}

/// Checked construction of one `TerminalTraceV1` runtime trace against its
/// reconstructed profile.
///
/// Events are admitted only at declared ordinary-event sites with values
/// conforming to the row's exact schemas, and outcomes bind a declared crash
/// site or the root result schema. Guard substitution and profile refinement
/// are the separate later rung — construction establishes only that the
/// recorded trace is site-exact and schema-exact.
pub struct TerminalTraceV1TraceBuilder<'profile> {
    profile: &'profile TerminalTraceV1Profile,
    events: Vec<TerminalTraceV1Event>,
}

impl<'profile> TerminalTraceV1TraceBuilder<'profile> {
    /// Admit one ordinary external event at a declared site.
    ///
    /// Events must be pushed in execution order; revisiting a site is a new
    /// invocation of it, not a conflict.
    pub fn push_event(
        &mut self,
        event: TerminalTraceV1Event,
    ) -> Result<(), TerminalTraceV1ConstructionError> {
        let row = self
            .profile
            .ordinary_event_row(event.machine, event.block, event.operation)
            .ok_or(TerminalTraceV1ConstructionError::UnknownOrdinaryEventSite {
                machine: event.machine,
                block: event.block,
                operation: event.operation,
            })?;
        validate_event_arguments(
            &row.scalar_arguments,
            &event.scalar_arguments,
            &row.structural_arguments,
            &event.structural_arguments,
        )?;
        validate_result(&row.result, &event.result)?;
        self.events.push(event);
        Ok(())
    }

    /// Close the trace with the root's return outcome.
    pub fn finish_return(
        self,
        result: TerminalTraceResultValue,
    ) -> Result<TerminalTraceV1RuntimeTrace, TerminalTraceV1ConstructionError> {
        validate_result(&self.profile.root.result, &result)?;
        Ok(self.complete(Some(TerminalTraceV1Outcome::Return(result))))
    }

    /// Close the trace at a declared terminator-edge crash site.
    pub fn finish_edge_crash(
        self,
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
        cause: CrashCause,
    ) -> Result<TerminalTraceV1RuntimeTrace, TerminalTraceV1ConstructionError> {
        let row = self
            .profile
            .crash_sites
            .iter()
            .find(|row| row.machine == machine && row.block == block && row.edge == edge)
            .ok_or(TerminalTraceV1ConstructionError::UnknownCrashSite {
                machine,
                block,
                edge,
            })?;
        if row.cause != cause {
            return Err(TerminalTraceV1ConstructionError::CrashCauseMismatch {
                declared: row.cause,
                actual: cause,
            });
        }
        Ok(self.complete(Some(TerminalTraceV1Outcome::EdgeCrash {
            machine,
            block,
            edge,
            cause,
        })))
    }

    /// Close the trace at a declared operation-level crash site.
    pub fn finish_operation_crash(
        self,
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        cause: CrashCause,
    ) -> Result<TerminalTraceV1RuntimeTrace, TerminalTraceV1ConstructionError> {
        let row = self
            .profile
            .operation_crash_sites
            .iter()
            .find(|row| row.machine == machine && row.block == block && row.operation == operation)
            .ok_or(
                TerminalTraceV1ConstructionError::UnknownOperationCrashSite {
                    machine,
                    block,
                    operation,
                },
            )?;
        if row.cause != cause {
            return Err(TerminalTraceV1ConstructionError::CrashCauseMismatch {
                declared: row.cause,
                actual: cause,
            });
        }
        Ok(self.complete(Some(TerminalTraceV1Outcome::OperationCrash {
            machine,
            block,
            operation,
            cause,
        })))
    }

    /// Close the trace by resolving one `BoundaryCall` invocation to a
    /// declared crash route at its call operation.
    ///
    /// The resolved route must equal one of the operation's declared rows
    /// exactly, and the call's actual arguments conform to the operation's
    /// ordinary-event row so the refinement join can substitute them.
    pub fn finish_boundary_crash(
        self,
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        route: CrashRouteBucket,
        scalar_arguments: Vec<TerminalTraceScalarValue>,
        structural_arguments: Vec<TerminalTraceStructuralValue>,
    ) -> Result<TerminalTraceV1RuntimeTrace, TerminalTraceV1ConstructionError> {
        let mut site_rows = self.profile.boundary_crash_sites.iter().filter(|row| {
            row.machine == machine && row.block == block && row.operation == operation
        });
        if site_rows.next().is_none() {
            return Err(TerminalTraceV1ConstructionError::NoBoundaryCrashSite {
                machine,
                block,
                operation,
            });
        }
        if !self.profile.boundary_crash_sites.iter().any(|row| {
            row.machine == machine
                && row.block == block
                && row.operation == operation
                && row.route == route
        }) {
            return Err(
                TerminalTraceV1ConstructionError::UndeclaredBoundaryCrashRoute {
                    machine,
                    block,
                    operation,
                },
            );
        }
        let event_row = self
            .profile
            .ordinary_event_row(machine, block, operation)
            .ok_or(TerminalTraceV1ConstructionError::UnknownOrdinaryEventSite {
                machine,
                block,
                operation,
            })?;
        validate_event_arguments(
            &event_row.scalar_arguments,
            &scalar_arguments,
            &event_row.structural_arguments,
            &structural_arguments,
        )?;
        Ok(self.complete(Some(TerminalTraceV1Outcome::BoundaryCrash {
            machine,
            block,
            operation,
            route,
            scalar_arguments,
            structural_arguments,
        })))
    }

    /// Emit the observed finite prefix of a continuing execution.
    pub fn finish_prefix(self) -> TerminalTraceV1RuntimeTrace {
        self.complete(None)
    }

    fn complete(self, outcome: Option<TerminalTraceV1Outcome>) -> TerminalTraceV1RuntimeTrace {
        TerminalTraceV1RuntimeTrace {
            schema: self.profile.schema,
            module_identity: self.profile.module_identity,
            events: self.events,
            outcome,
        }
    }
}

/// Conformance of one exact scalar runtime value to its verifier-derived
/// schema, mirroring the bounded semantic-value comparator's rules.
fn validate_scalar_value(
    schema: &TerminalTraceScalarSchema,
    value: &TerminalTraceScalarValue,
) -> Result<(), TerminalTraceV1ConstructionError> {
    match schema.comparison {
        TerminalTraceValueComparison::ExactSemanticValue => {}
    }
    if matches!(schema.scalar_type, ScalarType::Integer(integer) if integer.is_address()) {
        return Err(TerminalTraceV1ConstructionError::UnsupportedScalarSchema {
            scalar_type: schema.scalar_type,
        });
    }
    if value.scalar_type() != schema.scalar_type {
        return Err(TerminalTraceV1ConstructionError::ScalarTypeMismatch {
            schema: schema.scalar_type,
            value: value.scalar_type(),
        });
    }
    if let TerminalTraceScalarValue::Integer { scalar_type, value } = *value
        && !scalar_type.admits(value)
    {
        return Err(TerminalTraceV1ConstructionError::InvalidIntegerValue { scalar_type });
    }
    Ok(())
}

/// Conformance of one opaque structural runtime value to its whole-root
/// schema, mirroring the bounded semantic-value comparator's rules.
fn validate_structural_value(
    schema: &TerminalTraceStructuralSchema,
    value: &TerminalTraceStructuralValue,
) -> Result<(), TerminalTraceV1ConstructionError> {
    match schema.comparison {
        TerminalTraceValueComparison::ExactSemanticValue => {}
    }
    if !schema.projected_qualifications.is_empty() {
        return Err(TerminalTraceV1ConstructionError::UnsupportedProjectedStructuralSchema);
    }
    if value.structural_type != schema.structural_type {
        return Err(TerminalTraceV1ConstructionError::StructuralTypeMismatch {
            schema: schema.structural_type,
            value: value.structural_type,
        });
    }
    if value
        .qualifications
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(TerminalTraceV1ConstructionError::StructuralQualificationsNonCanonical);
    }
    if let Some(domain) = schema
        .qualifications
        .iter()
        .find(|domain| !value.qualifications.contains(domain))
    {
        return Err(
            TerminalTraceV1ConstructionError::StructuralQualificationMissing { domain: *domain },
        );
    }
    if !value.path.is_empty() {
        return Err(TerminalTraceV1ConstructionError::NestedStructuralValue);
    }
    Ok(())
}

fn validate_result(
    schema: &TerminalTraceResultSchema,
    value: &TerminalTraceResultValue,
) -> Result<(), TerminalTraceV1ConstructionError> {
    if schema.kind() != value.kind() {
        return Err(TerminalTraceV1ConstructionError::ResultKindMismatch {
            schema: schema.kind(),
            value: value.kind(),
        });
    }
    match (schema, value) {
        (TerminalTraceResultSchema::Unit, TerminalTraceResultValue::Unit) => Ok(()),
        (TerminalTraceResultSchema::Scalar(schema), TerminalTraceResultValue::Scalar(value)) => {
            validate_scalar_value(schema, value)
        }
        (
            TerminalTraceResultSchema::Structural(schema),
            TerminalTraceResultValue::Structural(value),
        ) => validate_structural_value(schema, value),
        _ => unreachable!("equal result kinds pair the same variants"),
    }
}

fn validate_event_arguments(
    scalar_schemas: &[TerminalTraceScalarSchema],
    scalar_arguments: &[TerminalTraceScalarValue],
    structural_schemas: &[TerminalTraceStructuralSchema],
    structural_arguments: &[TerminalTraceStructuralValue],
) -> Result<(), TerminalTraceV1ConstructionError> {
    if scalar_arguments.len() != scalar_schemas.len() {
        return Err(TerminalTraceV1ConstructionError::ScalarArgumentCount {
            expected: scalar_schemas.len(),
            actual: scalar_arguments.len(),
        });
    }
    if structural_arguments.len() != structural_schemas.len() {
        return Err(TerminalTraceV1ConstructionError::StructuralArgumentCount {
            expected: structural_schemas.len(),
            actual: structural_arguments.len(),
        });
    }
    for (schema, value) in scalar_schemas.iter().zip(scalar_arguments) {
        validate_scalar_value(schema, value)?;
    }
    for (schema, value) in structural_schemas.iter().zip(structural_arguments) {
        validate_structural_value(schema, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BlockId, BoundaryMachineId, CrashCause, CrashRouteBucket, EdgeId, IntegerType,
        IntegerValue, MachineId, OperationId, ScalarType, ServiceId, StructuralAccess,
        StructuralDomainId, StructuralMultiplicity, StructuralPathSegment, StructuralTypeId,
        TerminalObservationSchema, TerminalPsiIdentity, TerminalTraceBoundaryCrashSiteRow,
        TerminalTraceCrashSiteRow, TerminalTraceOperationCrashSiteRow,
        TerminalTraceOrdinaryEventKind, TerminalTraceOrdinaryEventRow, TerminalTraceResultKind,
        TerminalTraceResultSchema, TerminalTraceResultValue, TerminalTraceRootRow,
        TerminalTraceScalarSchema, TerminalTraceScalarValue, TerminalTraceStructuralSchema,
        TerminalTraceStructuralValue, TerminalTraceV1ConstructionError, TerminalTraceV1Event,
        TerminalTraceV1Outcome, TerminalTraceV1Profile, TerminalTraceValueComparison,
        TrappingIntegerPrimitive,
    };
    use crate::{CrashRouteGuard, SemanticFingerprint, VocabularyMarker};
    use semantic_vocabulary::IntegerSign;

    fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero test identity")
    }

    fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("fixed integer type")
    }

    fn unsigned(bits: u16, value: u128) -> TerminalTraceScalarValue {
        TerminalTraceScalarValue::Integer {
            scalar_type: integer_type(IntegerSign::Unsigned, bits),
            value: IntegerValue::Unsigned(value),
        }
    }

    fn u16_schema() -> TerminalTraceScalarSchema {
        TerminalTraceScalarSchema {
            scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 16)),
            comparison: TerminalTraceValueComparison::ExactSemanticValue,
        }
    }

    fn structural_type(raw: u64) -> StructuralTypeId {
        id(raw, StructuralTypeId::new)
    }

    fn structural_domain(raw: u64) -> StructuralDomainId {
        id(raw, StructuralDomainId::new)
    }

    fn structural_schema(
        structural_type: StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> TerminalTraceStructuralSchema {
        TerminalTraceStructuralSchema {
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications,
            projected_qualifications: Vec::new(),
            comparison: TerminalTraceValueComparison::ExactSemanticValue,
        }
    }

    fn structural_value(
        opaque_identity: u64,
        structural_type: StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> TerminalTraceStructuralValue {
        TerminalTraceStructuralValue {
            opaque_identity,
            structural_type,
            qualifications,
            path: Vec::new(),
        }
    }

    fn crash_route(cause: CrashCause) -> CrashRouteBucket {
        CrashRouteBucket {
            cause,
            alternatives: vec![CrashRouteGuard::Truth],
        }
    }

    const MACHINE: u64 = 1;
    const BLOCK: u64 = 1;
    const CALL: u64 = 1;
    const WRITE: u64 = 2;
    const CRASH_EDGE: u64 = 7;
    const TRAPPING_ADD: u64 = 3;

    fn profile() -> TerminalTraceV1Profile {
        let machine = id(MACHINE, MachineId::new);
        let block = id(BLOCK, BlockId::new);
        let boundary = id(1, BoundaryMachineId::new);
        TerminalTraceV1Profile {
            schema: TerminalObservationSchema::TerminalTraceV1,
            module_identity: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
            },
            root: TerminalTraceRootRow {
                entry: machine,
                scalar_inputs: vec![u16_schema()],
                structural_inputs: Vec::new(),
                result: TerminalTraceResultSchema::Scalar(u16_schema()),
            },
            crash_sites: vec![TerminalTraceCrashSiteRow {
                machine,
                block,
                edge: id(CRASH_EDGE, EdgeId::new),
                cause: CrashCause::Trap,
            }],
            boundary_crash_sites: vec![TerminalTraceBoundaryCrashSiteRow {
                machine,
                block,
                operation: id(CALL, OperationId::new),
                boundary,
                boundary_identity: "pkg::Port".into(),
                route: crash_route(CrashCause::Abort),
            }],
            operation_crash_sites: vec![TerminalTraceOperationCrashSiteRow {
                machine,
                block,
                operation: id(TRAPPING_ADD, OperationId::new),
                cause: CrashCause::Trap,
                primitive: TrappingIntegerPrimitive::Add,
                result_type: integer_type(IntegerSign::Unsigned, 8),
                operand_type: integer_type(IntegerSign::Unsigned, 8),
            }],
            ordinary_events: vec![
                TerminalTraceOrdinaryEventRow {
                    machine,
                    block,
                    operation: id(CALL, OperationId::new),
                    kind: TerminalTraceOrdinaryEventKind::BoundaryCall {
                        boundary,
                        boundary_identity: "pkg::Port".into(),
                    },
                    scalar_arguments: vec![u16_schema()],
                    structural_arguments: Vec::new(),
                    result: TerminalTraceResultSchema::Unit,
                },
                TerminalTraceOrdinaryEventRow {
                    machine,
                    block,
                    operation: id(WRITE, OperationId::new),
                    kind: TerminalTraceOrdinaryEventKind::PortWrite {
                        service: id(1, ServiceId::new),
                        service_identity: "pkg::port".into(),
                    },
                    scalar_arguments: vec![
                        TerminalTraceScalarSchema {
                            scalar_type: ScalarType::Integer(integer_type(
                                IntegerSign::Unsigned,
                                16,
                            )),
                            comparison: TerminalTraceValueComparison::ExactSemanticValue,
                        },
                        TerminalTraceScalarSchema {
                            scalar_type: ScalarType::Integer(integer_type(
                                IntegerSign::Unsigned,
                                8,
                            )),
                            comparison: TerminalTraceValueComparison::ExactSemanticValue,
                        },
                    ],
                    structural_arguments: Vec::new(),
                    result: TerminalTraceResultSchema::Unit,
                },
            ],
        }
    }

    fn boundary_call_event(argument: TerminalTraceScalarValue) -> TerminalTraceV1Event {
        TerminalTraceV1Event {
            machine: id(MACHINE, MachineId::new),
            block: id(BLOCK, BlockId::new),
            operation: id(CALL, OperationId::new),
            scalar_arguments: vec![argument],
            structural_arguments: Vec::new(),
            result: TerminalTraceResultValue::Unit,
        }
    }

    fn port_write_event(port: u128, byte: u128) -> TerminalTraceV1Event {
        TerminalTraceV1Event {
            machine: id(MACHINE, MachineId::new),
            block: id(BLOCK, BlockId::new),
            operation: id(WRITE, OperationId::new),
            scalar_arguments: vec![unsigned(16, port), unsigned(8, byte)],
            structural_arguments: Vec::new(),
            result: TerminalTraceResultValue::Unit,
        }
    }

    #[test]
    fn runtime_trace_records_events_in_order_and_resolves_return() {
        let profile = profile();
        let mut builder = profile.begin_trace();
        builder
            .push_event(port_write_event(3, 65))
            .expect("declared site with conforming values");
        builder
            .push_event(boundary_call_event(unsigned(16, 7)))
            .expect("revisited site is a new invocation");
        builder
            .push_event(boundary_call_event(unsigned(16, 8)))
            .expect("revisited site is a new invocation");

        let trace = builder
            .finish_return(TerminalTraceResultValue::Scalar(unsigned(16, 11)))
            .expect("root scalar result conforms");

        assert!(trace.terminated());
        assert_eq!(trace.schema, profile.schema);
        assert_eq!(trace.module_identity, profile.module_identity);
        assert_eq!(trace.events.len(), 3);
        assert_eq!(
            trace.events[0].scalar_arguments,
            vec![unsigned(16, 3), unsigned(8, 65)]
        );
        assert_eq!(
            trace.outcome,
            Some(TerminalTraceV1Outcome::Return(
                TerminalTraceResultValue::Scalar(unsigned(16, 11))
            ))
        );
    }

    #[test]
    fn runtime_trace_prefix_marks_a_continuing_execution() {
        let profile = profile();
        let mut builder = profile.begin_trace();
        builder
            .push_event(port_write_event(1, 0))
            .expect("declared site");

        let trace = builder.finish_prefix();
        assert!(!trace.terminated());
        assert_eq!(trace.outcome, None);
        assert_eq!(trace.events.len(), 1);
    }

    #[test]
    fn event_at_an_undeclared_site_rejects() {
        let profile = profile();
        let mut builder = profile.begin_trace();
        let mut event = boundary_call_event(unsigned(16, 7));
        event.operation = id(99, OperationId::new);
        assert_eq!(
            builder.push_event(event),
            Err(TerminalTraceV1ConstructionError::UnknownOrdinaryEventSite {
                machine: id(MACHINE, MachineId::new),
                block: id(BLOCK, BlockId::new),
                operation: id(99, OperationId::new),
            }),
        );
    }

    #[test]
    fn event_arguments_must_match_site_schemas() {
        let profile = profile();
        let mut builder = profile.begin_trace();

        let mut short = port_write_event(1, 2);
        short.scalar_arguments.pop();
        assert_eq!(
            builder.push_event(short),
            Err(TerminalTraceV1ConstructionError::ScalarArgumentCount {
                expected: 2,
                actual: 1,
            }),
        );

        assert_eq!(
            builder.push_event(boundary_call_event(TerminalTraceScalarValue::Boolean(true))),
            Err(TerminalTraceV1ConstructionError::ScalarTypeMismatch {
                schema: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 16)),
                value: ScalarType::Boolean,
            }),
        );

        assert_eq!(
            builder.push_event(boundary_call_event(unsigned(16, 0x1_0000))),
            Err(TerminalTraceV1ConstructionError::InvalidIntegerValue {
                scalar_type: integer_type(IntegerSign::Unsigned, 16),
            }),
        );
    }

    #[test]
    fn event_rejects_address_carrier_and_result_kind_drift() {
        let mut address_profile = profile();
        let address = IntegerType::address(64).expect("address carrier type");
        address_profile.ordinary_events[0].scalar_arguments = vec![TerminalTraceScalarSchema {
            scalar_type: ScalarType::Integer(address),
            comparison: TerminalTraceValueComparison::ExactSemanticValue,
        }];
        let mut builder = address_profile.begin_trace();
        assert_eq!(
            builder.push_event(boundary_call_event(TerminalTraceScalarValue::Integer {
                scalar_type: address,
                value: IntegerValue::Unsigned(0),
            })),
            Err(TerminalTraceV1ConstructionError::UnsupportedScalarSchema {
                scalar_type: ScalarType::Integer(address),
            }),
        );

        let profile = profile();
        let mut builder = profile.begin_trace();
        let mut event = port_write_event(1, 2);
        event.result = TerminalTraceResultValue::Scalar(unsigned(8, 0));
        assert_eq!(
            builder.push_event(event),
            Err(TerminalTraceV1ConstructionError::ResultKindMismatch {
                schema: TerminalTraceResultKind::Unit,
                value: TerminalTraceResultKind::Scalar,
            }),
        );
    }

    #[test]
    fn structural_arguments_follow_whole_root_schema_rules() {
        let mut profile = profile();
        let value_type = structural_type(1);
        let domain = structural_domain(1);
        profile.ordinary_events[0].structural_arguments =
            vec![structural_schema(value_type, vec![domain])];
        let profile = profile;

        let mut event = boundary_call_event(unsigned(16, 7));
        event
            .structural_arguments
            .push(structural_value(1, value_type, vec![domain]));

        let mut builder = profile.begin_trace();
        let mut nested = event.clone();
        nested.structural_arguments[0]
            .path
            .push(StructuralPathSegment::FixedIndex(0));
        assert_eq!(
            builder.push_event(nested),
            Err(TerminalTraceV1ConstructionError::NestedStructuralValue),
        );

        let mut unordered = event.clone();
        unordered.structural_arguments[0].qualifications =
            vec![structural_domain(2), structural_domain(1)];
        assert_eq!(
            builder.push_event(unordered),
            Err(TerminalTraceV1ConstructionError::StructuralQualificationsNonCanonical),
        );

        let mut missing = event.clone();
        missing.structural_arguments[0].qualifications = Vec::new();
        assert_eq!(
            builder.push_event(missing),
            Err(TerminalTraceV1ConstructionError::StructuralQualificationMissing { domain }),
        );

        builder
            .push_event(event)
            .expect("whole-root value conforms");
    }

    #[test]
    fn projected_structural_schemas_stay_fenced() {
        let mut profile = profile();
        let value_type = structural_type(1);
        let domain = structural_domain(1);
        let mut schema = structural_schema(value_type, vec![domain]);
        schema
            .projected_qualifications
            .push(crate::StructuralPathQualification {
                path: vec![StructuralPathSegment::Field("item".into())],
                domain,
            });
        profile.ordinary_events[0].structural_arguments = vec![schema];
        let profile = profile;

        let mut event = boundary_call_event(unsigned(16, 7));
        event
            .structural_arguments
            .push(structural_value(1, value_type, vec![domain]));

        let mut builder = profile.begin_trace();
        assert_eq!(
            builder.push_event(event),
            Err(TerminalTraceV1ConstructionError::UnsupportedProjectedStructuralSchema),
        );
    }

    #[test]
    fn edge_crash_binds_a_declared_site_and_cause() {
        let profile = profile();
        let machine = id(MACHINE, MachineId::new);
        let block = id(BLOCK, BlockId::new);
        let edge = id(CRASH_EDGE, EdgeId::new);

        let trace = profile
            .begin_trace()
            .finish_edge_crash(machine, block, edge, CrashCause::Trap)
            .expect("declared crash site");
        assert_eq!(
            trace.outcome,
            Some(TerminalTraceV1Outcome::EdgeCrash {
                machine,
                block,
                edge,
                cause: CrashCause::Trap,
            })
        );

        assert_eq!(
            profile.begin_trace().finish_edge_crash(
                machine,
                block,
                id(9, EdgeId::new),
                CrashCause::Trap
            ),
            Err(TerminalTraceV1ConstructionError::UnknownCrashSite {
                machine,
                block,
                edge: id(9, EdgeId::new),
            }),
        );
        assert_eq!(
            profile
                .begin_trace()
                .finish_edge_crash(machine, block, edge, CrashCause::Abort),
            Err(TerminalTraceV1ConstructionError::CrashCauseMismatch {
                declared: CrashCause::Trap,
                actual: CrashCause::Abort,
            }),
        );
    }

    #[test]
    fn operation_crash_binds_its_own_operation_site_and_cause() {
        let profile = profile();
        let machine = id(MACHINE, MachineId::new);
        let block = id(BLOCK, BlockId::new);
        let add = id(TRAPPING_ADD, OperationId::new);

        let trace = profile
            .begin_trace()
            .finish_operation_crash(machine, block, add, CrashCause::Trap)
            .expect("declared operation crash site");
        assert_eq!(
            trace.outcome,
            Some(TerminalTraceV1Outcome::OperationCrash {
                machine,
                block,
                operation: add,
                cause: CrashCause::Trap,
            })
        );
        // A boundary call operation is not an operation crash site, and an
        // edge crash cannot be redirected onto the Trapping operation.
        let call = id(CALL, OperationId::new);
        assert_eq!(
            profile
                .begin_trace()
                .finish_operation_crash(machine, block, call, CrashCause::Trap),
            Err(
                TerminalTraceV1ConstructionError::UnknownOperationCrashSite {
                    machine,
                    block,
                    operation: call,
                }
            ),
        );
        assert_eq!(
            profile
                .begin_trace()
                .finish_operation_crash(machine, block, add, CrashCause::Abort),
            Err(TerminalTraceV1ConstructionError::CrashCauseMismatch {
                declared: CrashCause::Trap,
                actual: CrashCause::Abort,
            }),
        );
    }

    #[test]
    fn boundary_crash_resolves_a_declared_route_with_call_actuals() {
        let profile = profile();
        let machine = id(MACHINE, MachineId::new);
        let block = id(BLOCK, BlockId::new);
        let call = id(CALL, OperationId::new);

        let trace = profile
            .begin_trace()
            .finish_boundary_crash(
                machine,
                block,
                call,
                crash_route(CrashCause::Abort),
                vec![unsigned(16, 5)],
                Vec::new(),
            )
            .expect("declared route with conforming actuals");
        assert_eq!(
            trace.outcome,
            Some(TerminalTraceV1Outcome::BoundaryCrash {
                machine,
                block,
                operation: call,
                route: crash_route(CrashCause::Abort),
                scalar_arguments: vec![unsigned(16, 5)],
                structural_arguments: Vec::new(),
            })
        );

        assert_eq!(
            profile.begin_trace().finish_boundary_crash(
                machine,
                block,
                id(WRITE, OperationId::new),
                crash_route(CrashCause::Abort),
                Vec::new(),
                Vec::new(),
            ),
            Err(TerminalTraceV1ConstructionError::NoBoundaryCrashSite {
                machine,
                block,
                operation: id(WRITE, OperationId::new),
            }),
        );
        assert_eq!(
            profile.begin_trace().finish_boundary_crash(
                machine,
                block,
                call,
                crash_route(CrashCause::Trap),
                vec![unsigned(16, 5)],
                Vec::new(),
            ),
            Err(
                TerminalTraceV1ConstructionError::UndeclaredBoundaryCrashRoute {
                    machine,
                    block,
                    operation: call,
                }
            ),
        );
        assert_eq!(
            profile.begin_trace().finish_boundary_crash(
                machine,
                block,
                call,
                crash_route(CrashCause::Abort),
                vec![TerminalTraceScalarValue::Boolean(true)],
                Vec::new(),
            ),
            Err(TerminalTraceV1ConstructionError::ScalarTypeMismatch {
                schema: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 16)),
                value: ScalarType::Boolean,
            }),
        );
    }

    #[test]
    fn return_outcome_must_conform_to_the_root_result_schema() {
        let profile = profile();
        assert_eq!(
            profile
                .begin_trace()
                .finish_return(TerminalTraceResultValue::Unit),
            Err(TerminalTraceV1ConstructionError::ResultKindMismatch {
                schema: TerminalTraceResultKind::Scalar,
                value: TerminalTraceResultKind::Unit,
            }),
        );
        assert_eq!(
            profile
                .begin_trace()
                .finish_return(TerminalTraceResultValue::Scalar(unsigned(8, 3))),
            Err(TerminalTraceV1ConstructionError::ScalarTypeMismatch {
                schema: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 16)),
                value: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 8)),
            }),
        );
    }
}
