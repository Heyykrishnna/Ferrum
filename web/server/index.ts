import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';

import sirv from 'sirv';

import { gatewayForPort, webPort } from './gateway.ts';

const port = webPort(4173);
const gateway = gatewayForPort(port);
const staticFiles = sirv(fileURLToPath(new URL('../dist/', import.meta.url)), {
  etag: true,
});

createServer((request, response) => {
  response.setHeader('X-Content-Type-Options', 'nosniff');
  response.setHeader('Referrer-Policy', 'no-referrer');
  response.setHeader(
    'Content-Security-Policy',
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; worker-src 'self' blob:; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
  );
  void gateway(request, response, () =>
    staticFiles(request, response, () => {
      response.writeHead(404);
      response.end('Not found');
    }),
  );
}).listen(port, '127.0.0.1', () => {
  console.log(`Ferrum Playground is listening at http://127.0.0.1:${port}`);
});
