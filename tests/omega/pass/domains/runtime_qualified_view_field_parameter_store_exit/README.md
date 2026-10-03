# Qualified view replacement in local storage

`verify` constructs a local record, replaces its UTF-8 view field from a
parameter, and lends the record to an ordinary reader. Three calls must print
`Gate`, `hi`, and an empty line, then exit 70. The output checks the descriptor's
backing and extent; the initializer `old` must not survive replacement.

Run `omega run --both tests/omega/pass/domains/runtime_qualified_view_field_parameter_store_exit/main.omg`
after ordinary project/package review. Repository regression coverage uses
`python3 tools/corpus_gate.py --both --filter runtime_qualified_view_field_parameter_store_exit`.
The corpus policy does not constitute public package acceptance.
On macOS ARM64, the public command currently exits 200 pending ordinary package
review; the corpus route executes both engines with matching output and exit 70.

This extends `domains/utf8_field_write_from_param` with an execution oracle.
Store authority comes from the local record's owned storage, not eligibility
for plain-value transfer. Native entry capture must retain both the source
descriptor address and the destination storage address. Predicate, backing
lifetime and establishment checks remain independent.
