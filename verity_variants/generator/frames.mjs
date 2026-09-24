import {chromium} from 'playwright';import fs from 'fs';
// usage: node frames.mjs variants w h mode   mode=stills|video|export
const [vs,w,h,mode]=[process.argv[2],+process.argv[3],+process.argv[4],process.argv[5]];
const b=await chromium.launch({args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const p=await b.newPage({viewport:{width:w,height:h}});p.on('pageerror',e=>console.log('ERR',e.message));p.on('console',m=>m.type()==='error'&&console.log(m.text()));
const plan={Idle:3,Walk:2.4,Wave:2};
for(const v of vs.split(',')){
 await p.goto(`http://127.0.0.1:8123/index.html?v=${v}&w=${w}&h=${h}`);await p.waitForFunction('window.done',null,{timeout:180000});
 if(mode==='stills'){for(const [c,t] of [['Idle',0.4],['Walk',0.3],['Wave',0.5]]){await p.evaluate(([c,t])=>renderAt(c,t),[c,t]);await p.screenshot({path:`st_${v}_${c}.png`});}}
 if(mode==='video'){fs.mkdirSync(`fr_${v}`,{recursive:true});let i=0;
  for(const [c,d] of Object.entries(plan))for(let f=0;f<d*24;f++){await p.evaluate(([c,t])=>renderAt(c,t),[c,f/24]);await p.screenshot({path:`fr_${v}/${String(i++).padStart(4,'0')}.png`});}}
 if(mode==='export')fs.writeFileSync(`verity_monster_${v}.glb`,Buffer.from(await p.evaluate('exportGLB()'),'base64'));
 console.log('ok',v,mode);}
await b.close();
