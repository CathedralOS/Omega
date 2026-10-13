# Layered seismic refraction (W2-17)

A bounded surface-source/surface-receiver travel-time application for one to
four isotropic horizontal layers. Velocities strictly increase with depth.
The final layer is a half-space; every earlier layer has positive thickness.
The application emits the surface direct wave and physically admissible
critically refracted head waves. It reports every active branch, including
branches that never become the first arrival. There are no reflections,
velocity inversions, attenuation, curved layers or anisotropy in this model.

Whitespace-separated unsigned decimal input ends at EOF:

```
layer_count offset_count
velocity_0 ... velocity_(layer_count-1)
thickness_0 ... thickness_(layer_count-2)
offset_0 ... offset_(offset_count-1)
```

Counts are integral: 1..4 layers and 0..16 offsets. Velocities and finite-layer
thicknesses are 0.000001..1000000; offsets are 0..1000000. Choose consistent
units (for example km, km/s, seconds). Tokens have at most twelve digits with
one optional decimal point. Signs, exponent notation, NaN, infinity, missing
or surplus fields and non-whitespace bytes reject. The complete stream is
at most 1023 bytes; EOF must be observable within the 1024-read budget.
Input acknowledges host blocking and traps; fixtures always have finite EOF.

The explicit Linux x86_64 entry is `Main::main` in `build.omg`. Exit 0 means
valid input, 64 malformed syntax/record shape, 65 invalid physical range or
ordering, and 66 a nonfinite report value. Host/provider traps and package
admission failures are separate refusals. An invalid late offset may follow
previously emitted valid rows; any nonzero exit invalidates the whole report.

Output is numeric whitespace-delimited rows:

- `B refractor critical_offset intercept_time` for each deeper layer.
- `R offset_index branch offset time` for direct branch 0 and each admissible
  head wave at that offset. Indexes and refractors are zero based.
- `F offset_index branch offset time` selects the first arrival.

For refractor j, horizontal slowness is `1 / velocity_j`. In overlying layer i,
`r = velocity_i / velocity_j` and `q = sqrt((velocity_j-velocity_i)/velocity_j*(1+r))`. The critical offset is
`sum(2*thickness_i*r/q)`; the intercept is
`sum(2*thickness_i*q/velocity_i)`. Head-wave time is
`intercept + offset/velocity_j`, admitted exactly at or above the computed
critical offset. Equality admits a zero-length interface leg. Direct time is
`offset/velocity_0`. Branches are considered in increasing ID; an improvement
must exceed `4e-15*current_first_time`. This numerical tie convention retains
the lowest ID at analytic crossovers. Reported times retain their own values.
Nine significant decimal digits give bounded-fixture absolute/relative error
at most 1e-7; the physical tie convention is separate from validation tolerance.

For velocities 3 and 5 with finite thickness 2, the critical offset is 3 and
the direct/head crossover is 8. At offset 0 the direct time is 0; at 3 both
branches are emitted and the direct arrives first; at 8 the deterministic
winner is direct branch 0; just above 8 the head wave wins.

## Independent frozen qualification

`validation/oracle.json` is hashed by `validation/oracle.sha256` and was frozen
before any Omega sample execution. `reference.py` independently minimizes the
strictly convex Fermat travel-time functional in horizontal leg coordinates
using 70-digit Decimal arithmetic and 240 derivative bisections. It does not
reuse the application's Snell/cosine calculation. The oracle independently chooses the mathematical minimum with only a 1e-55
relative Decimal tie allowance. Its two-layer crossover
calculation is separately analytic. The validator checks branch activation,
row identity, critical offsets, times, winner IDs, finite numbers and input
hashes with relative error at most 1e-7 for every nonzero expected value and exact zero
for zero-valued output (stronger for tiny times than an absolute 1e-7 floor).

Fixtures cover zero offset, just below/at/above activation, crossover ties and
both sides, consistent unit scaling, length scaling, decimal values, nearly
equal velocities, four layers and sixteen offsets, empty offset lists, EOF
and CRLF. Runtime controls cover malformed, nonfinite, overflowing, missing,
surplus, overlong and invalid physical records. The package-free
`controls/invalid_interface` program must reject specifically for accessing
interface index 3 in a three-interface extent.

From the repository root, using a compiler of recorded source provenance:

```
omega update --project samples/cli/independent/layered_seismic_refraction --target linux_x86_64
# Inspect any ordinary package review decisions; resume only after acceptance.
omega update --resume --project samples/cli/independent/layered_seismic_refraction
omega run --keep samples/cli/independent/layered_seismic_refraction/main.omg < samples/cli/independent/layered_seismic_refraction/input.txt
python3 samples/cli/independent/layered_seismic_refraction/validation/run.py --omega /absolute/path/to/omega
```

For captured Terminal values, `validation/build_terminal.py` links the private
observer against the exact dependency fingerprints of the verified CLI build.
Its `terminal.rs` consumes the retained accepted native product's Terminal
artifact through the ordinary compiler runtime. It uses no trust fixture,
source interpreter or identity override. The observer's own normal package
review and acceptance are required; `--review` and `--resume-review` use the
ordinary package manager. Run the observer with the sample directory and an
ignored output directory, then `check_output.py OUTPUT_DIRECTORY`. Actual
captured Terminal stdout, stderr and explicit exits are compared independently
against the frozen oracle; CLI exit parity alone is insufficient.


The captured-value observer can be built and run without another Cargo build:

```
python3 samples/cli/independent/layered_seismic_refraction/validation/build_terminal.py --omega target/release/omega --output build/w2-17/terminal-observer
build/w2-17/terminal-observer --review samples/cli/independent/layered_seismic_refraction
# Resume only if the ordinary review required and received decisions.
build/w2-17/terminal-observer samples/cli/independent/layered_seismic_refraction samples/cli/independent/layered_seismic_refraction/build/validation/terminal-api
python3 samples/cli/independent/layered_seismic_refraction/validation/check_output.py samples/cli/independent/layered_seismic_refraction/build/validation/terminal-api
```

`--native-executable` on `validation/run.py` reuses a qualified retained image
for the numerical sweep; it does not replace compilation or source-invalid
checking. The repository sample gates discover both this explicit-entry program
and the nested intended-invalid interface project. `input.txt`,
`expected_stdout.txt` and `expected_native_exit.txt` also give the normal native
sample sweep a finite input and an exact checked output.
