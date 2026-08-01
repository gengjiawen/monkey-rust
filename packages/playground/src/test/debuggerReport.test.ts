import { describe, expect, it } from 'vitest'

import { parseDebuggerRunEnvelope } from '../debuggerReport'
import {
  emptyHeap,
  errorEnvelope,
  frame,
  hit,
  inlineValue,
  okEnvelope,
  refValue,
  slot,
} from './debuggerFixtures'

function richEnvelope() {
  return okEnvelope(
    [
      hit(1, {
        frames: [
          frame(),
          frame({
            name: 'makePoint',
            currentSpan: { start: 40, end: 49 },
            callee: refValue(3, '[closure function]', 'closure'),
            locals: [
              slot('x', 0, inlineValue('3')),
              slot('y', 1, inlineValue('2')),
              slot('p', 2, refValue(7, '[3, 2]')),
              slot('late', 3, null),
            ],
            captures: [{ name: 'b', index: 0, value: inlineValue('20') }],
            temporaries: [{ slot: 5, value: inlineValue('99') }],
          }),
        ],
        globals: [slot('makePoint', 0, refValue(3, '[closure function]'))],
        heap: emptyHeap({
          objects: [
            {
              id: 3,
              kind: 'closure',
              label: 'Closure(makePoint)',
              members: [],
            },
            {
              id: 7,
              kind: 'array',
              label: 'Array',
              members: [
                {
                  relation: { kind: 'arrayElement', index: 0 },
                  display: '3',
                },
              ],
            },
          ],
          edges: [
            {
              from: 7,
              to: 3,
              relation: { kind: 'arrayElement', index: 1 },
            },
          ],
          omittedObjects: 2,
          omittedEdges: 1,
        }),
      }),
      hit(2),
    ],
    { result: '3', stdout: 'before\n', droppedHits: 4 }
  )
}

describe('parseDebuggerRunEnvelope', () => {
  it('round-trips a full ok envelope', () => {
    const envelope = richEnvelope()

    expect(parseDebuggerRunEnvelope(JSON.stringify(envelope))).toEqual(
      envelope
    )
  })

  it('round-trips an error envelope that still carries hits', () => {
    const envelope = errorEnvelope([hit(1)], { stdout: 'partial\n' })

    expect(parseDebuggerRunEnvelope(JSON.stringify(envelope))).toEqual(
      envelope
    )
  })

  it('rejects malformed JSON with a wrapped message', () => {
    expect(() => parseDebuggerRunEnvelope('{nope')).toThrow(
      /Debugger response is not valid JSON/
    )
  })

  it('rejects unknown statuses', () => {
    const envelope = { ...okEnvelope([]), status: 'maybe' }

    expect(() => parseDebuggerRunEnvelope(JSON.stringify(envelope))).toThrow(
      'Debugger response status must be ok or error'
    )
  })

  it('rejects unknown error stages', () => {
    const envelope = { ...errorEnvelope([]), stage: 'link' }

    expect(() => parseDebuggerRunEnvelope(JSON.stringify(envelope))).toThrow(
      'stage must be parse, compile, or runtime'
    )
  })

  it('rejects hits that are out of execution order', () => {
    const envelope = okEnvelope([hit(2)])

    expect(() => parseDebuggerRunEnvelope(JSON.stringify(envelope))).toThrow(
      'hits[0].index must be 1'
    )
  })

  it('rejects an initialized slot without a value', () => {
    const broken = hit(1, {
      globals: [{ name: 'a', slot: 0, initialized: true, value: null }],
    })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow(
      'hits[0].globals[0].value must be null exactly when the slot is uninitialized'
    )
  })

  it('rejects an uninitialized slot that carries a value', () => {
    const broken = hit(1, {
      globals: [
        { name: 'a', slot: 0, initialized: false, value: inlineValue('1') },
      ],
    })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow(
      'hits[0].globals[0].value must be null exactly when the slot is uninitialized'
    )
  })

  it('rejects hits without a main frame', () => {
    const broken = { ...hit(1), frames: [] }

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow('hits[0].frames must contain at least the main frame')
  })

  it('rejects duplicate heap object ids', () => {
    const broken = hit(1, {
      heap: emptyHeap({
        objects: [
          { id: 4, kind: 'array', label: 'Array', members: [] },
          { id: 4, kind: 'hash', label: 'Hash', members: [] },
        ],
      }),
    })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow('hits[0].heap.objects must not contain duplicate id values')
  })

  it('rejects edges pointing at unrecorded objects', () => {
    const broken = hit(1, {
      heap: emptyHeap({
        objects: [{ id: 4, kind: 'array', label: 'Array', members: [] }],
        edges: [
          { from: 4, to: 9, relation: { kind: 'arrayElement', index: 0 } },
        ],
      }),
    })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow('hits[0].heap.edges[0].to references unknown object 9')
  })

  it('rejects negative omitted counts', () => {
    const broken = hit(1, { heap: emptyHeap({ omittedObjects: -1 }) })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow('hits[0].heap.omittedObjects must be a non-negative safe integer')
  })

  it('rejects unknown value kinds', () => {
    const envelope = okEnvelope([
      hit(1, { globals: [slot('a', 0, inlineValue('1'))] }),
    ])
    const raw = JSON.parse(JSON.stringify(envelope)) as {
      hits: { globals: { value: { kind: string } }[] }[]
    }
    raw.hits[0].globals[0].value.kind = 'mystery'

    expect(() => parseDebuggerRunEnvelope(JSON.stringify(raw))).toThrow(
      'hits[0].globals[0].value.kind must be a known value kind'
    )
  })

  it('rejects a non-null non-integer heapId', () => {
    const broken = hit(1, {
      globals: [
        {
          name: 'a',
          slot: 0,
          initialized: true,
          value: { kind: 'array', display: '[]', heapId: 1.5 },
        },
      ],
    })

    expect(() =>
      parseDebuggerRunEnvelope(JSON.stringify(okEnvelope([broken])))
    ).toThrow(
      'hits[0].globals[0].value.heapId must be null or a non-negative safe integer'
    )
  })
})
