import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    target: 'esnext',
  },
  optimizeDeps: {
    // Perspective uses WASM and workers that need to be excluded from optimization
    exclude: ['@finos/perspective', '@finos/perspective-viewer'],
  },
  worker: {
    format: 'es',
  },
})
