# Periodic quantum wavepacket evolution

This complete Omega CLI evolves a one-dimensional wavefunction in units
`hbar = m = 1`. The input consists of `cells spacing timestep steps`, followed
by exactly `cells` triples `potential real_amplitude imaginary_amplitude`.
Cells are periodic and centred; no wall boundary or continuum dispersion is
substituted for the discrete Hamiltonian.

For example:

```text
8 1 0.01 1
0 1 0
0 -1 0
0 1 0
0 -1 0
0 1 0
0 -1 0
0 1 0
0 -1 0
```

Accept 8–32 cells, positive spacing and timestep, and 0–256 steps. Dimension
and step tokens must be mathematically integral even when a tiny fractional
part would round away in `f64`. The bounded decimal grammar accepts an optional
minus sign and at most one decimal point, with 1–24 digits and magnitude at most
1,000,000. Exponents, NaN, infinity, malformed tokens, extra/missing values,
and streams exceeding 4096 bytes reject. ASCII space, tab, CR and LF separate
tokens. The supplied nonzero amplitudes are normalized once to
`spacing * sum(abs(psi)^2) = 1`; an exactly zero norm rejects.

`H[j,j] = 1/spacing^2 + potential[j]` and the two periodic neighbours have
`H[j,j±1] = -1/(2*spacing^2)`. Crank–Nicolson solves
`(I + i*timestep*H/2) psi_next = (I - i*timestep*H/2) psi` using one reusable
complex LU factorization and forward/back substitution. All complex operations
are owned in `domain/complex.omg`. Every leading principal factor is nonsingular
in exact arithmetic because its real part is the identity. Double precision
still loses that identity at extreme stiffness, so the CLI refuses when any
`timestep * (1/spacing^2 + abs(potential[j])) > 1000`.

Output starts with `quantum_wavepacket_evolution hbar=m=1`. Each checkpoint
(including step zero) contains one `step norm energy` line followed by one
`real imaginary spatial_probability` line per cell. Numbers use an integer
mantissa and decimal exponent, for example `350000000000000000e-18`.
Probability is the cell mass `spacing * abs(psi[j])^2`, and energy is the
Hamiltonian expectation. Its equivalent kinetic difference form avoids
cancellation for nearly constant wavefunctions. No renormalization occurs
between checkpoints.

Exit 0 means evolution completed. Exit 2 writes `INVALID quantum input` for
invalid dimensions, grammar, bounds, nonfinite text or zero norm. Exit 3 writes
`REFUSED quantum conditioning` for the documented double-precision regime.
`controls/complex_array_bound` is a separate intended-invalid source project:
an index of 32 in a 32-element complex array must reject at source checking.

Run with an optimized compiler built from a recorded source revision:

```sh
omega update --project samples/cli/independent/quantum_wavepacket_evolution --target linux_x86_64 < /dev/null
# Inspect any generated pending review. Only Console stdin/stdout/exit is
# required; std source-risk disclosures do not grant filesystem authority.
omega --build-dir build/quantum --target linux_x86_64 samples/cli/independent/quantum_wavepacket_evolution/main.omg < /dev/null
python3 samples/cli/independent/quantum_wavepacket_evolution/validate.py --executable build/quantum/omega-program
```

`reference.py` froze 41 cases before the first sample execution, independently
using NumPy's pivoted dense complex solve and SciPy's matrix exponential.
`data/FROZEN.sha256` seals the generator, input bytes and reference results;
validation checks it without regenerating anything. Cases cover the analytic
discrete eigenmode, timestep halving, wavepackets, potential barriers, both cell
limits, maximum steps, signed potential, very small spacing with a compatible
timestep, wide spacing/timestep and refusal controls. Complex checkpoints must
agree with the dense solve within `1e-8`; norm and energy drift are checked at
every step. Actual error against the exponential must agree with the frozen
CN truncation error. Wavepacket/barrier and eigenphase errors decrease by about
four under timestep halving at fixed elapsed time.

For eigenmodes, energy is independently checked against the analytic eigenvalue.
The frozen dense matvec's zero-mode energy at spacing `1e-8` visibly loses
precision (a spurious `0.28963` after a step), so its cancellative energy entry
is preserved as evidence and the exact eigenvalue supplies that check. Frozen
complex vectors, probabilities, norms and exponential results remain unchanged.

Actual Terminal numerical stdout uses the public runtime sponsor, not exit
parity or native stdout replay:

```sh
mbx build --release -p omega --message-format=json > build/quantum-cli.jsonl
python3 samples/cli/independent/quantum_wavepacket_evolution/validation/link_terminal.py build/quantum-cli.jsonl . build/quantum-terminal
python3 samples/cli/independent/quantum_wavepacket_evolution/validate.py --terminal-sponsor build/quantum-terminal
```

The sponsor links only exact metadata and libraries from that CLI build. It
prepares the accepted sample, retains its produced Terminal artifact, publishes
the native product and executes fresh Terminal requests for every sealed input.
Reports and captures live under ignored `build/validation`; a capture receipt
binds the sponsor binary, frozen inventory and captured stdout/stderr/exits.
Saved captures are reusable only with that matching receipt. NumPy and SciPy
are needed to reproduce the reference freeze, not to execute or validate the CLI.
