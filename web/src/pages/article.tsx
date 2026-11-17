import type { ReactNode } from 'react';

import { SiteFooter, SiteHeader } from '../chrome';

export function DocumentPage({
  eyebrow,
  title,
  lede,
  updated,
  sections,
  children,
}: {
  eyebrow: string;
  title: string;
  lede: string;
  updated: string;
  sections: { id: string; label: string }[];
  children: ReactNode;
}) {
  return (
    <div className="min-h-dvh bg-lp-canvas font-sans text-lp-ink antialiased">
      <SiteHeader />
      <main className="mx-auto max-w-384 px-4 pt-10 pb-6 sm:px-8 sm:pt-14 lg:px-10">
        <header className="max-w-3xl">
          <p className="text-[13px] font-medium text-lp-body">{eyebrow}</p>
          <h1 className="mt-3 text-4xl font-semibold tracking-tight text-lp-ink sm:text-5xl">
            {title}
          </h1>
          <p className="mt-5 max-w-2xl text-base leading-7 text-lp-body">
            {lede}
          </p>
          <p className="mt-4 text-[12px] text-lp-fade">Updated {updated}</p>
        </header>
        <div className="mt-10 grid items-start gap-10 lg:mt-14 lg:grid-cols-[15rem_minmax(0,1fr)] lg:gap-16">
          <nav aria-label="On this page" className="lg:sticky lg:top-24">
            <p className="text-[12px] font-medium text-lp-ink">On this page</p>
            <ul className="mt-3 grid gap-2 border-l border-lp-line">
              {sections.map((section) => (
                <li key={section.id}>
                  <a
                    href={`#${section.id}`}
                    className="block border-l border-transparent py-0.5 pl-4 text-[13px] text-lp-body transition-colors duration-300 ease-[var(--ease-soft)] hover:border-lp-ink hover:text-lp-ink"
                  >
                    {section.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
          <div className="grid min-w-0 gap-10">{children}</div>
        </div>
      </main>
      <SiteFooter />
    </div>
  );
}

export function Section({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <section id={id} className="scroll-mt-24 border-t border-lp-line pt-8">
      <h2 className="text-2xl font-semibold tracking-tight text-lp-ink">
        {title}
      </h2>
      <div className="mt-4 grid max-w-3xl gap-4 text-[15px] leading-7 text-lp-body">
        {children}
      </div>
    </section>
  );
}

export function TextLink({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) {
  const external = href.startsWith('http');
  return (
    <a
      href={href}
      className="text-lp-ink underline decoration-lp-line underline-offset-4 transition-colors duration-300 ease-[var(--ease-soft)] hover:decoration-lp-ink"
      {...(external ? { target: '_blank', rel: 'noreferrer' } : {})}
    >
      {children}
    </a>
  );
}

export function CodeBlock({
  label,
  children,
}: {
  label?: string;
  children: string;
}) {
  return (
    <figure className="max-w-3xl overflow-hidden rounded-xl border border-lp-night-line bg-lp-night">
      {label && (
        <figcaption className="border-b border-lp-night-line px-4 py-2 font-mono text-[11px] text-lp-night-fade">
          {label}
        </figcaption>
      )}
      <pre className="overflow-x-auto p-4 font-mono text-xs leading-6 text-lp-night-text sm:p-5">
        <code>{children}</code>
      </pre>
    </figure>
  );
}
