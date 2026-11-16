import { chromium } from 'playwright';
const response = await fetch('http://127.0.0.1:9455/json/version', {signal:AbortSignal.timeout(2000)}).catch(error => {
  if(error.cause?.code === 'ECONNREFUSED')return null;
  throw error;
});
if(response){
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9455',{timeout:3000});
 try{const session=await browser.newBrowserCDPSession();await session.send('Browser.close');}
 finally{await browser.close();}
}
