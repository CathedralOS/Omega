# Generic constructor literal violates its receiving domain

The matching executable positive uses literal10. This control changes only
that constructor field to21 and must reject `value <=20` before producing
the fixed-array extent. A later array-size mismatch is not the intended refusal.
