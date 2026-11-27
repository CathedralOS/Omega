# build_dup_build_edge

The first Build registers the same package directory under two aliases and selects `configure.omg` for the next Build. The successor imports `main.omg` through each alias and checks that both calls return 3 before it returns the application declaration.

Run `omega --check build.omg` in this directory. Removing either registration makes the successor's corresponding import unavailable. Changing the package's return value trips the successor's check.
