# Projected case beyond native load displacement

`omega run --both main.omg` must report native and interpreter exit 70 after
package preparation (`omega update --project . --dangerously-accept-all`).
The selected three-case sum follows 32768 bytes of padding. Tag and payload
loads need derived-address staging on AArch64; initializing the padding also
exceeds the direct byte-store displacement. Each access retains its original
root-relative semantic byte offset.
