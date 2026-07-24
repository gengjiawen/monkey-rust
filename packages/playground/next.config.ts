import type { NextConfig } from 'next'
import path from 'node:path'

const nextConfig: NextConfig = {
  allowedDevOrigins: ['eu.gengjiawen.com'],
  output: 'export',
  trailingSlash: true,
  experimental: {
    // TypeScript 7 (tsgo) no longer ships the compiler API Next.js uses,
    // so type checking has to go through the TypeScript CLI.
    useTypeScriptCli: true,
  },
  transpilePackages: ['prettier-plugin-monkey'],
  turbopack: {
    root: path.resolve(__dirname, '../..'),
    resolveAlias: {
      prettier: {
        browser: 'prettier/standalone',
      },
    },
  },
}

export default nextConfig
