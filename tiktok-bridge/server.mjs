/**
 * ClipForge TikTok OAuth bridge (Node.js 22+).
 *
 * IMPORTANT: This service must run behind HTTPS reverse proxy.
 * Set CLIENT_KEY, CLIENT_SECRET, PUBLIC_ORIGIN and TOKEN_ENCRYPTION_KEY
 * on the SERVER (not in ClipForge, GitHub variables, frontend or source).
 * It supports TikTok's video.upload scope => creator inbox drafts only.
 * Direct Post requires separate audited video.publish permission and UI.
 */
import {createServer} from 'node:http';
import {randomBytes,createHash,createCipheriv,createDecipheriv} from 'node:crypto';
import {readFileSync,writeFileSync,renameSync,mkdirSync} from 'node:fs';
import {dirname,resolve} from 'node:path';

const PORT=Number(process.env.PORT||8787);
const HOST='127.0.0.1';
const ORIGIN=(process.env.PUBLIC_ORIGIN||'').replace(/\/+$/,'');
const KEY=process.env.CLIENT_KEY||'';
const SECRET=process.env.CLIENT_SECRET||'';
const PASSWORD=process.env.TOKEN_ENCRYPTION_KEY||'';
const DB=resolve(process.env.STORE_PATH||'./data/tiktok-accounts.json');
if(!ORIGIN.startsWith('https://')||!KEY||!SECRET||!/^[a-f0-9]{64}$/i.test(PASSWORD)){
  throw Error('Required: PUBLIC_ORIGIN=https://..., CLIENT_KEY, CLIENT_SECRET, TOKEN_ENCRYPTION_KEY=64-hex');
}
const SECRET_KEY=Buffer.from(PASSWORD,'hex');
const REDIRECT=ORIGIN+'/oauth/tiktok/callback';
const SESS_TTL=30*24*60*60*1000;
const AUTH_TTL=5*60*1000;
const sessions=Object.create(null);
const pendingLinks=new Map();
const pendingStates=new Map();
const now=()=>Date.now();
const nonce=(n=32)=>randomBytes(n).toString('base64url');
const sha=s=>createHash('sha256').update(s).digest('hex');
const safeText=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function encrypt(obj){
 const iv=randomBytes(12);
 const cipher=createCipheriv('aes-256-gcm',SECRET_KEY,iv);
 const ciphertext=Buffer.concat([cipher.update(JSON.stringify(obj)),cipher.final()]);
 return {iv:iv.toString('base64'),body:ciphertext.toString('base64'),tag:cipher.getAuthTag().toString('base64')};
}
function decrypt(payload){
 const decipher=createDecipheriv('aes-256-gcm',SECRET_KEY,Buffer.from(payload.iv,'base64'));
 decipher.setAuthTag(Buffer.from(payload.tag,'base64'));
 return JSON.parse(Buffer.concat([decipher.update(Buffer.from(payload.body,'base64')),decipher.final()]).toString('utf8'));
}
try{
 const obj=JSON.parse(readFileSync(DB,'utf8'));
 for(const [id,value] of Object.entries(obj.sessions||{})){
  if(value.expires>now()&&value.accounts)sessions[id]=value;
 }
}catch(e){if(e.code!=='ENOENT')throw e}
function save(){
 mkdirSync(dirname(DB),{recursive:true,mode:0o700});
 const temp=DB+'.tmp';
 writeFileSync(temp,JSON.stringify({sessions}),{mode:0o600});
 renameSync(temp,DB);
}
function response(res,status,data){
 res.writeHead(status,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});
 res.end(JSON.stringify(data));
}
function html(res,status,message){
 res.writeHead(status,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store','Referrer-Policy':'no-referrer','X-Content-Type-Options':'nosniff'});
 res.end('<!doctype html><meta charset="utf-8"><meta name="referrer" content="no-referrer"><style>body{background:#10171d;color:#e6f7ee;font:16px system-ui;padding:48px;max-width:680px;margin:auto}h1{color:#8de8c9}</style><h1>ClipForge · TikTok</h1><p>'+safeText(message)+'</p><p>Вернись в ClipForge.</p>');
}
function jsonBody(req){
 return new Promise((yes,no)=>{
  let buf='',size=0;
  req.on('data',chunk=>{size+=chunk.length;if(size>8192){req.destroy();no(Error('Request too large'));return}buf+=chunk.toString('utf8')});
  req.on('end',()=>{try{yes(JSON.parse(buf||'{}'))}catch{no(Error('Invalid JSON'))}});
  req.on('error',no);
 });
}
function checkSession(input){
 if(typeof input!=='string'||input.length<30)return null;
 const id=sha(input),session=sessions[id];
 if(!session||session.expires<now())return null;
 return {id,session};
}
async function oauthToken(params){
 const reply=await fetch('https://open.tiktokapis.com/v2/oauth/token/',{
  method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded','Cache-Control':'no-store'},
  body:new URLSearchParams({client_key:KEY,client_secret:SECRET,...params}),signal:AbortSignal.timeout(20000)
 });
 const body=await reply.json();
 if(!reply.ok||!body.access_token)throw Error('TikTok OAuth rejected token exchange: '+String(body.error_description||body.error||reply.status));
 return body;
}
async function getTikTokUser(access){
 const reply=await fetch('https://open.tiktokapis.com/v2/user/info/?fields=open_id,display_name',{
  headers:{Authorization:'Bearer '+access},signal:AbortSignal.timeout(15000)
 });
 const body=await reply.json();
 const user=body?.data?.user;
 if(!reply.ok||body?.error?.code!=='ok'||!user?.open_id)throw Error('TikTok user.info.basic permission required');
 return user;
}
async function handle(req,res){
 const url=new URL(req.url||'/',ORIGIN);
 if(req.method==='GET'&&url.pathname==='/health'){response(res,200,{status:'ok',login:'tiktok',mode:'inbox-draft'});return}
 if(req.method==='GET'&&url.pathname==='/connect'){
  const pending=pendingLinks.get(url.searchParams.get('nonce')||'');
  if(!pending||pending.expires<now()){html(res,400,'Ссылка входа истекла. Начни подключение TikTok ещё раз.');return}
  pendingLinks.delete(url.searchParams.get('nonce'));
  const state=nonce(32);
  pendingStates.set(state,{sessionId:pending.sessionId,expires:now()+AUTH_TTL});
  const auth=new URL('https://www.tiktok.com/v2/auth/authorize/');
  auth.searchParams.set('client_key',KEY);
  auth.searchParams.set('scope','user.info.basic,video.upload');
  auth.searchParams.set('response_type','code');
  auth.searchParams.set('redirect_uri',REDIRECT);
  auth.searchParams.set('state',state);
  res.writeHead(302,{Location:auth.toString(),'Cache-Control':'no-store','Referrer-Policy':'no-referrer'});res.end();return;
 }
 if(req.method==='GET'&&url.pathname==='/oauth/tiktok/callback'){
  const state=url.searchParams.get('state')||'';
  const pending=pendingStates.get(state);
  if(!pending||pending.expires<now()){html(res,400,'Сессия авторизации истекла или не совпала.');return}
  pendingStates.delete(state);
  const session=sessions[pending.sessionId];
  if(!session||session.expires<now()){html(res,400,'Сессия ClipForge истекла.');return}
  if(url.searchParams.get('error')){html(res,400,'Вход TikTok отменён: '+url.searchParams.get('error'));return}
  const code=url.searchParams.get('code');
  if(!code){html(res,400,'TikTok не вернул код авторизации.');return}
  try{
   const token=await oauthToken({code,grant_type:'authorization_code',redirect_uri:REDIRECT});
   const scopes=String(token.scope||'').split(',').map(s=>s.trim());
   if(!scopes.includes('video.upload')||!scopes.includes('user.info.basic'))throw Error('В TikTok не предоставлены запрошенные разрешения');
   const user=await getTikTokUser(token.access_token);
   const id='tiktok:'+user.open_id;
   session.accounts[id]={name:user.display_name||'TikTok',encrypted:encrypt(token)};
   save();
   html(res,200,'Аккаунт '+(user.display_name||'TikTok')+' подключён! Вернись в ClipForge.');return;
  }catch(e){html(res,400,'TikTok OAuth: '+e.message);return}
 }
 if(req.method!=='POST'){response(res,404,{error:'not_found'});return}
 const data=await jsonBody(req);
 if(url.pathname==='/v1/session'){
  let pair=checkSession(data.pair_token);
  let token=data.pair_token;
  if(!pair){
   token=nonce(32);
   const id=sha(token);
   sessions[id]={expires:now()+SESS_TTL,accounts:Object.create(null)};
   pair={id,session:sessions[id]};
   save();
  }
  const oneTime=nonce(24);
  pendingLinks.set(oneTime,{sessionId:pair.id,expires:now()+AUTH_TTL});
  response(res,200,{pair_token:token,authorize_url:ORIGIN+'/connect?nonce='+encodeURIComponent(oneTime)});
  return;
 }
 const pair=checkSession(data.pair_token);
 if(!pair){response(res,401,{error:'invalid_session',message:'Повтори подключение TikTok'});return}
 if(url.pathname==='/v1/accounts'){
  response(res,200,{accounts:Object.entries(pair.session.accounts).map(([id,account])=>({id,name:account.name,platform:'tiktok'}))});return;
 }
 if(url.pathname==='/v1/remove'){
  const id=String(data.account_id||'');
  if(!id.startsWith('tiktok:')||!pair.session.accounts[id]){response(res,404,{error:'account_not_found'});return}
  delete pair.session.accounts[id];save();
  response(res,200,{ok:true});return;
 }
 if(url.pathname==='/v1/access'){
  const id=String(data.account_id||'');
  const account=pair.session.accounts[id];
  if(!id.startsWith('tiktok:')||!account){response(res,404,{error:'account_not_found'});return}
  let token=decrypt(account.encrypted);
  // Refresh at least 2 minutes before access expiry; refresh tokens rotate.
  if((token.expires_at||0)<now()+120000){
   token=await oauthToken({grant_type:'refresh_token',refresh_token:token.refresh_token});
   token.expires_at=now()+Number(token.expires_in||7200)*1000;
   account.encrypted=encrypt(token);save();
  }
  response(res,200,{access_token:token.access_token,expires_in:token.expires_in||7200});return;
 }
 response(res,404,{error:'not_found'});
}
const hits=new Map();
const server=createServer((req,res)=>{
 const ip=req.socket.remoteAddress||'';
 const stat=hits.get(ip)||{count:0,start:now()};
 if(now()-stat.start>60000){stat.count=0;stat.start=now()}
 stat.count++;hits.set(ip,stat);
 if(stat.count>100){response(res,429,{error:'rate_limit'});return}
 handle(req,res).catch(()=>response(res,500,{error:'server_error',message:'Проверь конфигурацию TikTok OAuth на сервере'}));
});
server.listen(PORT,HOST,()=>console.log('ClipForge TikTok OAuth bridge listening on '+HOST+':'+PORT));
