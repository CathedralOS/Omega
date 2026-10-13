# build_domain_context

Domains are ordinary declarations inside build.omg: `domain u64::BuildPositive(value) requires value > 0` + `u64 in BuildPositive` locals flowing through the build machine itself.

Compile-checked only: run a check target, not native.
