import json,re
from pathlib import Path
out=Path('build/remaining-first-checking')
for name in ['phase','diagnostic']:
    records_path=out/(name+'.json')
    if not records_path.exists(): continue
    records=json.loads(records_path.read_text())
    text=(out/records[0]['transcript']).read_text(encoding='utf-8')
    scopes=[dict(milliseconds=float(time),count=int(count),scope=scope) for time,count,scope in re.findall(r'^\s*([\d.]+) ms\s+(\d+) x\s+(\S+)',text,re.M)]
    scopes.sort(key=lambda row:row['milliseconds'],reverse=True)
    (out/(name+'-scopes.json')).write_text(json.dumps(scopes,indent=2),encoding='utf-8')
    print(name,'wall',records[0]['seconds'])
    print(json.dumps(scopes[:15],indent=2))
    print(json.dumps([row for row in scopes if row['scope'].startswith('05.remaining.')],indent=2))
