import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import { gatewayForPort, webPort } from './server/gateway.ts';

const port = webPort(5173);

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    {
      name: 'ferrum-gateway',
      configureServer(server) {
        server.middlewares.use(gatewayForPort(port));
      },
    },
  ],
  server: { host: '127.0.0.1', port, strictPort: true },
});
