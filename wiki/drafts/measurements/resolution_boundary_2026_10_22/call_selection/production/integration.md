The demand-comparison producer is frozen at ffb25ea905eab9206527d93f57d5a06f7a4a309a
(implementation over ec7f75640e2e0eed5d6558889ef487ca7f4e2d1f plus the recorded hook).
Its 17 exact selection comparisons and 18 full/project checking outcomes are
historical observations, not relabeled as current-main captures.

The next production producer integrates ed491c85f11ea56f35ce7b525bef760e1457073c.
The intervening Rust change is confined to checking's write-frame projection,
collection-content overlap and ranking preservation. It does not change the
resolver, call-root inventory, lexical symbol assignment, declaration identities,
conformance/target routing or selected-placement consumer. The added library
module is core/iteration.omg, separately imported; these witnesses do not import
it. No source-scope waiver is used for production: the new executable is built
from the integrated tree and the complete compiler/library closure is captured
by pinned qualification. Production acceptance compares actual native image
hashes, native/Terminal execution and diagnostics with the frozen full-resolver
baseline. The retained demand comparison remains bound to its exact producer.

The first integrated replay driver compared instrumented stderr without removing
the CALLRESULT diagnostic record. That harness assertion failed after capture;
no compiler comparison failed. The corrected driver excludes only those prefixed
instrumentation lines and compares every remaining diagnostic byte. The original
raw observations are retained unchanged.
