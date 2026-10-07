# Captured source generates a checked, sealed artifact

The build reads an immutable source asset, copies its bytes into a required
output, seals it and includes it for ordinary Product checking. Generated
scalar constants use normal typing and initializer evidence, not a private
structured-value recognizer. No authored body references the later stratum.

Run `omega tests/omega/pass/build/captured_source_generated_constant/main.omg`.
Explicit selection needs `--build-input main.omg --build-input build.omg
--build-input asset.txt`; already-required source members cannot be omitted.
Successful publication contains `files/generated.omg` with the asset's bytes.
