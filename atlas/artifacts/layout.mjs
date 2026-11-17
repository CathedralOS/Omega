// The main route is ordered before supporting calls are routed. Return edges
// must not reorder the compiler or stretch every stage into a separate region.
import fs from 'node:fs';
import {execFileSync} from 'node:child_process';
import ELK from 'elkjs/lib/elk.bundled.js';
const model=JSON.parse(fs.readFileSync(0,'utf8'));
const records=new Map(model.nodes.map(n=>[n.id,n]));
const main=new Set(model.spine);
const graph=await new ELK().layout({id:'spine',layoutOptions:{
  'elk.algorithm':'layered','elk.direction':'DOWN','elk.edgeRouting':'ORTHOGONAL',
  'elk.layered.spacing.nodeNodeBetweenLayers':'18','elk.spacing.nodeNode':'18',
  'elk.padding':'[top=50,left=0,bottom=20,right=0]',
},children:model.spine.map(id=>{const n=records.get(id);return{id,width:n.role==='transform'?8:220,height:n.role==='transform'?8:(model.view==='proposed'?n.height:60)};}),
 edges:model.edges.filter(e=>e.main).map(e=>({id:e.id,sources:[e.start],targets:[e.end]}))});
const positions=new Map(graph.children.map(n=>[n.id,n]));
const nodes=new Map();
let offset=0;
const phaseGaps=model.phaseGaps||{compiler:24,closure:32,'native-0':36,publish:24};
for(const [index,id] of model.spine.entries()){
  const incoming=index?model.edges.find(e=>e.main&&e.end===id):null;
  if(incoming?.label&&records.get(incoming.start).role!=='transform')offset+=Math.max(0,label(incoming.id,incoming.label).height+16-18);
  offset+=phaseGaps[id]||0;
  const n=positions.get(id);
  nodes.set(id,{...n,x:360-n.width/2,y:n.y+offset,labels:[]});
}
function place(id,x,y){
  let blocked;
  do{blocked=false;for(const n of nodes.values())if(x<n.x+n.width+20&&x+220>n.x-20&&y<n.y+n.height+24&&y+60>n.y-24){y=n.y+n.height+32;blocked=true;break;}}while(blocked);
  nodes.set(id,{id,x,y,width:220,height:60,labels:[]});
}
if(model.placements){
  for(const p of model.placements)place(p.id,p.x,nodes.get(p.anchor).y+p.offset);
}else{
place('source-eval',680,nodes.get('compiler').y);
place('policy-inputs',680,nodes.get('closure').y);
place('policy-eval',1020,nodes.get('closure').y);
place('admit',680,nodes.get('selected').y+60);
place('runtime-host',680,nodes.get('admit').y+140);
place('engine',1020,nodes.get('runtime-host').y);
place('worker',680,nodes.get('runtime-host').y+140);
place('installations',1020,nodes.get('target').y);
}
if(nodes.size!==model.nodes.length)throw Error('Every component needs an explicit place beside or on the main route');
function label(id,text,max=20){
  const lines=[];let line='';
  for(const word of text.split(' ')){if(line&&(line+' '+word).length>max){lines.push(line);line='';}line+=(line?' ':'')+word;}
  if(line)lines.push(line);
  return{id,text,lines,width:Math.max(...lines.map(s=>s.length))*9+8,height:lines.length*26};
}
const groups=(model.phases||[
  ['source-group','compiler','accepted',['1. Source','compilation','Psi 00–06']],
  ['providers-group','closure','selected',['2. Execution','selection']],
  ['native-group','native-0','image',['3. Native','compilation','Omega 00–09']],
  ['product-group','publish','native-run',['4. Publish','and run']],
]).map(([id,first,last,lines])=>({id,title:lines.join(' · '),x:150,y:nodes.get(first).y-10,width:340,
 height:nodes.get(last).y+nodes.get(last).height-nodes.get(first).y+20,
 caption:{x:16,y:nodes.get(first).y+4,lines,width:130,height:lines.length*26}}));
const reserved=[];
for(const n of nodes.values()){
  if(records.get(n.id).role==='transform'){
    const l=label(n.id+'-label',records.get(n.id).title);
    l.x=360-l.width-24;l.y=n.y+n.height/2-l.height/2;
    n.labels=[l];reserved.push(l);
  }
}
for(const g of groups)reserved.push(g.caption);
const entry=nodes.get(model.spine[0]);
const startLabel={id:'start',text:'START',lines:['START'],x:entry.x,y:entry.y-32,width:100,height:26};
reserved.push(startLabel);
const edges=model.edges.map(e=>({...e,labels:[],sections:[]}));
for(const e of edges.filter(e=>e.main)){
  const a=nodes.get(e.start),b=nodes.get(e.end);
  e.sections=[{points:[{x:a.x+a.width/2,y:a.y+a.height},{x:b.x+b.width/2,y:b.y}]}];
  if(e.label&&records.get(e.start).role!=='transform'){
    const l=label(e.id+'-label',e.label);
    l.x=360-l.width-24;l.y=(a.y+a.height+b.y)/2-l.height/2;
    e.labels=[l];reserved.push(l);
  }
}
// neato -n2 routes around the fixed boxes without rearranging the spine.
// Text on the spine is also an obstacle, rather than a wire crossing a label.
if(edges.some(e=>!e.main)){
const height=Math.max(...[...nodes.values()].map(n=>n.y+n.height))+40;
const quote=JSON.stringify;
const dot=['digraph G {','graph [splines=ortho,overlap=true,margin=0,pad=0];','node [shape=box,fixedsize=true,label="",margin=0];'];
const fixed=(id,n)=>`${quote(id)} [pos="${n.x+n.width/2},${height-n.y-n.height/2}!",width=${n.width/72},height=${n.height/72}];`;
for(const n of nodes.values())dot.push(fixed(n.id,n));
for(const [i,l] of reserved.entries())dot.push(fixed('_text_'+i,l));
for(const e of edges.filter(e=>!e.main))dot.push(`${quote(e.start)} -> ${quote(e.end)} [id=${quote(e.id)}];`);
dot.push('}');
const routed=JSON.parse(execFileSync('neato',['-n2','-Tjson'],{input:dot.join('\n'),encoding:'utf8',maxBuffer:8*1024*1024}));
const reference=routed.objects.find(n=>n.name===entry.id).pos.split(',').map(Number);
const dx=reference[0]-(entry.x+entry.width/2),dy=reference[1]-(height-entry.y-entry.height/2);
const point=([x,y])=>({x:Number((x-dx).toFixed(3)),y:Number((height-y+dy).toFixed(3))});
const routes=new Map(routed.edges.map(e=>[e.id,e]));
for(const e of edges.filter(e=>!e.main)){
  const r=routes.get(e.id);if(!r)throw Error('Missing routed connection: '+e.id);
  const draws=r._draw_.filter(d=>d.op==='b');
  const points=draws.flatMap(d=>d.points.map(point));
  const head=r.pos.split(/\s+/).find(s=>s.startsWith('e,'));
  if(head)points.push(point(head.slice(2).split(',').map(Number)));
  e.sections=[{points:points.filter((p,i,a)=>i===0||p.x!==a[i-1].x||p.y!==a[i-1].y)}];
}
}
const occupied=[...nodes.values(),...reserved].map(n=>({...n}));
const overlap=(a,b)=>a.x<b.x+b.width+5&&a.x+a.width+5>b.x&&a.y<b.y+b.height+5&&a.y+a.height+5>b.y;
for(const e of edges.filter(e=>!e.main&&e.label)){
  const l=label(e.id+'-label',e.label);
  const candidates=[];
  for(const s of e.sections)for(let i=1;i<s.points.length;i++){
    const a=s.points[i-1],b=s.points[i],length=Math.abs(a.x-b.x)+Math.abs(a.y-b.y);
    if(length<12)continue;
    for(const t of [0.5,0.3,0.7]){
      const x=a.x+(b.x-a.x)*t,y=a.y+(b.y-a.y)*t;
      for(const gap of [8,20,36,60]){
        const locations=a.y===b.y?[[x-l.width/2,y-l.height-gap],[x-l.width/2,y+gap]]:[[x-l.width-gap,y-l.height/2],[x+gap,y-l.height/2]];
        for(const [lx,ly] of locations)candidates.push({...l,x:lx,y:ly,cost:1000/(length+1)+Math.abs(t-.5)*10+gap/8});
      }
    }
  }
  const chosen=candidates.sort((a,b)=>a.cost-b.cost).find(c=>!occupied.some(o=>overlap(c,o)));
  if(!chosen)throw Error('No clear label position: '+e.id);
  // Supporting labels appear one at a time when their arrow is selected.
  e.labels=[chosen];
}
const allLabels=[...reserved,...edges.flatMap(e=>e.labels)];
const allPoints=edges.flatMap(e=>e.sections.flatMap(s=>s.points));
const minX=Math.min(0,...allLabels.map(l=>l.x),...allPoints.map(p=>p.x));
const minY=Math.min(0,...allLabels.map(l=>l.y),...allPoints.map(p=>p.y));
const sx=20-minX,sy=20-minY;
for(const n of nodes.values()){n.x+=sx;n.y+=sy;for(const l of n.labels){l.x+=sx;l.y+=sy;}}
for(const g of groups){g.x+=sx;g.y+=sy;g.caption.x+=sx;g.caption.y+=sy;}
startLabel.x+=sx;startLabel.y+=sy;
for(const e of edges){for(const s of e.sections)for(const p of s.points){p.x+=sx;p.y+=sy;}for(const l of e.labels){l.x+=sx;l.y+=sy;}}
const width=Math.max(...[...nodes.values()].map(n=>n.x+n.width),...allPoints.map(p=>p.x),...allLabels.map(l=>l.x+l.width))+30;
const bottom=Math.max(...[...nodes.values()].map(n=>n.y+n.height),...allPoints.map(p=>p.y),...allLabels.map(l=>l.y+l.height))+30;
process.stdout.write(JSON.stringify({width,height:bottom,nodes:[...nodes.values()],edges,groups,startLabel}));
