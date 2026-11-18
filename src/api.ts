import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';

export type Tab = 'dashboard'|'streams'|'highlights'|'studio'|'publishing'|'settings';
export type Channel = { id:string; login:string; display_name:string; enabled:boolean; rights_confirmed:boolean; is_live:boolean; messages:number; last_score:number };
export type Highlight = {id:number;channel_id:string;channel_login:string;title:string;score:number;length:number;detected_at:string;status:string;clip_id:string|null;clip_url:string|null;source_path:string|null;output_path:string|null;explanation:string|null};
export type Settings = {auto_publish:boolean;auto_publish_confirmed:boolean;auto_targets:string[];auto_youtube_privacy:'private'|'unlisted'|'public';auto_tiktok_mode:'MEDIA_UPLOAD'|'DIRECT_POST';auto_daily_limit:number;client_id:string;demo_mode:boolean;auto_clip:boolean;auto_export:boolean;threshold:number;cooldown_seconds:number;output_dir:string;banner_path:string;ffmpeg_path:string;vision_endpoint:string;vision_model:string};
export type Snapshot = { settings:Settings; channels:Channel[]; highlights:Highlight[]; monitoring:boolean; authenticated_as:string|null };
export type Device = {device_code:string;user_code:string;verification_uri:string;interval:number};
export type HostedState={connected:boolean;provider:string;error:string|null};
export type ConnectedAccount={id:string;name:string;platform:'youtube'|'tiktok'};
export type PublishResult={account_id:string;account_name:string;platform:'youtube'|'tiktok'|string;state:'queued'|'uploading'|'success'|'draft'|'error';detail:string};
export type UpdateStatus={current_version:string;latest_version:string|null;available:boolean;can_install:boolean;release_url:string;download_url:string|null;message:string};
export type UpdateProgress={stage:string;percent:number};
export type IntegrationDiagnostics={twitch_configured:boolean;twitch_connected:boolean;youtube_configured:boolean;youtube_accounts:number;tiktok_configured:boolean;ffmpeg_ready:boolean;update_feed_public:boolean};
const defaults:Settings={auto_publish:false,auto_publish_confirmed:false,auto_targets:[],auto_youtube_privacy:'private',auto_tiktok_mode:'MEDIA_UPLOAD',auto_daily_limit:5,client_id:'',demo_mode:true,auto_clip:false,auto_export:false,threshold:68,cooldown_seconds:120,output_dir:'',banner_path:'',ffmpeg_path:'ffmpeg',vision_endpoint:'https://api.openai.com/v1/chat/completions',vision_model:''};
export const desktop=isTauri();
const key='clipforge-browser-demo-v1';
let browser:Snapshot=load();
function load():Snapshot {try {const saved=localStorage.getItem(key);if(saved){const p=JSON.parse(saved) as Snapshot;return {...p,monitoring:false,authenticated_as:null}}}catch{ /* browser blocked storage */ }
 return {settings:{...defaults},channels:[],highlights:[],monitoring:false,authenticated_as:null};}
function persist(){try{localStorage.setItem(key,JSON.stringify(browser));}catch{/* noop */}}
export async function getSnapshot():Promise<Snapshot>{return desktop?invoke<Snapshot>('snapshot'):JSON.parse(JSON.stringify(browser));}
export async function saveSettings(settings:Settings){if(desktop)return invoke<void>('save_settings',{settings});browser.settings={...settings};persist()}
export async function addChannel(login:string):Promise<Channel>{if(desktop)return invoke<Channel>('add_channel',{login});const clean=login.trim().replace(/^@/,'').toLowerCase();if(!/^[a-z0-9_]+$/.test(clean))throw Error('Неверный ник Twitch');const c:Channel={id:'demo:'+clean,login:clean,display_name:clean,enabled:true,rights_confirmed:false,is_live:true,messages:0,last_score:0};browser.channels=[c,...browser.channels.filter(x=>x.id!==c.id)];persist();return c}
export async function removeChannel(id:string){if(desktop)return invoke<void>('remove_channel',{id});browser.channels=browser.channels.filter(c=>c.id!==id);persist()}
export async function updateChannel(id:string,enabled:boolean,rights_confirmed:boolean){if(desktop)return invoke<void>('update_channel',{id,enabled,rightsConfirmed:rights_confirmed});browser.channels=browser.channels.map(c=>c.id===id?{...c,enabled,rights_confirmed}:c);persist()}
export async function startMonitor(){if(desktop)return invoke<void>('start_monitor');if(!browser.channels.some(c=>c.enabled))throw Error('Сначала добавь стримера');browser.monitoring=true;persist()}
export async function stopMonitor(){if(desktop)return invoke<void>('stop_monitor');browser.monitoring=false;persist()}
export async function demoBurst(){if(desktop)return invoke<void>('demo_burst');const c=browser.channels.find(c=>c.enabled);if(!c)throw Error('Сначала добавь стримера');if(!browser.monitoring)throw Error('Сначала включи мониторинг');const score=88;browser.highlights.unshift({id:Date.now(),channel_id:c.id,channel_login:c.login,title:`${c.display_name} — яркий момент #shorts`,score,length:55,detected_at:new Date().toISOString(),status:'detected',clip_id:null,clip_url:null,source_path:null,output_path:null,explanation:'Demo: 24 сообщения за 10 с, всплеск чата'});browser.channels=browser.channels.map(x=>x.id===c.id?{...x,messages:x.messages+24,last_score:score}:x);persist()}
export async function beginDevice():Promise<Device>{if(!desktop)throw Error('OAuth доступен в Windows-приложении');return invoke<Device>('twitch_device_start')}
export async function pollDevice():Promise<{display_name:string;user_id:string}|null>{if(!desktop)return null;return invoke<{display_name:string;user_id:string}|null>('twitch_device_poll')}
export async function disconnect(){if(desktop)return invoke<void>('disconnect_twitch')}
export async function createClip(id:number){if(!desktop)throw Error('Создание Twitch-клипа доступно только в Windows-приложении');return invoke<void>('create_twitch_clip',{id})}
export async function pickFile(filters:{name:string;extensions:string[]}[]):Promise<string|null>{if(!desktop)throw Error('Выбор локального файла доступен в Windows-приложении');const selected=await open({multiple:false,filters});return typeof selected==='string'?selected:null}
export async function pickDirectory():Promise<string|null>{if(!desktop)throw Error('Выбор папки доступен в Windows-приложении');const selected=await open({directory:true,multiple:false});return typeof selected==='string'?selected:null}
export async function attachVideo(id:number,path:string){if(!desktop)throw Error('Только Windows-приложение');return invoke<void>('attach_video',{id,path})}
export async function renderVideo(id:number):Promise<string>{if(!desktop)throw Error('FFmpeg доступен в Windows-приложении');return invoke('render_highlight',{id})}
export async function analyzeVideo(id:number,apiKey:string){if(!desktop)throw Error('Vision доступен в Windows-приложении');return invoke<string>('analyze_highlight',{id,apiKey})}
export async function githubUpdateTokenStatus():Promise<boolean>{if(!desktop)return false;return invoke<boolean>('github_update_token_status')}
export async function githubUpdateTokenSave(token:string):Promise<void>{if(!desktop)throw Error('Только Windows');return invoke<void>('github_update_token_save',{token})}
export async function githubUpdateTokenForget():Promise<void>{if(!desktop)throw Error('Только Windows');return invoke<void>('github_update_token_forget')}
export async function hostedStatus():Promise<HostedState>{if(!desktop)return {connected:false,provider:'upload-post',error:'Только в Windows'};return invoke<HostedState>('upload_post_status')}
export async function hostedSaveKey(apiKey:string):Promise<void>{if(!desktop)throw Error('Только в Windows');return invoke<void>('upload_post_save_key',{apiKey})}
export async function hostedForgetKey():Promise<void>{if(!desktop)throw Error('Только в Windows');return invoke<void>('upload_post_forget_key')}
export async function hostedConnect(profile:string):Promise<string>{if(!desktop)throw Error('Только в Windows');return invoke<string>('upload_post_connect',{profile})}
export async function hostedAccounts():Promise<ConnectedAccount[]>{if(!desktop)return [];return invoke<ConnectedAccount[]>('upload_post_accounts')}
export async function connectedAccounts():Promise<ConnectedAccount[]>{if(!desktop)return [];return invoke<ConnectedAccount[]>('connected_accounts')}
export async function beginTikTok():Promise<string>{if(!desktop)throw Error('TikTok OAuth доступен в Windows-приложении');return invoke<string>('tiktok_oauth_start')}
export async function watchPublishProgress(callback:(result:PublishResult)=>void){if(!desktop)return ()=>{};return listen<PublishResult>('clipforge:publish-progress',e=>callback(e.payload))}
export async function publishBatch(id:number,accountIds:string[],title:string,privacy:string,confirmed:boolean):Promise<PublishResult[]>{
 if(!desktop)throw Error('Публикация доступна в Windows-приложении');
 return invoke<PublishResult[]>('publish_batch',{id,accountIds,title,privacy,confirmed});
}
export async function beginYouTube():Promise<string>{if(!desktop)throw Error('OAuth доступен в Windows-приложении');return invoke<string>('youtube_oauth_start')}
export async function removeAccount(id:string):Promise<void>{if(!desktop)throw Error('Только в Windows-приложении');return invoke('disconnect_social_account',{id})}
export async function integrationDiagnostics():Promise<IntegrationDiagnostics>{
 if(!desktop)return {twitch_configured:false,twitch_connected:false,youtube_configured:false,youtube_accounts:0,tiktok_configured:false,ffmpeg_ready:false,update_feed_public:false};
 return invoke<IntegrationDiagnostics>('integration_diagnostics')
}
export async function installUpdate():Promise<string>{if(!desktop)throw Error('Автообновление доступно только в Windows');return invoke<string>('update_install')}
export async function watchUpdateProgress(callback:(progress:UpdateProgress)=>void){if(!desktop)return ()=>{};return listen<UpdateProgress>('clipforge:update-progress',e=>callback(e.payload))}
export async function checkUpdates():Promise<UpdateStatus>{if(!desktop)return {current_version:'0.6.0',latest_version:null,available:false,can_install:false,release_url:'https://github.com/apollo-170/GTA6inTTandYT/releases',download_url:null,message:'Проверка обновлений доступна в Windows-приложении'};return invoke<UpdateStatus>('update_check')}
export async function publishVideo(id:number,accountId:string,title:string,privacy:string,confirmed:boolean){if(!desktop)throw Error('Публикация доступна в Windows-приложении');return invoke<string>('publish_highlight',{id,accountId,title,privacy,confirmed})}
export async function externalUrl(url:string){if(desktop){await openUrl(url)}else{window.open(url,'_blank','noopener,noreferrer')}}
export async function watchRefresh(refresh:()=>void,notice:(str:string)=>void,connected?:(str:string)=>void){if(!desktop)return ()=>{};const a=await listen('clipforge:refresh',refresh);const b=await listen<string>('clipforge:notice',(e)=>notice(e.payload));const c=await listen<string>('clipforge:connected',(e)=>(connected||notice)(e.payload));return ()=>{a();b();c()}}
