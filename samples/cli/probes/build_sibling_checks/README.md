# build_sibling_checks

Sibling pooled-check probe: three sibling packages are checked as one
occurrence batch, and each sibling's build consults the same `shared`
package through a nested build edge before staging its own generated
product source through the required-output obligation flow. The shared
build must evaluate once across the sibling batch and across every
requested target, while each sibling still seals and settles its own
obligation. The product sums the three generated constants:

```text
mbx run -p omega -- --check --target linux_x86_64 --target macos_arm64 \
    samples/cli/probes/build_sibling_checks/main.omg
```

`controls/` pins the rejection shapes around the pool: a divergent sibling
between two convergent ones (the diagnostic names the failing occurrence),
a Product edge alias imported into build scope and a Build edge alias
imported into product scope (purpose-keyed rows the request never asked
for), a non-application package binding an executable root, and a
dependency location carrying no build declaration.
