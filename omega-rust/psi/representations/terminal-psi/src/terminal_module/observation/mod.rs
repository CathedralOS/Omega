//! Static observer profiles, distinct from execution traces.

mod profile;

pub use profile::{
    TerminalObservationSchema, TerminalTraceBoundaryCrashSiteRow, TerminalTraceCrashSiteRow,
    TerminalTraceOrdinaryEventKind, TerminalTraceOrdinaryEventRow, TerminalTraceResultKind,
    TerminalTraceResultSchema, TerminalTraceResultValue, TerminalTraceRootRow,
    TerminalTraceScalarSchema, TerminalTraceScalarValue, TerminalTraceStructuralSchema,
    TerminalTraceStructuralValue, TerminalTraceV1ConstructionError, TerminalTraceV1Event,
    TerminalTraceV1Outcome, TerminalTraceV1Profile, TerminalTraceV1Rows,
    TerminalTraceV1RuntimeTrace, TerminalTraceV1TraceBuilder, TerminalTraceValueComparison,
};
