# Application identities

This crate defines canonical type, constant and application identities. Source
settlement writes these records; portable decoding and independent checks read
the same encodings. The existing tags, framing and commitments remain unchanged.

The crate has no source files, symbol-table mutation, declaration lookup or
preparation entry points. Authored Build selections belong to `source-compiler`.
Declaration and service contracts belong to `declaration-contracts`.

The tests retain the original identity, malformed-input and package-owner cases.
