// The recorded compiler has three long runs and a cluster of shared services.
// Put the runs in separate columns and give calls and returns separate ports.
import fs from 'node:fs';
const model=JSON.parse(fs.readFileSync(0,'utf8'));
const nodes=new Map(model.nodes.map(n=>[n.id,{...n,labels:[]}]));
const width=248;
const source=54,services=504,runtime=904,native=1312,detours=1780,publication=2238;
function place(id,x,y,height=52){
  Object.assign(nodes.get(id),{x,y,width,height});
}
function stack(first,last,x){
  let y=108;
  for(const id of model.spine.slice(model.spine.indexOf(first),model.spine.indexOf(last)+1)){
    const n=nodes.get(id);
    const center=x+width/2;
    // A transform is a point on the wire. Its two cited handoffs meet here,
    // while its clickable caption sits beside the wire.
    if(n.role==='transform'){
      Object.assign(n,{x:center,y,width:0,height:0});
    }else{
      const boxWidth=n.role==='owner'?width:222;
      Object.assign(n,{x:center-boxWidth/2,y,width:boxWidth,height:n.role==='owner'?54:48});
    }
    y+=n.height+43;
  }
}
stack('build','accepted',source);
stack('native-0','encoded',native);
stack('native-8','native-run',publication);
const yOf=id=>nodes.get(id).y;
const middleY=id=>yOf(id)+nodes.get(id).height/2;
place('source-eval',services,yOf('compiler')+1);
place('engine',services,430);
place('policy-eval',services,650);
place('policy-inputs',services,872);
place('closure',services,middleY('accepted')-26);
place('selected',runtime,yOf('closure'));
place('runtime-host',runtime,yOf('engine'));
place('worker',runtime,yOf('source-eval'));
place('admit',runtime,888);
place('runtime-layout',runtime,yOf('policy-eval'));
place('callbacks',detours,middleY('objects')-26);
place('installations',detours,middleY('target')-26);

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
// The first and last links in each run are real compiler handoffs too.
connect('accepted','closure','r','l');
connect('closure','selected','r','l');
connect('selected','native-0','r','l',0,0,viaX(native-62));
connect('encoded','native-8','r','l',0,0,viaX(publication-70));
function pair(a,b,direction,send,reply){
  const vertical=direction==='up'||direction==='down';
  const sides={up:['t','b'],down:['b','t'],right:['r','l'],left:['l','r']}[direction];
  const d=vertical?78:13;
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
connect('callbacks','compose','b','l',0,0,(a,b)=>[{x:a.x,y:b.y}],
  {text:'private objects',x:detours+width-16,y:yOf('compose')-10});
connect('selected','admit','t','b',0,0,null,
  {text:'runtime inputs',x:runtime+width/2,y:(yOf('admit')+yOf('selected'))/2});
connect('admit','runtime-host','r','r',0,0,viaX(runtime+width+46));
connect('build','closure','r','l',0,-14,viaX(services-104),
  {text:'Build choices',x:services-104,y:yOf('engine')+100});
connect('accepted','runtime-host','b','b',0,-84,(a,b)=>[
  {x:a.x,y:a.y+45},{x:runtime-72,y:a.y+45},{x:runtime-72,y:b.y+64},{x:b.x,y:b.y+64}],
  {text:'publication identity',x:services+width/2,y:yOf('accepted')+82});

// Reserve the space between the two native columns for the audited detours.
// Each caption sits above its horizontal segment, outside both endpoint boxes.
connect('target','installations','r','l',8,8,null,
  {text:'required handlers',x:(native+width+detours)/2,y:middleY('target')-3});
connect('installations','native-3','b','r',0,0,(a,b)=>[{x:a.x,y:b.y}],
  {text:'bound requirements',x:(native+width+detours)/2,y:yOf('native-3')-10});
connect('target','emit','r','l',-8,0,(a,b)=>[
  {x:a.x+32,y:a.y},{x:a.x+32,y:b.y}],
  {text:'retained EmissionFacts',x:detours+width/2,y:yOf('emit')-11});

const edges=model.edges.map(e=>{
  const key=e.start+'>'+e.end;
  const a=nodes.get(e.start),b=nodes.get(e.end);
  let points=routes.get(key);
  if(!points){
    if(!e.main||a.x+a.width/2!==b.x+b.width/2||a.y>=b.y)throw Error('Missing handoff layout: '+key);
    points=[port(a,'b'),port(b,'t')];
  }
  const label=captions.get(key);
  return {...e,sections:[{points}],labels:label?[label]:[]};
});
const bottom=Math.max(...[...nodes.values()].map(n=>n.y+n.height))+54;
const groups=[
  ['source','Source compilation',source,width],
  ['runtime','Evaluation & provider selection',services,runtime+width-services],
  ['native','Native code · 00–07',native,width],
  ['encoding','Layout & publication',publication,width],
].map(([id,title,x,w])=>({id,title,x:x-8,y:12,width:w+16,height:bottom-38,caption:{x,y:44,lines:[title]}}));
process.stdout.write(JSON.stringify({width:publication+width+110,height:bottom,nodes:[...nodes.values()],edges,groups,startLabel:{x:source,y:58}}));
