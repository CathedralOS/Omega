// The recorded compiler has three long runs and a cluster of shared services.
// Put the runs in separate columns and give calls and returns separate ports.
import fs from 'node:fs';
const model=JSON.parse(fs.readFileSync(0,'utf8'));
const nodes=new Map(model.nodes.map(n=>[n.id,{...n,labels:[]}]));
const width=202;
function place(id,x,y,height=38){
  Object.assign(nodes.get(id),{x,y,width,height});
}
function stack(first,last,x){
  let y=68;
  for(const id of model.spine.slice(model.spine.indexOf(first),model.spine.indexOf(last)+1)){
    const n=nodes.get(id);
    const height=n.role==='transform'?24:n.role==='owner'?34:23;
    place(id,x,y,height);
    y+=height+10;
  }
}
stack('build','accepted',22);
stack('native-0','encoded',838);
stack('native-8','native-run',1282);
const yOf=id=>nodes.get(id).y;
const services=316,runtime=581;
place('source-eval',services,yOf('compiler'));
place('engine',services,245);
place('policy-eval',services,362);
place('policy-inputs',services,482);
place('closure',services,yOf('accepted')-5);
place('selected',runtime,yOf('accepted')-5);
place('runtime-host',runtime,yOf('engine'));
place('worker',runtime,yOf('source-eval'));
place('admit',runtime,445);
place('runtime-layout',runtime,362);
place('callbacks',1060,yOf('objects')-5,34);
place('installations',1060,yOf('target')+59,36);

const port=(node,side,offset=0)=>{
  if(side==='l'||side==='r')return {x:node.x+(side==='r'?node.width:0),y:node.y+node.height/2+offset};
  return {x:node.x+node.width/2+offset,y:node.y+(side==='b'?node.height:0)};
};
const captions=new Map();
const routes=new Map();
function route(a,b,start,end,bends=[],caption=null){
  routes.set(a+'>'+b,[start,...bends,end]);
  if(caption)captions.set(a+'>'+b,caption);
}
function connect(a,b,sideA,sideB,offsetA=0,offsetB=0,via=null,caption=null){
  const start=port(nodes.get(a),sideA,offsetA),end=port(nodes.get(b),sideB,offsetB);
  route(a,b,start,end,via?via(start,end):[],caption);
}
const viaX=x=>(a,b)=>[{x,y:a.y},{x,y:b.y}];
const viaY=y=>(a,b)=>[{x:a.x,y},{x:b.x,y}];
// The first and last links in each run are real compiler handoffs too.
connect('accepted','closure','r','l');
connect('closure','selected','r','l');
connect('selected','native-0','r','l',0,0,viaX(809));
connect('encoded','native-8','r','l',0,0,viaX(1270));
function pair(a,b,direction,send,reply){
  const vertical=direction==='up'||direction==='down';
  const sides={up:['t','b'],down:['b','t'],right:['r','l'],left:['l','r']}[direction];
  const d=vertical?66:10;
  for(const [start,end,sideA,sideB,offset,text] of [
    [a,b,...sides,-d,send],[b,a,sides[1],sides[0],d,reply]]){
    const p=port(nodes.get(start),sideA,offset),q=port(nodes.get(end),sideB,offset);
    const x=(p.x+q.x)/2,y=(p.y+q.y)/2;
    route(start,end,p,q,[],{text,x,y:y+(vertical?0:offset<0?-5:14)});
  }
}
pair('compiler','source-eval','right','request','value');
pair('source-eval','engine','down','invoke','result + work');
pair('policy-eval','engine','up','invoke','result + work');
pair('policy-inputs','policy-eval','up','evaluate policy','validated plan');
pair('closure','policy-inputs','up','changed reach','repeat with inputs');
pair('runtime-host','engine','left','invoke','status');
pair('runtime-host','worker','up','external call','reply');
pair('admit','runtime-layout','up','query 02','layouts');
connect('objects','callbacks','l','r',0,0);
connect('callbacks','compose','b','l',0,0,(a,b)=>[{x:a.x,y:b.y}],{text:'private objects',x:1219,y:yOf('compose')+4});
connect('selected','admit','t','b',0,0,null,{text:'runtime inputs',x:682,y:(yOf('admit')+yOf('selected'))/2+18});
connect('admit','runtime-host','r','r',0,0,viaX(790));
connect('build','closure','r','l',0,-10,viaX(253),{text:'Build choices',x:259,y:455});
connect('accepted','runtime-host','b','b',0,-84,(a,b)=>[
  {x:a.x,y:a.y+26},{x:551,y:a.y+26},{x:551,y:b.y+44},{x:b.x,y:b.y+44}],
  {text:'publication identity',x:482,y:yOf('accepted')+47});
connect('target','installations','r','t',-6,-58,(a,b)=>[{x:b.x,y:a.y}],{text:'required handlers',x:1140,y:yOf('target')-1});
connect('installations','native-3','t','r',61,0,(a,b)=>[{x:a.x,y:b.y}],{text:'bound requirements',x:1173,y:yOf('native-3')+7});
connect('target','emit','r','l',6,0,(a,b)=>[
  {x:a.x+24,y:a.y},{x:a.x+24,y:a.y-56},{x:1246,y:a.y-56},{x:1246,y:b.y}],
  {text:'retained EmissionFacts',x:1155,y:yOf('target')-43});

const edges=model.edges.map(e=>{
  const key=e.start+'>'+e.end;
  const a=nodes.get(e.start),b=nodes.get(e.end);
  let points=routes.get(key);
  if(!points){
    if(!e.main||a.x!==b.x||a.y>=b.y)throw Error('Missing handoff layout: '+key);
    points=[port(a,'b'),port(b,'t')];
  }
  const label=captions.get(key);
  return {...e,sections:[{points}],labels:label?[label]:[]};
});
const bottom=Math.max(...[...nodes.values()].map(n=>n.y+n.height))+54;
const groups=[
  ['source','Source compilation',22,202],
  ['runtime','Evaluation & provider selection',316,467],
  ['native','Native code · 00–07',838,202],
  ['encoding','Layout & publication',1282,202],
].map(([id,title,x,w])=>({id,title,x:x-8,y:12,width:w+16,height:bottom-38,caption:{x,y:37,lines:[title]}}));
process.stdout.write(JSON.stringify({width:1508,height:bottom,nodes:[...nodes.values()],edges,groups,startLabel:{x:22,y:44}}));
