# Derivation physical-layout gate

Run `sh tests/gamma/derivation-layout/run.sh` on macOS arm64 with `python3` and
`codesign`, or Windows x64 through Git Bash with `python3`. Windows execution
is not yet validated. Other hosts fail explicitly.

The gate materializes the complete role-selected derivation-checker source
closure with one explicit ordinary-Gamma diagnostic entry. The diagnostic
returns process status zero only to publish an owned outcome: tag 3 plus three
u32 section ends is 13 bytes; tags 1 and 2 plus their four existing failure
fields are 17 bytes. Tag 3 means physical layout, never formation, proof
acceptance, or ownership of an artifact. The exact source identity is pinned in
`source.tsv` before execution.

`fixtures.py` coordinates literal encoders in `wire.py`, physically complete
controls in `physical_cases.py`, byte/field mutations in `mutations.py`, and
fixed-group boundaries in `groups.py`. The host only encodes authored fields,
frames source and input, and compares complete bytes/status/stderr. It does not
decode records, infer sorts, validate references, or implement proof rules.

The 228-byte FORMAT example anchors every inner magic byte and every payload
word's high bit. Other vectors cover all term forms and proof-rule layouts,
variable/application distinctions, mode 1, zero/one/multiple vector elements,
record and nested-table containment, missing fixed groups, surplus fields,
late failures, and the specified error order. Physically valid controls retain
deliberately invalid sorts, references, slots, roots, and clause cardinalities
to prevent physical admission from silently becoming semantic checking.

There are 188 vectors and 374 expected observations. The 186 small vectors run
twice under a 60-second host watchdog. A 46,484-row source
spine runs once under 600 seconds: child references are physical words and must
not cause one Gamma call frame per logical term. Its 929,848-byte request is
not the complete Beta certificate. One 8-MiB-plus-one outer request also runs
once; outer capacity refusal must forward unchanged without scanning invalid
inner contents. No exact-8-MiB inner traversal is claimed or performed here.
Host timeouts and outer evaluator failures are not checker outcomes.
