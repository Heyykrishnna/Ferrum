import type { IncomingMessage, ServerResponse } from 'node:http';
import { pipeline } from 'node:stream/promises';

import { createSchema } from '../shared/contracts.ts';

const maxBodyBytes = 800 * 1024;
const submissionRoute =
  /^\/api\/submissions\/[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}(\/cancel)?$/i;
const idempotencyKey = /^[\w.:-]{1,128}$/;

interface GatewayOptions {
  port: number;
  upstream?: string;
  token?: string;
}

class RequestError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export function webPort(fallback: number) {
  const port = Number(process.env.FERRUM_WEB_PORT ?? fallback);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('FERRUM_WEB_PORT must be an integer from 1 to 65535.');
  }
  return port;
}

async function readSubmission(request: IncomingMessage) {
  if (
    request.headers['content-type']?.split(';')[0].trim().toLowerCase() !==
    'application/json'
  ) {
    throw new RequestError(415, 'Send the submission as JSON.');
  }
  let size = 0;
  const chunks: Buffer[] = [];
  for await (const chunk of request.iterator({ destroyOnReturn: false })) {
    if (!Buffer.isBuffer(chunk))
      throw new RequestError(400, 'Invalid submission body.');
    size += chunk.length;
    if (size > maxBodyBytes) {
      request.resume();
      throw new RequestError(
        413,
        'The submission exceeds the 800 KiB request limit.',
      );
    }
    chunks.push(chunk);
  }
  let body: unknown;
  try {
    body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    throw new RequestError(400, 'The submission body must be valid JSON.');
  }
  const submission = createSchema.safeParse(body);
  if (!submission.success) {
    throw new RequestError(
      400,
      submission.error.issues[0]?.message ?? 'Invalid submission.',
    );
  }
  return JSON.stringify(submission.data);
}

function sendError(response: ServerResponse, status: number, message: string) {
  response.writeHead(status, {
    'Content-Type': 'application/json',
    'Cache-Control': 'no-store',
  });
  response.end(JSON.stringify({ error: { message } }));
}

export function createGateway({
  port,
  upstream = 'http://127.0.0.1:8080',
  token,
}: GatewayOptions) {
  let api: URL;
  try {
    api = new URL(upstream);
  } catch {
    throw new Error('FERRUM_WEB_API_URL must be an HTTP or HTTPS origin.');
  }
  const local = ['127.0.0.1', 'localhost', '[::1]'].includes(api.hostname);
  if (
    !(api.protocol === 'https:' || (api.protocol === 'http:' && local)) ||
    api.username ||
    api.password ||
    api.pathname !== '/' ||
    api.search ||
    api.hash
  ) {
    throw new Error(
      'FERRUM_WEB_API_URL must be a loopback HTTP origin or an HTTPS origin without credentials.',
    );
  }

  const hosts = new Set([`127.0.0.1:${port}`, `localhost:${port}`]);

  return async (
    request: IncomingMessage,
    response: ServerResponse,
    next: () => void,
  ) => {
    const host = request.headers.host;
    if (!host || !hosts.has(host)) {
      sendError(response, 403, 'Use the local playground address.');
      return;
    }
    const path = request.url ?? '';
    if (!path.startsWith('/api/')) {
      next();
      return;
    }
    const method = request.method;
    const requestOrigin = request.headers.origin;
    if (
      request.headers['sec-fetch-site'] === 'cross-site' ||
      (requestOrigin !== undefined && requestOrigin !== `http://${host}`) ||
      (method === 'POST' && requestOrigin === undefined)
    ) {
      sendError(response, 403, 'Send requests from the local playground.');
      return;
    }

    const match = submissionRoute.exec(path);
    const isCreate = path === '/api/submissions' && method === 'POST';
    const isRead =
      method === 'GET' &&
      (path === '/api/languages' ||
        path === '/api/execution/status' ||
        (match && !match[1]));
    const isCancel = method === 'POST' && match?.[1] === '/cancel';
    if (!(isCreate || isRead || isCancel)) {
      sendError(response, 404, 'This playground endpoint does not exist.');
      return;
    }
    if (!token) {
      sendError(
        response,
        503,
        'Set the Ferrum API token in the server environment.',
      );
      return;
    }

    try {
      const headers = new Headers({
        Authorization: `Bearer ${token}`,
        Accept: 'application/json',
      });
      let body: string | undefined;
      if (isCreate) {
        const key = request.headers['idempotency-key'];
        if (typeof key !== 'string' || !idempotencyKey.test(key)) {
          throw new RequestError(400, 'Supply a valid idempotency key.');
        }
        body = await readSubmission(request);
        headers.set('Idempotency-Key', key);
        headers.set('Content-Type', 'application/json');
      }
      const upstreamResponse = await fetch(
        new URL(`/v1${path.slice(4)}`, api),
        {
          method,
          headers,
          body,
          redirect: 'error',
          signal: AbortSignal.timeout(10_000),
        },
      );
      if (
        !upstreamResponse.headers
          .get('content-type')
          ?.startsWith('application/json') ||
        !upstreamResponse.body
      ) {
        throw new RequestError(502, 'Ferrum returned an unexpected response.');
      }
      response.writeHead(upstreamResponse.status, {
        'Content-Type': 'application/json',
        'Cache-Control': 'no-store',
        'X-Content-Type-Options': 'nosniff',
      });
      await pipeline(upstreamResponse.body, response);
    } catch (error) {
      if (response.headersSent || response.destroyed) return;
      if (error instanceof RequestError)
        sendError(response, error.status, error.message);
      else
        sendError(
          response,
          502,
          'Could not reach Ferrum. Check that its API is running.',
        );
    }
  };
}

export function gatewayForPort(port: number) {
  return createGateway({
    port,
    upstream: process.env.FERRUM_WEB_API_URL,
    token: process.env.FERRUM_WEB_API_TOKEN || process.env.FERRUM_DEV_TOKEN,
  });
}
