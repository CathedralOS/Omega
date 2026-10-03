# Framed payload checksum

The program borrows the middle three elements of `[255, 10, 20, 30, 0]`
with `full[1..4]`, then sums the shrinking slice through a machine-entry
state edge. The checksum is the process exit status, not merely text in
the banner. Keep that loop and its shared loan when repairing the compiler.

From the repository root, after completing this checkout's package review:

```sh
omega run samples/cli/systems/framed_payload/main.omg
```

Press Enter at the prompt. EOF must also complete the program. Both runs
must exit **60** and produce exactly these 165 stdout bytes (LF after every
line, including the last):

```text
framed_payload: subslicing the payload out of a framed message and summing it
result: the payload checksum, 60 for this frame (the exit code)
[press Enter to close]
```

The program emits no stderr. The CLI separately reports `native exit 60`
on stderr. `omega run --both` with finite redirected input additionally
requires `interp exit: 60`; a declined interpreter is not agreement.
Interpreter comparison currently compares exits, not output bytes.

The sample declares Windows x86-64, Linux x86-64, Linux ARM64 and macOS
ARM64 entry points. Run on each matching host: an explicit `--target`
only compiles and cannot establish runtime success.

For disposable local test acceptance and the header's exit oracle:

```sh
python3 tools/sample_native_gate.py --filter cli/systems/framed_payload --jobs 1 --target macos_arm64 --cli <current-omega-binary>
```

Choose the matching host target. This gate accepts pending package-review
decisions for measurement, not as a dependency audit. It currently checks
the documented exit only; compare all output bytes and exercise Enter as
well as EOF before claiming this sample's complete host acceptance.
