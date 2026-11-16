import {
  ArrowRight,
  ArrowUpRight,
  BookOpen,
  Check,
  CircleCheck,
  Clock,
  Cpu,
  FileWarning,
  FolderLock,
  GitBranch,
  Layers,
  LifeBuoy,
  Menu,
  MemoryStick,
  ShieldOff,
  Trash2,
  X,
} from 'lucide-react';
import type { CSSProperties, ReactNode } from 'react';
import { useEffect, useState } from 'react';
import type { SimpleIcon } from 'simple-icons';
import {
  siDocker,
  siGrafana,
  siLinux,
  siOpentelemetry,
  siPostgresql,
  siPrometheus,
  siRust,
  siTokio,
} from 'simple-icons';

import { CandyButton } from '@/components/ui/candy-button';
import type {
  CandyButtonSize,
  CandyButtonVariant,
} from '@/components/ui/candy-button';
import { cn } from '@/lib/utils';

const repository = 'https://github.com/Heyykrishnna/Ferrum';
const doc = (path: string) => `${repository}/blob/main/${path}`;

const navigation = [
  { label: 'Products', href: '#products' },
  { label: 'Languages', href: '#languages' },
  { label: 'Security', href: '#security' },
  { label: 'Deploy', href: '#deploy' },
  { label: 'Support', href: '#support' },
];

const requestSamples = {
  Submit: `curl -i http://127.0.0.1:8080/v1/submissions \\
  -H "Authorization: Bearer $FERRUM_TOKEN" \\
  -H "Idempotency-Key: first-python-run" \\
  -H "Content-Type: application/json" \\
  --data '{
    "language": "python",
    "source": "print(input())\\n",
    "stdin": "hello\\n",
    "expected_output": "hello\\n",
    "comparison_mode": "exact"
  }'`,
  Poll: `# 202 Accepted
# Location: /v1/submissions/$SUBMISSION_ID

curl -i "http://127.0.0.1:8080/v1/submissions/$SUBMISSION_ID" \\
  -H "Authorization: Bearer $FERRUM_TOKEN"

# Cancel while it is still running
curl -X DELETE \\
  "http://127.0.0.1:8080/v1/submissions/$SUBMISSION_ID" \\
  -H "Authorization: Bearer $FERRUM_TOKEN"`,
  Result: `{
  "id": "4f0c9a52-8d1e-4c4b-9a7e-2f1d6b0c3e11",
  "state": "completed",
  "verdict": "accepted",
  "result": {
    "outcome": "success",
    "stdout": "hello\\n",
    "stderr": "",
    "exit_code": 0,
    "signal": null,
    "cpu_time_ms": 18,
    "wall_time_ms": 41,
    "peak_memory_kib": 9216
  }
}`,
} as const;

type RequestSample = keyof typeof requestSamples;

const stack: { icon: SimpleIcon; role: string }[] = [
  { icon: siRust, role: 'Core services' },
  { icon: siTokio, role: 'Async runtime' },
  { icon: siPostgresql, role: 'Storage & queue' },
  { icon: siLinux, role: 'Isolate sandbox' },
  { icon: siDocker, role: 'Local services' },
  { icon: siOpentelemetry, role: 'Tracing' },
  { icon: siPrometheus, role: 'Metrics' },
  { icon: siGrafana, role: 'Dashboards' },
];

const products = [
  {
    id: 'api',
    name: 'Ferrum API',
    summary:
      'An asynchronous HTTP API for running and judging untrusted code. Submissions are stored durably, queued through a transactional outbox, and executed inside Isolate on dedicated Linux workers. Use it for coding assessments, e-learning platforms, competitive programming, AI-generated code checks, and online editors.',
    features: [
      'Asynchronous submissions with tenant-scoped idempotency keys',
      'Transactional outbox into a PostgreSQL-backed Apalis queue',
      'Sandboxed compilation and execution with fixed toolchain paths',
      'Six languages: Python 3, C, C++, JavaScript, Java and Rust',
      'Per-request CPU, wall-clock and memory limits within profile ceilings',
      'Detailed results: stdout, stderr, exit code, signal, CPU, wall time and peak memory',
      'Bounded batches of 1 to 20 submissions',
      'Scoped API keys, cancellation and signed webhooks',
    ],
    action: {
      label: 'Read the API guide',
      href: doc('docs/submission-api.md'),
    },
  },
  {
    id: 'playground',
    name: 'Ferrum Playground',
    summary:
      'A Monaco-based workspace running on top of the same execution engine. Write a program, give it standard input, and run it, or submit it against an expected output and read the verdict. No local toolchains needed: everything compiles and runs on a Ferrum worker.',
    features: [
      'Code execution in all six supported languages',
      'Monaco editor with syntax highlighting',
      'Light and dark editor themes',
      'Custom standard input and expected output',
      'Exact, trailing-whitespace and token comparison',
      'Live worker status and cancellation of running submissions',
      'Run with Ctrl / ⌘ + Enter',
    ],
    action: { label: 'Open the playground', href: '/playground' },
  },
  {
    id: 'judge',
    name: 'Ferrum Judge',
    summary:
      'A judging engine kept separate from the sandbox. It compares raw program output under an explicit policy, scores weighted test cases, and resolves one deterministic verdict, while the platform’s processing state stays distinct from your program’s outcome.',
    features: [
      'Exact, whitespace-tolerant and token comparison modes',
      'Multiple weighted test cases per submission',
      'Per-case verdicts with partial scoring',
      'Deterministic, documented verdict precedence',
      'Raw output preserved alongside the comparison result',
    ],
    action: {
      label: 'Read the judging guide',
      href: doc('docs/phase6-judging.md'),
    },
  },
] as const;

const languages = [
  {
    name: 'Python 3',
    file: 'main.py',
    mode: 'Interpreted',
    cpu: '2 s',
    memory: '128 MiB',
  },
  {
    name: 'C++',
    file: 'main.cpp',
    mode: 'Compiled',
    cpu: '2 s',
    memory: '128 MiB',
  },
  {
    name: 'JavaScript',
    file: 'main.cjs',
    mode: 'Node.js',
    cpu: '3 s',
    memory: '256 MiB',
  },
  {
    name: 'C',
    file: 'main.c',
    mode: 'Compiled',
    cpu: '2 s',
    memory: '128 MiB',
  },
  {
    name: 'Java',
    file: 'Main.java',
    mode: 'JVM',
    cpu: '3 s',
    memory: '256 MiB',
  },
  {
    name: 'Rust',
    file: 'main.rs',
    mode: 'Compiled',
    cpu: '2 s',
    memory: '128 MiB',
  },
];

const lifecycle = [
  ['Submit', 'The API validates the request and checks its idempotency key.'],
  ['Persist', 'The submission and an outbox event commit in one transaction.'],
  ['Queue', 'A dispatcher publishes a small job containing only identifiers.'],
  [
    'Execute',
    'A worker reserves capacity, then compiles and runs inside Isolate.',
  ],
  [
    'Judge',
    'Output is compared under the requested policy and a verdict is resolved.',
  ],
  [
    'Retrieve',
    'The result is persisted, the sandbox is destroyed, and you poll by ID.',
  ],
] as const;

const controls = [
  {
    icon: Cpu,
    title: 'CPU time',
    text: 'Each stage has its own CPU ceiling enforced by the kernel.',
  },
  {
    icon: Clock,
    title: 'Wall clock',
    text: 'Sleeps and blocking calls are bounded by a wall-time limit.',
  },
  {
    icon: MemoryStick,
    title: 'Memory',
    text: 'cgroup memory limits protect the worker host from exhaustion.',
  },
  {
    icon: GitBranch,
    title: 'Processes',
    text: 'Process counts are capped to stop fork bombs and storms.',
  },
  {
    icon: FileWarning,
    title: 'Output',
    text: 'stdout and stderr are each capped at 64 KiB while collected.',
  },
  {
    icon: ShieldOff,
    title: 'Network',
    text: 'Submitted programs have no network access.',
  },
  {
    icon: FolderLock,
    title: 'Filesystem',
    text: 'Only the box workspace is writable, under a fixed quota.',
  },
  {
    icon: Trash2,
    title: 'Cleanup',
    text: 'Every sandbox is destroyed and cleanup failures are recorded.',
  },
];

const deployments = [
  {
    name: 'Playground',
    scope: 'Local',
    headline: 'One machine',
    summary: 'Try Ferrum on your own computer with the loopback playground.',
    features: [
      'API, PostgreSQL and web playground on loopback',
      'Linux worker, or an ARM64 VM on Apple Silicon',
      'Single-operator development token',
      'Docker Compose for supporting services',
    ],
    action: { label: 'Local setup', href: doc('web/README.md') },
    featured: false,
  },
  {
    name: 'Self-hosted',
    scope: 'Recommended',
    headline: 'Your Linux host',
    summary: 'Run the full pipeline on infrastructure you control.',
    features: [
      'Separate API and worker processes',
      'Qualified Isolate install with cgroup limits',
      'Pinned compiler and runtime toolchains',
      'Scoped API keys, quotas and webhooks',
      'Graceful worker drain and recovery',
    ],
    action: { label: 'Worker setup', href: doc('docs/phase4-worker.md') },
    featured: true,
  },
  {
    name: 'Production',
    scope: 'Dedicated',
    headline: 'Worker fleet',
    summary: 'Scale workers by measured capacity on dedicated hosts.',
    features: [
      'Dedicated, hardened Linux worker hosts',
      'Prometheus metrics and Grafana dashboards',
      'OpenTelemetry traces from API to sandbox',
      'Operational runbooks for incidents',
    ],
    action: { label: 'Deployment guide', href: doc('deployment/README.md') },
    featured: false,
  },
];

const support = [
  {
    icon: BookOpen,
    title: 'Documentation',
    text: 'Architecture, API reference, language profiles and the threat model.',
    meta: ['OpenAPI spec', 'Phase guides'],
    action: { label: 'Browse docs', href: doc('docs/README.md') },
  },
  {
    icon: LifeBuoy,
    title: 'Runbooks',
    text: 'Step-by-step recovery for stuck executions, outbox dispatch and sandbox cleanup.',
    meta: ['Incident containment', 'Recovery'],
    action: {
      label: 'Open runbooks',
      href: `${repository}/tree/main/docs/runbooks`,
    },
  },
  {
    icon: Layers,
    title: 'Issues',
    text: 'Report a bug, propose a language profile or ask a question on GitHub.',
    meta: ['Public tracker', 'Discussion'],
    action: { label: 'Open an issue', href: `${repository}/issues` },
  },
];

function isExternal(href: string) {
  return href.startsWith('http');
}

function TextLink({
  href,
  className,
  children,
}: {
  href: string;
  className: string;
  children: ReactNode;
}) {
  return (
    <a
      href={href}
      className={className}
      {...(isExternal(href) ? { target: '_blank', rel: 'noreferrer' } : {})}
    >
      {children}
    </a>
  );
}

function ActionButton({
  href,
  children,
  variant = 'obsidian',
  size = 'default',
  className,
}: {
  href: string;
  children: ReactNode;
  variant?: CandyButtonVariant;
  size?: CandyButtonSize;
  className?: string;
}) {
  const external = isExternal(href);
  const Icon = external ? ArrowUpRight : ArrowRight;
  return (
    <CandyButton
      href={href}
      variant={variant}
      size={size}
      // CandyButton marks anchors as buttons; these navigate, so keep link semantics.
      role={undefined}
      rightIcon={
        <Icon
          aria-hidden="true"
          className={`${hoverMotion} transition-transform group-hover:translate-x-0.5`}
        />
      }
      {...(external ? { target: '_blank', rel: 'noreferrer' } : {})}
      className={cn(
        'group duration-300 ease-[var(--ease-soft)] focus-visible:ring-lp-ink/30 focus-visible:ring-offset-lp-canvas',
        variant === 'pearl' && 'border border-lp-line',
        className,
      )}
    >
      {children}
    </CandyButton>
  );
}

function revealDelay(index: number): CSSProperties {
  return { '--reveal-delay': `${Math.min(index, 7) * 70}ms` } as CSSProperties;
}

function useReveal() {
  useEffect(() => {
    const nodes = document.querySelectorAll<HTMLElement>('[data-reveal]');
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          (entry.target as HTMLElement).dataset.revealed = 'true';
          observer.unobserve(entry.target);
        }
      },
      { rootMargin: '0px 0px -8% 0px', threshold: 0.1 },
    );
    nodes.forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, []);
}

const hoverMotion = 'duration-300 ease-[var(--ease-soft)]';

const cardMotion = `transition-[box-shadow,border-color] ${hoverMotion} hover:border-[#c9c9c6] hover:shadow-[0_12px_32px_-20px_#00000040]`;

function SectionHeading({
  strong,
  faint,
  trailing,
  children,
}: {
  strong: string;
  faint: string;
  trailing?: string;
  children?: ReactNode;
}) {
  return (
    <div
      data-reveal
      className="grid items-end gap-4 border-b border-lp-line pb-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,24rem)] lg:gap-10"
    >
      <div>
        <h2 className="text-3xl leading-tight font-semibold tracking-tight text-lp-ink sm:text-4xl xl:text-5xl">
          {strong} <span className="text-lp-fade">{faint}</span>
          {trailing && <> {trailing}</>}
        </h2>
      </div>
      {children && (
        <p className="max-w-prose text-base leading-7 text-lp-body">
          {children}
        </p>
      )}
    </div>
  );
}

function Frame({ children }: { children: ReactNode }) {
  return (
    <div
      data-reveal
      className="min-w-0 rounded-2xl border border-lp-line bg-lp-card p-2 shadow-sm sm:p-3 lg:p-4"
    >
      {children}
    </div>
  );
}

const tokenPattern =
  /("(?:[^"\\]|\\.)*"|'[^']*'|#.*$|--?[A-Za-z][\w-]*|(?<![\w.:/])\d+(?![\w.:])|\bnull\b)/g;

function CodeLine({ line }: { line: string }) {
  return (
    <>
      {line.split(tokenPattern).map((part, index) => {
        if (!part) return null;
        let tone = 'text-lp-night-text';
        if (part.startsWith('#')) tone = 'text-lp-night-fade';
        else if (part.startsWith('"') || part.startsWith("'"))
          tone = 'text-lp-code-string';
        else if (part.startsWith('-')) tone = 'text-white';
        else if (/^\d+$|^null$/.test(part)) tone = 'text-lp-accent-soft';
        return (
          <span key={index} className={tone}>
            {part}
          </span>
        );
      })}
    </>
  );
}

function CodePanel() {
  const [sample, setSample] = useState<RequestSample>('Submit');
  const samples = Object.keys(requestSamples) as RequestSample[];
  return (
    <div className="flex min-w-0 flex-col rounded-xl border border-lp-night-line bg-lp-night p-4 sm:p-6 xl:p-8">
      <div
        role="group"
        aria-label="API example step"
        className="relative grid grid-cols-3 rounded-lg border border-lp-night-line p-1"
      >
        <span
          aria-hidden="true"
          className="absolute inset-y-1 left-1 w-[calc((100%-0.5rem)/3)] rounded-md bg-lp-night-raise transition-transform duration-300 ease-[var(--ease-soft)]"
          style={{
            transform: `translateX(${samples.indexOf(sample) * 100}%)`,
          }}
        />
        {samples.map((name) => (
          <button
            key={name}
            type="button"
            aria-pressed={sample === name}
            onClick={() => setSample(name)}
            className={`relative min-h-10 rounded-md px-2 text-sm font-medium transition-colors ${hoverMotion} focus-visible:outline-lp-accent-soft ${
              sample === name
                ? 'text-lp-night-text'
                : 'text-lp-night-fade hover:text-lp-night-text'
            }`}
          >
            {name}
          </button>
        ))}
      </div>
      <pre
        aria-label={`${sample} example`}
        tabIndex={0}
        className="mt-4 h-100 flex-1 overflow-auto rounded-lg border border-lp-night-line p-4 font-mono text-xs leading-6 focus-visible:outline-lp-accent-soft sm:p-6 sm:text-sm sm:leading-7"
      >
        <code key={sample} className="lp-fade-in block">
          {requestSamples[sample].split('\n').map((line, index) => (
            <span key={`${sample}-${index}`} className="block">
              <CodeLine line={line} />
              {line === '' && ' '}
            </span>
          ))}
        </code>
      </pre>
      <ActionButton
        href="/playground"
        variant="pearl"
        size="lg"
        className="mt-4 w-full border-transparent text-sm focus-visible:ring-offset-lp-night"
      >
        Try it in the playground
      </ActionButton>
    </div>
  );
}

function Navigation() {
  const [open, setOpen] = useState(false);
  return (
    <header className="sticky top-0 z-20 border-b border-lp-line bg-lp-card/90 backdrop-blur-md">
      <nav
        aria-label="Primary"
        className="mx-auto flex min-h-18 max-w-384 items-center justify-between gap-3 px-4 sm:px-8 lg:px-10"
      >
        <a
          href="/"
          className="text-xl font-semibold tracking-tight text-lp-ink"
        >
          Ferrum
        </a>
        <div className="hidden items-center gap-7 md:flex">
          {navigation.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className={`relative py-1 text-[13px] text-lp-body transition-colors ${hoverMotion} after:absolute after:inset-x-0 after:-bottom-0.5 after:h-px after:origin-left after:scale-x-0 after:bg-lp-ink after:transition-transform after:duration-300 after:ease-[var(--ease-soft)] hover:text-lp-ink hover:after:scale-x-100`}
            >
              {item.label}
            </a>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <TextLink
            href={doc('docs/README.md')}
            className={`hidden h-9 items-center rounded-md px-4 text-[13px] text-lp-body transition-colors ${hoverMotion} hover:text-lp-ink sm:flex`}
          >
            Docs
          </TextLink>
          <ActionButton href="/playground">Playground</ActionButton>
          <CandyButton
            type="button"
            variant="pearl"
            size="icon"
            aria-label={open ? 'Close menu' : 'Open menu'}
            aria-expanded={open}
            aria-controls="mobile-navigation"
            onClick={() => setOpen(!open)}
            className="border border-lp-line focus-visible:ring-lp-ink/30 focus-visible:ring-offset-lp-canvas md:hidden"
          >
            {open ? <X aria-hidden="true" /> : <Menu aria-hidden="true" />}
          </CandyButton>
        </div>
      </nav>
      <div
        id="mobile-navigation"
        inert={!open}
        className={`grid transition-[grid-template-rows,opacity] duration-500 ease-[var(--ease-soft)] md:hidden ${
          open ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'
        }`}
      >
        <div className="overflow-hidden">
          <div className="border-t border-lp-line px-4 py-3 sm:px-8">
            {navigation.map((item) => (
              <a
                key={item.href}
                href={item.href}
                onClick={() => setOpen(false)}
                className={`block py-2 text-[14px] text-lp-body transition-colors ${hoverMotion} hover:text-lp-ink`}
              >
                {item.label}
              </a>
            ))}
          </div>
        </div>
      </div>
    </header>
  );
}

function Hero() {
  return (
    <section aria-labelledby="hero-heading" className="pt-6 sm:pt-9 lg:pt-12">
      <Frame>
        <div className="grid gap-4 lg:min-h-156 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <div className="flex flex-col justify-between gap-10 rounded-xl border border-lp-line bg-lp-panel p-5 sm:p-9 lg:p-10 xl:p-12">
            <div>
              <h1
                id="hero-heading"
                className="max-w-2xl text-4xl leading-[1.04] font-semibold tracking-[-0.055em] text-balance text-lp-ink sm:text-5xl xl:text-6xl 2xl:text-7xl"
              >
                Run untrusted code{' '}
                <span className="text-lp-fade">in a Linux sandbox</span>
              </h1>
              <p className="mt-6 max-w-xl text-base leading-7 text-lp-body xl:text-lg xl:leading-8">
                Ferrum accepts source code over an HTTP API, queues it durably
                in PostgreSQL, and compiles and runs it inside Isolate on
                dedicated Linux workers. Pick a language, send stdin, and poll
                for the verdict.
              </p>
              <div className="mt-8 grid gap-3 sm:flex sm:flex-wrap">
                <ActionButton href="/playground" size="lg" className="text-sm">
                  Open playground
                </ActionButton>
                <ActionButton
                  href={doc('docs/submission-api.md')}
                  variant="pearl"
                  size="lg"
                  className="text-sm"
                >
                  API documentation
                </ActionButton>
              </div>
            </div>
            <figure className="border-t border-lp-line pt-6">
              <blockquote className="max-w-xl text-base leading-7 font-medium text-lp-ink xl:text-lg">
                Correctness, isolation, recoverability and observability{' '}
                <span className="text-lp-fade">
                  matter more than feature count or development speed.
                </span>
              </blockquote>
              <figcaption className="mt-3">
                <TextLink
                  href={doc('AGENTS.md')}
                  className={`text-sm text-lp-fade underline decoration-transparent underline-offset-4 transition-colors ${hoverMotion} hover:text-lp-ink hover:decoration-current`}
                >
                  Ferrum engineering guide
                </TextLink>
              </figcaption>
            </figure>
          </div>
          <CodePanel />
        </div>
      </Frame>
    </section>
  );
}

function Stack() {
  return (
    <section aria-labelledby="stack-heading" className="pt-10 sm:pt-14">
      <h2 id="stack-heading" className="sr-only">
        Technology stack
      </h2>
      <ul className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-lp-line bg-lp-line sm:grid-cols-4 xl:grid-cols-8">
        {stack.map(({ icon, role }, index) => (
          <li
            key={icon.slug}
            data-reveal
            style={revealDelay(index)}
            className={`group flex flex-col items-center justify-center gap-3 bg-lp-card px-4 py-6 text-center transition-colors ${hoverMotion} hover:bg-lp-panel`}
          >
            <svg
              aria-hidden="true"
              viewBox="0 0 24 24"
              className={`size-8 fill-[#8a8a87] transition-[fill] ${hoverMotion} group-hover:fill-lp-ink`}
            >
              <path d={icon.path} />
            </svg>
            <span className="grid gap-0.5">
              <span className="text-sm font-semibold tracking-tight text-lp-ink">
                {icon.title}
              </span>
              <span className="text-xs text-lp-fade">{role}</span>
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function FeatureList({
  items,
  dark = false,
}: {
  items: readonly string[];
  dark?: boolean;
}) {
  return (
    <ul className="grid gap-2.5">
      {items.map((item) => (
        <li
          key={item}
          className={`flex gap-2.5 text-sm leading-5 ${dark ? 'text-lp-night-text' : 'text-lp-body'}`}
        >
          <Check
            size={15}
            aria-hidden="true"
            className={`mt-0.5 shrink-0 ${dark ? 'text-lp-accent-soft' : 'text-lp-accent'}`}
          />
          {item}
        </li>
      ))}
    </ul>
  );
}

function ApiVisual() {
  const rows = [
    ['POST', '/v1/submissions', 'Create'],
    ['POST', '/v1/submissions/batch', 'Batch'],
    ['GET', '/v1/submissions/{id}', 'Retrieve'],
    ['DELETE', '/v1/submissions/{id}', 'Cancel'],
    ['GET', '/v1/languages', 'Profiles'],
    ['GET', '/v1/execution/status', 'Workers'],
    ['POST', '/v1/webhooks', 'Webhooks'],
  ];
  return (
    <div className="min-w-0 rounded-xl border border-lp-night-line bg-lp-night p-5 sm:p-8 xl:p-10">
      <ul className="divide-y divide-lp-night-line rounded-lg border border-lp-night-line">
        {rows.map(([method, path, label]) => (
          <li
            key={`${method}${path}`}
            className="grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-3 px-3 py-3 font-mono text-xs sm:px-4 lg:py-4 xl:grid-cols-[auto_minmax(0,1fr)_auto]"
          >
            <span className="w-14 text-lp-accent-soft">{method}</span>
            <span className="wrap-anywhere text-lp-night-text">{path}</span>
            <span className="hidden text-lp-night-fade xl:inline">{label}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function PlaygroundVisual() {
  return (
    <div className="flex min-w-0 flex-col overflow-hidden rounded-xl border border-lp-line bg-lp-card">
      <div className="flex items-center justify-between border-b border-lp-line px-4 py-3 text-[12px]">
        <span className="font-medium text-lp-ink">
          Ferrum <span className="text-lp-fade">/ Playground</span>
        </span>
        <span className="flex items-center gap-1.5 text-lp-body">
          <span className="size-1.5 rounded-md bg-lp-accent" /> Worker online
        </span>
      </div>
      <div className="grid flex-1 xl:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]">
        <pre className="overflow-x-auto border-b border-lp-line p-5 font-mono text-xs leading-7 text-lp-ink sm:p-7 xl:border-r xl:border-b-0">
          <span className="text-lp-fade">1 </span>def greet(name):{'\n'}
          <span className="text-lp-fade">2 </span> return f"Hello, {'{name}'}!"
          {'\n'}
          <span className="text-lp-fade">3 </span>
          {'\n'}
          <span className="text-lp-fade">4 </span>print(greet(input().strip()))
          {'\n'}
        </pre>
        <div className="grid content-start gap-4 bg-lp-panel p-5 text-sm sm:p-7">
          <div>
            <p className="font-medium text-lp-ink">Standard input</p>
            <p className="mt-1 rounded-md border border-lp-line bg-lp-card px-2.5 py-1.5 font-mono text-lp-body">
              Ada
            </p>
          </div>
          <div>
            <p className="font-medium text-lp-ink">Expected output</p>
            <p className="mt-1 rounded-md border border-lp-line bg-lp-card px-2.5 py-1.5 font-mono text-lp-body">
              Hello, Ada!
            </p>
          </div>
          <p className="flex items-center gap-1.5 rounded-md bg-lp-tint px-3 py-1.5 font-medium text-lp-accent">
            <CircleCheck size={13} aria-hidden="true" /> Accepted · 1 / 1
          </p>
        </div>
      </div>
    </div>
  );
}

function JudgeVisual() {
  const cases = [
    ['Case 1', 'accepted', '10 / 10'],
    ['Case 2', 'accepted', '10 / 10'],
    ['Case 3', 'wrong_answer', '0 / 20'],
    ['Case 4', 'time_limit_exceeded', '0 / 10'],
  ];
  return (
    <div className="min-w-0 rounded-xl border border-lp-line bg-lp-panel p-5 sm:p-8 xl:p-10">
      <div className="flex items-center justify-between text-[12px]">
        <span className="font-medium text-lp-ink">Test cases</span>
        <span className="font-mono text-lp-body">score 20 / 50</span>
      </div>
      <ul className="mt-4 grid gap-2">
        {cases.map(([name, verdict, score]) => (
          <li
            key={name}
            className="flex flex-wrap items-baseline justify-between gap-2 rounded-lg border border-lp-line bg-lp-card px-3 py-3 text-xs sm:px-4"
          >
            <span className="text-lp-ink">{name}</span>
            <span className="flex min-w-0 items-baseline gap-2 sm:gap-4">
              <span
                className={`rounded-md px-2 py-0.5 font-mono text-xs wrap-anywhere ${
                  verdict === 'accepted'
                    ? 'bg-lp-tint text-lp-accent'
                    : 'bg-lp-panel text-lp-body'
                }`}
              >
                {verdict}
              </span>
              <span className="shrink-0 text-right font-mono text-lp-fade sm:w-12">
                {score}
              </span>
            </span>
          </li>
        ))}
      </ul>
      <p className="mt-4 flex items-center justify-between rounded-lg bg-lp-night px-4 py-3 font-mono text-[11.5px]">
        <span className="text-lp-night-fade">verdict</span>
        <span className="text-white">wrong_answer</span>
      </p>
    </div>
  );
}

const productVisuals = {
  api: ApiVisual,
  playground: PlaygroundVisual,
  judge: JudgeVisual,
};

function Products() {
  return (
    <section id="products" className="pt-20 sm:pt-26 lg:pt-32">
      <SectionHeading
        strong="One execution engine,"
        faint="three ways to use it"
      >
        Integrate the API, experiment in the browser, or judge programs against
        test cases. Every path runs through the same sandbox.
      </SectionHeading>
      <div className="mt-8 grid gap-5 sm:mt-10 sm:gap-6">
        {products.map((product) => {
          const Visual = productVisuals[product.id];
          return (
            <Frame key={product.id}>
              <article
                id={`product-${product.id}`}
                className="grid gap-4 lg:min-h-110 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
              >
                <div className="flex flex-col justify-between gap-8 rounded-xl border border-lp-line bg-lp-panel p-6 sm:p-9 xl:p-11">
                  <div>
                    <h3 className="text-3xl font-semibold tracking-tight text-lp-ink xl:text-4xl">
                      {product.name}
                    </h3>
                    <p className="mt-4 max-w-prose text-base leading-7 text-lp-body">
                      {product.summary}
                    </p>
                    <div className="mt-6">
                      <FeatureList items={product.features} />
                    </div>
                  </div>
                  <ActionButton
                    href={product.action.href}
                    variant="pearl"
                    className="w-fit"
                  >
                    {product.action.label}
                  </ActionButton>
                </div>
                <Visual />
              </article>
            </Frame>
          );
        })}
      </div>
    </section>
  );
}

function Languages() {
  return (
    <section id="languages" className="pt-20 sm:pt-26 lg:pt-32">
      <SectionHeading
        strong="Six languages,"
        faint="pinned and"
        trailing="server-controlled"
      >
        Each language is a validated profile with a fixed executable path,
        structured arguments and its own runtime ceilings. Users never choose a
        host binary.
      </SectionHeading>
      <ul className="mt-8 grid gap-4 sm:mt-10 sm:grid-cols-2 lg:grid-cols-3 lg:gap-5">
        {languages.map((language, index) => (
          <li
            key={language.name}
            data-reveal
            style={revealDelay(index)}
            className={`flex min-h-52 flex-col rounded-xl border border-lp-line bg-lp-card p-5 sm:p-7 xl:p-8 ${cardMotion}`}
          >
            <div className="mb-6 flex items-start justify-between gap-3">
              <div>
                <h3 className="text-xl font-semibold tracking-tight text-lp-ink">
                  {language.name}
                </h3>
                <p className="mt-1 font-mono text-[11.5px] text-lp-fade">
                  {language.file}
                </p>
              </div>
              <span className="rounded-md border border-lp-line px-2.5 py-1 text-[11px] text-lp-body">
                {language.mode}
              </span>
            </div>
            <dl className="mt-auto grid grid-cols-2 gap-3 border-t border-lp-line pt-4 text-sm">
              <div>
                <dt className="text-lp-fade">CPU ceiling</dt>
                <dd className="mt-0.5 font-mono text-lp-ink">{language.cpu}</dd>
              </div>
              <div>
                <dt className="text-lp-fade">Memory ceiling</dt>
                <dd className="mt-0.5 font-mono text-lp-ink">
                  {language.memory}
                </dd>
              </div>
            </dl>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Lifecycle() {
  return (
    <section
      aria-labelledby="lifecycle-heading"
      className="pt-20 sm:pt-26 lg:pt-32"
    >
      <Frame>
        <div className="rounded-xl border border-lp-night-line bg-lp-night p-5 sm:p-9 xl:p-12">
          <h2
            id="lifecycle-heading"
            className="max-w-3xl text-3xl font-semibold tracking-tight text-white sm:text-4xl xl:text-5xl"
          >
            From request to verdict,{' '}
            <span className="text-lp-night-fade">
              without losing a submission
            </span>
          </h2>
          <ol className="mt-10 grid gap-px overflow-hidden rounded-lg border border-lp-night-line bg-lp-night-line sm:grid-cols-2 lg:grid-cols-3">
            {lifecycle.map(([title, text], index) => (
              <li
                key={title}
                className={`min-h-40 bg-lp-night p-5 transition-colors ${hoverMotion} hover:bg-lp-night-raise sm:p-7`}
              >
                <span className="font-mono text-[11px] text-lp-night-fade">
                  {String(index + 1).padStart(2, '0')}
                </span>
                <h3 className="mt-3 text-[16px] text-white">{title}</h3>
                <p className="mt-2 text-sm leading-6 text-lp-night-fade">
                  {text}
                </p>
              </li>
            ))}
          </ol>
        </div>
      </Frame>
    </section>
  );
}

function Security() {
  return (
    <section id="security" className="pt-20 sm:pt-26 lg:pt-32">
      <SectionHeading strong="Every input" faint="is treated as hostile">
        Limits are enforced by Isolate and the kernel’s cgroups, not just
        written in configuration. Each attempt gets a fresh sandbox.
      </SectionHeading>
      <ul className="mt-8 grid gap-4 sm:mt-10 sm:grid-cols-2 lg:grid-cols-4 lg:gap-5">
        {controls.map(({ icon: Icon, title, text }, index) => (
          <li
            key={title}
            data-reveal
            style={revealDelay(index)}
            className={`group flex min-h-52 flex-col rounded-xl border border-lp-line bg-lp-card p-5 sm:p-7 xl:p-8 ${cardMotion}`}
          >
            <span
              className={`grid size-9 place-items-center rounded-lg bg-lp-panel text-lp-ink transition-colors ${hoverMotion} group-hover:bg-lp-night group-hover:text-lp-night-text`}
            >
              <Icon size={17} aria-hidden="true" />
            </span>
            <h3 className="mt-5 text-lg font-semibold text-lp-ink">{title}</h3>
            <p className="mt-1.5 text-sm leading-6 text-lp-body">{text}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Deploy() {
  return (
    <section id="deploy" className="pt-20 sm:pt-26 lg:pt-32">
      <SectionHeading
        strong="Start on your laptop,"
        faint="grow into a worker fleet"
      >
        The API and workers are separate process types. Scale workers by
        measured capacity, not request rate.
      </SectionHeading>
      <div className="mt-8 grid gap-4 sm:mt-10 lg:grid-cols-3 lg:gap-5">
        {deployments.map((plan, index) => (
          <article
            key={plan.name}
            data-reveal
            style={revealDelay(index)}
            className={`flex min-h-105 flex-col justify-between gap-8 rounded-xl border p-6 sm:p-8 xl:p-9 ${cardMotion} ${
              plan.featured
                ? 'border-lp-night-line bg-lp-night text-white hover:border-[#4a4a4a]'
                : 'border-lp-line bg-lp-card'
            }`}
          >
            <div>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h3
                  className={`text-xl font-semibold ${plan.featured ? 'text-white' : 'text-lp-ink'}`}
                >
                  {plan.name}
                </h3>
                <span
                  className={`rounded-md px-2.5 py-1 text-[11px] ${
                    plan.featured
                      ? 'bg-lp-accent-deep text-lp-accent-soft'
                      : 'border border-lp-line text-lp-body'
                  }`}
                >
                  {plan.scope}
                </span>
              </div>
              <p
                className={`mt-3 text-sm leading-6 lg:min-h-12 ${plan.featured ? 'text-lp-night-fade' : 'text-lp-body'}`}
              >
                {plan.summary}
              </p>
              <p
                className={`mt-6 text-2xl font-semibold tracking-tight xl:text-3xl ${plan.featured ? 'text-white' : 'text-lp-ink'}`}
              >
                {plan.headline}
              </p>
              <div
                className={`my-6 border-t ${plan.featured ? 'border-lp-night-line' : 'border-lp-line'}`}
              />
              <FeatureList items={plan.features} dark={plan.featured} />
            </div>
            <ActionButton
              href={plan.action.href}
              variant={plan.featured ? 'pearl' : 'obsidian'}
              className={`w-full ${plan.featured ? 'border-transparent focus-visible:ring-offset-lp-night' : ''}`}
            >
              {plan.action.label}
            </ActionButton>
          </article>
        ))}
      </div>
    </section>
  );
}

function Support() {
  return (
    <section id="support" className="pt-20 sm:pt-26 lg:pt-32">
      <SectionHeading strong="Get help" faint="when you need it">
        Whether you are building a coding platform, checking AI-generated code
        or running assessments, start with the docs and runbooks.
      </SectionHeading>
      <div className="mt-8 grid gap-4 sm:mt-10 md:grid-cols-3 lg:gap-5">
        {support.map(({ icon: Icon, title, text, meta, action }, index) => (
          <article
            key={title}
            data-reveal
            style={revealDelay(index)}
            className={`group flex min-h-76 flex-col justify-between gap-8 rounded-xl border border-lp-line bg-lp-card p-6 sm:p-8 xl:p-9 ${cardMotion}`}
          >
            <div>
              <span
                className={`grid size-10 place-items-center rounded-lg bg-lp-panel text-lp-ink transition-colors ${hoverMotion} group-hover:bg-lp-night group-hover:text-lp-night-text`}
              >
                <Icon size={18} aria-hidden="true" />
              </span>
              <h3 className="mt-6 text-xl font-semibold text-lp-ink">
                {title}
              </h3>
              <p className="mt-2 text-sm leading-6 text-lp-body">{text}</p>
              <ul className="mt-5 flex flex-wrap gap-2">
                {meta.map((item) => (
                  <li
                    key={item}
                    className="rounded-md bg-lp-panel px-2.5 py-1 text-[11px] text-lp-body"
                  >
                    {item}
                  </li>
                ))}
              </ul>
            </div>
            <ActionButton href={action.href} variant="pearl" className="w-fit">
              {action.label}
            </ActionButton>
          </article>
        ))}
      </div>
    </section>
  );
}

function CallToAction() {
  return (
    <section className="pt-20 sm:pt-26 lg:pt-32">
      <Frame>
        <div className="grid min-h-58 items-center gap-8 rounded-xl border border-lp-line bg-lp-panel p-6 sm:p-9 lg:grid-cols-[minmax(0,1fr)_minmax(0,24rem)] xl:p-12">
          <div>
            <h2 className="text-3xl font-semibold tracking-tight text-lp-ink sm:text-4xl xl:text-5xl">
              Run your first program{' '}
              <span className="text-lp-fade">in under a minute</span>
            </h2>
            <p className="mt-4 max-w-lg text-base leading-7 text-lp-body">
              Start the API and a worker, open the playground, and press Ctrl /
              ⌘ + Enter. The same request works from any HTTP client.
            </p>
          </div>
          <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-1">
            <ActionButton href="/playground" size="lg" className="text-sm">
              Open playground
            </ActionButton>
            <ActionButton
              href={repository}
              variant="pearl"
              size="lg"
              className="text-sm"
            >
              View on GitHub
            </ActionButton>
          </div>
        </div>
      </Frame>
    </section>
  );
}

const footerColumns = [
  {
    title: 'Product',
    links: [
      ['Execution API', '#product-api'],
      ['Playground', '/playground'],
      ['Judge', '#product-judge'],
      ['Languages', '#languages'],
    ],
  },
  {
    title: 'Documentation',
    links: [
      ['Architecture', doc('docs/architecture.md')],
      ['Submission API', doc('docs/submission-api.md')],
      ['OpenAPI spec', doc('docs/openapi.yaml')],
      ['Threat model', doc('docs/threat-model.md')],
    ],
  },
  {
    title: 'Operations',
    links: [
      ['Deployment', doc('deployment/README.md')],
      ['Observability', doc('docs/phase9-observability-performance.md')],
      ['Runbooks', `${repository}/tree/main/docs/runbooks`],
      ['Capacity report', doc('docs/capacity-report.md')],
    ],
  },
] as const;

function Footer() {
  return (
    <footer className="mx-auto mt-20 max-w-384 px-4 pb-10 sm:mt-26 sm:px-8 lg:mt-32 lg:px-10">
      <div className="grid gap-10 border-t border-lp-line pt-12 md:grid-cols-[1.5fr_repeat(3,1fr)]">
        <div>
          <a
            href="/"
            className="flex items-center gap-2.5 text-[15px] font-semibold text-lp-ink"
          >
            Ferrum
          </a>
          <p className="mt-4 max-w-xs text-[13px] leading-6 text-lp-body">
            A Rust code execution and judging engine. Sandboxed, durable and
            observable by design.
          </p>
        </div>
        {footerColumns.map((column) => (
          <div key={column.title}>
            <h2 className="text-[12px] font-medium text-lp-ink">
              {column.title}
            </h2>
            <ul className="mt-4 grid gap-2.5">
              {column.links.map(([label, href]) => (
                <li key={label}>
                  <TextLink
                    href={href}
                    className={`text-[13px] text-lp-body transition-colors ${hoverMotion} hover:text-lp-ink`}
                  >
                    {label}
                  </TextLink>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      <div className="mt-12 flex flex-wrap items-center justify-between gap-3 border-t border-lp-line pt-6 text-[12px] text-lp-fade">
        <span>Ferrum Execution Engine</span>
        <span>Rust · Axum · PostgreSQL · Apalis · Isolate</span>
      </div>
    </footer>
  );
}

export default function Landing() {
  useReveal();
  return (
    <div className="min-h-dvh bg-lp-canvas font-sans text-lp-ink antialiased">
      <Navigation />
      <main className="mx-auto max-w-384 px-4 sm:px-8 lg:px-10">
        <Hero />
        <Stack />
        <Products />
        <Languages />
        <Lifecycle />
        <Security />
        <Deploy />
        <Support />
        <CallToAction />
      </main>
      <Footer />
    </div>
  );
}
