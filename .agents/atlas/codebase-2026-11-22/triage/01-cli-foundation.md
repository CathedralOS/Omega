# CLI and foundation findings

These fixtures reproduce failures on `1ab250720c8c`. Each passes after the repair in its owning crate. This is a partial review of [area 01](../digests/01-cli-foundation.md).

| Finding | Before the fix | Regression coverage |
| --- | --- | --- |
| A set indexed by a numeric ID accepts the same identities as its table. Found while reviewing the dense-ID storage described in section 1. | Inserting ID zero subtracted one and panicked for a type whose first valid ID is zero. | `arena::id_table::tests::dense_set_accepts_zero_when_the_identity_type_does` uses IDs 0, 1 and 64 and checks removal preserves the neighbors. |
| Section 5 reports that booleans and floats claim to be signed integers. | The primitive-type predicate returned true for all three. Its callers have other checks, so this test establishes the API error only. | `symbols::primitive::tests::only_signed_integer_carriers_are_signed_integers` compares signed and unsigned integers, both floats, a bool and other noninteger types. |
| Section 5 reports a staging collision for filenames that cannot be represented as UTF-8. | Publishing two different payloads for a native filename and the fallback name in one directory failed exact-byte replay. | `platform-custody::file_publication::tests::distinct_native_filenames_keep_their_own_staged_contents` installs both files and reads both destinations. The Windows case uses an unpaired surrogate. The Unix case uses a non-UTF-8 byte. Only the Windows case was run here. |

The digest already records fixes for out-of-range arena handle offsets, toggling float-literal signs and parsing a misplaced verbosity flag. Its other findings still need individual dispositions.
