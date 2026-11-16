import '@fontsource-variable/inter';
import '@fontsource/ibm-plex-mono/400.css';
import './styles.css';

import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';

import Landing from './Landing';

const App = lazy(() => import('./App'));

const container = document.getElementById('root');
if (!container) throw new Error('Missing Ferrum root.');

const playground = /^\/playground\/?$/.test(window.location.pathname);
document.title = playground
  ? 'Ferrum Playground'
  : 'Ferrum — Sandboxed code execution';

createRoot(container).render(
  <StrictMode>
    {playground ? (
      <Suspense>
        <App />
      </Suspense>
    ) : (
      <Landing />
    )}
  </StrictMode>,
);
