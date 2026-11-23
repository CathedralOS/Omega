# Captured source generates a checked, sealed artifact

The Build reads the captured bytes of `asset.txt` and publishes them as
`generated.omg` for Product checking. It also writes the same bytes to a
promised output and seals the file. Generated
scalar constants use normal typing and initializer evidence, not a private
structured-value recognizer. No authored body references the later stratum.

Run `omega build.omg` from this directory.
Explicit selection needs `--build-input main.omg --build-input build.omg
--build-input asset.txt`; already-required source members cannot be omitted.
Successful publication contains `files/generated.omg` with the asset's bytes.
