# Selected body ready for allocation

`SelectedBody` is the issued stage 06-to-07 handoff. It owns the selected
program, target register environment, optimization settings and rewrite history,
plus the completed liveness and live-range records. A rewrite may also publish
legality for the register views it used.

The records contain shared plans, receipts and per-function counts. Publishing
moves those records without traversing their plans. There are no callbacks,
analysis procedures or replay methods in this representation.

Only selected optimization may link `issuer/` and issue a body. Allocation
consumes the body and decides its required register availability. Its analysis
implementation reuses published legality when that availability matches, or
derives legality from the published ranges when it differs. The native driver
keeps optional producer replay evidence outside this product.

Start at [`src/lib.rs`](src/lib.rs). [`analysis_records.rs`](src/analysis_records.rs)
defines completed analysis metadata; [`history.rs`](src/history.rs) records
which rewrites produced the selected program.
