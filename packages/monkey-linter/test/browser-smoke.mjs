import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { build } from 'vite'
import wasm from 'vite-plugin-wasm'

// Exercise the artifact selected by package.json's `browser` field. In
// particular, the Wasm dependency must remain an ESM import: turning it into a
// CommonJS require makes bundlers reject its asynchronous Wasm initialization.
const packageJson = JSON.parse(
  readFileSync(new URL('../package.json', import.meta.url), 'utf8')
)

await build({
  logLevel: 'silent',
  plugins: [wasm()],
  build: {
    write: false,
    rollupOptions: {
      input: fileURLToPath(
        new URL(`../${packageJson.browser}`, import.meta.url)
      ),
    },
  },
})
