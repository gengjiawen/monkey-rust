#!/usr/bin/env node

import { runCli } from './cli-lib'

try {
  const { output, exitCode } = runCli(process.argv.slice(2))
  if (output) {
    process.stdout.write(output)
  }
  process.exitCode = exitCode
} catch (error) {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`
  )
  process.exitCode = 1
}
