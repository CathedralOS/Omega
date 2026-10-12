# Weighted lattice

Enumerate every binary configuration of a finite open chain or periodic ring,
apply site and occupied-bond weights, and print the exact partition sum,
occupancy histogram, and first two moment numerators. The denominator is the
partition sum. All calculations use trapping integers; tolerances are zero.

Input is four ASCII characters `NABT`, followed by EOF or one LF and EOF:
`N=1..8`, site activity `A=1..3`, bond weight `B=1..3`, and topology `T=o|r`.
A ring needs at least three sites. Valid input exits 0; malformed input prints
the fixed invalid-input message and exits 64. The application owns input,
enumeration, weighting, accumulation, and decimal formatting. Its only host
service is Console input, output, and termination of its own process.

```sh
omega update --project samples/cli/independent/weighted_lattice --offline --target linux_x86_64
# Review the generated package decisions, then resume the same proposal.
omega update --resume --project samples/cli/independent/weighted_lattice --offline
omega run samples/cli/independent/weighted_lattice/main.omg < samples/cli/independent/weighted_lattice/input.txt
```

The root example is the four-site ring with activity 2 and bond weight 2.
`expected_stdout.txt` contains its complete authored output; stderr is empty.
The original validation roster has eight scientific cases (including the
256-configuration upper bound), the valid EOF variant, and ten malformed inputs.
`invalid/` separately rejects index 9 into a length-9 array.

The independent Cartesian-product reference and all original scientific input
and output bytes were recovered from the original authoring history. The frozen
`data/expected.json` SHA256 remains
`d2e3447b3b8d0a8dad85e1339aa45867b8f9baf23ef04cbcd511e094c311f131`.
An independent two-state transfer recurrence also reproduces all eight frozen
histograms and moments. Application source was recovered from literals and
explicitly recorded text edits; complete historical archive identity has not
been established. No old package decisions or lock files are transferred.

For qualification, compile the accepted application once through the ordinary
native route, then supply that exact executable to
`validate.py --route native --executable PATH`. The public SDK sponsor in
`validation/terminal_capture.rs` captures the retained Terminal artifact and
actual stdout, stderr, and exit for the same 19 inputs. Link it against the
exact compiler dependency closure with `tools/link_terminal_sponsor.py`, then
run `validate.py --route terminal --sponsor PATH`. Run the separate refusal with
`validate.py --route invalid --omega PATH`. The validator checks every frozen
file, exact stream, exit, case count, and intended diagnostic; it never accepts
package decisions or regenerates expected results.
