import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { createServer } from 'node:http';
import type { RequestListener, Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { test } from 'node:test';

import { createGateway } from './gateway.ts';

async function listen(server: Server) {
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  return (server.address() as AddressInfo).port;
}

async function fixture(upstreamHandler: RequestListener) {
  const upstream = createServer(upstreamHandler);
  const upstreamPort = await listen(upstream);
  const server = createServer((request, response) => {
    void gateway(request, response, () => response.writeHead(404).end());
  });
  const port = await listen(server);
  const origin = `http://127.0.0.1:${port}`;
  const gateway = createGateway({
    port,
    upstream: `http://127.0.0.1:${upstreamPort}`,
    token: 'fixture-token',
  });
  return {
    origin,
    close: () => {
      server.close();
      upstream.close();
    },
  };
}

const requestHeaders = (origin: string, key: string) => ({
  Origin: origin,
  'Content-Type': 'application/json',
  'Idempotency-Key': key,
});

test('Run and Submit relay distinct payloads and keep the token server-side', async (context) => {
  const seen: {
    path?: string;
    authorization?: string;
    key?: string | string[];
    payload: unknown;
  }[] = [];
  const id = randomUUID();
  const target = await fixture((request, response) => {
    const chunks: Buffer[] = [];
    request.on('data', (data: Buffer) => chunks.push(data));
    request.on('end', () => {
      seen.push({
        path: request.url,
        authorization: request.headers.authorization,
        key: request.headers['idempotency-key'],
        payload: chunks.length
          ? JSON.parse(Buffer.concat(chunks).toString())
          : undefined,
      });
      const cancel = request.url?.endsWith('/cancel');
      response.writeHead(request.method === 'POST' && !cancel ? 202 : 200, {
        'Content-Type': 'application/json',
      });
      response.end(
        JSON.stringify(
          cancel
            ? { id, outcome: 'cancelled' }
            : { id, state: 'created', verdict: null },
        ),
      );
    });
  });
  context.after(target.close);

  const run = {
    language: 'python',
    source: 'print(input())',
    stdin: 'hello\n',
  };
  const submit = {
    ...run,
    expected_output: 'hello\n',
    comparison_mode: 'exact',
  };
  for (const payload of [run, submit]) {
    const key = randomUUID();
    const response = await fetch(`${target.origin}/api/submissions`, {
      method: 'POST',
      headers: requestHeaders(target.origin, key),
      body: JSON.stringify(payload),
    });
    assert.equal(response.status, 202);
    assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.equal(response.headers.has('authorization'), false);
    assert.equal((await response.json()).id, id);
    assert.deepEqual(seen.at(-1), {
      path: '/v1/submissions',
      authorization: 'Bearer fixture-token',
      key,
      payload,
    });
  }

  const retrieved = await fetch(`${target.origin}/api/submissions/${id}`);
  assert.equal((await retrieved.json()).id, id);
  assert.equal(seen.at(-1)?.path, `/v1/submissions/${id}`);
  const cancelled = await fetch(
    `${target.origin}/api/submissions/${id}/cancel`,
    {
      method: 'POST',
      headers: { Origin: target.origin },
    },
  );
  assert.equal((await cancelled.json()).outcome, 'cancelled');
  assert.equal(seen.at(-1)?.path, `/v1/submissions/${id}/cancel`);
});

test('the playground relays Apalis worker availability', async (context) => {
  const target = await fixture((request, response) => {
    assert.equal(request.url, '/v1/execution/status');
    assert.equal(request.headers.authorization, 'Bearer fixture-token');
    response.writeHead(200, { 'Content-Type': 'application/json' });
    response.end(JSON.stringify({ worker: 'offline' }));
  });
  context.after(target.close);

  const response = await fetch(`${target.origin}/api/execution/status`);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('cache-control'), 'no-store');
  assert.deepEqual(await response.json(), { worker: 'offline' });
});

test('invalid requests are rejected without forwarding them', async (context) => {
  let calls = 0;
  const target = await fixture((_request, response) => {
    calls += 1;
    response.writeHead(500).end();
  });
  context.after(target.close);
  const url = `${target.origin}/api/submissions`;

  for (const [origin, body, expected] of [
    ['https://other.example', '{}', 403],
    [target.origin, '{', 400],
    [
      target.origin,
      JSON.stringify({ language: 'python', source: '\0', stdin: '' }),
      400,
    ],
    [
      target.origin,
      JSON.stringify({
        language: 'python',
        source: 'x'.repeat(801 * 1024),
        stdin: '',
      }),
      413,
    ],
  ] satisfies [string, string, number][]) {
    const response = await fetch(url, {
      method: 'POST',
      headers: requestHeaders(origin, 'sample'),
      body,
    });
    assert.equal(response.status, expected);
    await response.json();
  }

  const unrelated = await fetch(`${target.origin}/api/api-keys`);
  assert.equal(unrelated.status, 404);
  await unrelated.json();
  assert.equal(calls, 0);
});
