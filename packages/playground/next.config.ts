import type { NextConfig } from 'next'
import path from 'node:path'

const nextConfig: NextConfig = {
  allowedDevOrigins: ['eu.gengjiawen.com'],
  output: 'export',
  trailingSlash: true,
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
