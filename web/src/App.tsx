import {
  ArrowUpRight,
  Brackets,
  ChevronDown,
  Code2,
  Moon,
  Play,
  RotateCw,
  Sun,
} from 'lucide-react';
import { lazy, Suspense, useEffect, useEffectEvent, useState } from 'react';

import {
  comparisonSchema,
  createSchema,
  languageSchema,
} from '../shared/contracts';
import type { Comparison, Language } from '../shared/contracts';
import { CandyButton } from '@/components/ui/candy-button';

import { errorMessage, fetchExecutionStatus, fetchLanguages } from './api';
import { ExecutionPanel } from './ExecutionPanel';
import { languageChoices, languages } from './languages';
import { useExecution } from './use-execution';

const CodeEditor = lazy(() => import('./CodeEditor'));

type WorkerState =
  Awaited<ReturnType<typeof fetchExecutionStatus>>['worker'] | 'unknown';

const workerMessages = {
  offline:
    'No Ferrum worker is online. Start a qualified Linux worker connected to the API’s PostgreSQL database.',
  queue_uninitialized:
    'The execution queue is not initialized. Apply the Ferrum worker migrations before starting the worker.',
  unknown:
    'Could not verify worker status. The playground will check again shortly.',
} as const;

export default function App() {
  const [language, setLanguage] = useState<Language>('python');
  const [drafts, setDrafts] = useState<Partial<Record<Language, string>>>({});
  const [input, setInput] = useState('Ada\n');
  const [expected, setExpected] = useState('Hello, Ada!\n');
  const [comparison, setComparison] = useState<Comparison>('exact');
  const [dark, setDark] = useState(false);
  const [position, setPosition] = useState({ line: 1, column: 1 });
  const [check, setCheck] = useState(0);
  const [profiles, setProfiles] =
    useState<Awaited<ReturnType<typeof fetchLanguages>>>();
  const [connectionError, setConnectionError] = useState<string>();
  const [worker, setWorker] = useState<WorkerState>();
  const [inputError, setInputError] = useState<string>();
  const execution = useExecution();
  const source = drafts[language] ?? languages[language].source;
  const activeProfile = profiles?.find(
    (profile) => profile.language === language,
  );
  const apiConnected = profiles !== undefined;
  const workerOnline = worker === 'available';
  const canRun =
    workerOnline &&
    !!activeProfile &&
    !execution.pending &&
    execution.status !== 'sending';
  const workerMessage =
    apiConnected && worker && worker !== 'available'
      ? workerMessages[worker]
      : undefined;

  useEffect(() => {
    const controller = new AbortController();
    fetchLanguages(controller.signal)
      .then((available) => {
        if (!controller.signal.aborted) setProfiles(available);
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted) setConnectionError(errorMessage(error));
      });
    return () => controller.abort();
  }, [check]);

  useEffect(() => {
    if (!apiConnected) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function checkWorker() {
      try {
        const result = await fetchExecutionStatus(controller.signal);
        if (!controller.signal.aborted) setWorker(result.worker);
      } catch {
        if (!controller.signal.aborted) setWorker('unknown');
      } finally {
        if (!controller.signal.aborted)
          timer = setTimeout(() => void checkWorker(), 5000);
      }
    }
    void checkWorker();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [apiConnected]);

  function start(mode: 'run' | 'submit') {
    if (!canRun) return;
    const payload = createSchema.safeParse({
      language,
      source,
      stdin: input,
      ...(mode === 'submit'
        ? { expected_output: expected, comparison_mode: comparison }
        : {}),
    });
    if (!payload.success) {
      const issue = payload.error.issues[0];
      const field = issue.path[0];
      const section =
        field === 'stdin'
          ? 'Standard input'
          : field === 'expected_output'
            ? 'Expected output'
            : 'Source';
      setInputError(`${section}. ${issue.message}`);
      return;
    }
    setInputError(undefined);
    void execution.send({
      mode,
      payload: payload.data,
      key: crypto.randomUUID(),
    });
  }

  const runShortcut = useEffectEvent(() => start('run'));
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.key !== 'Enter' ||
        !(event.metaKey || event.ctrlKey)
      )
        return;
      event.preventDefault();
      runShortcut();
    }
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, []);

  return (
    <main
      data-theme={dark ? 'dark' : 'light'}
      className="relative isolate min-h-dvh overflow-hidden bg-canvas px-3 py-4 text-ink sm:px-7 sm:py-9 xl:px-18 xl:py-[7vh]"
    >
      <img
        src="/countryside.jpg"
        alt=""
        aria-hidden="true"
        className="pointer-events-none absolute top-0 left-0 -z-10 hidden h-[74%] w-[51%] object-cover object-left sm:block"
      />
      <img
        src="/countryside.jpg"
        alt=""
        aria-hidden="true"
        className="pointer-events-none absolute right-0 bottom-0 -z-10 hidden h-[62%] w-[42%] object-cover object-right sm:block"
      />
      <div className="mx-auto flex max-w-[1250px] flex-col overflow-hidden rounded-[12px] border border-edge bg-surface shadow-[0_12px_50px_#18202024]">
        <header className="flex min-h-13 flex-wrap items-center justify-between gap-x-5 gap-y-2 border-b border-edge bg-chrome px-4 py-2 sm:px-6">
          <div className="flex items-center gap-3 text-[13px]">
            <a
              href="/"
              className="font-semibold tracking-tight hover:underline"
            >
              Ferrum
            </a>
            <span aria-hidden="true" className="text-muted">
              /
            </span>
            <span className="text-muted">Playground</span>
          </div>
          <div className="flex items-center gap-3.5">
            {connectionError ? (
              <button
                type="button"
                title={connectionError}
                onClick={() => {
                  setConnectionError(undefined);
                  setProfiles(undefined);
                  setWorker(undefined);
                  setCheck((previous) => previous + 1);
                }}
                className="flex items-center gap-2 text-xs text-danger hover:underline"
              >
                <RotateCw size={12} /> Reconnect API
              </button>
            ) : (
              <span
                aria-live="polite"
                className="flex items-center gap-2 text-xs text-muted"
              >
                <span
                  className={`size-1.5 ${workerOnline ? 'bg-accent-ink' : 'bg-warning'}`}
                />
                {workerOnline
                  ? 'Worker online'
                  : workerMessage
                    ? 'Execution unavailable'
                    : apiConnected
                      ? 'Checking worker'
                      : 'Connecting'}
              </span>
            )}
            <span aria-hidden="true" className="h-5 border-l border-edge" />
            <button
              type="button"
              aria-label={dark ? 'Use light theme' : 'Use dark theme'}
              title={dark ? 'Use light theme' : 'Use dark theme'}
              onClick={() => setDark(!dark)}
              className="grid size-7 place-items-center rounded-[4px] text-muted hover:bg-subtle hover:text-ink"
            >
              {dark ? <Sun size={16} /> : <Moon size={16} />}
            </button>
          </div>
        </header>
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-edge px-4 py-4 sm:px-6 sm:py-5">
          <div>
            <h1 className="text-[24px] leading-tight font-semibold tracking-[-0.055em] sm:text-[28px]">
              Code playground
            </h1>
            <p className="mt-1 text-xs text-muted sm:text-[13px]">
              Write, run, and judge code with Ferrum.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <CandyButton
              type="button"
              variant="pearl"
              disabled={!canRun}
              onClick={() => start('run')}
              title="Run with standard input"
              leftIcon={<Play aria-hidden="true" />}
              className="border border-edge font-semibold focus-visible:ring-accent-ink/40 focus-visible:ring-offset-surface"
            >
              Run
            </CandyButton>
            <CandyButton
              type="button"
              variant="obsidian"
              disabled={!canRun}
              onClick={() => start('submit')}
              title="Judge against expected output"
              rightIcon={<ArrowUpRight aria-hidden="true" />}
              className="font-semibold focus-visible:ring-accent-ink/40 focus-visible:ring-offset-surface"
            >
              Submit
            </CandyButton>
          </div>
        </div>

        {(connectionError || workerMessage || inputError) && (
          <p
            role="alert"
            className="border-b border-edge px-4 py-2 text-xs text-danger sm:px-6"
          >
            {connectionError ?? workerMessage ?? inputError}
          </p>
        )}

        <div className="grid lg:min-h-[clamp(558px,calc(86vh-184px),980px)] lg:grid-cols-[minmax(0,1.68fr)_minmax(338px,1fr)]">
          <section
            aria-label="Source editor"
            className="flex min-w-0 flex-col bg-editor lg:border-r lg:border-edge"
          >
            <div className="flex min-h-11 items-stretch justify-between border-b border-edge bg-chrome pr-2.5 sm:pr-3.5">
              <div className="flex items-center gap-2 border-t-2 border-r border-t-accent-ink border-r-edge bg-editor px-3.5 font-mono text-xs sm:px-4">
                <Code2
                  size={15}
                  aria-hidden="true"
                  className="text-accent-ink"
                />
                {languages[language].file}
              </div>
              <label className="relative flex items-center self-center">
                <span className="sr-only">Language</span>
                <select
                  value={language}
                  onChange={(event) => {
                    const selection = languageSchema.safeParse(
                      event.currentTarget.value,
                    );
                    if (selection.success) {
                      setLanguage(selection.data);
                      setPosition({ line: 1, column: 1 });
                      setInputError(undefined);
                    }
                  }}
                  className="h-7 appearance-none rounded-[4px] border border-edge bg-editor pr-7 pl-2.5 text-[11px] font-medium hover:bg-subtle"
                >
                  {languageChoices.map((choice) => (
                    <option
                      key={choice}
                      value={choice}
                      disabled={
                        profiles &&
                        !profiles.some((profile) => profile.language === choice)
                      }
                    >
                      {languages[choice].label}
                    </option>
                  ))}
                </select>
                <ChevronDown
                  size={13}
                  aria-hidden="true"
                  className="pointer-events-none absolute right-2 text-muted"
                />
              </label>
            </div>
            <div className="relative min-h-100 flex-1 lg:min-h-0">
              <Suspense
                fallback={
                  <div role="status" className="p-5 text-xs text-muted">
                    Loading editor…
                  </div>
                }
              >
                <CodeEditor
                  key={language}
                  source={source}
                  language={language}
                  dark={dark}
                  onSource={(value) =>
                    setDrafts((current) => ({ ...current, [language]: value }))
                  }
                  onPosition={(line, column) => setPosition({ line, column })}
                  onRun={() => start('run')}
                />
              </Suspense>
            </div>
          </section>

          <aside
            aria-label="Execution workspace"
            className="flex min-w-0 flex-col gap-4 border-t border-edge bg-surface px-4 py-4 sm:px-5 lg:border-t-0"
          >
            <section
              aria-labelledby="input-heading"
              className="flex flex-col gap-2.5"
            >
              <h2
                id="input-heading"
                className="flex items-center gap-2 text-[13px] font-semibold"
              >
                <Brackets size={15} aria-hidden="true" className="text-muted" />
                Test input
              </h2>
              <label className="grid gap-1.5">
                <span className="flex items-center justify-between text-xs font-medium">
                  Standard input
                  <span className="font-mono text-[10px] font-normal text-muted">
                    stdin
                  </span>
                </span>
                <textarea
                  rows={2}
                  value={input}
                  spellCheck={false}
                  onChange={(event) => setInput(event.target.value)}
                  placeholder="Input for your program"
                  className="min-h-14 w-full resize-y rounded-[4px] border border-edge bg-editor px-2.5 py-2 font-mono text-xs leading-5 placeholder:text-muted"
                />
              </label>
              <div className="grid gap-1.5">
                <div className="flex flex-wrap items-center justify-between gap-1">
                  <label
                    htmlFor="expected-output"
                    className="text-xs font-medium"
                  >
                    Expected output
                  </label>
                  <label className="relative">
                    <span className="sr-only">Output comparison</span>
                    <select
                      value={comparison}
                      onChange={(event) => {
                        const selection = comparisonSchema.safeParse(
                          event.currentTarget.value,
                        );
                        if (selection.success) setComparison(selection.data);
                      }}
                      className="appearance-none rounded-[3px] border border-edge bg-editor py-0.5 pr-6 pl-2 text-[10px] text-muted hover:text-ink"
                    >
                      <option value="exact">Exact match</option>
                      <option value="whitespace">
                        Ignore trailing whitespace
                      </option>
                      <option value="tokens">Compare tokens</option>
                    </select>
                    <ChevronDown
                      size={11}
                      aria-hidden="true"
                      className="pointer-events-none absolute top-1.5 right-1.5 text-muted"
                    />
                  </label>
                </div>
                <textarea
                  id="expected-output"
                  rows={2}
                  value={expected}
                  spellCheck={false}
                  onChange={(event) => setExpected(event.target.value)}
                  placeholder="Output to compare on submit"
                  className="min-h-14 w-full resize-y rounded-[4px] border border-edge bg-editor px-2.5 py-2 font-mono text-xs leading-5 placeholder:text-muted"
                />
              </div>
            </section>
            <div className="border-t border-edge" />
            <ExecutionPanel
              key={execution.submission?.id ?? 'draft'}
              mode={execution.mode}
              submission={execution.submission}
              status={execution.status}
              error={execution.error}
              pending={execution.pending}
              cancelling={execution.cancelling}
              onCancel={() => void execution.cancel()}
              onRefresh={execution.refresh}
              workerUnavailable={workerMessage !== undefined}
              retry={workerOnline ? execution.retry : undefined}
            />
          </aside>
        </div>
        <footer className="flex min-h-9 flex-wrap items-center justify-between gap-x-4 gap-y-1 border-t border-edge bg-chrome px-4 py-1.5 font-mono text-[10px] text-muted sm:px-6">
          <span>
            Ln {position.line}, Col {position.column}{' '}
            <span className="mx-2">·</span> UTF-8
          </span>
          <span className="flex items-center gap-2">
            <kbd className="rounded-[3px] border border-edge px-1.5 py-0.5 text-[9px]">
              Ctrl / ⌘ + Enter
            </kbd>{' '}
            Run code
          </span>
        </footer>
      </div>
    </main>
  );
}
