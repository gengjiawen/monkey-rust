#!/usr/bin/env node

import { readFileSync } from 'node:fs'

import { minify, type MinifyOptions } from './node'

function main(argv: string[]): void {
  const options: MinifyOptions = {}
  const files: string[] = []
  for (const argument of argv) {
    switch (argument) {
      case '--no-mangle':
        options.mangle = false
        break
      case '--no-fold':
        options.fold = false
        break
      case '-h':
      case '--help':
        process.stdout.write(
          'Usage: monkey-minify [--no-mangle] [--no-fold] [file ...]\n' +
            'With no files, source is read from stdin.\n'
        )
        return
      default:
        if (argument.startsWith('-')) {
          throw new Error(`unknown option: ${argument}`)
        }
        files.push(argument)
    }
  }
  const source = files.length
    ? files.map((file) => readFileSync(file, 'utf8')).join('\n')
    : readFileSync(0, 'utf8')
  process.stdout.write(`${minify(source, options).code}\n`)
}

try {
  main(process.argv.slice(2))
} catch (error) {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`
  )
  process.exitCode = 1
}
