The application of `run/testing/registered_group_runs_on_both_engines` without
its test-group registration: `main.omg` is byte-identical and `build.omg` drops
only the `builder.tests.group` line. That fixture names this one in
`expected_application_baseline.txt`, so registering a group must leave the
application's Terminal program unchanged.
