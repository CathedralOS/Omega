import json,sys
u=json.load(open('fdn/usage.json'))
c=sys.argv[1]; mods=sys.argv[2:]
for k,v in sorted(u['usage2'].get(c,{}).items()):
  mod=k.split('::')[0]
  if mods and mod not in mods: continue
  ps=sorted(set(u['pos'].get(x,x) for x in v))
  print(f"{k} ({len(ps)}): {', '.join(ps)}")
