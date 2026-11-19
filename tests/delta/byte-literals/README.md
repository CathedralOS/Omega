# Byte literals for the next compiler's source encoder

Delta byte literals let the checked Epsilon-to-Delta compiler write readable
source templates using the existing `Bytes` meaning and runtime. The production
encoder is `bootstrap/independent/epsilon/emission.delta`; these two Delta entries
exercise its name/signature/body rendering and execute the emitted function.
They do not claim that the Epsilon compiler pass is complete or that its entire
bootstrap compiler customer has been compiled.

Run from the repository root on Linux x86-64:

```sh
python3 bootstrap/independent/successor_test.py --literals --omega
```

`--reuse-constructed` explicitly reuses the existing source-constructed native
Gamma compiler after identity checks. Controls cover exact decoded bytes, all
escapes, EOF/closing-token boundaries, scalar-profile runtime inclusion, malformed
literals, and expression/name/head/type roles. The emitted program contains
shared semantic type declarations and generated functions; it contains no
Epsilon source parser or expression/state dispatcher. The ordinary Delta and
Epsilon customers also run, including the bound canonical Epsilon receipt.
