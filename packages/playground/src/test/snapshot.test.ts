import { describe, expect, it } from 'vitest'

import {
  formatByteOffset,
  groupRegionsBySection,
  hexToBytes,
  parseSnapshotBuildEnvelope,
  parseSnapshotRunEnvelope,
  regionHex,
  type SnapshotRegion,
} from '../snapshot'

function headerLayout() {
  return {
    byteLength: 10,
    formatVersion: 1,
    abiFingerprint: '0x0000002a',
    hasDebugInfo: false,
    regions: [
      {
        offset: 0,
        length: 4,
        section: 'header',
        label: 'magic',
        detail: 'file signature "MBC\\0"',
      },
      {
        offset: 4,
        length: 1,
        section: 'header',
        label: 'version',
        detail: 'container format version 1',
      },
      {
        offset: 5,
        length: 4,
        section: 'header',
        label: 'abi fingerprint',
        detail: '0x0000002a',
      },
      {
        offset: 9,
        length: 1,
        section: 'main',
        label: 'main length',
        detail: '0 bytes of main instructions follow (ULEB128)',
      },
    ],
  }
}

function buildEnvelope(overrides: Record<string, unknown> = {}) {
  return JSON.stringify({
    status: 'ok',
    bytesHex: '4d424300012a00000000',
    layout: headerLayout(),
    ...overrides,
  })
}

describe('parseSnapshotBuildEnvelope', () => {
  it('parses a success envelope into bytes and a validated layout', () => {
    const envelope = parseSnapshotBuildEnvelope(buildEnvelope())
    if (envelope.status !== 'ok') {
      throw new Error('expected a success envelope')
    }
    expect(envelope.bytes).toBeInstanceOf(Uint8Array)
    expect(Array.from(envelope.bytes.slice(0, 4))).toEqual([
      0x4d, 0x42, 0x43, 0x00,
    ])
    expect(envelope.layout.byteLength).toBe(10)
    expect(envelope.layout.regions).toHaveLength(4)
    expect(envelope.layout.regions[0].label).toBe('magic')
  })

  it('parses error envelopes for every build stage', () => {
    for (const stage of ['parse', 'compile', 'snapshot'] as const) {
      expect(
        parseSnapshotBuildEnvelope(
          JSON.stringify({ status: 'error', stage, message: 'boom' })
        )
      ).toEqual({ status: 'error', stage, message: 'boom' })
    }
    expect(() =>
      parseSnapshotBuildEnvelope(
        JSON.stringify({ status: 'error', stage: 'runtime', message: 'boom' })
      )
    ).toThrow(/stage must be/)
  })

  it('rejects bytes that disagree with the layout length', () => {
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ bytesHex: '4d4243' }))
    ).toThrow(/bytesHex length must match/)
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ bytesHex: '4D424300012A00000000' }))
    ).toThrow(/lowercase hex/)
  })

  it('rejects regions that do not tile the buffer', () => {
    const gap = headerLayout()
    gap.regions[1] = { ...gap.regions[1], offset: 5, length: 1 }
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ layout: gap }))
    ).toThrow(/must start at byte 4/)

    const short = headerLayout()
    short.regions.pop()
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ layout: short }))
    ).toThrow(/tile the buffer/)

    const empty = headerLayout()
    empty.regions[0] = { ...empty.regions[0], length: 0 }
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ layout: empty }))
    ).toThrow(/at least 1/)
  })

  it('rejects unknown sections and debug regions in stripped layouts', () => {
    const unknown = headerLayout()
    unknown.regions[0] = { ...unknown.regions[0], section: 'trailer' }
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ layout: unknown }))
    ).toThrow(/known snapshot section/)

    const strippedWithDebug = headerLayout()
    strippedWithDebug.regions[3] = {
      ...strippedWithDebug.regions[3],
      section: 'debug',
    }
    expect(() =>
      parseSnapshotBuildEnvelope(buildEnvelope({ layout: strippedWithDebug }))
    ).toThrow(/must not contain debug regions/)
  })
})

describe('parseSnapshotRunEnvelope', () => {
  it('parses success, runtime error with span, and snapshot rejection', () => {
    expect(
      parseSnapshotRunEnvelope(JSON.stringify({ status: 'ok', result: '3' }))
    ).toEqual({ status: 'ok', result: '3' })

    expect(
      parseSnapshotRunEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'runtime',
          message: 'not a function',
          span: { start: 22, end: 36 },
        })
      )
    ).toEqual({
      status: 'error',
      stage: 'runtime',
      message: 'not a function',
      span: { start: 22, end: 36 },
    })

    expect(
      parseSnapshotRunEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'snapshot',
          message: 'BadMagic',
          span: null,
        })
      )
    ).toEqual({
      status: 'error',
      stage: 'snapshot',
      message: 'BadMagic',
      span: null,
    })
  })

  it('rejects unknown statuses and stages', () => {
    expect(() =>
      parseSnapshotRunEnvelope(JSON.stringify({ status: 'maybe' }))
    ).toThrow(/status must be ok or error/)
    expect(() =>
      parseSnapshotRunEnvelope(
        JSON.stringify({ status: 'error', stage: 'parse', message: 'x' })
      )
    ).toThrow(/stage must be snapshot or runtime/)
  })
})

describe('hexdump helpers', () => {
  it('formats offsets as 4-digit hex', () => {
    expect(formatByteOffset(0)).toBe('0000')
    expect(formatByteOffset(255)).toBe('00ff')
    expect(formatByteOffset(0x1a2b)).toBe('1a2b')
  })

  it('decodes hex and slices region bytes', () => {
    const bytes = hexToBytes('4d424300ff')
    expect(Array.from(bytes)).toEqual([0x4d, 0x42, 0x43, 0x00, 0xff])
    const region: SnapshotRegion = {
      offset: 1,
      length: 3,
      section: 'header',
      label: 'x',
      detail: 'y',
    }
    expect(regionHex(bytes, region)).toBe('42 43 00')
    expect(() => hexToBytes('4d4')).toThrow(/hex byte pairs/)
  })

  it('groups consecutive regions by section', () => {
    const envelope = parseSnapshotBuildEnvelope(buildEnvelope())
    if (envelope.status !== 'ok') {
      throw new Error('expected a success envelope')
    }
    const groups = groupRegionsBySection(envelope.layout.regions)
    expect(groups.map((group) => group.section)).toEqual(['header', 'main'])
    expect(groups[0].regions).toHaveLength(3)
    expect(groups[1].regions).toHaveLength(1)
  })
})
