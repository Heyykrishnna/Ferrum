import '@fontsource-variable/inter';
import '@fontsource/ibm-plex-mono/400.css';
import './styles.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import App from './App';

const container = document.getElementById('root');
if (!container) throw new Error('Missing Ferrum root.');

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
