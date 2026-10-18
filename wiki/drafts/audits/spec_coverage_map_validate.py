#!/usr/bin/env python3
"""Validate the frozen coverage inventory against this checkout; run no compiler tests.

Run in the saved audit checkout to reproduce its validation. On another revision,
changed source/fixture hashes indicate drift requiring review, not compiler failure.
"""
import json,re,hashlib,subprocess,collections
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
report=Path(__file__).with_name('spec_coverage_map.md')
data=json.loads(report.with_suffix('.json').read_text())
errors=[];checks=collections.Counter()
def need(ok,message):
 checks['assertions']+=1
 if not ok:errors.append(message)
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def file_ref(row):
 p=ROOT/row['path']; need(p.is_file(),'missing file '+str(p))
 if p.is_file():need(digest(p)==row['sha256'],'hash drift '+str(p));checks['file_hashes']+=1
tracked=subprocess.check_output(['git','ls-files','wiki/spec'],text=True,cwd=ROOT).splitlines();tracked={p for p in tracked if p.endswith('.md')}
need({d['source']['path'] for d in data['documents']}==tracked,'spec manifest does not match tracked roster')
need(subprocess.run(['git','cat-file','-e',data['source_revision']+'^{commit}'],cwd=ROOT,capture_output=True).returncode==0,'snapshot source commit is unavailable')
ids=[]
for d in data['documents']:
 file_ref(d['source'])
 if not (ROOT/d['source']['path']).is_file():continue
 lines=(ROOT/d['source']['path']).read_text().splitlines()
 headings=[{'line':i,'level':len(m[1]),'title':m[2]} for i,t in enumerate(lines,1) if (m:=re.match(r'^(#{1,6})\s+(.+?)\s*#*\s*$',t))]
 need(headings==d['sections'],'section inventory differs '+d['id'])
 need(len(lines)==d['source']['line_count'],'line count differs '+d['id'])
 need(bool(d['families']),'empty families '+d['id'])
 for f in d['families']:
  ids.append(f['id']);need(bool(f['requirement']),'empty requirement '+f['id'])
  need(f['source_path']==d['source']['path'],'wrong source '+f['id'])
  for eid in f['direct_candidate_ids']:need(eid in data['direct_candidates'],'missing candidate '+eid)
 for a in d['corpus_area_candidates']:need((ROOT/a['path']).is_dir(),'missing area '+a['path'])
 for r in d['owner_test_candidates']:file_ref(r)
need(len(ids)==len(set(ids))==1024,'family identity/count mismatch')
for e in data['direct_candidates'].values():
 for row in e['sources']:file_ref(row)
for n in data['next_qualification_slices']:
 for fid in n['requirement_ids']:need(fid in ids,'missing next-family '+fid)
for f in data['fact_inventory']:
 if f['readme']:file_ref(f['readme'])
 need((ROOT/f['root']).is_dir(),'missing fact folder '+f['root'])
 need(len({p['name'] for p in f['places']})==len(f['places']),'duplicate fact place '+f['name'])
 for p in f['places']:
  file_ref(p['source'])
  for c in p['control_candidates']:
   file_ref(c['source'])
   if c['expected']:file_ref(c['expected'])
  for r in p['regression_candidates']:need((ROOT/r).is_file(),'missing regression '+r)
for d in data['documents']:
 for f in d['fact_area_candidates']:need(any(row['name']==f for row in data['fact_inventory']),'unknown fact '+f)
md=report.read_text();anchors=set(re.findall(r'<a id="([^"]+)"',md))
need(len(re.findall(r'^#### ',md,re.M))==98,'Markdown document map does not have exactly 98 entries')
need(len(re.findall(r'^- `(?:language|proofs|resources|layouts|terminal-psi|build|packages)-[^`]+`',md,re.M))==1024,'Markdown family map count differs')
for d in data['documents']:need(md.count('<a id="'+d['id']+'"></a>')==1,'duplicate or absent Markdown document '+d['id'])
for target in re.findall(r'\[[^\]\n]+\]\(([^)\n]+)\)',md):
 p,_,anchor=target.partition('#')
 if not p:need(anchor in anchors,'bad internal anchor '+anchor)
 else:need((report.parent/p).exists(),'bad markdown path '+p)
 checks['markdown_links']+=1
for n in data['review_notes']:
 for s in n['sources']:
  p=ROOT/s['path'];need(p.is_file(),'missing note source '+str(p))
  if p.is_file():need(1<=s['line']<=len(p.read_text().splitlines()),'bad note line '+n['id'])
need(data['summary']['documents']==len(data['documents'])==98,'document summary mismatch')
need(data['summary']['sections']==sum(len(d['sections']) for d in data['documents'])==744,'section summary mismatch')
need(data['summary']['nine_place_rows_with_located_controls']==24,'unexpected control census')
for tier,expected in data['corpus_area_inventory'].items():
 actual=collections.Counter(p.parent.parent.name for p in (ROOT/'tests/omega'/tier).rglob('main.omg'))
 need(dict(actual)==expected,'corpus census drift '+tier)
result={'passed':not errors,'source_revision':data['source_revision'],'checks':dict(checks),'errors':errors}
print(json.dumps(result,indent=2))
raise SystemExit(bool(errors))
