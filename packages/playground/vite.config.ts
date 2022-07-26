import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import topLevelAwait from "vite-plugin-top-level-await"
import wasm from "vite-plugin-wasm"


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
})
