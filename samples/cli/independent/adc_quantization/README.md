# Calibrated ADC quantization and saturation (IND16)

This complete Omega CLI reads settings and raw samples from stdin and emits CSV
through the existing Console provider. All algorithms and data structures are
private to this project; no worker candidate or numerics package is imported.

The nearest existing sample, `arithmetic/sensor_min_max`, only folds floats into
extrema. This workflow composes two-point sensor calibration, endpoint-inclusive
ADC conversion, signed code mapping, ties-to-even rounding, reconstruction and
saturation-aware error bounds. It studies a synthetic ADC; it does not claim
that every physical ADC uses this transfer function.

Overlap review also covered `arithmetic/led_mixer` (type-level u8 saturation
on fixed scalar operands), `arithmetic/vector_distance` (float magnitude and
clamp), and `samples/stdin_checksum` (byte stream digest). This application adds
runtime converter settings, measured calibration endpoints, exact threshold
rounding, signed code interpretation and a row-by-row reconstruction/error
study; none of those existing programs performs that composition.

## Input

Signed decimal integers separated by ASCII space, tab, carriage return or
newline, without `+` or exponent notation:

```
width signedness reference_low reference_high raw_low raw_high physical_low physical_high rounding count
raw_0 raw_1 ...
```

Width is 2..8; signedness is 0 (unsigned) or 1 (two's-complement signed codes);
rounding is 0 (nearest, ties to even); count is 1..32. Reference and calibration
endpoints and samples are integers in [-1000,1000]. Each low endpoint must be
strictly less than its high endpoint. Only the specified sample count is
accepted. Bad settings, bad tokens, missing samples and extra tokens return 64
with `ERROR: malformed ADC settings or readings`. A failed reading can follow
already-emitted rows; consumers must check exit status before accepting output.

## Transfer function

Let D = raw_high - raw_low, L = 2^width - 1, and R = reference_high - reference_low.
The calibrated physical reading is

```
x = physical_low + (raw - raw_low) * (physical_high - physical_low) / D
rank = round_even((clamp(x, reference_low, reference_high) - reference_low) * L / R)
code = rank                          (unsigned)
code = rank - 2^(width - 1)           (signed)
reconstructed = reference_low + rank * R / L
error = reconstructed - x
```

Codes include both endpoints. Exact reference endpoints are not clipped.
`clip` is -1 below the low rail, +1 above the high rail, and 0 otherwise.
Rounding parity in rank and signed code agrees because the signed offset is even
for every admitted width. Negative-code ties therefore also round to even.

CSV emits raw, code, reconstructed numerator/denominator, clipping flag,
signed error numerator/denominator and absolute-error-bound numerator/denominator.
Fractions intentionally remain unreduced. The bound is half a step for ordinary
readings, and distance beyond the rail plus half a step for clipped readings.
Calibration and rounding use integer fractions, with zero numerical tolerance.
The bounded envelope keeps all i64 intermediates below 10^12, so the explicitly
Wrapping arithmetic never actually wraps on accepted inputs.

`domain/settings.omg` validates input, `domain/adc.omg` owns conversion,
`io/input.omg` parses bounded integers, and `io/output.omg` writes decimal output.
`main.omg` is the actual build-bound program entry and sequences the whole study.

## Frozen acceptance

`ORACLES.json` hashes all expected CSV before the first Omega execution.
`reference.py` independently uses Python Fraction and Python round; it is an
oracle, not the delivered application. `data/` contains five valid studies and
17 malformed-input controls. The valid studies cover unsigned threshold ties,
signed ties after nonidentity calibration, fractional calibration, 8-bit extrema,
and a 2-bit signed converter. They include rail endpoints and saturation on
both sides. `intended_invalid/` is a separate complete build-bound Omega project
with a constant-zero division; it must be rejected independently.

## Commands

From the repository root, with a compiler built from this candidate and
`OMEGA_LIBRARY_ROOT` set to that checkout's `source/library`:

```
omega update --project samples/cli/independent/adc_quantization
# Review the generated package decisions and resume the documented review.
omega --check --target linux_x86_64 samples/cli/independent/adc_quantization/main.omg
omega run --both --keep samples/cli/independent/adc_quantization/main.omg < samples/cli/independent/adc_quantization/data/signed_calibrated.txt
python3 samples/cli/independent/adc_quantization/validate.py --omega /absolute/path/to/omega
```

Validation runs one Omega command at a time, retains commands, status, stdout
and stderr in ignored `build/validation`, requires each valid CSV to equal its
frozen oracle, checks rational error bounds, checks runtime-invalid status and
error text, and checks the separate source-invalid diagnostic. `--both` compares
native and Terminal completion status using replayed identical stdin. The public
CLI does not compare their stdout; native CSV is independently checked exactly. A declined comparison is
reported as unsupported rather than agreement.
