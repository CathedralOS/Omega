# CLI and foundation findings

These fixtures reproduce failures on `1ab250720c8c`. Each passes after the repair in its owning crate. This is a partial review of [area 01](../digests/01-cli-foundation.md).

| Finding | Before the fix | Regression coverage |
| --- | --- | --- |
| A set indexed by a numeric ID accepts the same identities as its table. Found while reviewing the dense-ID storage described in section 1. | Inserting ID zero subtracted one and panicked for a type whose first valid ID is zero. | `arena::id_table::tests::dense_set_accepts_zero_when_the_identity_type_does` uses IDs 0, 1 and 64 and checks removal preserves the neighbors. |
| Section 5 reports that booleans and floats claim to be signed integers. | The primitive-type predicate returned true for all three. Its callers have other checks, so this test establishes the API error only. | `symbols::primitive::tests::only_signed_integer_carriers_are_signed_integers` compares signed and unsigned integers, both floats, a bool and other noninteger types. |
| Digest line 238. Restoring a floating literal bypassed the checks on decimal text and supplied bits. | The record reader returned a literal with value zero for malformed text and accepted an f32 presentation wider than 32 bits. | The numeric-literals recovery test covers invalid spellings, format, width and conflicting text. Valid decimals, infinity, signed zero and a NaN payload round-trip. |
| Section 5 reports a staging collision for filenames that cannot be represented as UTF-8. | Publishing two different payloads for a native filename and the fallback name in one directory failed exact-byte replay. | `platform-custody::file_publication::tests::distinct_native_filenames_keep_their_own_staged_contents` installs both files and reads both destinations. The Windows case uses an unpaired surrogate. The Unix case uses a non-UTF-8 byte. Only the Windows case was run here. |

The digest already records fixes for out-of-range arena handle offsets, toggling float-literal signs and parsing a misplaced verbosity flag. Its other findings still need individual dispositions.

## Original findings still under review

Sections 5 and 6 contain 62 top-level observations. The linked ranges cover each observation once in this table. Individual dispositions above apply only to their stated conditions.

| Subject | Digest lines | Evidence and open work |
| --- | --- | --- |
| Command inputs and results | [187–197](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L187-L197) | The saved lock-pin conversion at 189 cannot admit an empty digest. ReviewedHostPackages::open requires 64 hex digits before opening the file. Other command, display and host-specific behaviors retain their survey status. |
| Compiler entry and consent | [200–202](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L200-L202) | Trace current compilation, selected-provider grants and request outcomes at their callers. |
| Arena identity and storage | [205–214](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L205-L214) | 207 and 213 have kept arena regressions. Cached-index equality, stale lookups and retirement semantics need separate dispositions. |
| Host and storage foundations | [217–224](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L217-L224) | 220 is the native filename fix in #5270. Trace remaining normalization, resource and build observations. |
| Builtin names and semantic vocabularies | [227–239](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L227-L239) | 227 is specified: all digit-only widths reserve the type spelling, including invalid widths. Bare i/u name unbounded integers. See the binding rule below. 228 is fixed in #5270. At 238 sign negation and validation of restored literals have both been repaired. |
| CLI and foundation drops | [243–259](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/01-cli-foundation.md#L243-L259) | 243 and the repeated build-directory part of 244 have regressions. The other entries include flag handling, defaults, declaration edges and reporting. |

The [builtin type name decision](../../../../wiki/spec/language/modules.md#builtin-type-names) reserves names by role. The [width-indexed integer rule](../../../../wiki/spec/language/numeric_values.md#width-indexed-integers) separately defines which widths form types. The test `builtin_type_spellings_cover_the_roster_and_the_integer_family` explicitly checks `u0`.
