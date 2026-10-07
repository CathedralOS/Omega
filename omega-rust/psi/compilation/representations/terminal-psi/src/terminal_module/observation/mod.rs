//! Static observer profiles, distinct from execution traces.

mod profile;

pub use profile::{
    TerminalObservationSchema, TerminalOperationCrashDenotation, TerminalTraceBoundaryCrashSiteRow,
    TerminalTraceCrashSiteRow, TerminalTraceExternalCompletionSiteRow,
    TerminalTraceOperationCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceOrdinaryEventRow, TerminalTraceResultKind, TerminalTraceResultSchema,
    TerminalTraceResultValue, TerminalTraceRootRow, TerminalTraceScalarSchema,
    TerminalTraceScalarValue, TerminalTraceStructuralSchema, TerminalTraceStructuralValue,
    TerminalTraceV1ConstructionError, TerminalTraceV1Event, TerminalTraceV1Outcome,
    TerminalTraceV1Profile, TerminalTraceV1Rows, TerminalTraceV1RuntimeTrace,
    TerminalTraceV1TraceBuilder, TerminalTraceValueComparison,
};
