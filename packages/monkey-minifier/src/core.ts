import { eliminateDeadLets, foldConstants } from './fold'
import { mangle, type MangleOptions } from './mangle'
import { printProgram } from './printer'
import type { Program } from './types'

export interface MinifyOptions {
  mangle?: boolean | MangleOptions
  fold?: boolean
}

export interface MinifyResult {
  code: string
}

export type ParseLossless = (source: string) => string

export function minifyWithParser(
  parseLossless: ParseLossless,
  source: string,
  options: MinifyOptions = {}
): MinifyResult {
  const program = parseProgramWithParser(parseLossless, source)
  if (options.fold !== false) {
    foldConstants(program)
    eliminateDeadLets(program)
  }
  if (options.mangle !== false) {
    mangle(program, typeof options.mangle === 'object' ? options.mangle : {})
  }
  return { code: printProgram(program) }
}

export function parseProgramWithParser(
  parseLossless: ParseLossless,
  source: string
): Program {
  try {
    const json = parseLossless(source)
    const root = JSON.parse(json) as { Program?: Program } | Program
    const program =
      'Program' in root && root.Program ? root.Program : (root as Program)
    if (program.type !== 'Program' || !Array.isArray(program.body)) {
      throw new Error('parser returned an invalid Program')
    }
    return program
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new SyntaxError(`Monkey parse error: ${message}`)
  }
}
