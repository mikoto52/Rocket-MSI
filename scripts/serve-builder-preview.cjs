// Local, read-only UI preview. The browser preview cannot read projects or build MSI.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../builder/ui');
const files = {'/':'index.html','/index.html':'index.html','/app.js':'app.js','/style.css':'style.css','/icon.png':'icon.png'};
const types = {'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.png':'image/png'};
http.createServer((req,res)=>{
  const file = files[new URL(req.url, 'http://127.0.0.1').pathname];
  if(!file){res.writeHead(404).end();return;}
  res.setHeader('Content-Type', types[path.extname(file)]);
  fs.createReadStream(path.join(root,file)).pipe(res);
}).listen(4178,'127.0.0.1',()=>console.log('Builder UI preview: http://127.0.0.1:4178'));
