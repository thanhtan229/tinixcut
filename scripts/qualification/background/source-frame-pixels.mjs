import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {createReadStream,existsSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve,sep} from 'node:path';
import {build} from 'vite';
import {chromium} from 'playwright';
const tmp=mkdtempSync(join(tmpdir(),'tinix-frame-pixels-'));let browser,server;
try {
 const frames=Buffer.alloc(64*64*3*48);for(let n=0;n<48;n++)frames.fill(n*5,n*64*64*3,(n+1)*64*64*3);
 const video=join(tmp,'frames.webm');const ff=spawnSync(resolve('src-tauri/resources/bin/ffmpeg.exe'),['-v','error','-f','rawvideo','-pix_fmt','rgb24','-s','64x64','-r','24','-i','pipe:0','-c:v','libvpx-vp9','-lossless','1','-an','-y',video],{input:frames,maxBuffer:1024*1024});assert.equal(ff.status,0,String(ff.stderr));const bytes=readFileSync(video);
 const probe=spawnSync(resolve('src-tauri/resources/bin/ffprobe.exe'),['-v','error','-select_streams','v:0','-show_entries','frame=best_effort_timestamp_time','-of','json',video],{encoding:'utf8'});assert.equal(probe.status,0,probe.stderr);const timestamps=JSON.parse(probe.stdout).frames.map(frame=>Number(frame.best_effort_timestamp_time));assert.equal(timestamps.length,48);
 const site=join(tmp,'site');await build({configFile:false,root:process.cwd(),logLevel:'error',build:{outDir:site,rollupOptions:{input:resolve('scripts/qualification/background/source-frame-test.html')}}});
 server=createServer((req,res)=>{if(req.url==='/video.webm'){const m=/bytes=(\d+)-(\d*)/.exec(req.headers.range??'');const a=m?Number(m[1]):0,b=m&&m[2]?Math.min(Number(m[2]),bytes.length-1):bytes.length-1;res.writeHead(m?206:200,{'Content-Type':'video/webm','Accept-Ranges':'bytes','Content-Length':b-a+1,...(m?{'Content-Range':`bytes ${a}-${b}/${bytes.length}`}:{})});res.end(bytes.subarray(a,b+1));return;}const path=resolve(site,'.'+new URL(req.url,'http://localhost').pathname);if(!path.startsWith(site+sep)||!existsSync(path)){res.writeHead(404).end();return;}res.setHeader('Content-Type',path.endsWith('.html')?'text/html':path.endsWith('.css')?'text/css':'text/javascript');createReadStream(path).pipe(res);});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 browser=await chromium.launch({headless:true,args:['--mute-audio']});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/source-frame-test.html`);await page.waitForFunction(()=>document.querySelector('video')?.readyState>=2);
 const samples=[];for(let n=1;n<=30;n++){await page.getByRole('button',{name:'Next project frame'}).click();await page.waitForFunction(t=>{const v=document.querySelector('video');return !v.seeking&&v.readyState>=2&&Math.abs(v.currentTime-t)<0.00001},n*1001/30000);const pixel=await page.evaluate(()=>{const v=document.querySelector('video'),c=document.createElement('canvas');c.width=64;c.height=64;const ctx=c.getContext('2d');ctx.drawImage(v,0,0);return ctx.getImageData(32,32,1,1).data[0];});const time=n*1001/30000;const expected=timestamps.reduce((last,t,i)=>t<=time?i:last,0);samples.push({projectFrame:n,time,sourceFrame:Math.round(pixel/5),expected});}
 const mismatches=samples.filter(s=>s.sourceFrame!==s.expected);writeFileSync('artifacts/background/source-frame-pixels.json',JSON.stringify({sourceFps:24,projectFps:'30000/1001',scope:'headless Chromium canvas decoded pixels; synthetic video, paused seeks',samples,mismatches},null,2));assert.deepEqual(mismatches,[]);console.log('PASS: 30 paused seeks decoded expected 24 fps source frames on 30000/1001 project grid');
}finally{if(browser)await browser.close();if(server?.listening)await new Promise(r=>server.close(r));rmSync(tmp,{recursive:true,force:true});}
