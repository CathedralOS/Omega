# Laminate stiffness response (NEW074)

See [scientific validation](../README.md) for the current status of this sample’s Rust execution driver.

The laminate solver runs on Linux x86-64 and ARM64 macOS. On the Mac, all
16 test inputs produced the expected exit and output. Every reported number
matched `data/expected.json` within its absolute tolerance of 0.000002. The
Mac native executable and the Windows-hosted Psi interpreter targeting macOS
produced identical output bytes.

A complete bounded classical laminate workflow in Omega. It assembles rotated
orthotropic plane-stress stiffness, membrane A, coupling B and bending D, solves
`[A B; B D] [strain; curvature] = [force; moment]`, and reports the global in-plane
stress at each ply midpoint. The implementation owns its fixed storage,
transforms, partial-pivot elimination, input selection and decimal reporting.
No numerical library or worker candidate is imported.

This is an explicitly synthetic scientific sample, not a structural design tool.
Material reduced stiffness is Q11=12, Q22=4, Q12=1, Q66=2 in synthetic units.
Engineering shear is used; z increases from the bottom of the centered stack.
Load is `(1, .25, -.125, .1, -.05, .025)`. Total thickness is 1 in valid fixtures.
Positive orientation rotates material axis 1 toward global axis 2. Stress output
is `(sigma_x, sigma_y, tau_xy)` in global coordinates at the midpoint, not an
interface or failure criterion. No transverse shear, thermal loads or damage.

Input is one ASCII selector with LF, or exactly one byte followed by EOF.
Empty, overlong, unknown or multiple-character input exits 2. CRLF is rejected
explicitly. Reading is bounded to four bytes, and a Full read is rejected.

| Input | Declared stack, bottom to top | Result |
| --- | --- | --- |
| 0 | 0/90, equal thickness | coupled response |
| 1 | 90/0, equal thickness | reversed B, same A and D |
| 2 | 0/90/90/0, equal thickness | B cancels |
| 3 | +45/-45/-45/+45, equal thickness | B and A16 cancel; D16 remains |
| 4 | cos=.6,sin=.8 / -45 / 90, thickness .2/.3/.5 | oblique unequal stack |
| 5 | 0/90 with zero stiffness | SINGULAR, exit 3 |
| 6 | negative thickness | INVALID MODEL, exit 2 |
| 7 | empty stack | INVALID MODEL, exit 2 |
| 8 | inconsistent orientation tensor | INVALID MODEL, exit 2 |

Orientation is declared as `(cos^2, sin^2, cos*sin)` so exact zero/right-angle
fixtures do not inherit trigonometric rounding. Active layers require thickness
in `(0,1]`, nonnegative squares, sum within 1e-12 of 1 and product identity within
1e-12. The solver rejects pivots at or below 1e-12 in these declared units.

Output starts `LAMINATE micro-units v1`, then a Q section for every ply, A, B,
and D (each nine row-major values), RESPONSE (six values), and STRESS per ply
(three values). Every numeric line is a signed integer divided by 1,000,000;
conversion truncates toward zero. Valid runs end OK and exit 0. Singular runs
retain Q/A/B/D and end SINGULAR with no invented response or stress.

`data/expected.json` was frozen before the first Omega check/run. The independent
70-digit Decimal oracle rotates a fourth-order elasticity tensor and solves
with symmetric LDL^T, whereas Omega uses closed-form transforms and pivoted
Gauss-Jordan elimination. Its exact cardinal checks, reversed-stack identities,
coupling cancellation and load residuals must pass before freezing. All numeric
output uses absolute tolerance 2e-6 and relative tolerance 0; headings, counts,
status and exits match exactly. Verification never rewrites the oracle.
The application independently compares all 81 computed storage values with the
frozen reference at absolute tolerance 1e-8 before reporting. A mismatch exits 4.
The public SDK sponsor captures Terminal stdout and verifies every reported
value against the same frozen oracle as native execution. The recorded comparison separately checked termination. Report conversion is explicitly Trapping,
with a separate bounded signed-decimal range check.

Existing samples compared: `collections/matrix_multiply` only multiplies two
integer matrices; `simulation/particle_sim` only integrates a scalar motion;
`arithmetic/stats_compute` accumulates scalar statistics. None assembles a
laminate, integrates through thickness, solves coupled constitutive loads or
recovers ply stress. This is a new workflow rather than renamed constants.

Valid runs exit 0. Empty input, unknown bytes, CRLF and overlong records exit 2.
The native sample gate feeds root `input.txt` (mode 0, the equal 0/90 stack);
a direct run with empty stdin is the invalid-input rejection.

Run these commands from the sample directory with the compiler and its matching
bundled library. On Linux x86-64, set `target=linux_x86_64`.

```sh
target=macos_arm64
set --
for source in build.omg main.omg mechanics/*/*.omg; do
    set -- "$@" --build-input "$source"
done
omega packages update --project . --target "$target" "$@"
# After reviewing the generated package decisions, resume the update.
omega packages update --project . --resume
omega run --target "$target" "$@" --input data/mode_0.stdin --outputs build/psi build.omg
omega --target "$target" "$@" --build-dir build/native build.omg
mbx reserve --cpus 1 -- build/native/omega-program < data/mode_0.stdin
```

The bounds control is a separate complete application intentionally writing
index 4 of a four-element array. A bounds diagnostic is required; package,
entry-slot or unrelated type rejection does not count as acceptance.
