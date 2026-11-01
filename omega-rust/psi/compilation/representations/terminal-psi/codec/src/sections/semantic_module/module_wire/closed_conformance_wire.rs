//! Closed conformance applications on the wire: the telescope, subject and
//! trait identities, realization callables, rows and commitment of one
//! application.

use super::super::wire_fields::{
    Bytes, Commitment, Digest, Id, Index, List, Nominal, Optional, ReachArgument, Scalar, Strings,
    Text, U32, U64, wire_enum, wire_record,
};
use super::target_family_wire::Callable;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, ClosedConformanceSchema,
    ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
};

wire_record! {
    Application: ClosedConformanceApplication {
        owner: Id,
        declaration_identity: Index,
        telescope: List<Binding>,
        trait_identity: Index,
        trait_lifetime_arguments: Strings,
        trait_arguments: Strings,
        realization_callables: List<RealizationCallable>,
        schemas: List<Schema>,
        static_applications: List<StaticApplication>,
        rows: List<Row>,
        report_fingerprint: U64,
        commitment: Commitment,
    }
    Binding: ClosedConformanceParameterBinding { parameter: Text, kind: ParameterKind, argument: Text }
    RealizationCallable: ClosedConformanceRealizationCallable {
        source_callable_identity: Index,
        callable: Callable,
        result: CallableResult,
    }
    Schema: ClosedConformanceSchema {
        requirement_declaration: Nominal,
        declaring_trait_identity: Index,
        public_requirement_identity: Index,
        requirement_identity: Index,
        realization_identity: Index,
        definition: Bytes,
        kind: SchemaKind,
        lifetime_parameter_count: U32,
        parameter_kinds: Bytes,
        finite_tuples: Optional<List<List<Index>>>,
    }
    StaticApplication: ClosedConformanceStaticApplication {
        public_requirement_identity: Index,
        realization_identity: Index,
        source_callable_identity: Index,
        callable: Callable,
        application_commitment: Optional<Digest>,
        method_arguments: List<ReachArgument>,
    }
    Row: ClosedConformanceRow {
        requirement_declaration: Nominal,
        declaring_trait_identity: Index,
        public_requirement_identity: Index,
        family_tuple: List<Index>,
        requirement_identity: Index,
        realization_identity: Index,
        realization_callable_identity: Optional<Index>,
    }
}

wire_enum! {
    ParameterKind: ClosedConformanceParameterKind, "ClosedConformanceParameterKind" {
        1 => Lifetime,
        2 => Type,
        3 => Const,
        4 => Machine,
    }
    CallableResult: ClosedConformanceCallableResult, "ClosedConformanceCallableResult" {
        1 => Unit,
        2 => Scalar(scalar: Scalar),
        3 => Structural(identity: Id),
    }
    SchemaKind: ClosedConformanceSchemaKind, "ClosedConformanceSchemaKind" {
        1 => Nongeneric,
        2 => Finite,
        3 => Open,
    }
}
