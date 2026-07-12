import { describe, expect, it } from 'vitest'

import {
  formatEdgeRelation,
  parseGcRunEnvelope,
  rebuildWitnessPath,
  scanResultLabel,
} from '../gcReport'

const emptyCounts = {
  class: 0,
  instance: 0,
  boundMethod: 0,
  closure: 0,
  array: 0,
  hash: 0,
  integer: 0,
  boolean: 0,
  string: 0,
  null: 0,
  error: 0,
  compiledFunction: 0,
  builtin: 0,
  other: 0,
}

function fullReport(overrides: Record<string, unknown> = {}) {
  const base = {
    before: {
      objectCount: 4,
      trackedBytes: 160,
      byValueKind: emptyCounts,
    },
    after: {
      objectCount: 2,
      trackedBytes: 80,
      byValueKind: emptyCounts,
    },
    objects: [
      { id: 1, kind: 'array', label: 'Array#1' },
      { id: 3, kind: 'class', label: 'Class(Node)#3' },
      { id: 4, kind: 'instance', label: 'Instance(Node)#4' },
      { id: 5, kind: 'instance', label: 'Instance(Node)#5' },
    ],
    globalRoots: [{ name: 'holder', objectId: 1 }],
    phases: {
      trialDeletion: {
        edgesVisited: 3,
        candidates: 3,
        objectDecisions: [
          {
            objectId: 1,
            refCountBefore: 2,
            heapIncomingEdges: 0,
            trialRefCount: 2,
            decision: 'survivor',
            final: 'retained',
          },
          {
            objectId: 3,
            refCountBefore: 1,
            heapIncomingEdges: 1,
            trialRefCount: 0,
            decision: 'candidate',
            final: 'retained',
          },
          {
            objectId: 4,
            refCountBefore: 1,
            heapIncomingEdges: 1,
            trialRefCount: 0,
            decision: 'candidate',
            final: 'freed',
          },
          {
            objectId: 5,
            refCountBefore: 1,
            heapIncomingEdges: 1,
            trialRefCount: 0,
            decision: 'candidate',
            final: 'freed',
          },
        ],
        visitedEdges: [
          {
            fromId: 1,
            toId: 3,
            relation: { kind: 'arrayElement', index: 0 },
          },
          {
            fromId: 4,
            toId: 5,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 5,
            toId: 4,
            relation: { kind: 'instanceField', name: 'next' },
          },
        ],
        omittedObjectDecisions: 0,
        omittedEdgeDetails: 0,
      },
      scan: {
        restored: 1,
        garbageCandidates: 2,
        restoredObjects: [{ id: 3, kind: 'class', label: 'Class(Node)#3' }],
        garbageCandidateObjects: [
          { id: 4, kind: 'instance', label: 'Instance(Node)#4' },
          { id: 5, kind: 'instance', label: 'Instance(Node)#5' },
        ],
        restorationWitnesses: [
          {
            objectId: 3,
            rootId: 1,
            predecessorId: 1,
            relation: { kind: 'arrayElement', index: 0 },
          },
        ],
        omittedWitnesses: 0,
      },
      freeCycles: { freed: 2 },
    },
    collectedByValueKind: { ...emptyCounts, instance: 2 },
  }

  const phaseOverrides =
    (overrides.phases as Record<string, unknown> | undefined) ?? undefined
  const trialOverrides = phaseOverrides?.trialDeletion as
    | Record<string, unknown>
    | undefined
  const scanOverrides = phaseOverrides?.scan as
    | Record<string, unknown>
    | undefined

  return {
    ...base,
    ...overrides,
    phases: {
      ...base.phases,
      ...phaseOverrides,
      trialDeletion: {
        ...base.phases.trialDeletion,
        ...trialOverrides,
      },
      scan: {
        ...base.phases.scan,
        ...scanOverrides,
      },
    },
  }
}

function okEnvelope(report = fullReport()) {
  return JSON.stringify({
    status: 'ok',
    result: 'null',
    report,
  })
}

function expectReportRejected(
  mutate: (report: ReturnType<typeof fullReport>) => void,
  message: string
) {
  const report = fullReport()
  mutate(report)
  expect(() => parseGcRunEnvelope(okEnvelope(report))).toThrow(message)
}

describe('parseGcRunEnvelope', () => {
  it('rejects an untagged partial report', () => {
    expect(() => parseGcRunEnvelope('{"report":{}}')).toThrow(
      'GC response status must be ok or error'
    )
  })

  it('accepts a structured stage error', () => {
    expect(
      parseGcRunEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'parse',
          message: 'expected expression',
          span: null,
        })
      )
    ).toEqual({
      status: 'error',
      stage: 'parse',
      message: 'expected expression',
      span: null,
    })
  })

  it('accepts a full report with teaching telemetry', () => {
    const envelope = parseGcRunEnvelope(okEnvelope())
    expect(envelope.status).toBe('ok')
    if (envelope.status !== 'ok') {
      return
    }

    expect(envelope.report.objects).toHaveLength(4)
    expect(envelope.report.phases.trialDeletion.objectDecisions).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          objectId: 1,
          decision: 'survivor',
          final: 'retained',
        }),
        expect.objectContaining({
          objectId: 3,
          decision: 'candidate',
          final: 'retained',
        }),
        expect.objectContaining({
          objectId: 4,
          decision: 'candidate',
          final: 'freed',
        }),
      ])
    )
    expect(envelope.report.phases.trialDeletion.visitedEdges[0]).toEqual({
      fromId: 1,
      toId: 3,
      relation: { kind: 'arrayElement', index: 0 },
    })
    expect(envelope.report.phases.scan.restorationWitnesses).toEqual([
      {
        objectId: 3,
        rootId: 1,
        predecessorId: 1,
        relation: { kind: 'arrayElement', index: 0 },
      },
    ])
    expect(envelope.report.phases.scan.restoredObjects).toEqual([
      { id: 3, kind: 'class', label: 'Class(Node)#3' },
    ])
    expect(envelope.report.globalRoots).toEqual([
      { name: 'holder', objectId: 1 },
    ])
  })

  it('validates global roots against the catalog and name uniqueness', () => {
    expectReportRejected((report) => {
      report.globalRoots.push({ name: 'ghost', objectId: 99 })
    }, 'references unknown object 99')
    expectReportRejected((report) => {
      report.globalRoots.push({ name: 'holder', objectId: 3 })
    }, 'must not contain duplicate names')
    expectReportRejected((report) => {
      ;(report as Record<string, unknown>).globalRoots = undefined
    }, 'report.globalRoots must be an array')
    expectReportRejected((report) => {
      report.globalRoots.push({ name: 'leak', objectId: 4 })
    }, 'names candidate object 4')
  })

  it('accepts distinct scalar and VM support value kinds', () => {
    const report = fullReport()
    report.objects[0] = {
      id: 1,
      kind: 'compiledFunction',
      label: 'CompiledFunction#1',
    }
    report.before.byValueKind.string = 2
    report.before.byValueKind.null = 1
    report.before.byValueKind.compiledFunction = 1

    const envelope = parseGcRunEnvelope(okEnvelope(report))
    expect(envelope.status).toBe('ok')
    if (envelope.status === 'ok') {
      expect(envelope.report.objects[0].kind).toBe('compiledFunction')
      expect(envelope.report.before.byValueKind).toMatchObject({
        string: 2,
        null: 1,
        compiledFunction: 1,
        other: 0,
      })
    }
  })

  it('rejects unknown relation kinds', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                visitedEdges: [
                  {
                    fromId: 1,
                    toId: 3,
                    relation: { kind: 'notARelation' },
                  },
                ],
                omittedEdgeDetails: 2,
              },
            },
          })
        )
      )
    ).toThrow('known edge relation kind')
  })

  it('rejects relations missing required fields', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                visitedEdges: [
                  {
                    fromId: 4,
                    toId: 5,
                    relation: { kind: 'instanceField' },
                  },
                ],
                omittedEdgeDetails: 2,
              },
            },
          })
        )
      )
    ).toThrow('name must be a string')
  })

  it('parses typed Hash key relations', () => {
    const envelope = parseGcRunEnvelope(
      okEnvelope(
        fullReport({
          phases: {
            trialDeletion: {
              visitedEdges: [
                {
                  fromId: 1,
                  toId: 3,
                  relation: {
                    kind: 'hashValue',
                    keyKind: 'integer',
                    key: '1',
                  },
                },
              ],
              omittedEdgeDetails: 2,
            },
          },
        })
      )
    )
    expect(envelope.status).toBe('ok')
    if (envelope.status === 'ok') {
      expect(
        envelope.report.phases.trialDeletion.visitedEdges[0].relation
      ).toEqual({ kind: 'hashValue', keyKind: 'integer', key: '1' })
    }
  })

  it('rejects Hash relations without a typed key kind', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                visitedEdges: [
                  {
                    fromId: 1,
                    toId: 3,
                    relation: { kind: 'hashValue', key: '1' },
                  },
                ],
                omittedEdgeDetails: 2,
              },
            },
          })
        )
      )
    ).toThrow('keyKind must be integer, boolean, or string')
  })

  it('rejects dangling object IDs', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                visitedEdges: [
                  {
                    fromId: 1,
                    toId: 99,
                    relation: { kind: 'arrayElement', index: 0 },
                  },
                ],
                omittedEdgeDetails: 2,
              },
            },
          })
        )
      )
    ).toThrow('unknown object 99')
  })

  it('rejects illegal final values that contradict garbage candidates', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                objectDecisions: [
                  {
                    objectId: 1,
                    refCountBefore: 2,
                    heapIncomingEdges: 0,
                    trialRefCount: 2,
                    decision: 'survivor',
                    final: 'retained',
                  },
                  {
                    objectId: 3,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'retained',
                  },
                  {
                    objectId: 4,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'retained',
                  },
                  {
                    objectId: 5,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'freed',
                  },
                ],
              },
            },
          })
        )
      )
    ).toThrow('final must be freed iff decision is candidate')
  })

  it('enforces aggregate candidate and Scan summary counts', () => {
    expectReportRejected((report) => {
      report.phases.scan.restored = 2
    }, 'restoredObjects.length must equal restored')
    expectReportRejected((report) => {
      report.phases.scan.garbageCandidates = 3
    }, 'garbageCandidateObjects.length must equal garbageCandidates')
    expectReportRejected((report) => {
      report.phases.trialDeletion.candidates = 4
    }, 'candidates must equal scan.restored + scan.garbageCandidates')
  })

  it('requires unique, disjoint Scan candidate sets', () => {
    expectReportRejected((report) => {
      report.phases.scan.restoredObjects.push({
        ...report.phases.scan.restoredObjects[0],
      })
      report.phases.scan.restored = 2
      report.phases.trialDeletion.candidates = 4
    }, 'restoredObjects must not contain duplicate id values')
    expectReportRejected((report) => {
      report.phases.scan.garbageCandidateObjects.push({
        ...report.phases.scan.garbageCandidateObjects[0],
      })
      report.phases.scan.garbageCandidates = 3
      report.phases.trialDeletion.candidates = 4
    }, 'garbageCandidateObjects must not contain duplicate id values')
    expectReportRejected((report) => {
      report.phases.scan.restoredObjects[0] = {
        ...report.phases.scan.garbageCandidateObjects[0],
      }
    }, 'restored and garbage candidate objects must be disjoint')
  })

  it('validates decision identity, RC arithmetic, and candidate membership', () => {
    expectReportRejected((report) => {
      report.phases.trialDeletion.objectDecisions.push({
        ...report.phases.trialDeletion.objectDecisions[1],
      })
    }, 'objectDecisions must not contain duplicate objectId values')
    expectReportRejected((report) => {
      report.phases.trialDeletion.objectDecisions[1].trialRefCount = 1
    }, 'trialRefCount must equal refCountBefore - heapIncomingEdges')
    expectReportRejected((report) => {
      const decision = report.phases.trialDeletion.objectDecisions[1]
      decision.refCountBefore = 2
      decision.trialRefCount = 1
    }, 'decision must be candidate iff trialRefCount is zero')
    expectReportRejected((report) => {
      const decision = report.phases.trialDeletion.objectDecisions[1]
      decision.refCountBefore = 2
      decision.trialRefCount = 1
      decision.decision = 'survivor'
    }, 'decision must be candidate iff the object appears in Scan candidate results')
  })

  it('requires unique witnesses for restored objects', () => {
    expectReportRejected((report) => {
      report.phases.scan.restorationWitnesses.push({
        ...report.phases.scan.restorationWitnesses[0],
      })
    }, 'restorationWitnesses must not contain duplicate objectId values')
    expectReportRejected((report) => {
      report.phases.scan.restorationWitnesses[0].objectId = 4
    }, 'objectId must reference a restored object')
  })

  it('rejects witness chains that do not end at a survivor', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              scan: {
                restorationWitnesses: [
                  {
                    objectId: 3,
                    rootId: 4,
                    predecessorId: 4,
                    relation: { kind: 'instanceField', name: 'next' },
                  },
                ],
              },
            },
          })
        )
      )
    ).toThrow('must end at a survivor decision')
  })

  it('rejects cyclic witness chains', () => {
    expect(() =>
      parseGcRunEnvelope(
        okEnvelope(
          fullReport({
            phases: {
              trialDeletion: {
                objectDecisions: [
                  {
                    objectId: 1,
                    refCountBefore: 2,
                    heapIncomingEdges: 0,
                    trialRefCount: 2,
                    decision: 'survivor',
                    final: 'retained',
                  },
                  {
                    objectId: 3,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'retained',
                  },
                  {
                    objectId: 4,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'retained',
                  },
                  {
                    objectId: 5,
                    refCountBefore: 1,
                    heapIncomingEdges: 1,
                    trialRefCount: 0,
                    decision: 'candidate',
                    final: 'freed',
                  },
                ],
              },
              scan: {
                restored: 2,
                garbageCandidates: 1,
                restoredObjects: [
                  { id: 3, kind: 'class', label: 'Class(Node)#3' },
                  { id: 4, kind: 'instance', label: 'Instance(Node)#4' },
                ],
                garbageCandidateObjects: [
                  { id: 5, kind: 'instance', label: 'Instance(Node)#5' },
                ],
                restorationWitnesses: [
                  {
                    objectId: 3,
                    rootId: 1,
                    predecessorId: 4,
                    relation: { kind: 'arrayElement', index: 0 },
                  },
                  {
                    objectId: 4,
                    rootId: 1,
                    predecessorId: 3,
                    relation: { kind: 'instanceField', name: 'next' },
                  },
                ],
                omittedWitnesses: 0,
              },
            },
          })
        )
      )
    ).toThrow('contains a cycle')
  })
})

describe('teaching helpers', () => {
  it('formats typed edge relations', () => {
    expect(formatEdgeRelation({ kind: 'arrayElement', index: 0 })).toBe(
      'items[0]'
    )
    expect(
      formatEdgeRelation({ kind: 'hashValue', keyKind: 'string', key: '1' })
    ).toBe('values["1"]')
    expect(
      formatEdgeRelation({ kind: 'hashValue', keyKind: 'integer', key: '1' })
    ).toBe('values[1]')
    expect(
      formatEdgeRelation({ kind: 'hashValue', keyKind: 'boolean', key: 'true' })
    ).toBe('values[true]')
    expect(formatEdgeRelation({ kind: 'instanceField', name: 'next' })).toBe(
      'fields["next"]'
    )
    expect(formatEdgeRelation({ kind: 'unknown' })).toBe('unknown')
  })

  it('rebuilds witness paths from the forest', () => {
    expect(
      rebuildWitnessPath(
        [
          {
            objectId: 11,
            rootId: 3,
            predecessorId: 8,
            relation: { kind: 'arrayElement', index: 0 },
          },
          {
            objectId: 8,
            rootId: 3,
            predecessorId: 3,
            relation: { kind: 'arrayElement', index: 0 },
          },
        ],
        11
      )
    ).toEqual([
      {
        fromId: 3,
        toId: 8,
        relation: { kind: 'arrayElement', index: 0 },
      },
      {
        fromId: 8,
        toId: 11,
        relation: { kind: 'arrayElement', index: 0 },
      },
    ])
  })

  it('rejects inconsistent Scan labels instead of guessing Garbage', () => {
    const candidate = {
      objectId: 3,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate' as const,
      final: 'retained' as const,
    }
    expect(() => scanResultLabel(candidate, new Set(), new Set())).toThrow(
      'missing from Scan candidate results'
    )
    expect(() =>
      scanResultLabel(candidate, new Set([3]), new Set([3]))
    ).toThrow('cannot be both restored and garbage')
  })
})
