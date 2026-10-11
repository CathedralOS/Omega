# Bounded sensor compression (NEW092)

A complete streaming console application with private encode, byte-decode,
input admission, and reporting modules. It has no worker or numerical-library
imports. The scientific sample model is a **synthetic**, uniformly sampled
12-bit ADC measuring temperature in units of 1/16 degree C relative to an
arbitrary zero; it makes no claim about physical calibration or measured data.
Each invocation handles 1–16 records in [0,4095].

## Model and format frozen before execution

The user declares integer error tolerance E in [0,7]. Quantization rounds to
the nearest multiple of 2E+1 anchored at zero, then clips to 4095. Odd grid
spacing precludes halfway integer ties. For each record the distance to its
reconstruction is at most E units (E/16 degrees C). E=0 reconstructs exactly.
This is quantized prediction from the previous reconstruction, initially zero;
there is no accumulating prediction drift.

A stream starts with decimal bytes `[83,67,1,count,E]`. A delta in [-127,127]
is one byte `delta+127`. Every other delta emits `[255,low,high]`, the absolute
reconstruction as unsigned little-endian 16-bit. This avoids signed delta
overflow on abrupt transitions. The decoder rejects truncation, extra bytes,
invalid header/count/E, out-of-range reconstructions, and escapes whose delta
fits the compact range. The decoder's accepted reconstruction values need not
be grid multiples; E cannot certify error without the original observations.

Size is exactly 5 + N + 2A bytes, where A is the escape count, and bounded by
5+3N. The report compares this with 2N raw bytes; some short or abrupt streams
expand. Smooth 16-record input compresses from 32 to 23 bytes. Worst-size
alternating input expands to 53 bytes. The certificate is calculated from the
original observations and independently decoded bytes, rather than presumed
from the encoder. Every decoded value and output byte is emitted at runtime.

`data/oracle.json`, `data/cases.json`, and the per-case binary/stdout fixtures
were frozen by `reference.py` before the first Omega check or execution.
`data/FROZEN.sha256` identifies the two oracle manifests. All output and exit
checks are exact; there is no floating epsilon. Cases include exact, smooth,
abrupt, grid endpoints, compact delta boundaries, singleton, worst size, four
seeded variants, corresponding independent decode requests, 12 malformed
streams and 10 invalid input requests. The intended-invalid source control
is separate from all runtime invalid inputs.

## Input and output

Binary stdin is finite and terminated by EOF, with a maximum of 56 bytes.

* Mode 0: `[0,count,E]` followed by count unsigned 16-bit little-endian values.
* Mode 1: `[1]` followed by a complete sensor stream.

Successful output is decimal encoded bytes, reconstructed readings, raw,
encoded and worst-case byte sizes, tolerance, and (compression only) observed
maximum error and certification status. Decode-only output explicitly says
that originals are unavailable. Exit 10 rejects input syntax/bounds, exit 12
rejects malformed streams, exit 13 signals internal certificate/report failure.
Input is validated before processing and no invalid partial output is reported.

## Commands

From repository root, with a compiler built from the recorded base:

```sh
omega update --project samples/cli/independent/bounded_sensor_compression --target linux_x86_64
# Review the generated omega.review decisions; accept only understood claims.
omega update --resume --project samples/cli/independent/bounded_sensor_compression
omega --check --target linux_x86_64 samples/cli/independent/bounded_sensor_compression/main.omg
omega --target linux_x86_64 samples/cli/independent/bounded_sensor_compression/main.omg
omega run --both samples/cli/independent/bounded_sensor_compression/main.omg < samples/cli/independent/bounded_sensor_compression/data/smooth.bin
omega --check samples/cli/independent/bounded_sensor_compression/controls/invalid_bound/main.omg
```

The last command must reject for the exact out-of-bounds index fragment in
`controls/invalid_bound/expected_fragment.txt`; unrelated failure is not a pass.
`validate.py --binary <native-image>` checks all frozen cases directly.
`validate.py --omega <compiler> --both` separately runs the public native plus
Terminal comparison route. Terminal CLI agreement covers exit only; the
validator independently checks native stdout. Retained compiler reports and
route statuses are recorded in `VALIDATION.md`.

## Existing sample comparison

`sensor_min_max` scans extrema of floats but does not encode or decode data.
`utf8_byte_class` classifies bytes, and `format_number` formats a fixed number.
`array_max`, `bitset`, and `heat_grid` exercise bounded storage and indexing.
`systems/wire_protocol` supplies schema-driven lossless sensor round trips;
it has no predictive quantizer, escape admission, or error certificate.
No existing sample implements predictive sensor quantization, absolute escape
recovery, strict stream admission, or an original-versus-reconstruction error
certificate. This sample provides that entire workflow and runtime input.

The separate bounds control declares its discovery role in
`controls/invalid_bound/sample.json`, using the same frozen diagnostic fragment.
It enters checking only; malformed runtime inputs remain cases of this program.
