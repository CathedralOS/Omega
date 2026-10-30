import json,re,sys
from pathlib import Path
out=Path('build/integer-premise-preparation')
name=sys.argv[1] if len(sys.argv)>1 else 'diagnostic'
records=json.loads((out/(name+'.json')).read_text())
text=(out/records[0]['transcript']).read_text(encoding='utf-8')
scopes=[dict(milliseconds=float(time),count=int(count),scope=scope) for time,count,scope in re.findall(r'^\s*([\d.]+) ms\s+(\d+) x\s+(\S+)',text,re.M)]
scopes.sort(key=lambda row:row['milliseconds'],reverse=True)
(out/(name+'-scopes.json')).write_text(json.dumps(scopes,indent=2),encoding='utf-8')
print('wall',records[0]['seconds'])
print(json.dumps([row for row in scopes if row['scope'].startswith('05.premise.')],indent=2))

counters={scope:int(count) for count,scope in re.findall(r"^\s*(\d+) n\s+(05\.premise\.\S+)",text,re.M)}
(out/(name+'-counters.json')).write_text(json.dumps(counters,indent=2),encoding='utf-8')
print('counters', counters)
