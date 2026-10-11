# Microphone delay beamforming (NEW081)

An independent bounded acquisition, validation, delay-and-sum and direction
ambiguity application. No numerics package or worker candidate is imported.

Scientific model: synthetic synchronized far-field pressure records, three
microphones, eight signed samples each, and four cardinal directions in the
plane (0 east, 1 west, 2 north, 3 south). Geometry coordinates 0..2 denote
positions -1..1 in units c/f_s. The candidate delay is exactly r dot u samples;
no interpolation, fractional delay, acoustic calibration, padding or live
microphone claim is made. Align t=2..5, average active channels, and score the
sum of squared averaged samples. All maximizing directions are reported,
including complete silence and colocated-array ambiguity. Dropout removes a
channel from both the sum and denominator; a disabled channel's timestamp is
irrelevant. Every active timestamp must equal the common epoch zero.

Input is exactly 36 digits with optional ASCII space/tab/CR/LF, terminated by
EOF. Each of three records is `enabled timestamp x y amplitude[8]`. Amplitudes
are digits 0..8 representing signed values -4..4. Enabled must be 0 or 1;
x/y must be 0..2. Invalid digit, short/long input, invalid geometry/flag,
inconsistent timing and no active channels yield exit 64 and `ERROR code`.
Codes: 10 stream shape, 1 geometry/flag, 2 timing, 3 no channels, 4 internal
bounded-storage invariant (unreachable through valid inputs).

Output lists every aligned waveform as numerator/active-count, every energy
as numerator/active-count-squared, and every winner index. Fractions need not
be reduced. Bounded amplitudes imply absolute sums <=12 and energy numerator
<=576; wrapping arithmetic cannot wrap within admitted data. Input storage is
36 digits; processing examines 4 directions x 4 times x 3 channels. Whitespace
may wait until EOF, as with an ordinary stream reader.

## Frozen reference

`data/oracle.json`, `.stdout` files and `FROZEN.sha256` were authored before any
Omega check or execution. Both absolute and relative tolerances are zero.
`reference.py` constructs delays independently and uses Python Fraction direct
rational sums to generate the oracle; it is supplementary, not the application.
The reference defaults to a separate build/reference directory, preserving the
frozen data. `validate.py` compares entire stdout bytes and exact exits.

Existing sample comparison: audio/sine_tone, square_tone and scale_melody
synthesize and play WAV files; they do not ingest multiple recordings or infer
direction. collections/matrix_multiply exercises dense products; it has no
geometry, time alignment, dropout or direction ambiguity. No existing sample
contains beamforming, microphone or acoustic direction scoring.

## Commands (from repository root)

```
OMEGA_LIBRARY_ROOT="$PWD/source/library" /absolute/path/to/omega update --project samples/cli/independent/microphone_delay_beamforming --target linux_x86_64
# Review pending package decisions through the ordinary documented workflow.
OMEGA_LIBRARY_ROOT="$PWD/source/library" /absolute/path/to/omega --check samples/cli/independent/microphone_delay_beamforming/main.omg
OMEGA_LIBRARY_ROOT="$PWD/source/library" /absolute/path/to/omega run --both --keep samples/cli/independent/microphone_delay_beamforming/main.omg < samples/cli/independent/microphone_delay_beamforming/data/east.stdin
python3 samples/cli/independent/microphone_delay_beamforming/validate.py --omega /absolute/path/to/omega
```

`controls/bounds/main.omg` is a separate complete intended-invalid source with
one constant out-of-bounds microphone sample write. It must reject with exactly
the documented `cannot prove index `8` is within length 8` rule, not a package,
parse, unrelated type, or entry review error. Validation records it separately.
Build metadata binds the real Main::main entry for the supported Linux target;
other hosts require their documented entry/provider binding and validation.
Generated artifacts and execution evidence belong under ignored `build/`.

## Acceptance and validation records

The original preparation reviewed 285 standard-library and application decisions:
140 external callables, 140 external supplies, two dangerous capability rows
(FilesystemHost and Console), and three application terminal permissions
(stdin, stdout and process exit). Initial automatic approval review declined
that persistent surface. Following explicit user approval of the disclosed
inventory, acceptance succeeded through the ordinary original-origin transaction.
Compiler identity refreshes preserve the approved meanings; source and review
inventories are retained in the evidence packet. A consuming executor performs
its own ordinary package review; locks are host-local.

An older producer at `b4008bb3c7894aebad47eaccf8af12bb4a8006b1` encountered
`RegisterAllocation(Homes(Assignment(NoCommonCandidate)))` during native
production. Main's existing allocation repairs resolved this unchanged source's
failure. Neither scientific code nor frozen expected results were adjusted to
avoid the compiler defect.

The fixture matrix covers ten valid variants and eight runtime refusals. The
separate complete bounds source must reject for its authored index-eight rule.
`run --both` compares termination only. The ordinary harness checks native
stdout and interpreted exit agreement; the qualification packet also includes
an independently authored consumer of the public admitted Terminal runtime,
capturing full interpreted stdout for every fixture. Retained native executable
replay avoids recompiling the same source per input. Both routes use fresh
execution instances and compare entire stdout bytes and exact exit statuses.

The evidence packet records actual route results, exact commands, source and
binary hashes, the produced native executable and Terminal artifact, the frozen
zero-tolerance oracles, and an independent pairwise-energy calculation. That
calculation and author source review do not constitute a second-person review.
