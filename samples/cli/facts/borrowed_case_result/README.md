# Borrowing a returned case payload

Parent outcome: `WIRE-CODEC-LIBRARY`, required by the library decoder's borrowed
blob result and strict decoding of borrowed fields.

Fact: an active case selected on an owned result retains the source place needed to borrow its payload without changing the returned slice's input provenance.

The callback reads the returned slice's length and bytes after selecting the
actual case. The result holds a lifetime tied to the supplied input. A whole
owned carrier, its discriminator evidence, and a descriptor's pointee are
separate subjects; neither a type nor an input loan alone selects a payload.

The producer returns a slice window whose extent is read from the input,
reproducing the library blob-result failure. Direct parameter forwarding remains
an isolation control under `local/direct_forwarding`.

The row carries the owned case result through local storage, a field, a parameter,
a returned call, a match join, nested and separate machines, and a named selected
target implementation. The const-name place evaluates the same producer and
payload observation, then retains its scalar result as an ordinary constant.
A reference-containing case is not copy-eligible constant storage; no reference
or temporary input escapes constant evaluation.

All nine places have exact wrong-case reject twins and pins under
`tests/omega/{pass,fail}/facts/borrowed_case_result_through_*`. Every positive
binds all four hosted targets. Native and Terminal execution must agree at exit
zero. Wrong content, lost length, a wrong selected branch or a wrong payload
projection makes the positive trap. The independent local-loan escape control
remains under `local/loan_escape`.

The case source and borrowed pointee are separate: forming a descriptor from an
already authenticated payload read retains that exact source-access occurrence,
without inventing a pointee place. Its existing discriminator obligation still
must pass. Normalizing a terminal invocation into a call retains that same
call's original authored selection for ordinary constant-evaluation custody.
Neither repair reconstructs authorization from spelling, type or a prior value.
