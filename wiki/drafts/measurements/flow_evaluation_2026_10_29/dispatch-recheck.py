import sys
from pathlib import Path
sys.path.insert(0,str(Path.cwd()/'wiki/drafts/measurements/checking_simplification_2026_10_28'))
import compare
compare.SUBJECTS={'dispatch':'dyn_dispatch_selector_payload_bound/match_join'}
compare.main()
