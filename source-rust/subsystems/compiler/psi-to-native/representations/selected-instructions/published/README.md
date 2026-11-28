# Selected body ready for allocation

`SelectedBody` is the issued stage 06-to-07 handoff. It pairs body publication
metadata with completed selected-program facts. Only selected optimization may
issue it; register allocation consumes it.

The `facts/` package holds selected instructions, the target register environment,
optimization settings, rewrite history, and completed liveness and live-range
records. A rewrite may also publish legality for the register views it used.
The shared analysis implementation reads this package without importing body
metadata or the earlier program forms retained for explicit verification.

Publishing moves shared plans, receipts and per-function counts without walking
them. Neither package contains analysis procedures, callbacks or replay methods.
Allocation chooses its required register availability and asks the analysis
implementation to reuse matching legality or derive it from the issued ranges
when absent or different. The native driver keeps optional producer replay
evidence outside this product.

Start at [`src/lib.rs`](src/lib.rs) for the body handoff and
[`facts/src/lib.rs`](facts/src/lib.rs) for its analysis input.
[`facts/src/analysis_records.rs`](facts/src/analysis_records.rs) defines completed
analysis metadata; [`facts/src/history.rs`](facts/src/history.rs) records which
rewrites produced the selected program. Both constructors require the capability
in `issuer/`; dependency rules reserve its use to the producer and these records.
