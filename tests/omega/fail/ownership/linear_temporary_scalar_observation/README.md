# Observation does not consume a temporary owner

Reading a scalar field of a freshly constructed linear receipt observes the
receipt but does not consume it. Its claim remains live and scope exit must
reject the program, just as it does for a named receipt.
