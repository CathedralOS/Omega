# Base source cannot capture a later generated declaration

Build seals and includes a scalar constant as generated source. The authored
Product body tries to reference it, which the two-stratum rule forbids. An
authored wrapper cannot evade the generated-entry staging boundary; see
`wiki/spec/build/scoped_execution.md`. No successful product is published.
