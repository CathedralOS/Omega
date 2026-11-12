# Signed resident field access

The embedding entry initializes -17 through an eight-bit signed IntegerAt placement, reads -17, writes and reads -29, then extracts the sign-extended i64 -29. The host verifies byte 0xe3, unchanged initialized padding and the original allocation's unused tail.

Terminal embedding execution is implemented. Native initialization still requires concrete fitting evidence for a narrowing IntegerAt, and native scalar access refuses the narrowed codec. Cathedral's complete-width native witness does not establish this fixture.
