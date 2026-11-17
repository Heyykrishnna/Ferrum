import { repository } from '../chrome';
import { DocumentPage, Section, TextLink } from './article';

const updated = '16 November 2026';

const sections = [
  { id: 'scope', label: 'These terms' },
  { id: 'service', label: 'The service' },
  { id: 'credentials', label: 'Credentials' },
  { id: 'submissions', label: 'Your submissions' },
  { id: 'use', label: 'Acceptable use' },
  { id: 'results', label: 'Results and isolation' },
  { id: 'availability', label: 'Availability' },
  { id: 'software', label: 'The software' },
  { id: 'changes', label: 'Changes' },
  { id: 'contact', label: 'Contact' },
];

export default function TermsPage() {
  return (
    <DocumentPage
      eyebrow="Terms and conditions"
      title="Terms for using Ferrum"
      lede="These terms cover the Ferrum playground and API shipped with this project. If you run Ferrum yourself, you are the operator of that deployment and you should publish the terms that apply to your users."
      updated={updated}
      sections={sections}
    >
      <Section id="scope" title="These terms">
        <p>
          Ferrum is an open-source code execution and judging engine. This page
          is the project’s own statement of how the software is offered. It is
          not a contract with a company, and it does not name a court or a
          governing law. Where a deployment is operated by someone else, that
          operator’s terms control the relationship with their users.
        </p>
        <p>
          Using the playground, sending requests to an API that presents these
          terms, or deploying the software means you have read them. If you do
          not accept them, do not submit code and do not run the service.
        </p>
      </Section>

      <Section id="service" title="The service">
        <p>
          Ferrum accepts source code, optional standard input, and optional
          expected output. It stores the submission, queues it, and compiles and
          runs it inside an Isolate sandbox on a Linux worker. It then returns
          captured output, resource measurements, and, when you asked for a
          comparison, a verdict.
        </p>
        <p>
          The playground is a browser interface to that API. It does not execute
          your program in the browser. Supported languages, file names, and
          resource ceilings are chosen by the server. You cannot supply an
          executable path, a compiler flag, or a container image.
        </p>
      </Section>

      <Section id="credentials" title="Credentials">
        <p>
          API access uses a bearer token. A development deployment uses the
          token configured on the server. A production key is shown in full only
          when it is created. You are responsible for keeping tokens and keys
          confidential, for choosing who may use them, and for revoking a key
          you no longer trust.
        </p>
        <p>
          Idempotency keys identify a submission inside your tenant. Reusing a
          key with a different payload is rejected. Do not put secrets into an
          idempotency key; it is stored with the submission.
        </p>
      </Section>

      <Section id="submissions" title="Your submissions">
        <p>
          You may submit only code, input, and expected output that you have the
          right to send. Source, standard input, expected output, test cases,
          and captured program output are stored so the submission can be
          queued, executed, judged, and retrieved. Do not submit passwords,
          private keys, personal data, or anything else you do not want kept in
          the operator’s database.
        </p>
        <p>
          Size limits are part of the service. Source and stdin are each limited
          to 65,536 UTF-8 bytes, the encoded request to 800 KiB, and captured
          streams to 64 KiB each. Requests outside those limits are rejected or
          truncated at the cap. A tenant may have at most 50 submissions in
          progress at once.
        </p>
      </Section>

      <Section id="use" title="Acceptable use">
        <p>You will not use Ferrum to:</p>
        <ul className="grid list-disc gap-2 pl-5">
          <li>
            attack the API, the database, the worker host, or another tenant
          </li>
          <li>
            probe for a sandbox escape, or use a submission as a way to reach
            the host filesystem, internal services, or the network
          </li>
          <li>
            run code whose purpose is to disrupt the worker, including fork
            bombs, memory exhaustion, or output floods beyond what the resource
            limits already stop
          </li>
          <li>
            submit malware, exploits, or other people’s private data for
            distribution or storage
          </li>
          <li>
            bypass quotas, share a key in order to evade a limit, or send
            automated traffic that the deployment has not agreed to serve
          </li>
        </ul>
        <p>
          Resource limits exist so a program can be stopped. They are not
          permission to spend the entire limit on every request in order to
          degrade the service. An operator may cancel work, revoke keys, and
          reject traffic that breaks these rules.
        </p>
      </Section>

      <Section id="results" title="Results and isolation">
        <p>
          A verdict is the judge’s comparison of captured output under the mode
          you selected. Exact mode is byte-for-byte. Whitespace and token modes
          normalize only what their definitions say they normalize. A passing
          verdict is not a review of correctness beyond that comparison, and it
          is not a security assessment of the source.
        </p>
        <p>
          Programs run with network access disabled and with restricted
          filesystem, process, CPU, memory, and output limits. Isolation reduces
          the chance that a submission can affect the host or another tenant. It
          is not a promise that every program is contained on every kernel and
          configuration. Run workers on hosts you are prepared to trust with
          untrusted code, and keep Isolate, the kernel, and the toolchains
          patched.
        </p>
      </Section>

      <Section id="availability" title="Availability">
        <p>
          The project does not offer an uptime commitment, a support response
          time, or a backup of your submissions. Jobs can wait for a free
          worker. A missing worker, a full quota, or a database outage will
          delay or reject work. Cancel is the way to stop a submission you no
          longer want run; dropping the browser request does not stop it.
        </p>
        <p>
          The software is provided as available, without warranty of
          merchantability, fitness for a particular purpose, or
          non-infringement, to the extent the law that applies to you allows
          that disclaimer. The project is not liable for lost submissions,
          incorrect verdicts, or damage caused by code you chose to run.
        </p>
      </Section>

      <Section id="software" title="The software">
        <p>
          The source is published at{' '}
          <TextLink href={repository}>github.com/Heyykrishnna/Ferrum</TextLink>.
          These terms govern use of a deployment that presents them. They do not
          replace the license that applies to copying and modifying the source.
          The repository does not currently ship a license file; do not assume a
          license that the repository has not stated.
        </p>
        <p>
          Self-hosting does not make the project the operator of your instance.
          You choose the database, the tokens, the retention period, and the
          people who can reach your API.
        </p>
      </Section>

      <Section id="changes" title="Changes">
        <p>
          The project can update these terms by changing this page. The date at
          the top is the date of the current text. Continued use of a deployment
          that shows the updated page is acceptance of the update. An operator
          of another deployment is responsible for telling their own users when
          their terms change.
        </p>
      </Section>

      <Section id="contact" title="Contact">
        <p>
          Questions about these terms, and reports of abuse on a deployment you
          do not operate, can be opened as an issue on the{' '}
          <TextLink href={`${repository}/issues`}>project tracker</TextLink>.
          For a deployment someone else runs, contact that operator. The project
          cannot cancel their submissions or delete their database.
        </p>
      </Section>
    </DocumentPage>
  );
}
