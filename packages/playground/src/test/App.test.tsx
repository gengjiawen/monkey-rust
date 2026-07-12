import { Theme } from '@radix-ui/themes'
import {
  act,
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  forwardRef,
  useImperativeHandle,
  type ChangeEvent,
  type Ref,
} from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { GcRunEnvelope, ValueKindCounts } from '../gcReport'

const {
  runGcMock,
  parseMock,
  compileMock,
  highlightRangeMock,
  clearHighlightMock,
} = vi.hoisted(() => ({
  runGcMock: vi.fn(),
  parseMock: vi.fn(() => '{"Program":{"type":"Program","body":[]}}'),
  compileMock: vi.fn(() =>
    JSON.stringify({
      detail: '',
      mainDebugInfo: { pcSpans: [] },
      functionDebugInfo: {},
      instructionLines: [],
    })
  ),
  highlightRangeMock: vi.fn(),
  clearHighlightMock: vi.fn(),
}))

vi.mock('@gengjiawen/monkey-wasm', () => ({
  parse: parseMock,
  compile_with_debug: compileMock,
}))

vi.mock('../gcRunner', () => ({
  runGc: runGcMock,
}))

interface MockEditorProps {
  code?: string
  onChange?: (value: string) => void
  extra?: { readOnly?: boolean }
}

vi.mock('../Editor', () => ({
  Editor: forwardRef(function MockEditor(
    { code = '', onChange, extra }: MockEditorProps,
    ref: Ref<{ highlightRange(): void; clearHighlight(): void }>
  ) {
    useImperativeHandle(ref, () => ({
      highlightRange: highlightRangeMock,
      clearHighlight: clearHighlightMock,
    }))

    const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
      onChange?.(event.target.value)
    }

    return (
      <textarea
        aria-label={extra?.readOnly ? 'Output editor' : 'Source editor'}
        value={code}
        readOnly={extra?.readOnly}
        onChange={handleChange}
      />
    )
  }),
}))

import App from '../App'

const counts = (overrides: Partial<ValueKindCounts> = {}): ValueKindCounts => ({
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
  ...overrides,
})

function successEnvelope({
  before = 20,
  after = 18,
  result = 'null',
}: {
  before?: number
  after?: number
  result?: string
} = {}): GcRunEnvelope {
  const collected = before - after
  return {
    status: 'ok',
    result,
    report: {
      before: {
        objectCount: before,
        trackedBytes: 800,
        byValueKind: counts({
          class: 1,
          instance: 2,
          closure: 3,
          string: 9,
          null: 1,
          compiledFunction: 4,
        }),
      },
      after: {
        objectCount: after,
        trackedBytes: 720,
        byValueKind: counts({
          class: 1,
          closure: 3,
          string: 9,
          null: 1,
          compiledFunction: 4,
        }),
      },
      objects: [
        { id: 1, kind: 'array', label: 'Array#1' },
        { id: 7, kind: 'class', label: 'Class(Node)#7' },
        { id: 10, kind: 'closure', label: 'Closure(makeCycle)#10' },
        { id: 12, kind: 'instance', label: 'Instance(Node)#12' },
        { id: 13, kind: 'instance', label: 'Instance(Node)#13' },
        {
          id: 14,
          kind: 'boundMethod',
          label: 'BoundMethod(Node.connect)#14',
        },
      ],
      globalRoots: [{ name: 'holder', objectId: 1 }],
      phases: {
        trialDeletion: {
          edgesVisited: 11,
          candidates: 5,
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
              objectId: 7,
              refCountBefore: 1,
              heapIncomingEdges: 1,
              trialRefCount: 0,
              decision: 'candidate',
              final: 'retained',
            },
            {
              objectId: 10,
              refCountBefore: 1,
              heapIncomingEdges: 1,
              trialRefCount: 0,
              decision: 'candidate',
              final: 'retained',
            },
            {
              objectId: 12,
              refCountBefore: 1,
              heapIncomingEdges: 1,
              trialRefCount: 0,
              decision: 'candidate',
              final: 'retained',
            },
            {
              objectId: 13,
              refCountBefore: 1,
              heapIncomingEdges: 1,
              trialRefCount: 0,
              decision: 'candidate',
              final: 'freed',
            },
            {
              objectId: 14,
              refCountBefore: 1,
              heapIncomingEdges: 1,
              trialRefCount: 0,
              decision: 'candidate',
              final: 'freed',
            },
          ],
          visitedEdges: [
            {
              fromId: 12,
              toId: 13,
              relation: { kind: 'instanceField', name: 'next' },
            },
            {
              fromId: 13,
              toId: 12,
              relation: { kind: 'instanceField', name: 'next' },
            },
          ],
          omittedObjectDecisions: 0,
          omittedEdgeDetails: 9,
        },
        scan: {
          restored: 3,
          garbageCandidates: 2,
          restoredObjects: [
            { id: 7, kind: 'class', label: 'Class(Node)#7' },
            { id: 10, kind: 'closure', label: 'Closure(makeCycle)#10' },
            { id: 12, kind: 'instance', label: 'Instance(Node)#12' },
          ],
          garbageCandidateObjects: [
            { id: 13, kind: 'instance', label: 'Instance(Node)#13' },
            {
              id: 14,
              kind: 'boundMethod',
              label: 'BoundMethod(Node.connect)#14',
            },
          ],
          restorationWitnesses: [
            {
              objectId: 7,
              rootId: 1,
              predecessorId: 1,
              relation: { kind: 'arrayElement', index: 0 },
            },
            {
              objectId: 10,
              rootId: 1,
              predecessorId: 1,
              relation: { kind: 'arrayElement', index: 1 },
            },
            {
              objectId: 12,
              rootId: 1,
              predecessorId: 1,
              relation: { kind: 'arrayElement', index: 2 },
            },
          ],
          omittedWitnesses: 0,
        },
        freeCycles: { freed: collected },
      },
      collectedByValueKind: counts({ instance: collected }),
    },
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

function renderApp() {
  return render(
    <Theme>
      <App />
    </Theme>
  )
}

async function openGcTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'GC' }))
  return screen.getByRole('button', { name: 'Run GC' })
}

describe('GC playground', () => {
  afterEach(cleanup)

  beforeEach(() => {
    localStorage.clear()
    runGcMock.mockReset()
    parseMock.mockClear()
    compileMock.mockClear()
    highlightRangeMock.mockClear()
    clearHighlightMock.mockClear()
  })

  it('runs only on demand and renders the collection report', async () => {
    const user = userEvent.setup()
    runGcMock.mockResolvedValue(successEnvelope())
    renderApp()

    const runButton = await openGcTab(user)
    expect(runGcMock).not.toHaveBeenCalled()
    expect(
      screen.getByText('Editing never executes the program automatically.')
    ).toBeInTheDocument()

    await user.click(runButton)

    expect(
      await screen.findByLabelText(
        'Heap object count before and after collection'
      )
    ).toHaveTextContent('20 → 18')
    expect(screen.getByLabelText('Collected object count')).toHaveTextContent(
      '2'
    )
    const beforeSnapshot = screen.getByLabelText('Before heap snapshot')
    expect(
      within(beforeSnapshot).getByRole('row', { name: 'String 9' })
    ).toBeInTheDocument()
    expect(
      within(beforeSnapshot).getByRole('row', {
        name: 'Compiled function 4',
      })
    ).toBeInTheDocument()
    expect(
      within(beforeSnapshot).queryByRole('row', {
        name: /Other runtime object/,
      })
    ).not.toBeInTheDocument()
    expect(
      screen.getByText(/Heap snapshots include source values/)
    ).toBeInTheDocument()
    expect(screen.getByText('Trial deletion')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Scan' })).toBeInTheDocument()
    expect(screen.getByText('Free cycles')).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Heap topology' })
    ).toBeInTheDocument()
    expect(
      screen.getByText(/truncated edge or decision details/)
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Object decision walkthrough' })
    ).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /Candidates 5/ })).toBeChecked()
    expect(
      screen.getByRole('button', { name: /Expand details for Class\(Node\)#7/ })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: /Expand details for Closure\(makeCycle\)#10/,
      })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: /Expand details for Instance\(Node\)#12/,
      })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: /Expand details for Instance\(Node\)#13/,
      })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: /Expand details for BoundMethod\(Node.connect\)#14/,
      })
    ).toBeInTheDocument()
    expect(screen.getAllByText('Garbage').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Freed').length).toBeGreaterThan(0)

    await user.click(screen.getByRole('radio', { name: /Trial survivors/ }))
    expect(screen.getByText('holder')).toBeInTheDocument()
    await user.click(
      screen.getByRole('button', { name: /Expand details for Array#1/ })
    )
    expect(
      screen.getByText(/currently references this object/)
    ).toBeInTheDocument()

    expect(runGcMock).toHaveBeenCalledTimes(1)
  })

  it('uses complete Scan results when decisions are truncated', async () => {
    const user = userEvent.setup()
    const envelope = successEnvelope()
    if (envelope.status !== 'ok') {
      throw new Error('expected a successful test envelope')
    }
    const trial = envelope.report.phases.trialDeletion
    trial.objectDecisions = trial.objectDecisions.filter(
      (decision) => decision.objectId !== 14
    )
    trial.omittedObjectDecisions = 1
    trial.edgesVisited = 12
    trial.visitedEdges.push({
      fromId: 14,
      toId: 1,
      relation: { kind: 'boundMethodReceiver' },
    })
    runGcMock.mockResolvedValue(envelope)
    renderApp()

    await user.click(await openGcTab(user))

    expect(
      await screen.findByRole('radio', {
        name: 'Candidates 4 of 5 reported',
      })
    ).toBeChecked()
    expect(
      screen.getByText('Showing 3 candidate-related edges of 12 visited')
    ).toBeInTheDocument()
  })

  it('highlights the source span for GC errors', async () => {
    const user = userEvent.setup()
    runGcMock.mockResolvedValue({
      status: 'error',
      stage: 'runtime',
      message: "property 'next' does not exist on Node",
      span: { start: 120, end: 126 },
    } satisfies GcRunEnvelope)
    renderApp()

    await user.click(await openGcTab(user))

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('runtime error')
    expect(alert).toHaveTextContent("property 'next' does not exist on Node")
    expect(highlightRangeMock).toHaveBeenCalledWith(120, 126)

    highlightRangeMock.mockClear()
    await user.click(
      screen.getByRole('button', { name: 'Show in editor (120–126)' })
    )
    expect(highlightRangeMock).toHaveBeenCalledWith(120, 126)

    clearHighlightMock.mockClear()
    await user.type(screen.getByLabelText('Source editor'), 'x')
    expect(clearHighlightMock).toHaveBeenCalled()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('ignores a stale run after the source changes and a newer run finishes', async () => {
    const user = userEvent.setup()
    const firstRun = deferred<GcRunEnvelope>()
    runGcMock
      .mockReturnValueOnce(firstRun.promise)
      .mockResolvedValueOnce(
        successEnvelope({ before: 5, after: 1, result: 'new' })
      )
    renderApp()

    await user.click(await openGcTab(user))
    expect(runGcMock).toHaveBeenCalledTimes(1)

    await user.clear(screen.getByLabelText('Source editor'))
    await user.type(screen.getByLabelText('Source editor'), '1;')
    await user.click(screen.getByRole('button', { name: 'Run GC' }))

    expect(
      await screen.findByLabelText(
        'Heap object count before and after collection'
      )
    ).toHaveTextContent('5 → 1')

    await act(async () => {
      firstRun.resolve(successEnvelope())
      await firstRun.promise
    })

    await waitFor(() => {
      expect(
        screen.getByLabelText('Heap object count before and after collection')
      ).toHaveTextContent('5 → 1')
    })
    expect(screen.getByText('new')).toBeInTheDocument()
    expect(runGcMock).toHaveBeenCalledTimes(2)
  })
})
