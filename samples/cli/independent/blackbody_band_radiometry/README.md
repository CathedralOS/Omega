# Blackbody band radiometry

An explicit `Main::main` Console entry reads one record of seven decimal fields:

```
emitter_K ambient_K grey_emissivity area_m2 view_factor minimum_um maximum_um
300 280 0.85 2 0.7 8 14
```

Run `omega update --project samples/cli/independent/blackbody_band_radiometry
--target linux_x86_64`, review any requested package decisions, then
`omega run samples/cli/independent/blackbody_band_radiometry/main.omg` with the
record on stdin. The entry is bound for Linux x86_64, Windows x86_64 and macOS
arm64/x86_64. Input ends at LF or EOF and is bounded to 256 reads including the
terminator. Space, tab and carriage return separate fields. A decimal point and optional leading minus are accepted; exponents,
NaN, infinity, extra fields, incomplete tokens and overlong records are refused.
Numerical inputs are IEEE binary64 quantities, so extremely close temperatures and wavelengths refer to their parsed binary64 values rather than an exact decimal difference. Decimal conversion uses exact digit doubling and guard/sticky/parity rounding to match ordinary correctly rounded binary64 conversion; band bounds must remain distinct after parsing. There are no defaults.

Emitter temperature is 100–3000 K. Ambient temperature is either exactly zero
(no incident ambient radiation) or 100–3000 K. Emissivity and view factor lie in
[0,1], area in [0,10000] m², and wavelengths in [0.1,10000] µm with strictly
increasing bounds. Zero emissivity, area or view factor gives zero power.
Invalid syntax, nonfinite values and out-of-domain inputs print
`ERROR invalid input` and exit 64.

The CSV row reports emitted and ambient band powers in watts, signed net watts
(emitter minus ambient, integrated directly to preserve near-equilibrium accuracy), and the fraction of the emitter's total blackbody
emission within the band. Both powers include grey emissivity, area and view
factor. The fraction depends only on emitter temperature and band. This models
an isothermal grey diffuse surface exchanging radiation with an isothermal
blackbody environment through the supplied view factor; it does not solve
multiple reflections or wavelength-dependent emissivity.

The model integrates Planck spectral exitance with exact SI definitions of h,
c and k. A dimensionless substitution and composite four-point Gauss rule
resolve the band. The checked source contract bounds the quadrature at 320
panels. The private exponential is a range-reduced Taylor evaluator. The Wien
tail above dimensionless 80 is below the documented absolute floor throughout
the input envelope. Nine significant digits are printed in scientific notation.

`validation/reference.py` freezes an independent 80-digit log-wavelength
`tanh-sinh` quadrature using mpmath 1.3.0. The independent reference converts input tokens to binary64, then integrates their exact binary denotations at high precision. `FROZEN_SHA256.txt` pins the reference,
fixtures and frozen expectations before application execution. Validators use
these files without regeneration. Each numerical column must agree within
`max(1e-12, 1e-6 * abs(reference))`: the absolute floor is 1e-12 W for powers,
and 1e-12 for the fraction. Thus underflowed Wien tails are qualified by an
absolute bound rather than an unjustified relative claim. Resolved near-zero
net values retain this same rule; equal temperatures must give exactly zero.

The fixtures cover 300 K / 8–14 µm, hot narrow and visible bands, wide-band
Stefan–Boltzmann limits, equal temperatures, band additivity, temperature swap,
zero coupling, endpoint bounds, EOF and CRLF. Runtime refusal cases cover every
input bound, malformed/nonfinite tokens and capacity. Validate the actual source refusal with `python validation/check_source.py --omega /absolute/path/to/omega`. The separate source
control `model/intended_invalid.omg` (documented under `controls/intended_invalid`) violates the panel bound and must
fail checking for that proof obligation.

After retaining the native executable, run
`python validation/run.py --native /absolute/path/to/application` from this
sample. The private observation consumer in `validation/terminal.rs`
prepares the ordinary accepted package, retains the exact compiled Terminal
artifact and captures every case's actual stdout, stderr and exit. Validate
those streams with `python validation/run.py --terminal build/validation/terminal-api`.
Build it with `python validation/build_terminal.py --omega /absolute/path/to/omega`; it links the exact compiler libraries instead of rebuilding a nested Cargo workspace. Package review and acceptance use normal tool decisions and do not grant
unrequested host authority.

The input scanner passes its seven values through a shared slice stored in a
record. This also exercises the general compiler rule that indexing and named
calls use the same declared record-field extent. General regression fixtures
check owned, borrowed, nested and generic records, actual floating reads, and
refusal of missing bounds, sibling bounds and stale saved descriptors.
