import { z } from 'zod';

export const languageSchema = z.enum([
  'python',
  'cpp',
  'javascript',
  'c',
  'java',
  'rust',
]);
export type Language = z.infer<typeof languageSchema>;

export const languagesSchema = z.array(
  z.object({
    language: languageSchema,
    profile: z.string(),
    compiled: z.boolean(),
  }),
);

export const executionStatusSchema = z.object({
  worker: z.enum(['available', 'offline', 'queue_uninitialized']),
});

export const comparisonSchema = z.enum(['exact', 'whitespace', 'tokens']);
export type Comparison = z.infer<typeof comparisonSchema>;

const sourceText = z
  .string()
  .refine(
    (value) =>
      !value.includes('\0') && new TextEncoder().encode(value).length <= 65536,
    'Use at most 65,536 UTF-8 bytes and omit NUL characters.',
  );

export const createSchema = z.strictObject({
  language: languageSchema,
  source: sourceText.refine(
    (value) => value.trim().length > 0,
    'Enter source code.',
  ),
  stdin: sourceText,
  expected_output: sourceText.optional(),
  comparison_mode: comparisonSchema.optional(),
});
export type CreateSubmission = z.infer<typeof createSchema>;

const state = z.enum([
  'created',
  'queued',
  'processing',
  'completed',
  'failed',
  'cancelled',
]);
const stage = z.object({
  outcome: z.string(),
  stdout: z.string(),
  stderr: z.string(),
  exit_code: z.number().nullable(),
  signal: z.number().nullable(),
  cpu_time_ms: z.number().nullable(),
  wall_time_ms: z.number().nullable(),
  peak_memory_kib: z.number().nullable(),
});

export const submissionSchema = z.object({
  id: z.uuid(),
  state,
  verdict: z.string().nullable(),
  result: stage
    .extend({
      compilation: stage.optional(),
      score: z.number().optional(),
      max_score: z.number().optional(),
      cases: z
        .array(
          z.object({
            index: z.number(),
            verdict: z.string(),
            score: z.number(),
            max_score: z.number(),
          }),
        )
        .optional(),
    })
    .optional(),
});

export const cancelSchema = z.object({
  id: z.uuid(),
  outcome: z.enum(['cancelled', 'cancellation_requested', 'already_terminal']),
});

export const errorSchema = z.object({
  error: z.object({ message: z.string() }),
});

export type Submission = z.infer<typeof submissionSchema>;
export const isPending = (submission: Submission) =>
  ['created', 'queued', 'processing'].includes(submission.state);
