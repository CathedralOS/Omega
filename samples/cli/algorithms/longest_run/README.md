# Longest equal-byte run

Scan `aaabbbbccdaa` using its live byte length, track equal-byte runs, and exit
with the computed maximum, **4**. The algorithm keeps its authored `i32`
cursor, `u64` length comparison, bounded indexed reads, and wrapping counters.

From the repository root, on macOS ARM64:

```sh
mbx run -p omega -- update --project samples/cli/algorithms/longest_run --target macos_arm64
```

If review is requested, inspect the source findings and
`samples/cli/algorithms/longest_run/build/package-manager/review-macos_arm64.txt`.
Record each pending decision, then resume and run:

```sh
mbx run -p omega -- update --resume --project samples/cli/algorithms/longest_run
mbx run -p omega -- run samples/cli/algorithms/longest_run/main.omg
```

Use Cargo if `mbx` is unavailable. These commands also work in PowerShell;
select `windows_x86_64` during update on Windows, or `linux_x86_64` /
`linux_arm64` on the corresponding Linux host. Leave `--target` off `run`:
an explicit target builds without executing. Enter closes the native program;
EOF also completes it.

Exact stdout (166 bytes with LF line endings):

```text
longest_run: finding the longest run of equal bytes in aaabbbbccdaa
result: the longest run length, 4 (the bbbb) for this text (the exit code)
[press Enter to close]
```

The CLI exits **4** and reports `native exit 4` followed by LF on stderr.
The exit comes from the computed counter; matching the literal banner alone
does not establish the result. This complete native oracle is verified on
macOS ARM64 at `2bcfab660cf`; matching-host Windows and Linux runs remain open
under **SAMPLE-CORPUS**. Source checking alone is not native acceptance.
