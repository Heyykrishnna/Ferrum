import { ArrowRight, ArrowUpRight, Menu, X } from 'lucide-react';
import type { ReactNode } from 'react';
import { useState } from 'react';

import { CandyButton } from '@/components/ui/candy-button';
import type {
  CandyButtonSize,
  CandyButtonVariant,
} from '@/components/ui/candy-button';
import { cn } from '@/lib/utils';

export const repository = 'https://github.com/Heyykrishnna/Ferrum';
export const doc = (path: string) => `${repository}/blob/main/${path}`;

export const hoverMotion = 'duration-300 ease-[var(--ease-soft)]';

const navigation = [
  { label: 'Products', href: '/#products' },
  { label: 'Languages', href: '/#languages' },
  { label: 'Security', href: '/#security' },
  { label: 'Deploy', href: '/#deploy' },
  { label: 'Support', href: '/#support' },
];

const footerColumns = [
  {
    title: 'Product',
    links: [
      ['Execution API', '/#product-api'],
      ['Playground', '/playground'],
      ['Judge', '/#product-judge'],
      ['Languages', '/#languages'],
    ],
  },
  {
    title: 'Documentation',
    links: [
      ['Guides', '/docs'],
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

const legalLinks = [
  ['Docs', '/docs'],
  ['Terms', '/terms'],
  ['Privacy', '/privacy'],
] as const;

function isExternal(href: string) {
  return href.startsWith('http');
}

export function TextLink({
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

export function ActionButton({
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

function currentPath() {
  return window.location.pathname.replace(/\/+$/, '') || '/';
}

export function SiteHeader() {
  const [open, setOpen] = useState(false);
  const docsPage = currentPath() === '/docs';
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
          <a
            href="/docs"
            aria-current={docsPage ? 'page' : undefined}
            className={`hidden h-9 items-center rounded-md px-4 text-[13px] transition-colors sm:flex ${hoverMotion} ${
              docsPage ? 'text-lp-ink' : 'text-lp-body hover:text-lp-ink'
            }`}
          >
            Docs
          </a>
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
            <a
              href="/docs"
              aria-current={docsPage ? 'page' : undefined}
              onClick={() => setOpen(false)}
              className={`block py-2 text-[14px] transition-colors ${hoverMotion} ${
                docsPage ? 'text-lp-ink' : 'text-lp-body hover:text-lp-ink'
              }`}
            >
              Docs
            </a>
          </div>
        </div>
      </div>
    </header>
  );
}

export function SiteFooter() {
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
          <nav
            aria-label="Legal"
            className="mt-5 flex flex-wrap gap-x-4 gap-y-2"
          >
            {legalLinks.map(([label, href]) => (
              <a
                key={href}
                href={href}
                className={`text-[13px] text-lp-body transition-colors ${hoverMotion} hover:text-lp-ink`}
              >
                {label}
              </a>
            ))}
          </nav>
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
