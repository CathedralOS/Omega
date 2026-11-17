# Original map source

`original.html` is the exact authored page from commit
`4dcadfb359b143f87df4cda6d2b0c5b55203e7ef`, path
`atlas/pages/pipeline-map.html`. Keep it unchanged.

`atlas/build_pages.py` preserves its SVG drawing, layout and colors on the
landing and established pipeline-map URL. It uses the equivalent local design
tokens/fonts and adds a short historical status/navigation strip. Historical
labels, estimates and the disproved declaration-selection claim remain visible
as part of the original; the strip links to the reviewed current account.

`map.md` owns that reviewed account. The interactive Canvas2D architecture is a
separate page; it does not replace this presentation.
