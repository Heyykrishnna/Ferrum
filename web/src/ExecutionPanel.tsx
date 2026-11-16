import { Check, Clipboard, RotateCw, Square, Terminal } from 'lucide-react';
import { useState } from 'react';

import type { Submission } from '../shared/contracts';

type OutputTab = 'Output' | 'Errors' | 'Compiler';
const tabs: OutputTab[] = ['Output', 'Errors', 'Compiler'];

interface ExecutionPanelProps {
  mode?: 'run' | 'submit';
  submission?: Submission;
  status: 'idle' | 'sending' | 'polling' | 'paused';
  error?: string;
  pending: boolean;
  cancelling: boolean;
  onCancel: () => void;
  onRefresh: () => void;
  workerUnavailable: boolean;
  retry?: () => void;
}

function displayLabel(value: string) {
  const phrase = value.replaceAll('_', ' ');
  return phrase.charAt(0).toUpperCase() + phrase.slice(1);
}

export function ExecutionPanel({
  mode,
  submission,
  status,
  error,
  pending,
  cancelling,
  onCancel,
  onRefresh,
  workerUnavailable,
  retry,
}: ExecutionPanelProps) {
  const [tab, setTab] = useState<OutputTab>('Output');
  const [copied, setCopied] = useState(false);
  const result = submission?.result;
  const output = {
    Output: result?.stdout,
    Errors: result?.stderr,
    Compiler: [result?.compilation?.stdout, result?.compilation?.stderr]
      .filter(Boolean)
      .join('\n'),
  };
  const text = output[tab];
  const busy = status === 'polling' || status === 'sending';
  const label = submission?.verdict
    ? displayLabel(submission.verdict)
    : submission?.state === 'completed' && result
      ? result.outcome === 'success'
        ? 'Finished'
        : displayLabel(result.outcome)
      : submission
        ? displayLabel(submission.state)
        : status === 'sending'
          ? 'Sending'
          : 'Idle';

  async function copyOutput() {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <section
      aria-label="Execution results"
      className="flex min-h-0 flex-1 flex-col gap-3"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="flex items-center gap-2 text-[13px] font-semibold">
          <Terminal size={15} aria-hidden="true" className="text-muted" />
          Console
        </h2>
        <div
          aria-live="polite"
          className="flex items-center gap-2 text-xs text-muted"
        >
          <span
            className={`size-1.5 ${busy ? 'bg-warning' : result ? 'bg-accent-ink' : 'bg-muted'}`}
          />
          {label}
        </div>
      </div>
      <div className="flex min-h-43 flex-1 flex-col overflow-hidden rounded-[5px] border border-console-edge bg-console text-console-text">
        <div className="flex min-h-9 items-stretch justify-between border-b border-console-edge px-2">
          <div
            role="group"
            aria-label="Console view"
            className="flex items-stretch"
          >
            {tabs.map((name) => (
              <button
                key={name}
                type="button"
                aria-pressed={tab === name}
                onClick={() => {
                  setCopied(false);
                  setTab(name);
                }}
                className="border-b-2 border-transparent px-2.5 text-[11px] text-console-muted hover:text-console-text aria-pressed:border-console-accent aria-pressed:text-console-text"
              >
                {name}
              </button>
            ))}
          </div>
          <button
            type="button"
            title={copied ? 'Copied' : 'Copy console output'}
            aria-label={copied ? 'Copied' : 'Copy console output'}
            disabled={!text}
            onClick={() => void copyOutput()}
            className="mx-1 self-center p-1.5 text-console-muted hover:text-console-text disabled:opacity-40"
          >
            {copied ? <Check size={14} /> : <Clipboard size={14} />}
          </button>
        </div>
        <div className="max-h-60 min-h-24 flex-1 overflow-auto p-3.5">
          {text ? (
            <pre className="font-mono text-xs leading-5 wrap-break-word whitespace-pre-wrap">
              {text}
            </pre>
          ) : (
            <p className="font-mono text-[11px] leading-5 text-console-muted">
              <span aria-hidden="true" className="mr-2 text-console-accent">
                ›
              </span>
              {workerUnavailable && pending
                ? 'This run is saved. It needs a Ferrum worker to finish.'
                : workerUnavailable && !submission
                  ? 'Execution is unavailable while the worker is offline.'
                  : busy
                    ? submission?.state === 'processing'
                      ? 'Executing on a Ferrum worker…'
                      : 'Waiting for a Ferrum worker…'
                    : result
                      ? `No ${tab.toLowerCase()} recorded.`
                      : submission && !pending
                        ? 'No execution output was recorded.'
                        : 'Run your code to see output here.'}
            </p>
          )}
        </div>
        {result && (
          <div className="flex flex-wrap gap-x-3 gap-y-1 border-t border-console-edge px-3 py-2 font-mono text-[10px] text-console-muted">
            {result.exit_code !== null && <span>Exit {result.exit_code}</span>}
            {result.signal !== null && <span>Signal {result.signal}</span>}
            {result.wall_time_ms !== null && (
              <span>{result.wall_time_ms} ms wall</span>
            )}
            {result.peak_memory_kib !== null && (
              <span>{result.peak_memory_kib} KiB</span>
            )}
            {result.score !== undefined && result.max_score !== undefined && (
              <span>
                Score {result.score}/{result.max_score}
              </span>
            )}
          </div>
        )}
      </div>
      {error && (
        <p role="alert" className="text-xs leading-relaxed text-danger">
          {error}
        </p>
      )}
      {submission && (
        <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted">
          <span title={submission.id} className="font-mono">
            {mode === 'submit' ? 'Submission' : 'Run'}{' '}
            {submission.id.slice(0, 8)}
          </span>
          {pending && (
            <div className="flex gap-3">
              {status === 'paused' && (
                <button
                  type="button"
                  onClick={onRefresh}
                  className="flex items-center gap-1.5 hover:text-ink"
                >
                  <RotateCw size={12} /> Refresh
                </button>
              )}
              <button
                type="button"
                disabled={cancelling}
                onClick={onCancel}
                className="flex items-center gap-1.5 hover:text-ink disabled:opacity-60"
              >
                <Square size={11} /> {cancelling ? 'Cancelling…' : 'Cancel'}
              </button>
            </div>
          )}
        </div>
      )}
      {retry && (
        <button
          type="button"
          onClick={retry}
          className="flex w-fit items-center gap-1.5 text-xs font-medium text-accent-ink"
        >
          <RotateCw size={12} /> Retry request
        </button>
      )}
    </section>
  );
}
