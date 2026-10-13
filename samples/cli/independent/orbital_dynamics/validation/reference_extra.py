"""Pre-execution review amendment; preserve all original frozen oracle bytes.
A radial approach crosses the central threshold although both discrete endpoints
are outside it. This distinguishes segment checking from endpoint-only refusal.
"""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
input_path=ROOT/'data/interior_collision.stdin'
input_path.write_text('0.000000001 0.001 0 -2 0 0.001 1\n')
reference=ROOT/'validation/extra_oracle.json'
reference.write_text(json.dumps({'cases':{'interior_collision':{'input':[1e-9,.001,0,-2,0,.001,1],'exit':2,'status':'collision_threshold'}},'reason':'independent pre-execution review: initial and would-be drift endpoint outside threshold, radial segment crosses origin'},indent=2)+'\n')
paths=[Path(__file__).resolve(),input_path,reference]
(ROOT/'validation/extra_frozen.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))
print('Additional interior collision control frozen before any sample execution')
