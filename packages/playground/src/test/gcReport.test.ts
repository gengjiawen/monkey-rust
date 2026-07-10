import { describe, expect, it } from 'vitest'

import { parseGcRunEnvelope } from '../gcReport'

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

  it('parses typed scan object labels', () => {
    const emptyCounts = {
      class: 0,
      instance: 0,
      boundMethod: 0,
      closure: 0,
      array: 0,
      hash: 0,
      other: 0,
    }
    const envelope = parseGcRunEnvelope(
      JSON.stringify({
        status: 'ok',
        result: 'null',
        report: {
          before: {
            objectCount: 2,
            trackedBytes: 80,
            byValueKind: emptyCounts,
          },
          after: {
            objectCount: 1,
            trackedBytes: 40,
            byValueKind: emptyCounts,
          },
          phases: {
            trialDeletion: { edgesVisited: 2, candidates: 2 },
            scan: {
              restored: 1,
              garbageCandidates: 1,
              restoredObjects: [
                { id: 3, kind: 'class', label: 'Class(Node)#3' },
              ],
              garbageCandidateObjects: [
                { id: 4, kind: 'instance', label: 'Instance(Node)#4' },
              ],
            },
            freeCycles: { freed: 1 },
          },
          collectedByValueKind: emptyCounts,
        },
      })
    )

    expect(envelope).toMatchObject({
      status: 'ok',
      report: {
        phases: {
          scan: {
            restoredObjects: [{ id: 3, kind: 'class', label: 'Class(Node)#3' }],
            garbageCandidateObjects: [
              { id: 4, kind: 'instance', label: 'Instance(Node)#4' },
            ],
          },
        },
      },
    })
  })
})
