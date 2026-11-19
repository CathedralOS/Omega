# build_same_declared_name

Two different local source roots declare `twin_kernel`. The Build gives
`dep_one` the alias `k1` and `dep_two` the alias `k2`. The Product imports
both package-root modules and calls `k1::value()` and `k2::other()`.
Their result contracts expose 7 and 9 to the caller for its exact sum.
