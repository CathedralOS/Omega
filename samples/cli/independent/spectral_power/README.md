# Calibrated Welch spectral power

This Omega program estimates a measured time series' one-sided power spectral
density in physical units squared per Hz. It accepts supplied calibration,
fixed FFT sizes 8, 16, 32 or 64, rectangular or periodic Hann windows, and any
integer overlap from zero through FFT size minus one. It implements an in-place
radix-2 FFT with bit reversal and averages complete segment periodograms.
Incomplete trailing segments are discarded. It does not subtract a mean:
DC and an instrument's supplied zero offset remain observable power.

Input is whitespace-separated signed decimal integers, ending at EOF:

```
sampling_hz gain_milli offset_milli fft_size overlap window sample_count
sample_milli ...
```

Sampling is 1..1,000,000 Hz; gain is 1..10,000 milli-units per measured unit;
window is 0 (rectangular) or 1 (periodic Hann). Sample count is at least one FFT
and at most 64. All tokens have magnitude at most 1,000,000 and at most seven
digits. The stream must be shorter than 4096 bytes. Signs without digits,
noninteger/nonfinite spellings, missing or extra samples, zero/negative sampling
or gain, unsupported sizes, invalid overlap/window and lengths reject with
`ERROR input or options` and exit 10.

Calibration precedes windowing: `x = sample_milli/1000 * gain_milli/1000 +
offset_milli/1000`. For each segment `y[j]=x[j]*w[j]`, the density is
`|FFT(y)[k]|² / (sampling_hz * sum(w²))`, doubled for interior positive bins
only; DC and Nyquist are never doubled. Welch takes the arithmetic average
across segments. Frequency is `k*sampling_hz/fft_size`. Summed density times
bin spacing equals the averaged window-weighted mean square
`sum(x²*w²)/sum(w²)` (Parseval); this is physical power, including offset.

Output gives `segments`, then alternating `frequency_microhz` and
`density_nano` integer rows for bins from DC through Nyquist. Values truncate
toward zero; divide them by 1e6 Hz and 1e9 physical-units²/Hz respectively.
The bounded domain keeps these reports inside signed i64 without saturation.

`fixtures.json` freezes independent Python direct complex DFT outputs before
execution. `validate.py` recomputes those pinned outputs, independently checks
Parseval, and checks actual numerical stdout, frequency bins, integrated physical
power and status separately for native and Terminal. Fixtures include all four
FFT sizes, rectangular and periodic Hann windows, overlapping segments,
calibration gain/offset, DC, Nyquist, silence and incomplete trailing segments.
`validation/` uses normal package preparation and native compilation, then
executes the exact retained Terminal artifact and the published native product.
It captures both outputs; `omega run --both` alone compares only termination.
The harness grants domain exit only to its isolated containing domain, as the
normal CLI does; package and root authority admission remain intact.

From repository root, with the documented cloud environment activated:

```
omega update --project samples/cli/independent/spectral_power --target linux_x86_64
omega run samples/cli/independent/spectral_power/main.omg < samples/cli/independent/spectral_power/input.txt
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p spectral-power-validation
python3 samples/cli/independent/spectral_power/validate.py --runner /absolute/path/to/spectral-power-validation --evidence /absolute/path/to/evidence
```

The Build root binds Linux x86-64, Windows x86-64 and macOS ARM64 entries.
Numerical execution evidence from this task is Linux x86-64 only.
