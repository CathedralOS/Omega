# A field write retires the stored result's guarantee

An exact projected store carries its completed call-result provenance, but
later writes change both guaranteed fields. The earlier promise is not evidence
for their new values at the exit.
