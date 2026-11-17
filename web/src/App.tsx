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

const docsUrl =
  'https://github.com/Heyykrishnna/Ferrum/blob/main/docs/README.md';

const motion = 'duration-300 ease-[var(--ease-soft)]';
const control = `border border-edge bg-editor transition-[border-color,background-color,color,box-shadow] ${motion} hover:border-muted/50 focus-visible:border-ink/40 focus-visible:ring-2 focus-visible:ring-ink/10 focus-visible:outline-none`;
const buttonFocus =
  'focus-visible:ring-ink/30 focus-visible:ring-offset-canvas';

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
    <div
      data-theme={dark ? 'dark' : 'light'}
      className="flex min-h-dvh flex-col bg-canvas text-ink lg:h-dvh"
    >
      <header className="sticky top-0 z-20 border-b border-edge bg-surface/90 backdrop-blur-md">
        <div className="flex min-h-16 flex-wrap items-center justify-between gap-x-6 gap-y-3 px-4 py-3 sm:px-6 lg:px-8">
          <div className="flex items-center gap-3">
            <a
              href="/"
              className={`text-xl font-semibold tracking-tight transition-opacity ${motion} hover:opacity-70`}
            >
              Ferrum
            </a>
            <span aria-hidden="true" className="h-5 border-l border-edge" />
            <h1 className="text-[13px] font-medium text-muted">Playground</h1>
          </div>

          <div className="flex flex-wrap items-center gap-2 sm:gap-3">
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
                className={`flex h-9 items-center gap-2 rounded-full border border-danger/40 px-3.5 text-xs text-danger transition-colors ${motion} hover:bg-danger/10`}
              >
                <RotateCw size={12} aria-hidden="true" /> Reconnect API
              </button>
            ) : (
              <span
                aria-live="polite"
                className="flex h-9 items-center gap-2 rounded-full border border-edge bg-surface px-3.5 text-xs text-muted"
              >
                <span
                  className={`size-1.5 rounded-full transition-colors ${motion} ${workerOnline ? 'bg-ink' : 'bg-warning'}`}
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
            <a
              href={docsUrl}
              target="_blank"
              rel="noreferrer"
              className={`hidden h-9 items-center rounded-md px-3 text-[13px] text-muted transition-colors ${motion} hover:text-ink md:flex`}
            >
              Docs
            </a>
            <button
              type="button"
              aria-label={dark ? 'Use light theme' : 'Use dark theme'}
              title={dark ? 'Use light theme' : 'Use dark theme'}
              onClick={() => setDark(!dark)}
              className={`grid size-9 place-items-center rounded-lg border border-edge bg-surface text-muted transition-colors ${motion} hover:bg-subtle hover:text-ink`}
            >
              {dark ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <span
              aria-hidden="true"
              className="mx-1 hidden h-5 border-l border-edge sm:block"
            />
            <CandyButton
              type="button"
              variant="pearl"
              disabled={!canRun}
              onClick={() => start('run')}
              title="Run with standard input"
              leftIcon={<Play aria-hidden="true" />}
              className={`border border-edge font-semibold ${motion} ${buttonFocus}`}
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
              className={`font-semibold ${motion} ${buttonFocus}`}
            >
              Submit
            </CandyButton>
          </div>
        </div>
        {(connectionError || workerMessage || inputError) && (
          <p
            role="alert"
            className="border-t border-edge px-4 py-2 text-xs text-danger sm:px-6 lg:px-8"
          >
            {connectionError ?? workerMessage ?? inputError}
          </p>
        )}
      </header>

      <main className="grid flex-1 gap-3 p-3 sm:gap-4 sm:p-4 lg:min-h-0 lg:grid-cols-[minmax(0,1.7fr)_minmax(360px,1fr)] lg:p-5">
        <section
          aria-label="Source editor"
          className="flex min-h-120 min-w-0 flex-col overflow-hidden rounded-xl border border-edge bg-editor lg:min-h-0"
        >
          <div className="flex min-h-12 items-center justify-between gap-3 border-b border-edge bg-chrome px-3 sm:px-4">
            <div className="flex items-center gap-2 font-mono text-xs">
              <Code2 size={15} aria-hidden="true" className="text-muted" />
              {languages[language].file}
            </div>
            <label className="relative flex items-center">
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
                className={`h-8 appearance-none rounded-lg pr-8 pl-3 text-xs font-medium ${control}`}
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
                className="pointer-events-none absolute right-2.5 text-muted"
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
          <footer className="flex min-h-9 flex-wrap items-center justify-between gap-x-4 gap-y-1 border-t border-edge bg-chrome px-3 py-1.5 font-mono text-[10px] text-muted sm:px-4">
            <span>
              Ln {position.line}, Col {position.column}
              <span aria-hidden="true" className="mx-2">
                ·
              </span>
              UTF-8
            </span>
            <span className="flex items-center gap-2">
              <kbd className="rounded-md border border-edge bg-surface px-1.5 py-0.5 text-[9px]">
                Ctrl / ⌘ + Enter
              </kbd>
              Run code
            </span>
          </footer>
        </section>

        <aside
          aria-label="Execution workspace"
          className="flex min-w-0 flex-col gap-3 sm:gap-4 lg:min-h-0 lg:overflow-y-auto"
        >
          <section
            aria-labelledby="input-heading"
            className="flex flex-col gap-3 rounded-xl border border-edge bg-surface p-4"
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
                className={`min-h-16 w-full resize-y rounded-lg px-3 py-2 font-mono text-xs leading-5 placeholder:text-muted ${control}`}
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
                <label className="relative flex items-center">
                  <span className="sr-only">Output comparison</span>
                  <select
                    value={comparison}
                    onChange={(event) => {
                      const selection = comparisonSchema.safeParse(
                        event.currentTarget.value,
                      );
                      if (selection.success) setComparison(selection.data);
                    }}
                    className={`h-7 appearance-none rounded-md pr-7 pl-2.5 text-[11px] text-muted hover:text-ink ${control}`}
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
                    className="pointer-events-none absolute right-2 text-muted"
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
                className={`min-h-16 w-full resize-y rounded-lg px-3 py-2 font-mono text-xs leading-5 placeholder:text-muted ${control}`}
              />
            </div>
          </section>
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
      </main>
    </div>
  );
}
