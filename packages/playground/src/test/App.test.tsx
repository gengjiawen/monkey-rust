import { Theme } from '@radix-ui/themes'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  forwardRef,
  useImperativeHandle,
  type ChangeEvent,
  type Ref,
} from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { GcRunEnvelope, ValueKindCounts } from '../gcReport'

const { runGcMock, parseMock, compileMock } = vi.hoisted(() => ({
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
      highlightRange() {},
      clearHighlight() {},
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
  class: 1,
  instance: 0,
  boundMethod: 0,
  closure: 2,
  array: 0,
  hash: 0,
  other: 4,
  ...overrides,
})

function successEnvelope({
  before = 2,
  after = 0,
  collected = 2,
  result = 'null',
}: {
  before?: number
  after?: number
  collected?: number
  result?: string
} = {}): GcRunEnvelope {
  return {
    status: 'ok',
    result,
    report: {
      before: {
        objectCount: 20,
        trackedBytes: 800,
        byValueKind: counts({ instance: before }),
      },
      after: {
        objectCount: 18,
        trackedBytes: 720,
        byValueKind: counts({ instance: after }),
      },
      phases: {
        trialDeletion: { edgesVisited: 11, candidates: 5 },
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
        },
        freeCycles: { freed: collected },
      },
      collectedByValueKind: counts({
        class: 0,
        closure: 0,
        other: 0,
        instance: collected,
      }),
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
      await screen.findByLabelText('Instance count before and after collection')
    ).toHaveTextContent('2 → 0')
    expect(screen.getByLabelText('Collected instance count')).toHaveTextContent(
      '2'
    )
    expect(screen.getByText('Trial deletion')).toBeInTheDocument()
    expect(screen.getByText('Scan')).toBeInTheDocument()
    expect(screen.getByText('Free cycles')).toBeInTheDocument()
    expect(screen.getByText('Class(Node)#7')).toBeInTheDocument()
    expect(screen.getByText('Closure(makeCycle)#10')).toBeInTheDocument()
    expect(screen.getByText('Instance(Node)#12')).toBeInTheDocument()
    expect(screen.getByText('Instance(Node)#13')).toBeInTheDocument()
    expect(screen.getByText('BoundMethod(Node.connect)#14')).toBeInTheDocument()
    expect(runGcMock).toHaveBeenCalledTimes(1)
  })

  it('shows the stage, message, and source span for GC errors', async () => {
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
    expect(alert).toHaveTextContent('Source span: 120–126')
  })

  it('ignores a stale run after the source changes and a newer run finishes', async () => {
    const user = userEvent.setup()
    const firstRun = deferred<GcRunEnvelope>()
    runGcMock
      .mockReturnValueOnce(firstRun.promise)
      .mockResolvedValueOnce(
        successEnvelope({ before: 5, after: 1, collected: 4, result: 'new' })
      )
    renderApp()

    await user.click(await openGcTab(user))
    expect(runGcMock).toHaveBeenCalledTimes(1)

    await user.clear(screen.getByLabelText('Source editor'))
    await user.type(screen.getByLabelText('Source editor'), '1;')
    await user.click(screen.getByRole('button', { name: 'Run GC' }))

    expect(
      await screen.findByLabelText('Instance count before and after collection')
    ).toHaveTextContent('5 → 1')

    await act(async () => {
      firstRun.resolve(successEnvelope())
      await firstRun.promise
    })

    await waitFor(() => {
      expect(
        screen.getByLabelText('Instance count before and after collection')
      ).toHaveTextContent('5 → 1')
    })
    expect(screen.getByText('new')).toBeInTheDocument()
    expect(runGcMock).toHaveBeenCalledTimes(2)
  })
})
