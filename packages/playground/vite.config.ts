import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
// @ts-ignore
import topLevelAwait from "vite-plugin-top-level-await"
import wasm from "vite-plugin-wasm"
import visualizer from 'rollup-plugin-visualizer'
import path from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  base: process.env.GITHUB_ACTION ? '/monkey-rust/' : '/',
  server: {
    port: 3000
  },
  plugins: [
    react(),
    wasm(),
    topLevelAwait()
  ],
  resolve: {
    alias: {
      'prettier-plugin-monkey': path.resolve(__dirname, '../prettier-plugin-monkey/src/index.ts'),
    },
  },
  build: {
    rollupOptions: {
      plugins: [visualizer()],
    },
  },
})
