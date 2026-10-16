# Reviewed service rows

Expected native and Terminal exit: 0. The local dependency exports two checked
callables over one selected interface: `read_value` retains `ReadCapability`,
while `pure_value` retains an empty service row. Both retain their independent
`invokes Operations` ceiling. Package review must capture these separately.
The fixture under `tests/omega/pass/effects/reviewed_service_rows_exit` pins the
same program through direct corpus compilation; this sample also exercises the
real package-review and admission path.
