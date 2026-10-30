//! Ordinary native-input vocabulary. Field and variant coverage is exhaustive.

use super::record_fields;

record_fields!(effects::AssumedBindingContractSelection { required, supplied });

record_fields!(effects::AuthoredBehaviorExclusion {
    kind,
    selecting_machine,
    source_span
});

record_variants!(effects::AuthoredBehaviorExclusionKind {
    0 => CrashCause { cause, case_symbol },
    1 => PhysicalAuthorityClass { class, case_symbol },
    2 => Service { identity, name },
});

record_variants!(effects::CompilerIntrinsicExecutionIdentity {
    0 => HostedExitProcessI32,
    1 => HostedWriteByteI32,
    2 => HostedReadByte,
    3 => ProgramImageRetainedSuffixStart,
    4 => ProgramStorageEntry,
    5 => PlacementLifecycle(value0),
    6 => PlacedScalarAccess { contract, placement_commitment, access_slot, operation },
    7 => BuiltinFunction(value0),
    8 => PrimitiveFloatBinary { operation, format },
    9 => PrimitiveIntegerComparison { operation, integer_type },
    10 => NamedFloatNegation(value0),
    11 => NamedFloatConversion { source, target, domain },
});

record_variants!(effects::CompilerNumericType {
    0 => I8,
    1 => I16,
    2 => I32,
    3 => I64,
    4 => U8,
    5 => U16,
    6 => U32,
    7 => U64,
    8 => F32,
    9 => F64,
});

record_variants!(effects::CompilerPlacedScalarOperation {
    0 => Read,
    1 => Take,
    2 => Write,
});

record_variants!(effects::CompilerPrimitiveFloatBinaryOperation {
    0 => Add,
    1 => Subtract,
    2 => Multiply,
    3 => Divide,
    4 => Equal,
    5 => NotEqual,
    6 => Less,
    7 => LessOrEqual,
    8 => Greater,
    9 => GreaterOrEqual,
});

record_variants!(effects::CompilerPrimitiveIntegerComparisonOperation {
    0 => Equal,
    1 => NotEqual,
    2 => Less,
    3 => LessOrEqual,
    4 => Greater,
    5 => GreaterOrEqual,
});

record_variants!(backend_plan::DirectBoundaryExecution {
    0 => Foreign(value0),
    1 => Intrinsic(value0),
});

record_fields!(backend_plan::DirectBoundaryLeaf {
    declaration_identity,
    callable_identity,
    semantic_plan,
    plan,
    execution
});

record_fields!(effects::InstallationReachResolution {
    requirement_identity,
    provider_plan_report_identity,
    upper_bound,
    resolved_row
});

record_variants!(effects::provider_plan::ProviderBinding {
    0 => Import { evaluated },
    1 => Syscall { number },
    2 => CompilerIntrinsic { machine },
    3 => VtableSlot { index },
    4 => VtableField { table, field },
    5 => TableFunction { table, field },
    6 => CheckedAdapter { machine_identity, machine_package_identity },
});

record_fields!(terminal_installation::ProviderInstallationSelection {
    boundary,
    provider_identity,
    provider_declaration,
    candidate
});

record_fields!(effects::provider_plan::ProviderPlan {
    provider_application,
    name,
    provider_type,
    provider_type_package_identity,
    target,
    schema,
    identities,
    rows,
    origin_package_identity,
    origin_package,
    digest_memo
});

record_fields!(effects::provider_plan::ProviderPlanRow {
    method,
    requirement_identity,
    requirement_lifetime_partition,
    admitted_same_stack_bytes,
    binding
});

record_fields!(effects::SelectedExternalRequirementApplication {
    provider_plan_report_identity,
    provider_plan_digest,
    selected_requirement_identity,
    requirement_identity
});

record_fields!(effects::SelectedRequirementSpecialization {
    provider_plan_report_identity,
    selected_requirement_identity,
    selected_machine_identity,
    requirement_identity,
    machine_identity
});

record_variants!(effects::provider_plan::ServiceEntryAuthorityFlow {
    0 => Accepts,
});

record_fields!(effects::provider_plan::ServiceEntryClaim {
    parameter_index,
    carrier_identity,
    domain,
    predicate_presence,
    effective_carry,
    authority_flow
});

record_fields!(effects::provider_plan::ServiceMethod {
    completion,
    name,
    requirement_owner,
    requirement_owner_package_identity,
    requirement_identity,
    parameter_count,
    parameter_type_identities,
    entry_claims,
    has_result,
    result_type_identity,
    result_claims,
    service_reach,
    synchronous_invocations,
    may_suspend,
    may_block,
    terminates_guarantee,
    termination_premises,
    calling_plan_report_fingerprint,
    calling_plan_commitment
});

record_fields!(effects::provider_plan::ServiceProgressEstablishmentRoute {
    kind,
    requirement_identity
});

record_variants!(effects::provider_plan::ServiceProgressEstablishmentRouteKind {
    0 => CheckedRequirement,
    1 => BoundaryRequirement,
});

record_fields!(effects::provider_plan::ServiceProgressPremise {
    profile,
    subject,
    subject_projections,
    establishment_routes
});

record_variants!(effects::provider_plan::ServiceProgressSubject {
    0 => Parameter(value0),
});

record_fields!(effects::provider_plan::ServiceReceiverApplication {
    application,
    trait_name,
    trait_package_identity,
    argument_identities,
    lifetime_argument_identities,
    requirement_identities
});

record_fields!(effects::provider_plan::ServiceResultClaim {
    domain,
    effective_carry
});

record_fields!(effects::provider_plan::ServiceSchema {
    trait_name,
    identities,
    trait_package_identity,
    receiver_applications,
    methods
});

record_variants!(effects::TerminalAuthorityClass {
    0 => FilesystemContentRead,
    1 => FilesystemContentWrite,
    2 => FilesystemMetadataQuery,
    3 => DirectoryEnumeration,
    4 => FilesystemNamespaceMutation,
    5 => FilesystemMetadataMutation,
    6 => ProcessOutput,
    7 => ProcessTermination,
    8 => MachineControl,
    9 => PortIo,
    10 => InterruptControl,
    11 => InterruptEntry,
    12 => RootMemoryAccess,
    13 => ProcessInput,
});

record_fields!(backend_plan::TerminalTargetProviderPlan {
    plan,
    provider_declaration,
    row_numeric_realizations
});

record_fields!(backend_plan::TerminalTargetProviderProposal {
    target,
    terminal_psi,
    optimization_selection,
    selection
});

record_fields!(backend_plan::TerminalTargetProviderSelection {
    plans,
    assumed_bindings,
    checked_candidates
});
