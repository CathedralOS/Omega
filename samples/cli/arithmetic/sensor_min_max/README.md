# Sensor minimum, maximum and range

Fold six readings through floating `min`/`max`, clamp the peak to 25, and
return `range + clamped_peak`: **33**. Keep the runtime array scan and the
authored `clamp` expression; a reduced min/max probe is not this program.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- update --project samples/cli/arithmetic/sensor_min_max --target macos_arm64
mbx run -p omega -- run samples/cli/arithmetic/sensor_min_max/main.omg
```

If update requests review, inspect the generated source findings and
`samples/cli/arithmetic/sensor_min_max/build/package-manager/review-macos_arm64.txt`,
record each pending decision, then run
`mbx run -p omega -- update --resume --project samples/cli/arithmetic/sensor_min_max`.
Use Cargo when `mbx` is unavailable. These commands also work in PowerShell;
select the matching Windows or Linux target during update. Omit `--target`
from `run` so native execution is requested. Enter or EOF closes the program.

The required stdout is exactly these three LF-terminated lines, with exit 33:

```text
sensor_min_max: track running min/max/range over [20.5, 18, 24.5, 22, 19.5, 26]
result: range(8) + clamp(peak,0,25)=25 -> 33 (the exit code)
[press Enter to close]
```

At `4fe634b7c37` on macOS ARM64 the full native oracle passes: package
review, native compilation and execution print the three lines above and
exit 33 (`python3 tools/sample_native_gate.py --filter sensor_min_max`).
Linux and Windows runs remain unverified.
