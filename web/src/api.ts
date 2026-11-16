import type { ZodType } from 'zod';

import {
  cancelSchema,
  errorSchema,
  executionStatusSchema,
  languagesSchema,
  submissionSchema,
} from '../shared/contracts';
import type { CreateSubmission } from '../shared/contracts';

async function request<T>(
  path: string,
  schema: ZodType<T>,
  init?: RequestInit,
): Promise<T> {
  const timeout = AbortSignal.timeout(15_000);
  const signal = init?.signal
    ? AbortSignal.any([init.signal, timeout])
    : timeout;
  const response = await fetch(path, { ...init, signal });
  const body: unknown = await response.json();
  if (!response.ok) {
    const error = errorSchema.safeParse(body);
    throw new Error(
      error.success
        ? error.data.error.message
        : 'Ferrum could not complete the request.',
    );
  }
  const result = schema.safeParse(body);
  if (!result.success)
    throw new Error('Ferrum returned an unexpected response.');
  return result.data;
}

export const fetchLanguages = (signal: AbortSignal) =>
  request('/api/languages', languagesSchema, { signal });

export const fetchExecutionStatus = (signal: AbortSignal) =>
  request('/api/execution/status', executionStatusSchema, { signal });

export const createSubmission = (submission: CreateSubmission, key: string) =>
  request('/api/submissions', submissionSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'Idempotency-Key': key },
    body: JSON.stringify(submission),
  });

export const fetchSubmission = (id: string, signal: AbortSignal) =>
  request(`/api/submissions/${id}`, submissionSchema, { signal });

export const cancelSubmission = (id: string) =>
  request(`/api/submissions/${id}/cancel`, cancelSchema, { method: 'POST' });

export const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : 'An unexpected error occurred.';
