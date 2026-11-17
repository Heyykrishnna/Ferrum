import { doc, repository } from '../chrome';
import { CodeBlock, DocumentPage, Section, TextLink } from './article';

const updated = '16 November 2026';

const sections = [
  { id: 'overview', label: 'Overview' },
  { id: 'local', label: 'Run locally' },
  { id: 'submissions', label: 'Submissions' },
  { id: 'languages', label: 'Languages' },
  { id: 'limits', label: 'Resource limits' },
  { id: 'judging', label: 'Judging' },
  { id: 'playground', label: 'Playground' },
  { id: 'access', label: 'Access and quotas' },
  { id: 'reference', label: 'Reference' },
];

const languages = [
  ['Python 3', 'python', 'main.py', 'Interpreted', '2 s', '128 MiB'],
  ['C++', 'cpp', 'main.cpp', 'GCC 14, C++20', '2 s', '128 MiB'],
  ['JavaScript', 'javascript', 'main.cjs', 'Node.js 20', '3 s', '256 MiB'],
  ['C', 'c', 'main.c', 'GCC 14, C17', '2 s', '128 MiB'],
  ['Java', 'java', 'Main.java', 'OpenJDK 21', '3 s', '256 MiB'],
  ['Rust', 'rust', 'main.rs', 'rustc 1.85', '2 s', '128 MiB'],
];

const references = [
  [
    'Architecture',
    'docs/architecture.md',
    'Processes, durable state, lifecycle',
  ],
  ['Submission API', 'docs/submission-api.md', 'Startup and the HTTP contract'],
  ['Language runtimes', 'docs/phase5-languages.md', 'Profiles and ceilings'],
  [
    'Judging engine',
    'docs/phase6-judging.md',
    'Comparators, scores, hidden cases',
  ],
  [
    'Production API',
    'docs/phase7-production-api.md',
    'Keys, scopes, batches, webhooks',
  ],
  ['Threat model', 'docs/threat-model.md', 'Assets and release evidence'],
  ['Deployment', 'deployment/README.md', 'Local and host setup'],
  ['Runbooks', 'docs/runbooks', 'Recovery and incident steps'],
];

const submitExample = `{
  "language": "python",
  "source": "print(input())\\n",
  "stdin": "hello\\n",
  "expected_output": "hello\\n",
  "comparison_mode": "exact"
}`;

const testsExample = `{
  "language": "python",
  "source": "print('Hello, ' + input().strip() + '!')\\n",
  "comparison_mode": "exact",
  "tests": [
    { "input": "Ada\\n", "expected_output": "Hello, Ada!\\n", "weight": 10 },
    { "input": "Grace\\n", "expected_output": "Hello, Grace!\\n", "weight": 10 },
    {
      "input": "Lin\\n",
      "expected_output": "Hello, Lin!\\n",
      "weight": 20,
      "hidden": true
    }
  ]
}`;

export default function DocsPage() {
  return (
    <DocumentPage
      eyebrow="Documentation"
      title="Run, judge, and read the result"
      lede="Ferrum accepts source over HTTP, stores the submission with an outbox event, and compiles and runs it inside Isolate. These pages cover the path you use day to day. The repository holds the longer engineering notes."
      updated={updated}
      sections={sections}
    >
      <Section id="overview" title="Overview">
        <p>
          A submission moves from{' '}
          <code className="font-mono text-[13px]">created</code> to{' '}
          <code className="font-mono text-[13px]">queued</code>, then{' '}
          <code className="font-mono text-[13px]">processing</code>, and ends{' '}
          <code className="font-mono text-[13px]">completed</code>,{' '}
          <code className="font-mono text-[13px]">failed</code>, or{' '}
          <code className="font-mono text-[13px]">cancelled</code>. The
          processing state describes the platform. The verdict describes the
          program: accepted, wrong answer, a compilation or runtime error, or a
          time, memory, or output limit.
        </p>
        <p>
          The API does not wait for the program. Create returns as soon as the
          submission and its queue event are committed. Poll the submission, or
          receive a signed webhook, for the result.
        </p>
      </Section>

      <Section id="local" title="Run locally">
        <p>
          Intake needs three environment variables on the API process:{' '}
          <code className="font-mono text-[13px]">FERRUM_DATABASE_URL</code>,{' '}
          <code className="font-mono text-[13px]">FERRUM_DEV_TENANT_ID</code>,
          and <code className="font-mono text-[13px]">FERRUM_DEV_TOKEN</code>.
          Keep the same tenant id across restarts. Generate the token with{' '}
          <code className="font-mono text-[13px]">openssl rand -hex 32</code>{' '}
          and do not commit it.
        </p>
        <CodeBlock label="From the repository root">{`cargo run --locked -p ferrum-api -- migrate
cargo run --locked -p ferrum-api`}</CodeBlock>
        <p>
          The API listens on{' '}
          <code className="font-mono text-[13px]">127.0.0.1:8080</code> unless{' '}
          <code className="font-mono text-[13px]">listen_addr</code> says
          otherwise. The web playground proxies{' '}
          <code className="font-mono text-[13px]">/api</code> to that API. Point
          it elsewhere with{' '}
          <code className="font-mono text-[13px]">FERRUM_WEB_API_URL</code>. A
          qualified Linux worker, connected to the same database, is what
          actually runs the code. The{' '}
          <TextLink href={doc('docs/submission-api.md')}>
            submission API guide
          </TextLink>{' '}
          has the database setup and the{' '}
          <TextLink href={doc('docs/phase4-worker.md')}>worker guide</TextLink>{' '}
          has the host steps.
        </p>
      </Section>

      <Section id="submissions" title="Submissions">
        <p>
          <code className="font-mono text-[13px]">POST /v1/submissions</code>{' '}
          with a bearer token and an{' '}
          <code className="font-mono text-[13px]">Idempotency-Key</code> of
          1–128 ASCII letters, digits, or{' '}
          <code className="font-mono text-[13px]">-_.:</code>. A repeated key
          with the same payload returns the existing submission. The same key
          with a different payload returns 409.
        </p>
        <CodeBlock label="POST /v1/submissions">{submitExample}</CodeBlock>
        <p>
          A create returns{' '}
          <strong className="font-medium text-lp-ink">202</strong> and a{' '}
          <code className="font-mono text-[13px]">Location</code> header. Poll
          with{' '}
          <code className="font-mono text-[13px]">
            GET /v1/submissions/{'{id}'}
          </code>
          . Cancel with <code className="font-mono text-[13px]">DELETE</code> on
          the same path. Source and stdin are each limited to 65,536 UTF-8 bytes
          and cannot contain NUL. The encoded JSON body is capped at 800 KiB.
        </p>
        <p>
          Other routes:{' '}
          <code className="font-mono text-[13px]">
            POST /v1/submissions/batch
          </code>{' '}
          for 1–20 programs,{' '}
          <code className="font-mono text-[13px]">GET /v1/languages</code> for
          the configured profiles, and{' '}
          <code className="font-mono text-[13px]">
            GET /v1/execution/status
          </code>{' '}
          for whether a worker heartbeat is fresh.
        </p>
      </Section>

      <Section id="languages" title="Languages">
        <p>
          Each language is a server-owned profile with a fixed source name,
          executable path, and argument vector. A request selects a language
          slug. It cannot choose a host binary or extra compiler flags.
        </p>
        <div className="max-w-3xl overflow-x-auto rounded-xl border border-lp-line bg-lp-card">
          <table className="w-full min-w-[40rem] text-left text-[13px]">
            <thead className="bg-lp-panel text-[11px] tracking-wide text-lp-fade uppercase">
              <tr>
                {['Language', 'Slug', 'File', 'Toolchain', 'CPU', 'Memory'].map(
                  (heading) => (
                    <th key={heading} className="px-4 py-2.5 font-medium">
                      {heading}
                    </th>
                  ),
                )}
              </tr>
            </thead>
            <tbody>
              {languages.map((row) => (
                <tr key={row[1]} className="border-t border-lp-line">
                  {row.map((cell) => (
                    <td
                      key={cell}
                      className="px-4 py-3 font-mono text-[12px] text-lp-ink"
                    >
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section id="limits" title="Resource limits">
        <p>
          Requested <code className="font-mono text-[13px]">cpu_time_ms</code>,{' '}
          <code className="font-mono text-[13px]">wall_time_ms</code>, and{' '}
          <code className="font-mono text-[13px]">memory_kib</code> must be
          positive and within the profile ceiling. Omitted fields use the
          profile default. Wall time must be at least the effective CPU time.
        </p>
        <p>
          Python, C, C++, and Rust run for up to 2 seconds of CPU and 6 seconds
          of wall time, in 128 MiB. Node.js and Java use 3 seconds of CPU and
          256 MiB. Compilation has its own ceiling: 8 seconds of CPU and 12
          seconds of wall time, with 512 MiB for C, C++, and Java, and 768 MiB
          for Rust. Compiler and runtime output are each capped at 64 KiB per
          stream. Network access is off for the sandboxed program.
        </p>
      </Section>

      <Section id="judging" title="Judging and test cases">
        <p>
          Without <code className="font-mono text-[13px]">expected_output</code>{' '}
          or <code className="font-mono text-[13px]">tests</code>, Ferrum runs
          the program and returns the raw result with a null verdict. With
          either field, the judge compares captured stdout and stores a verdict.
        </p>
        <ul className="grid gap-2">
          <li>
            <strong className="font-medium text-lp-ink">exact</strong> requires
            the same bytes, including line endings and trailing spaces.
          </li>
          <li>
            <strong className="font-medium text-lp-ink">whitespace</strong>{' '}
            converts <code className="font-mono text-[13px]">\r\n</code> to{' '}
            <code className="font-mono text-[13px]">\n</code>, drops trailing
            whitespace on each line, and drops trailing empty lines.
          </li>
          <li>
            <strong className="font-medium text-lp-ink">tokens</strong> compares
            the sequence of non-whitespace tokens.
          </li>
        </ul>
        <p>
          <code className="font-mono text-[13px]">tests</code> is an ordered
          list. Each case needs{' '}
          <code className="font-mono text-[13px]">expected_output</code> and may
          set <code className="font-mono text-[13px]">input</code>,{' '}
          <code className="font-mono text-[13px]">weight</code> (default 1),{' '}
          <code className="font-mono text-[13px]">hidden</code>, and its own{' '}
          <code className="font-mono text-[13px]">comparison_mode</code>. A
          single <code className="font-mono text-[13px]">expected_output</code>{' '}
          is one public case of weight 1, using{' '}
          <code className="font-mono text-[13px]">stdin</code> as its input. The
          score is the sum of weights of passing cases. The maximum is the sum
          of every weight.
        </p>
        <CodeBlock label="Weighted cases">{testsExample}</CodeBlock>
        <p>
          One verdict covers the whole submission. Compilation error wins, then
          an internal error, then the first case that is not accepted, in suite
          order. Accepted is returned only when every case passes. A hidden case
          still counts toward the score, but the response omits its input,
          expected output, and the program’s stdout, stderr, exit code, and
          signal.
        </p>
      </Section>

      <Section id="playground" title="Playground">
        <p>
          The <TextLink href="/playground">playground</TextLink> is the same API
          behind a Monaco editor. Choose a language, edit the source, set
          standard input, and run. Submit adds the expected output and the
          comparison mode selected in the Test input card. Ctrl / ⌘ + Enter runs
          the current buffer. The editor keeps a draft per language until you
          reload the page.
        </p>
        <p>
          Run and Submit stay disabled until the language list has loaded and
          the worker reports available. Cancel stops a submission that is still
          queued or running. The console shows stdout, stderr, and compiler
          output from the stored result.
        </p>
      </Section>

      <Section id="access" title="Access and quotas">
        <p>
          Every submission route requires a bearer token. In development that
          token is{' '}
          <code className="font-mono text-[13px]">FERRUM_DEV_TOKEN</code>, which
          has admin scope. Production keys use the prefix{' '}
          <code className="font-mono text-[13px]">ferrum_live_</code>. Ferrum
          stores a SHA-256 hash, shows the raw secret once at creation, and
          compares it in constant time.
        </p>
        <p>
          Scopes are{' '}
          <code className="font-mono text-[13px]">submissions:read</code>,{' '}
          <code className="font-mono text-[13px]">submissions:write</code>,{' '}
          <code className="font-mono text-[13px]">submissions:cancel</code>,{' '}
          <code className="font-mono text-[13px]">webhooks:manage</code>, and{' '}
          <code className="font-mono text-[13px]">admin</code>. A tenant can
          have 50 submissions in{' '}
          <code className="font-mono text-[13px]">created</code>,{' '}
          <code className="font-mono text-[13px]">queued</code>, or{' '}
          <code className="font-mono text-[13px]">processing</code>. Beyond that
          the API returns 429. Reads never reveal whether an id belongs to
          another tenant.
        </p>
      </Section>

      <Section id="reference" title="Reference">
        <p>
          The notes below live in the{' '}
          <TextLink href={repository}>Ferrum repository</TextLink>. They are the
          detailed source for behavior this page summarizes.
        </p>
        <ul className="grid gap-2">
          {references.map(([label, path, detail]) => (
            <li key={path}>
              <TextLink
                href={
                  path.endsWith('.md') || path.endsWith('.yaml')
                    ? doc(path)
                    : `${repository}/tree/main/${path}`
                }
              >
                {label}
              </TextLink>
              <span className="text-lp-fade"> — {detail}</span>
            </li>
          ))}
        </ul>
      </Section>
    </DocumentPage>
  );
}
