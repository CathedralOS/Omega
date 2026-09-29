# Text algorithm native oracles

These four samples exercise runtime-length byte scans, indexed reads, and
integer operations through complete native execution. Preserve their authored
algorithms and cursor types; a banner containing the answer is not a computed
result. Each program pauses for Enter; EOF also completes it.

From the repository root, for example on macOS ARM64:

```sh
mbx run -p omega -- update --project samples/cli/text/parse_int --target macos_arm64
```

When review is requested, inspect the source findings and the project's
`build/package-manager/review-macos_arm64.txt`, record each pending decision,
then resume and execute:

```sh
mbx run -p omega -- update --resume --project samples/cli/text/parse_int
mbx run -p omega -- run samples/cli/text/parse_int/main.omg
```

Substitute the sample directory below for `parse_int`. Use Cargo when `mbx`
is unavailable. The commands also work in PowerShell. For other hosts select
`windows_x86_64`, `linux_x86_64`, or `linux_arm64` during update, then omit
`--target` from `run` so the program actually executes.

| Sample | Computed result | Native / CLI exit | Stdout bytes |
| --- | --- | --- | --- |
| [parse_int](parse_int/main.omg) | Decimal `150` parsed from three live bytes | 150 | 132 |
| [roman_numeral](roman_numeral/main.omg) | `MCMXCIV` parses to 1994, taking the success branch | 70 | 133 |
| [string_hash](string_hash/main.omg) | FNV-1a of `omega`, low digest byte 40 | 40 | 140 |
| [substring_search](substring_search/main.omg) | First `cad` in `abracadabra` starts at index 4 | 4 | 149 |

Each stdout is exactly three LF-terminated lines: the two sample-specific
lines below, then `[press Enter to close]`. CLI stderr is exactly
`native exit N` followed by LF, where `N` is the expected exit above.

```text
parse_int: atoi over a text carrier, digit by digit
result: the parsed value of the text 150 (the exit code)

roman_numeral: parses MCMXCIV with the subtractive-pair look-back rule
PASS: MCMXCIV parsed to 1994 (exit 70)

string_hash: FNV-1a hashes the bytes of the string omega
result: the digest's low byte, 40 for omega (the exit code)

substring_search: naive search finds cad inside abracadabra
result: the match index, 4 for cad in abracadabra (the exit code)
```

Blank separators above are not printed. Full native outputs and exits are
verified on macOS ARM64 at `2bcfab660cf`. Matching-host Windows and both Linux
runs remain open under **SAMPLE-CORPUS**; cross-target compilation is not a
substitute for those observations.
