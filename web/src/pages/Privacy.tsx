import { repository } from '../chrome';
import { DocumentPage, Section, TextLink } from './article';

const updated = '16 November 2026';

const sections = [
  { id: 'scope', label: 'This notice' },
  { id: 'stored', label: 'What is stored' },
  { id: 'playground', label: 'Playground' },
  { id: 'hidden', label: 'Hidden test cases' },
  { id: 'secrets', label: 'Secrets and logs' },
  { id: 'browser', label: 'Browser data' },
  { id: 'isolation', label: 'Programs you run' },
  { id: 'retention', label: 'Retention' },
  { id: 'operators', label: 'Operators' },
  { id: 'contact', label: 'Contact' },
];

export default function PrivacyPage() {
  return (
    <DocumentPage
      eyebrow="Privacy"
      title="What Ferrum keeps"
      lede="Ferrum stores the code you submit so it can run and judge it. This notice describes the data the software is built to handle. It is not a privacy policy for every deployment, and the project does not operate a public hosted service of its own."
      updated={updated}
      sections={sections}
    >
      <Section id="scope" title="This notice">
        <p>
          There is no separate Ferrum company collecting data through this
          repository. Whoever runs an API and a database is the operator of that
          copy. If you use a deployment you do not control, ask that operator
          where the database lives and how long they keep rows. The rest of this
          page is what the software itself does.
        </p>
      </Section>

      <Section id="stored" title="What is stored">
        <p>
          A submission row holds the tenant, the language profile, the source,
          standard input, requested limits, an idempotency key, processing
          state, and timestamps. When you judge a program, Ferrum also stores
          each test case: its input, expected output, weight, comparison mode,
          and whether it is hidden.
        </p>
        <p>
          After execution it stores the result: stdout, stderr, compiler output,
          exit code, signal, CPU time, wall time, and peak memory when the
          sandbox reports them. Judged submissions also store the verdict, the
          score, and the per-case outcome. Each captured stream is bounded to 64
          KiB. Source and stdin are each limited to 65,536 UTF-8 bytes.
        </p>
        <p>
          The queue payload carries identifiers only. Source, stdin, tokens, and
          results are not placed in the job message. They are read from the
          database by the worker.
        </p>
      </Section>

      <Section id="playground" title="Playground">
        <p>
          The editor keeps your draft in memory for the current page. It is not
          written to local storage, and a reload discards it. Nothing is sent to
          the API until you press Run or Submit.
        </p>
        <p>
          The browser talks only to the same origin, under{' '}
          <code className="font-mono text-[13px]">/api</code>. A small gateway
          on that origin forwards the request to the Ferrum API and attaches the
          deployment’s bearer token. The token stays on the server. The page
          does not receive it and does not put it in a cookie.
        </p>
      </Section>

      <Section id="hidden" title="Hidden test cases">
        <p>
          A test case marked hidden still runs and still counts toward the
          score. The API response for that case keeps the index, verdict, score,
          weight, and resource measurements. It omits the case input, the
          expected output, and the program’s stdout, stderr, exit code, and
          signal, so a contestant cannot read a private case back out of the
          result.
        </p>
        <p>
          Hidden does not mean deleted. The operator’s database still contains
          the case. Anyone with database access, or an API key allowed to read
          that tenant, can see what the application is written to reveal. The
          stripping applies to the contestant-facing case finding.
        </p>
      </Section>

      <Section id="secrets" title="Secrets and logs">
        <p>
          Production API keys are stored as a SHA-256 hash. The raw secret is
          returned once, at creation, and is not written into the database. The
          application is written to avoid logging authorization headers, API
          secrets, database passwords, webhook signing secrets, and full source
          by default. Traces are not supposed to carry source, stdin, or
          unbounded program output.
        </p>
        <p>
          That is a property of this codebase, not a guarantee about an
          operator’s log pipeline. An operator who enables request-body logging
          in a proxy will capture source even though Ferrum itself does not log
          it. Do not send secrets inside source or stdin.
        </p>
      </Section>

      <Section id="browser" title="Browser data">
        <p>
          The web app does not set cookies, does not use local or session
          storage, and does not load a third-party analytics or advertising
          script. The theme you pick in the playground lasts until you reload.
          Pages you open are ordinary navigations: the server sees the request
          for the page and, once you run code, the submission body.
        </p>
      </Section>

      <Section id="isolation" title="Programs you run">
        <p>
          Submitted programs run with network access disabled. They are not
          given the worker’s environment, database credentials, or queue
          credentials, and they cannot open a connection to the API as part of
          normal execution. A program can still print anything you placed in its
          source or its input; that output is stored as the result.
        </p>
      </Section>

      <Section id="retention" title="Retention">
        <p>
          Ferrum does not delete submissions on a timer. Rows, outbox events,
          and results stay in PostgreSQL until an operator removes them. Backups
          taken by the operator will keep copies for as long as that backup
          policy says. There is no in-product button that erases your account,
          because the software does not have user accounts separate from API
          keys and a tenant id.
        </p>
        <p>
          Cancelling a submission stops the run. It does not remove the stored
          source or any result already written.
        </p>
      </Section>

      <Section id="operators" title="Operators">
        <p>
          If you deploy Ferrum, you decide which database is used, who can
          connect to it, which keys exist, and how long backups are kept. You
          should tell your users those facts in your own notice. This page will
          not match a deployment that adds accounts, object storage, or extra
          logging.
        </p>
        <p>
          Webhooks, when configured, send event payloads to a URL the tenant
          registered. The destination is chosen by that tenant. Signing secrets
          for webhooks are treated as secrets and should not be logged.
        </p>
      </Section>

      <Section id="contact" title="Contact">
        <p>
          Privacy questions about this project can be opened on the{' '}
          <TextLink href={`${repository}/issues`}>issue tracker</TextLink>.
          Questions about a particular database, including a request to delete a
          submission, go to the operator who runs it. The project cannot delete
          rows in a database it does not host.
        </p>
      </Section>
    </DocumentPage>
  );
}
