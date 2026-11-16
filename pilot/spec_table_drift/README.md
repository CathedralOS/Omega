# Spec table drift pilot (throwaway; not for main)

This is a measurement pilot: does a mechanical comparison between `wiki/spec/`
tables and the code that implements them find real drift? The scripts here ran
on origin/main a57e7406 (2026-11-16). Run each one from the checkout root:

    python3 pilot/spec_table_drift/encoding_tags.py .   # encoding.md tag tables vs wire decoders
    python3 pilot/spec_table_drift/rosters.py .         # classicality.md rule roster vs Rust enums
    python3 pilot/spec_table_drift/ocout_codes.py .     # compiler_request.md outcome codes vs ocreq/outcome.omg

`encoding_map.json` is the hand-written map from each `<!-- marker -->` table to
its decoder. `null` means the script could not find a decoder for that table.

Real disagreements (verified by reading the code):
1. `encoding.md:2089` structural-type-shape table omits decoder tags 10, 12-19
   (`structural_wire.rs:2111-2200`). Tags 12 (refined PrimitiveScalar), 16
   (AtomicCell) and 19 (Opaque) appear nowhere on the page.
2. `encoding.md:2108` byte-sequence-carrier table omits tag 3 BoundedView
   (`structural_wire.rs:208`).
3. `encoding.md:2122` structural-field-type table omits tag 9 RefinedScalar and
   tag 10 AtomicBoundedInteger (`structural_wire.rs:329`, `:321`).
4. `encoding.md:2176` canonical-path-segment table omits tag 4 Referent
   (`structural_wire.rs:159`). The summary row at `encoding.md:458` lists it.
5. `encoding.md:2152` structural-path-segment table omits 7 Case,
   8 RepresentationView and 10 AllElements
   (`semantic-vocabulary/src/structural_path_encoding.rs:7-16`). The summary row
   at `encoding.md:457` lists them.
Unresolved: `encoding.md:561` scalar-field-carrier-path table lists tags 1-2,
while the summary row at `:460` lists 1-4.
