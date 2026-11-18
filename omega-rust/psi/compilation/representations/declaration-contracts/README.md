# Declaration contracts

These records carry declaration identities, service and permission contracts,
termination plans, provider declarations and settled selection vocabulary.
Their codecs preserve the exact published identity framing.

`source_identity` contains provenance coordinates and content digests. It does
not retain source contents. Source admission captures those coordinates; later
consumers can compare explicit identity bytes without opening a file or creating
a source owner. Hashing an identity grants no acceptance or execution authority.

Mutable interning tables and authored selection records live inside
`source-compiler`. Canonical application identities and immutable schema
descriptions have their own representation crates.
