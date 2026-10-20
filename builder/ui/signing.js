'use strict';
function signingPanel(){
 const p='Build/Signing',enabled=attr(p,'enabled','false')==='true',source=attr(p,'source','pfx');
 return card('MSI 전자서명','빌드 후 SHA-256으로 서명하고 검증합니다. Windows SDK SignTool이 필요합니다.',`<div class="grid">${check('MSI에 전자서명 적용',p,'enabled')}${enabled?field('인증서 방식',p,'source','',[['pfx','PFX 파일'],['store','Windows 인증서 저장소']],'wide')+(source==='pfx'?field('PFX 파일',p,'pfxFile','암호는 빌드할 때 입력하며 프로젝트에 저장하지 않습니다.',null,'wide browse'):field('인증서 저장소',p,'storeLocation','',[['currentUser','현재 사용자 · 개인'],['localMachine','로컬 컴퓨터 · 개인']],'wide')+field('인증서 지문 (SHA-1)',p,'thumbprint','',null,'wide')+'<button id="select-certificate" class="plain">인증서 선택…</button>')+field('타임스탬프 서버 (선택)',p,'timestampUrl','RFC 3161 HTTP(S) 주소. 비워두면 타임스탬프 없이 서명합니다.',null,'wide'):''}</div>`);
}
function signingPassword(){
 return new Promise(resolve=>{
  const dialog=document.createElement('dialog');
  dialog.innerHTML='<form method="dialog"><h2>PFX 암호</h2><p>서명 인증서의 암호를 입력하세요. 암호는 저장하지 않습니다.</p><input type="password" aria-label="PFX 암호" autocomplete="off"><div class="modal-actions"><button type="button" id="password-cancel">취소</button><button class="primary" value="sign">서명하고 빌드</button></div></form>';
  dialog.querySelector('#password-cancel').onclick=()=>dialog.close('cancel');
  dialog.addEventListener('close',()=>{const value=dialog.returnValue==='sign'?dialog.querySelector('input').value:null;dialog.querySelector('input').value='';dialog.remove();resolve(value);},{once:true});
  document.body.append(dialog);dialog.showModal();dialog.querySelector('input').focus();
 });
}
document.addEventListener('input',e=>{
 const bind=e.target.dataset.bind;
 if(!bind?.startsWith('Build/Signing|'))return;
 const p='Build/Signing';
 if(!attr(p,'source'))set(p,'source','pfx');
 if(!attr(p,'storeLocation'))set(p,'storeLocation','currentUser');
 if(['enabled','source','storeLocation'].includes(bind.split('|')[1]))renderPanel();
});
document.addEventListener('click',async e=>{
 if(e.target.closest('button')?.id!=='select-certificate'||busy)return;
 await task(async()=>{
  const certs=await invoke('list_certificates',{storeLocation:attr('Build/Signing','storeLocation','currentUser')});
  if(!certs.length)throw new Error('사용 가능한 코드 서명 인증서가 없습니다. 개인 키와 유효기간을 확인하세요.');
  $('#entry-title').textContent='서명 인증서 선택';$('#entry-error').textContent='';
  $('#entry-fields').innerHTML=`<div class="field wide"><label>인증서 · 만료일 · 지문</label><select id="signing-cert">${certs.map(c=>`<option value="${esc(c.thumbprint)}">${esc(c.subject)} · ${esc(c.expires)} · ${esc(c.thumbprint)}</option>`).join('')}</select></div>`;
  entrySave=async()=>{set('Build/Signing','thumbprint',$('#signing-cert').value);$('#entry-dialog').close();renderPanel();};
  $('#entry-dialog').showModal();
 });
});
