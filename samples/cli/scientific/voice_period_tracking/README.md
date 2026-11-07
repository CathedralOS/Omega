# Voice period tracking

NEW085 is a complete bounded synthetic audio analysis application. It reads
newline-separated PCM windows from stdin, computes exhaustive squared-difference
lag scores, selects period candidates, emits pitch tracks and reports maximal
uncertain spans. This is a scientific sample model, not a validated human voice
detector. Sampling is declared to be 8000 Hz; each ASCII digit denotes one linear
PCM amplitude from 0 to 9. DC offsets are permitted. No file is a real recording.

Each record contains 1..32 digits, with at most 16 records. EOF also terminates
an unterminated final record. Empty input, empty records, CR, nondigits, more than
32 samples or more than 16 records print `ERROR input` and exit 64. Already
completed record output remains visible on a later input failure. Valid input
exits 0. A window shorter than 12 samples is uncertain without scoring.

For each remaining window and every lag from 2 through 8, the algorithm computes
`S(lag) = sum((x[i] - x[i+lag])^2, i=0..n-lag-1)` and reports the sum and pair
count. Scores are normalized by pair count and compared by integer cross
products. The shortest minimum is the candidate. Constant amplitude is silence;
nonconstant windows with minimum mean squared difference above 1 are unvoiced;
more than one equally minimal lag is ambiguous. Both harmonic aliases and
nonharmonic ties count as ambiguous. A unique candidate below the threshold is
voiced. These are declared model rules, not physical probabilities. An amplitude-shape
variant tests the score calculation; amplitude scaling can change the
fixed threshold decision.

Output is ASCII rows:

- `FRAME index samples` begins a window.
- `LAG lag sum pairs` preserves the exact rational score `sum/pairs`.
- `TRACK index lag floor_hertz status` emits the estimate. Status is 0 silence,
  1 voiced, 2 ambiguous, 3 short, 4 unvoiced. Lag and hertz are zero when absent.
  The exact pitch is `8000/lag` Hz; the displayed integer is its floor.
- `CHANGE index previous_lag current_lag` reports changes between consecutive
  present estimates, including ambiguous ones. An absent estimate breaks the
  trajectory; the algorithm does not interpolate across silence.
- `SEGMENT start end` gives a half-open maximal consecutive span whose status
  is not 1. The final span closes at EOF.

The parser, streaming tracker and selection live in `main.omg`. Private domain
modules own exhaustive lag sums and decimal reporting. There is no numerics
library dependency and no dependency on another worker. Existing audio samples
(`square_tone`, `sine_tone`, `scale_melody`) synthesize/play WAV; they do not analyze
input records or emit uncertainty trajectories. Algorithm probes exercise array
and state semantics, rather than this workflow.

The largest squared-difference sum is `30*81 = 2430`; selected-score comparison products
are at most 72900. The initial `10000` sentinel comparison is at most 300000.
Both bounds fit `i32`. Runtime inputs and loop guards bound every indexed access.
Arithmetic uses declared Wrapping carriers, but those scientific bounds ensure
no overflow in the admitted model. Storage is one 32-sample window and scalar
tracking state. Every accepted sample is initialized before being read.

## Frozen independent expectations

`reference.py --freeze` was run before any Omega check or execution. The oracle
uses Python arbitrary integers, full pair enumeration and `Fraction`, independent
of the Omega streaming sum and cross-product selection. `data/oracle.json` freezes
all input and expected-output SHA-256 values; tolerance is **zero**, including
pitch flooring, sums, counts, ties, track changes and segment endpoints. Validation
never updates the oracle. The initial manifest hash is
`91a5c1f72a4a241e26a7ab3524df2d3db6a43279bfa4a35293000cca544c1a30`.

Nine cases cover constructed periods 2, 4, 5 and 7; harmonic ambiguity; silence;
changing periods both between and within windows; amplitude-shape variants; minimum
scored and maximum window sizes; EOF without LF; short windows; and six invalid
input classes. Each case has `.input`, `.stdout` and `.exit` artifacts in `data/`.
The invalid-record-count case verifies bounded streaming rejection after output.

## Commands

Run from the repository root with a compiler built from that checkout. Set
`OMEGA_LIBRARY_ROOT` to its `source/library` when reusing a compiler binary.

```sh
omega update --project samples/cli/independent/voice_period_tracking --target linux_x86_64
# Review the pending package decisions; resume only after their acceptance.
omega update --resume --project samples/cli/independent/voice_period_tracking
omega --check samples/cli/independent/voice_period_tracking/main.omg
omega run --keep samples/cli/independent/voice_period_tracking/main.omg < samples/cli/independent/voice_period_tracking/data/trajectory.input
python3 samples/cli/independent/voice_period_tracking/reference.py --omega /absolute/path/to/omega
python3 samples/cli/independent/voice_period_tracking/reference.py --omega /absolute/path/to/omega --both
omega --check samples/cli/independent/voice_period_tracking/controls/type_valid/main.omg
omega --check samples/cli/independent/voice_period_tracking/controls/type_mismatch/main.omg
```

The separate intended-invalid control assigns `true` to an `i32` period. The
ordinary type contract rejects incompatible types without needing any provider
or package acceptance. Only a diagnostic identifying that Boolean/integer type
mismatch counts; infrastructure errors or unrelated rejection do not pass. Its
provider-free positive twin stores integer `3` and must check successfully.
Native and Terminal results, compiler identity, and any unavailable routes are
recorded in `VALIDATION.md`. Generated logs/executables belong in ignored `build/`.
EOF replay permits Terminal comparison without any unbounded input wait.
