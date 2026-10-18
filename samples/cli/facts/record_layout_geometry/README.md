# Record layout geometry

Fact: the validated byte size and alignment of a padded structural record and fixed array combine to 100u64 when subtracting 100u64.

Owning Task: `CONSERVATION-CONTRACT`
Claim Ticket: `dd1724b6f7c74abca06de7fcab2ea51e`

## Requirements and Evidence

- **Establishment**: Portable layout queries `size_of<PaddedFrame>()` (88u64), `align_of<PaddedFrame>()` (8u64), and `align_of<u32>()` (4u64) evaluate exact byte sizes, alignment, and internal padding (7 padding bytes following `tag: u8` before `word: u64` and fixed array `tail: [u64; 9]`), summing to exactly `100u64`.
- **Identity and Lifetime**: The combined geometry value `100u64` is conserved across all nine language transport places, and exact subtraction `geometry - 100u64` produces `0u64` without overflow or narrowing loss.
- **Composition**: Exact layout arithmetic is preserved through locals, fields, machine parameters, call results, match joins, named constants, state transitions, forwarding machines, and target-specific bodies across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).
- **Soundness Boundaries**: Narrowing the fixed-extent array (`[u64; 8]` yielding 92u64 total geometry) breaks the `100u64` subtrahend bound, causing compile-time rejection with `exact arithmetic may overflow` (`exact subtraction underflow`, bounds `[-8, -8] lie outside [0, 18446744073709551615]`) across all nine places.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | The size and alignment queries combine in local bindings before subtracting 100u64. |
| field | `field/` | A carrier record carries the combined size and alignment before subtraction. |
| parameter | `parameter/` | A helper machine receives the combined geometry as an explicit parameter. |
| call result | `call_result/` | A helper machine returns the combined layout geometry as its call result. |
| match join | `match_join/` | Both branches of a match expression produce the combined layout geometry. |
| const name | `const_name/` | Named constants evaluate `size_of` and `align_of` before subtraction. |
| other state | `other_state/` | A transition forwards the combined geometry to a successor state. |
| other machine | `other_machine/` | A forwarding machine delegates the subtraction to an intermediate machine. |
| target body | `target_body/` | Each of the four target bodies computes the combined layout geometry. |

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/record_layout_geometry/main.omg
omega run --both samples/cli/facts/record_layout_geometry/local/main.omg
omega run --both samples/cli/facts/record_layout_geometry/field/main.omg
omega run --both samples/cli/facts/record_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/record_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/record_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/record_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/record_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/record_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/record_layout_geometry/target_body/main.omg
```

Each place has an invalid twin that queries a narrower fixed array (`[u64; 8]` instead of `[u64; 9]`), totaling 92u64 and failing exact subtraction `geometry - 100u64` at compile time with `exact subtraction underflow`.

The row is pinned by `tests/omega/pass/facts/record_layout_geometry_through_<place>_exit` and `tests/omega/fail/facts/record_layout_geometry_unproved_<place>` fixtures.
