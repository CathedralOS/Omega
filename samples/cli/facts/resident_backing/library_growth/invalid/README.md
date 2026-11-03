# Missing vacancy

This differs from the parent program only by omitting the admitted
`VacantContentTransfer::transfer` call. A granted extent is not necessarily
vacant. Checking must reject its annotation before Vec adopts it.
