# Target-specific provider availability

All four hosted targets have a `Picker` supply. The two x86-64 targets
also have a `Delta` supply. Build binds the entry on those two targets,
where it can call both requirements. Each implementation is checked
once under its own declared requirement.
