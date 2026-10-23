# Normalization ownership research, 2026-10-22

Question changed from selective call-result discovery to removing a measured
whole-arena snapshot during normalization. Query:
`site.doc.rust-lang.org std mem take replace owned value avoid clone`.
Read [Rust mem::take](https://doc.rust-lang.org/core/mem/fn.take.html), including
the owned-buffer example. It transfers a field and leaves its Default value.
This supports the transfer mechanism, not semantic equivalence or performance.
The local subtree copier already supports own-table reads with identical append
order and retained occurrence metadata. Extend that existing source abstraction
to statement/type normalization; compare complete carrier equality against the
frozen old normalizer. Reject if any handle, source span, selection, normalized
statement/type or original row differs. Timing requires a separate uninstrumented
binary; exact removed clone rows are the primary deterministic work metric.
