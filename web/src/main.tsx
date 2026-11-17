import '@fontsource-variable/inter';
import '@fontsource/ibm-plex-mono/400.css';
import './styles.css';

import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';

import Landing from './Landing';

const App = lazy(() => import('./App'));
const DocsPage = lazy(() => import('./pages/Docs'));
const TermsPage = lazy(() => import('./pages/Terms'));
const PrivacyPage = lazy(() => import('./pages/Privacy'));

const container = document.getElementById('root');
if (!container) throw new Error('Missing Ferrum root.');

const path = window.location.pathname.replace(/\/+$/, '') || '/';
const pages = {
  '/playground': { title: 'Ferrum Playground', Page: App },
  '/docs': { title: 'Docs — Ferrum', Page: DocsPage },
  '/terms': { title: 'Terms and conditions — Ferrum', Page: TermsPage },
  '/privacy': { title: 'Privacy — Ferrum', Page: PrivacyPage },
} as const;

const page = pages[path as keyof typeof pages];
document.title = page?.title ?? 'Ferrum — Sandboxed code execution';

createRoot(container).render(
  <StrictMode>
    {page ? (
      <Suspense fallback={<div className="min-h-dvh bg-lp-canvas" />}>
        <page.Page />
      </Suspense>
    ) : (
      <Landing />
    )}
  </StrictMode>,
);
