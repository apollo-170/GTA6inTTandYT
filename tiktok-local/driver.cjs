/* ClipForge: optional local TikTok Studio uploader.
 * No cookies are exported or printed. Each authorized account has an isolated browser profile.
 * No CAPTCHA solving, stealth plugins, proxy rotation, rate-limit evasion or login bypass.
 * This is experimental UI automation, not an official TikTok API integration.
 */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright-core');

const [mode, directory, videoFile, caption] = process.argv.slice(2);
const LOGIN_URL = 'https://www.tiktok.com/tiktokstudio/upload';
const LOGIN_WAIT_MS = 10 * 60 * 1000;
const UPLOAD_WAIT_MS = 3 * 60 * 1000;
const result = (state, detail) => process.stdout.write(JSON.stringify({state,detail})+'\n');
const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function validateInput() {
  if (!['login', 'post'].includes(mode)) throw Error('Unsupported mode');
  if (!directory || !path.isAbsolute(directory)) throw Error('Missing absolute profile path');
  if (mode === 'post') {
    if (!videoFile || !path.isAbsolute(videoFile) || !/\.mp4$/i.test(videoFile)) throw Error('Only absolute MP4 path accepted');
    const stats=fs.statSync(videoFile);
    if(!stats.isFile() || stats.size<1 || stats.size>500*1024*1024) throw Error('MP4 must be between 1B and 500MB');
    if(!caption || caption.trim().length<1) throw Error('Caption is empty');
  }
}
async function hasSession(context) {
  try {
    const cookies = await context.cookies('https://www.tiktok.com');
    // Only test existence; never read/emit the cookie value.
    return cookies.some(c => c.name === 'sessionid' && c.expires !== 0);
  } catch { return false; }
}
async function findFileInput(page) {
  const cutoff=Date.now()+60000;
  while(Date.now()<cutoff) {
    for(const frame of page.frames()) {
      try {
        const input=frame.locator('input[type="file"][accept*="video"], input[type="file"]');
        if(await input.count()) return input.first();
      }catch { /* frame may be navigating */ }
    }
    await delay(1200);
  }
  throw Error('TikTok Studio upload field was not found. Login may be required or the page layout has changed.');
}
async function fillCaption(page,text) {
  const captionText=text.trim().slice(0,150);
  for(const frame of page.frames()){
    const selectors=[
      '[contenteditable="true"][data-placeholder]',
      '[contenteditable="true"][role="textbox"]',
      '.public-DraftEditor-content[contenteditable="true"]',
      '[contenteditable="true"]',
      'textarea[placeholder*="caption" i]',
      'textarea[placeholder*="describe" i]'
    ];
    for(const selector of selectors) {
      try {
        const field=frame.locator(selector).first();
        if(await field.count() && await field.isVisible()){
          await field.click({timeout:2500});
          await field.fill(captionText,{timeout:4000});
          return;
        }
      } catch { /* different UI layout */ }
    }
  }
  throw Error('Could not locate TikTok caption field. No post was sent.');
}
async function findPostButton(page) {
  const regex=/^(Post|Publish|Опубликовать|Veröffentlichen|Publier|Publicar)$/i;
  for(const frame of page.frames()){
    try {
      const btn=frame.getByRole('button',{name:regex}).last();
      if(await btn.count()) return btn;
    }catch { /* transient frame */ }
  }
  throw Error('Post button unavailable; no post was submitted');
}
async function post(page,file,title){
  await page.goto(LOGIN_URL,{waitUntil:'domcontentloaded',timeout:60000});
  if(/login|captcha|verify/i.test(page.url())) throw Error('Sign in or verification required: use the Connect TikTok button first');
  const input=await findFileInput(page);
  await input.setInputFiles(file);
  await fillCaption(page,title);
  let button;
  const deadline=Date.now()+UPLOAD_WAIT_MS;
  while(Date.now()<deadline){
    try {
      button=await findPostButton(page);
      if(await button.isEnabled()) break;
    }catch { /* video is still processing */ }
    await delay(1500);
    button=null;
  }
  if(!button) throw Error('TikTok video processing timed out; nothing was posted');
  // One click only. The caller must never auto-retry an uncertain result.
  await button.click({timeout:15000});
  const success=/tiktokstudio\/content|tiktokstudio\/manage|post\/success|upload\/success/i;
  const confirmation=/posted|published|uploaded successfully|опубликовано|veröffentlicht|erfolgreich hochgeladen/i;
  const finish=Date.now()+90000;
  while(Date.now()<finish){
    if(success.test(page.url()))return {state:'success',detail:'TikTok Studio redirected after Post; check your profile for the new video'};
    try {
      const body=(await page.locator('body').innerText({timeout:3000})).slice(0,8000);
      if(confirmation.test(body))return {state:'success',detail:'TikTok Studio reported successful submission'};
    }catch { /* navigation */ }
    await delay(2000);
  }
  return {state:'queued',detail:'Post clicked once; confirmation was not detected. Verify the profile manually before any retry.'};
}
async function main(){
  validateInput();
  fs.mkdirSync(directory,{recursive:true});
  const context=await chromium.launchPersistentContext(directory,{
    channel:'msedge',headless:false,locale:'en-US',acceptDownloads:false,
    viewport:{width:1280,height:900},args:[],timeout:45000
  });
  try{
    const page=context.pages()[0]||await context.newPage();
    if(mode==='login'){
      await page.goto(LOGIN_URL,{waitUntil:'domcontentloaded',timeout:60000});
      let found=false;
      const expiry=Date.now()+LOGIN_WAIT_MS;
      while(Date.now()<expiry){
        if(await hasSession(context)){found=true;break}
        await delay(2000);
      }
      if(!found)throw Error('TikTok login not completed within 10 minutes. No account was connected.');
      await delay(1500);
      result('success','TikTok sign-in was detected in your isolated Edge profile. This does not verify permission to publish.');
    } else {
      if(!await hasSession(context))throw Error('TikTok login expired or account not connected. Reconnect in ClipForge.');
      const posting=await post(page,videoFile,caption);
      result(posting.state,posting.detail);
    }
  } finally {
    await context.close().catch(()=>{});
  }
}
main().catch(e=>{
  const message=String(e?.message||e).replace(/(?:sessionid|token|password)\s*[:=]\s*[^\s]+/gi,'[redacted]').slice(0,350);
  result('error',message);
  process.exitCode=1;
});
