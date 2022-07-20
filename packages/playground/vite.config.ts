import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasmPack from 'vite-plugin-wasm-pack';
import wasm from "vite-plugin-wasm";


// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 3000
  },
  plugins: [
    react(),
    // wasm(),
    wasmPack([], ['@gengjiawen/monkey-wasm']),
  ],
})
