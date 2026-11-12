# wire_field_numbers

Numbered wire fields: `#0 bytes: &\[u8\]` tags (not positions) + synthesized `Blob::encode` — fat slice frames RAW (length varint + bytes) vs packed `\[u8;N\]`.

Compile-checked only: run a check target, not native.
