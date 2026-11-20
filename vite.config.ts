import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

// https://vite.dev/config/
// Tauri expects a fixed port, fail if that port is not available
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev`
  // prevent vite from obscuring rust errors
  clearScreen: false,
  server: {
    port: 4379,
    strictPort: true,
    watch: {
      // Ignore rust files to prevent unnecessary reloads
      ignored: ['**/src-tauri/**'],
    },
  },
  // Environment variables starting with TAURI_ are exposed to the frontend
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    outDir: 'dist',
    // Tauri uses Chromium on Windows and WebKit on macOS and Linux
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    // don't minify for debug builds
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    // produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      output: {
        manualChunks: (id) => {
          // Rollup's CommonJS interop helpers (a virtual module, outside node_modules):
          // with React's chunk, which every page loads, so a lazy vendor chunk never holds them.
          if (id.includes('commonjsHelpers')) {
            return 'vendor-react'
          }
          if (id.includes('node_modules')) {
            // Flow editor (React Flow) - only /flow/editor. Before the React rule:
            // '@xyflow/react/' contains '/react/'.
            if (id.includes('/@xyflow/')) {
              return 'vendor-flow'
            }
            // Core React - most stable, cached long-term
            if (id.includes('/react-dom/') || id.includes('/react/') || id.includes('/scheduler/')) {
              return 'vendor-react'
            }
            // Router
            if (id.includes('react-router')) {
              return 'vendor-router'
            }
            // Radix UI primitives
            if (id.includes('@radix-ui')) {
              return 'vendor-radix'
            }
            // Icons - frequently updated
            if (id.includes('lucide-react')) {
              return 'vendor-icons'
            }
            // Syntax highlighting - only needed on code pages
            if (id.includes('react-syntax-highlighter') || id.includes('prismjs') || id.includes('refractor')) {
              return 'vendor-syntax'
            }
            // Chart tools (drawing library + 1,800 indicators) - only the Historify chart page
            if (
              id.includes('lightweight-charts-indicators') ||
              id.includes('lightweight-charts-drawing') ||
              id.includes('oakscriptjs')
            ) {
              return 'vendor-chart-tools'
            }
            // Charts - only needed on chart pages
            if (id.includes('recharts') || id.includes('d3-') || id.includes('lightweight-charts')) {
              return 'vendor-charts'
            }
            // Tauri
            if (id.includes('@tauri-apps')) {
              return 'vendor-tauri'
            }
          }
        },
      },
    },
  },
})
