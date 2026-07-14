import type { SourceSpan } from './gcReport'

export const snapshotSections = [
  'header',
  'main',
  'constants',
  'debug',
] as const

export type SnapshotSection = (typeof snapshotSections)[number]

export const snapshotSectionTitles: Record<SnapshotSection, string> = {
  header: 'Header',
  main: 'Main program',
  constants: 'Constant pool',
  debug: 'Debug info',
}

/**
 * One annotated byte range of a `.mbc` file. Regions arrive in file order
 * and tile the buffer exactly (no gaps, no overlaps) — `parseSnapshotBuildEnvelope`
 * re-checks that invariant so the hexdump can rely on it.
 */
export interface SnapshotRegion {
  offset: number
  length: number
  section: SnapshotSection
  label: string
  detail: string
}

export interface SnapshotLayout {
  byteLength: number
  formatVersion: number
  abiFingerprint: string
  hasDebugInfo: boolean
  regions: SnapshotRegion[]
}

export interface SnapshotBuildSuccess {
  status: 'ok'
  bytes: Uint8Array<ArrayBuffer>
  layout: SnapshotLayout
}

export type SnapshotBuildStage = 'parse' | 'compile' | 'snapshot'

export interface SnapshotBuildError {
  status: 'error'
  stage: SnapshotBuildStage
  message: string
}

export type SnapshotBuildEnvelope = SnapshotBuildSuccess | SnapshotBuildError

export interface SnapshotRunSuccess {
  status: 'ok'
  result: string
}

export type SnapshotRunStage = 'snapshot' | 'runtime'

export interface SnapshotRunError {
  status: 'error'
  stage: SnapshotRunStage
  message: string
  span: SourceSpan | null
}

export type SnapshotRunEnvelope = SnapshotRunSuccess | SnapshotRunError

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function readNumber(
  record: Record<string, unknown>,
  key: string,
  path: string
): number {
  const value = record[key]
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${path}.${key} must be a non-negative safe integer`)
  }
  return value
}

function readString(
  record: Record<string, unknown>,
  key: string,
  path: string
): string {
  const value = record[key]
  if (typeof value !== 'string') {
    throw new Error(`${path}.${key} must be a string`)
  }
  return value
}

function parseEnvelope(serialized: string, what: string): unknown {
  let value: unknown
  try {
    value = JSON.parse(serialized) as unknown
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`${what} is not valid JSON: ${message}`)
  }
  if (!isRecord(value)) {
    throw new Error(`${what} must be an object`)
  }
  return value
}

export function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
  if (!/^([0-9a-f]{2})*$/.test(hex)) {
    throw new Error('bytesHex must be lowercase hex byte pairs')
  }
  const bytes = new Uint8Array(hex.length / 2)
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16)
  }
  return bytes
}

function readRegions(value: unknown, byteLength: number): SnapshotRegion[] {
  if (!Array.isArray(value)) {
    throw new Error('layout.regions must be an array')
  }

  let cursor = 0
  const regions = value.map((entry, index) => {
    const path = `layout.regions[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${path} must be an object`)
    }
    const offset = readNumber(entry, 'offset', path)
    const length = readNumber(entry, 'length', path)
    if (length < 1) {
      throw new Error(`${path}.length must be at least 1`)
    }
    if (offset !== cursor) {
      throw new Error(
        `${path} must start at byte ${cursor}, the end of the previous region`
      )
    }
    cursor = offset + length
    const section = entry.section
    if (
      typeof section !== 'string' ||
      !snapshotSections.includes(section as SnapshotSection)
    ) {
      throw new Error(`${path}.section must be a known snapshot section`)
    }
    return {
      offset,
      length,
      section: section as SnapshotSection,
      label: readString(entry, 'label', path),
      detail: readString(entry, 'detail', path),
    }
  })
  if (cursor !== byteLength) {
    throw new Error('layout.regions must tile the buffer up to byteLength')
  }
  return regions
}

function readLayout(value: unknown): SnapshotLayout {
  if (!isRecord(value)) {
    throw new Error('layout must be an object')
  }

  const byteLength = readNumber(value, 'byteLength', 'layout')
  const abiFingerprint = readString(value, 'abiFingerprint', 'layout')
  if (!/^0x[0-9a-f]{8}$/.test(abiFingerprint)) {
    throw new Error('layout.abiFingerprint must be a 0x-prefixed u32 hex string')
  }
  if (typeof value.hasDebugInfo !== 'boolean') {
    throw new Error('layout.hasDebugInfo must be a boolean')
  }
  const regions = readRegions(value.regions, byteLength)
  if (
    !value.hasDebugInfo &&
    regions.some((region) => region.section === 'debug')
  ) {
    throw new Error('layout without debug info must not contain debug regions')
  }

  return {
    byteLength,
    formatVersion: readNumber(value, 'formatVersion', 'layout'),
    abiFingerprint,
    hasDebugInfo: value.hasDebugInfo,
    regions,
  }
}

export function parseSnapshotBuildEnvelope(
  serialized: string
): SnapshotBuildEnvelope {
  const value = parseEnvelope(serialized, 'Snapshot response') as Record<
    string,
    unknown
  >

  if (value.status === 'ok') {
    const layout = readLayout(value.layout)
    const bytes = hexToBytes(readString(value, 'bytesHex', 'envelope'))
    if (bytes.length !== layout.byteLength) {
      throw new Error('bytesHex length must match layout.byteLength')
    }
    return { status: 'ok', bytes, layout }
  }

  if (value.status === 'error') {
    const stage = value.stage
    if (stage !== 'parse' && stage !== 'compile' && stage !== 'snapshot') {
      throw new Error('stage must be parse, compile, or snapshot')
    }
    return {
      status: 'error',
      stage,
      message: readString(value, 'message', 'envelope'),
    }
  }

  throw new Error('Snapshot response status must be ok or error')
}

function readSpan(value: unknown): SourceSpan | null {
  if (value === null || value === undefined) {
    return null
  }
  if (!isRecord(value)) {
    throw new Error('span must be an object or null')
  }
  return {
    start: readNumber(value, 'start', 'span'),
    end: readNumber(value, 'end', 'span'),
  }
}

export function parseSnapshotRunEnvelope(
  serialized: string
): SnapshotRunEnvelope {
  const value = parseEnvelope(serialized, 'Snapshot run response') as Record<
    string,
    unknown
  >

  if (value.status === 'ok') {
    return { status: 'ok', result: readString(value, 'result', 'envelope') }
  }

  if (value.status === 'error') {
    const stage = value.stage
    if (stage !== 'snapshot' && stage !== 'runtime') {
      throw new Error('stage must be snapshot or runtime')
    }
    return {
      status: 'error',
      stage,
      message: readString(value, 'message', 'envelope'),
      span: readSpan(value.span),
    }
  }

  throw new Error('Snapshot run response status must be ok or error')
}

/** Hexdump-style offset: `0x2a` → `002a`. */
export function formatByteOffset(offset: number): string {
  return offset.toString(16).padStart(4, '0')
}

/** The bytes a region covers, as space-separated hex pairs: `4d 42 43 00`. */
export function regionHex(bytes: Uint8Array, region: SnapshotRegion): string {
  const parts: string[] = []
  for (let index = 0; index < region.length; index += 1) {
    parts.push(bytes[region.offset + index].toString(16).padStart(2, '0'))
  }
  return parts.join(' ')
}

export interface SnapshotSectionGroup {
  section: SnapshotSection
  regions: SnapshotRegion[]
}

/**
 * Group consecutive same-section regions for display. Sections appear in
 * file order (header, main, constants, debug), so this yields one group per
 * section that is present.
 */
export function groupRegionsBySection(
  regions: readonly SnapshotRegion[]
): SnapshotSectionGroup[] {
  const groups: SnapshotSectionGroup[] = []
  for (const region of regions) {
    const last = groups[groups.length - 1]
    if (last && last.section === region.section) {
      last.regions.push(region)
    } else {
      groups.push({ section: region.section, regions: [region] })
    }
  }
  return groups
}
