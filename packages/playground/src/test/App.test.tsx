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
import { Provider } from 'jotai'
import {
  forwardRef,
  useImperativeHandle,
  type ChangeEvent,
  type Ref,
} from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Arm64BuildEnvelope } from '../arm64'
import type { GcRunEnvelope, ValueKindCounts } from '../gcReport'
import type { SnapshotBuildSuccess, SnapshotRunEnvelope } from '../snapshot'

const {
  runGcMock,
  parseMock,
  compileMock,
  buildSnapshotMock,
  runSnapshotMock,
  buildArm64Mock,
  highlightRangeMock,
  highlightRangesMock,
  clearHighlightMock,
  runLintMock,
  formatMock,
  minifyMock,
  sourceEditorHooks,
  outputEditorHooks,
} = vi.hoisted(() => ({
  runGcMock: vi.fn(),
  parseMock: vi.fn(),
  compileMock: vi.fn(() =>
    JSON.stringify({
      detail: '',
      mainDebugInfo: { pcSpans: [] },
      functionDebugInfo: {},
      instructionLines: [],
    })
  ),
  buildSnapshotMock: vi.fn(),
  runSnapshotMock: vi.fn(),
  buildArm64Mock: vi.fn(),
  highlightRangeMock: vi.fn(),
  highlightRangesMock: vi.fn(),
  clearHighlightMock: vi.fn(),
  runLintMock: vi.fn(async () => {}),
  formatMock: vi.fn(),
  minifyMock: vi.fn(),
  // The mock editor below is a plain <textarea>, which cannot reproduce every
  // CodeMirror callback: change events whose text equals the current document,
  // or cursor movements. Tests drive those callbacks through these hooks.
  sourceEditorHooks: {} as {
    onChange?: (value: string) => void
    onSelectionChange?: (selection: { from: number; to: number }) => void
  },
  // Same for the read-only output pane (bytecode / arm64 assembly).
  outputEditorHooks: {} as {
    onSelectionChange?: (selection: { from: number; to: number }) => void
  },
}))

const defaultAstJson = '{"Program":{"type":"Program","body":[]}}'

vi.mock('@gengjiawen/monkey-wasm', () => ({
  parse: parseMock,
  compile_with_debug: compileMock,
}))

vi.mock('../gcRunner', () => ({
  runGc: runGcMock,
}))

vi.mock('../snapshotRunner', () => ({
  buildSnapshot: buildSnapshotMock,
  runSnapshot: runSnapshotMock,
}))

vi.mock('../arm64Runner', () => ({
  buildArm64: buildArm64Mock,
}))

vi.mock('prettier/standalone', () => ({
  format: formatMock,
}))

vi.mock('../../../prettier-plugin-monkey/src/index', () => ({
  default: {},
}))

vi.mock('../../../monkey-minifier/src/index', () => ({
  minify: minifyMock,
}))

interface MockEditorProps {
  code?: string
  onChange?: (value: string) => void
  onSelectionChange?: (selection: { from: number; to: number }) => void
  extra?: { readOnly?: boolean }
}

vi.mock('../Editor', () => ({
  Editor: forwardRef(function MockEditor(
    { code = '', onChange, onSelectionChange, extra }: MockEditorProps,
    ref: Ref<{
      highlightRange(): void
      highlightRanges(): void
      clearHighlight(): void
      runLint(): Promise<void>
    }>
  ) {
    useImperativeHandle(ref, () => ({
      highlightRange: highlightRangeMock,
      highlightRanges: highlightRangesMock,
      clearHighlight: clearHighlightMock,
      runLint: runLintMock,
    }))

    if (!extra?.readOnly) {
      sourceEditorHooks.onChange = onChange
      sourceEditorHooks.onSelectionChange = onSelectionChange
    } else {
      outputEditorHooks.onSelectionChange = onSelectionChange
    }

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
      omittedGlobalRoots: 0,
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

function snapshotEnvelope(): SnapshotBuildSuccess {
  return {
    status: 'ok',
    bytes: new Uint8Array([0x4d, 0x42, 0x43, 0x00]),
    layout: {
      byteLength: 4,
      formatVersion: 1,
      abiFingerprint: '0x0000002a',
      hasDebugInfo: true,
      regions: [
        {
          offset: 0,
          length: 4,
          section: 'header',
          label: 'magic',
          detail: 'file signature "MBC\\0"',
        },
      ],
    },
  }
}

// Assembly text offsets: 'main:' = {0,5}, ' mov x0, #7' line = {6,18},
// ' ret' line = {19,24}; the two code lines share source span {0,1}.
function arm64Envelope(): Arm64BuildEnvelope {
  const lines = [
    { text: 'main:', kind: 'label' as const, span: null },
    { text: '  mov x0, #7', kind: 'code' as const, span: { start: 0, end: 1 } },
    { text: '  ret', kind: 'code' as const, span: { start: 0, end: 1 } },
  ]
  return {
    status: 'ok',
    lines,
    text: lines.map((line) => line.text).join('\n'),
  }
}

function renderApp() {
  // A fresh jotai Provider per render keeps atom state (the persisted snippet
  // index) from leaking between tests through the module-level default store.
  return render(
    <Provider>
      <Theme>
        <App />
      </Theme>
    </Provider>
  )
}

async function openGcTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'GC' }))
  return screen.getByRole('button', { name: 'Run GC' })
}

async function openSnapshotTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'Snapshot' }))
  return screen.getByRole('button', { name: 'Run snapshot' })
}

async function openArm64Tab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'ARM64' }))
  // The build lands after the 200ms debounce; the download button only
  // renders on a successful build.
  return screen.findByRole('button', { name: 'Download .s' })
}

afterEach(cleanup)

beforeEach(() => {
  localStorage.clear()
  runGcMock.mockReset()
  parseMock.mockReset()
  parseMock.mockImplementation(() => defaultAstJson)
  compileMock.mockClear()
  buildSnapshotMock.mockReset()
  buildSnapshotMock.mockImplementation(() => snapshotEnvelope())
  runSnapshotMock.mockReset()
  buildArm64Mock.mockReset()
  buildArm64Mock.mockImplementation(() => arm64Envelope())
  formatMock.mockReset()
  formatMock.mockImplementation(async (source: string) => source)
  minifyMock.mockReset()
  minifyMock.mockImplementation(() => ({ code: '1;' }))
  highlightRangeMock.mockClear()
  highlightRangesMock.mockClear()
  clearHighlightMock.mockClear()
  runLintMock.mockClear()
  sourceEditorHooks.onChange = undefined
  sourceEditorHooks.onSelectionChange = undefined
  outputEditorHooks.onSelectionChange = undefined
})

describe('Examples', () => {
  it('starts with the concise Intro program', () => {
    renderApp()

    expect(screen.getByLabelText('Source editor')).toHaveValue(
      'let a = 1 + 1;\nprint(a)\n'
    )
  })

  it('loads the Lint demo snippet from the dropdown', async () => {
    const user = userEvent.setup()
    renderApp()

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Lint demo' }))

    expect(screen.getByLabelText('Source editor')).toHaveValue(
      'let unused = 1;\nlet s = "hi";\nlen(s, s);\n{1: "a", 1: "b"};\nif (true) { puts(s); }\n'
    )
  })
})

describe('Lint', () => {
  it('lints the source editor when the toolbar button is pressed', async () => {
    const user = userEvent.setup()
    renderApp()

    await user.click(screen.getByRole('button', { name: 'Lint' }))

    expect(runLintMock).toHaveBeenCalledTimes(1)
  })
})

describe('GC playground', () => {
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

  it('caps global name chips and prose for heavily aliased objects', async () => {
    const user = userEvent.setup()
    const envelope = successEnvelope()
    if (envelope.status !== 'ok') {
      throw new Error('expected a successful test envelope')
    }
    envelope.report.globalRoots = ['holder', 'ha', 'hb', 'hc', 'hd'].map(
      (name) => ({ name, objectId: 1 })
    )
    runGcMock.mockResolvedValue(envelope)
    renderApp()

    await user.click(await openGcTab(user))
    await user.click(screen.getByRole('radio', { name: /Trial survivors/ }))
    expect(screen.getByText('holder')).toBeInTheDocument()
    expect(screen.getByText('hb')).toBeInTheDocument()
    expect(screen.queryByText('hc')).toBeNull()
    expect(screen.getByText('+2 more')).toHaveAttribute('title', 'hc, hd')

    await user.click(
      screen.getByRole('button', { name: /Expand details for Array#1/ })
    )
    expect(
      screen.getByText(/and 2 more currently reference this object/)
    ).toBeInTheDocument()
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
      span: { start: 0, end: 5 },
    } satisfies GcRunEnvelope)
    renderApp()

    await user.click(await openGcTab(user))

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('runtime error')
    expect(alert).toHaveTextContent("property 'next' does not exist on Node")
    expect(highlightRangeMock).toHaveBeenCalledWith(0, 5)

    highlightRangeMock.mockClear()
    await user.click(
      screen.getByRole('button', { name: 'Show in editor (0–5)' })
    )
    expect(highlightRangeMock).toHaveBeenCalledWith(0, 5)

    clearHighlightMock.mockClear()
    await user.type(screen.getByLabelText('Source editor'), 'x')
    expect(clearHighlightMock).toHaveBeenCalled()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('converts UTF-8 GC error spans before highlighting Unicode source', async () => {
    const user = userEvent.setup()
    // 中 is three UTF-8 bytes but one UTF-16 unit, so the byte span {7, 11}
    // reported by the lexer lands on UTF-16 positions {5, 9}.
    runGcMock.mockResolvedValue({
      status: 'error',
      stage: 'runtime',
      message: 'identifier not found: boom',
      span: { start: 7, end: 11 },
    } satisfies GcRunEnvelope)
    renderApp()

    const sourceEditor = screen.getByLabelText('Source editor')
    await user.clear(sourceEditor)
    await user.type(sourceEditor, '"中"; boom;')

    highlightRangeMock.mockClear()
    await user.click(await openGcTab(user))

    await screen.findByRole('alert')
    expect(highlightRangeMock).toHaveBeenCalledWith(5, 9)

    highlightRangeMock.mockClear()
    await user.click(
      screen.getByRole('button', { name: 'Show in editor (7–11)' })
    )
    expect(highlightRangeMock).toHaveBeenCalledWith(5, 9)
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

describe('Snapshot playground', () => {
  it('does not compile snapshots while the snapshot tab is hidden', async () => {
    renderApp()

    await waitFor(() => {
      expect(parseMock).toHaveBeenCalled()
    })
    expect(buildSnapshotMock).not.toHaveBeenCalled()
  })

  it('builds the snapshot automatically and runs the bytes on demand', async () => {
    const user = userEvent.setup()
    runSnapshotMock.mockResolvedValue({
      status: 'ok',
      result: '3',
    } satisfies SnapshotRunEnvelope)
    renderApp()

    const runButton = await openSnapshotTab(user)

    expect(await screen.findByLabelText('Snapshot size')).toHaveTextContent(
      '4 bytes'
    )
    expect(screen.getByText('magic')).toBeInTheDocument()
    expect(screen.getByText('4d 42 43 00')).toBeInTheDocument()
    expect(buildSnapshotMock).toHaveBeenLastCalledWith(
      expect.any(String),
      false
    )
    expect(runSnapshotMock).not.toHaveBeenCalled()

    await user.click(runButton)

    expect(
      await screen.findByLabelText('Snapshot run result')
    ).toHaveTextContent('3')
    expect(runSnapshotMock).toHaveBeenCalledTimes(1)
    expect(Array.from(runSnapshotMock.mock.calls[0][0] as Uint8Array)).toEqual([
      0x4d, 0x42, 0x43, 0x00,
    ])
  })

  it('immediately invalidates stale bytes when the source changes', async () => {
    const user = userEvent.setup()
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    expect(runButton).toBeEnabled()
    const downloadButton = screen.getByRole('button', {
      name: 'Download .mbc',
    })
    expect(downloadButton).toBeEnabled()
    const buildCount = buildSnapshotMock.mock.calls.length

    await user.type(screen.getByLabelText('Source editor'), 'x')

    // The previous build stays mounted while the rebuild is announced; only
    // the actions that would hand out stale bytes lock up.
    expect(screen.getByRole('status')).toHaveTextContent('Rebuilding snapshot…')
    expect(screen.getByLabelText('Snapshot size')).toBeInTheDocument()
    expect(runButton).toBeDisabled()
    expect(downloadButton).toBeDisabled()
    expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount)

    await waitFor(() => expect(runButton).toBeEnabled())
    expect(screen.queryByText('Rebuilding snapshot…')).not.toBeInTheDocument()
    expect(downloadButton).toBeEnabled()
    expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount + 1)
  })

  it('keeps a fresh build when an edit reports identical text', async () => {
    const user = userEvent.setup()
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    expect(runButton).toBeEnabled()
    const buildCount = buildSnapshotMock.mock.calls.length
    const sourceEditor =
      screen.getByLabelText<HTMLTextAreaElement>('Source editor')

    // CodeMirror reports a document change even when the replacement text is
    // identical, e.g. typing over a selection with the same character.
    act(() => {
      sourceEditorHooks.onChange?.(sourceEditor.value)
    })

    expect(screen.queryByText('Rebuilding snapshot…')).not.toBeInTheDocument()
    expect(runButton).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Download .mbc' })).toBeEnabled()
    expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount)
  })

  it('invalidates and rebuilds the snapshot when switching snippets', async () => {
    const user = userEvent.setup()
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    expect(runButton).toBeEnabled()
    const buildCount = buildSnapshotMock.mock.calls.length

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Functions' }))

    expect(screen.getByRole('status')).toHaveTextContent('Rebuilding snapshot…')
    expect(runButton).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Download .mbc' })).toBeDisabled()

    await waitFor(() => {
      expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount + 1)
    })
    expect(buildSnapshotMock).toHaveBeenLastCalledWith(
      expect.stringContaining('let add = fn(a, b) { a + b };'),
      false
    )
    await waitFor(() => expect(runButton).toBeEnabled())
  })

  it('highlights the span for snapshot runtime errors until the source changes', async () => {
    const user = userEvent.setup()
    renderApp()

    const sourceEditor =
      screen.getByLabelText<HTMLTextAreaElement>('Source editor')
    const start = sourceEditor.value.indexOf('print')
    const end = start + 'print(a)'.length
    runSnapshotMock.mockResolvedValue({
      status: 'error',
      stage: 'runtime',
      message: 'not a function: Integer',
      span: { start, end },
    } satisfies SnapshotRunEnvelope)

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    await user.click(runButton)

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('runtime error')
    expect(alert).toHaveTextContent('not a function: Integer')
    expect(highlightRangeMock).toHaveBeenCalledWith(start, end)

    clearHighlightMock.mockClear()
    await user.type(sourceEditor, 'x')
    expect(clearHighlightMock).toHaveBeenCalled()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('converts UTF-8 runtime spans before highlighting Unicode source', async () => {
    const user = userEvent.setup()
    const source = '"é"; let not_callable = 5; not_callable()'
    runSnapshotMock.mockResolvedValue({
      status: 'error',
      stage: 'runtime',
      message: 'not a function: Integer',
      span: { start: 28, end: 42 },
    } satisfies SnapshotRunEnvelope)
    renderApp()

    const sourceEditor = screen.getByLabelText('Source editor')
    await user.clear(sourceEditor)
    await user.type(sourceEditor, source)

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    highlightRangeMock.mockClear()
    await user.click(runButton)

    await screen.findByRole('alert')
    expect(highlightRangeMock).toHaveBeenCalledWith(27, 41)

    highlightRangeMock.mockClear()
    await user.click(
      screen.getByRole('button', { name: 'Show in editor (28–42)' })
    )
    expect(highlightRangeMock).toHaveBeenCalledWith(27, 41)
  })

  it('rebuilds the snapshot when the strip toggle changes', async () => {
    const user = userEvent.setup()
    buildSnapshotMock.mockImplementation(
      (_source: string, stripDebug: boolean) => {
        const envelope = snapshotEnvelope()
        return {
          ...envelope,
          layout: { ...envelope.layout, hasDebugInfo: !stripDebug },
        }
      }
    )
    renderApp()

    await openSnapshotTab(user)
    expect(
      await screen.findByLabelText('Snapshot debug info')
    ).toHaveTextContent('included')
    expect(buildSnapshotMock).toHaveBeenLastCalledWith(
      expect.any(String),
      false
    )
    const runButton = screen.getByRole('button', { name: 'Run snapshot' })
    const buildCount = buildSnapshotMock.mock.calls.length

    const strippedToggle = screen.getByRole('radio', { name: 'Stripped' })
    await user.click(strippedToggle)

    expect(screen.getByRole('status')).toHaveTextContent('Rebuilding snapshot…')
    expect(runButton).toBeDisabled()
    // The toggle survives the rebuild without losing keyboard focus because
    // the panel is never unmounted.
    expect(strippedToggle).toHaveFocus()
    expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount)

    await waitFor(() => {
      expect(buildSnapshotMock).toHaveBeenLastCalledWith(
        expect.any(String),
        true
      )
    })
    await waitFor(() => {
      expect(screen.getByLabelText('Snapshot debug info')).toHaveTextContent(
        'stripped'
      )
    })
    expect(strippedToggle).toHaveFocus()
  })

  it('ignores a snapshot run that resolves after the source changes', async () => {
    const user = userEvent.setup()
    const staleRun = deferred<SnapshotRunEnvelope>()
    runSnapshotMock.mockReturnValueOnce(staleRun.promise)
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    await user.click(runButton)
    expect(runSnapshotMock).toHaveBeenCalledTimes(1)

    await user.type(screen.getByLabelText('Source editor'), 'x')

    await act(async () => {
      staleRun.resolve({ status: 'ok', result: 'stale' })
      await staleRun.promise
    })

    expect(screen.queryByText('stale')).not.toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Rebuilding snapshot…')

    await waitFor(() => expect(runButton).toBeEnabled())
    expect(
      screen.getByText(/executes the bytes above on the GC VM/)
    ).toBeInTheDocument()
    expect(screen.queryByText('stale')).not.toBeInTheDocument()
  })
})

describe('Format', () => {
  it('keeps the GC report when formatting is a no-op', async () => {
    const user = userEvent.setup()
    runGcMock.mockResolvedValue(successEnvelope())
    renderApp()

    await user.click(await openGcTab(user))
    await screen.findByLabelText(
      'Heap object count before and after collection'
    )

    const formatButton = screen.getByRole('button', { name: 'Format' })
    await user.click(formatButton)
    await waitFor(() => expect(formatButton).toBeEnabled())

    expect(
      screen.getByLabelText('Heap object count before and after collection')
    ).toHaveTextContent('20 → 18')
  })

  it('keeps the snapshot when formatting is a no-op', async () => {
    const user = userEvent.setup()
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    expect(runButton).toBeEnabled()
    const buildCount = buildSnapshotMock.mock.calls.length

    const formatButton = screen.getByRole('button', { name: 'Format' })
    await user.click(formatButton)
    await waitFor(() => expect(formatButton).toBeEnabled())

    expect(screen.queryByText('Rebuilding snapshot…')).not.toBeInTheDocument()
    expect(runButton).toBeEnabled()
    expect(buildSnapshotMock).toHaveBeenCalledTimes(buildCount)
  })

  it('rebuilds the snapshot after formatting changes the source', async () => {
    const user = userEvent.setup()
    formatMock.mockResolvedValue('let formatted = 1;\n')
    renderApp()

    const runButton = await openSnapshotTab(user)
    await screen.findByLabelText('Snapshot size')
    const sourceEditor = screen.getByLabelText('Source editor')

    await user.click(screen.getByRole('button', { name: 'Format' }))

    await waitFor(() =>
      expect(sourceEditor).toHaveValue('let formatted = 1;\n')
    )
    await waitFor(() => {
      expect(buildSnapshotMock).toHaveBeenLastCalledWith(
        'let formatted = 1;\n',
        false
      )
    })
    await waitFor(() => expect(runButton).toBeEnabled())
  })

  it('discards a format result that resolves after further edits', async () => {
    const user = userEvent.setup()
    const pendingFormat = deferred<string>()
    formatMock.mockReturnValueOnce(pendingFormat.promise)
    renderApp()

    const sourceEditor =
      screen.getByLabelText<HTMLTextAreaElement>('Source editor')
    const formatButton = screen.getByRole('button', { name: 'Format' })
    await user.click(formatButton)
    await waitFor(() => expect(formatMock).toHaveBeenCalledTimes(1))

    await user.type(sourceEditor, 'x')
    const edited = sourceEditor.value
    expect(edited).toContain('x')

    await act(async () => {
      pendingFormat.resolve('let clobbered = 1;')
      await pendingFormat.promise
    })
    await waitFor(() => expect(formatButton).toBeEnabled())

    // Applying the late result would silently revert the edit.
    expect(sourceEditor).toHaveValue(edited)
  })
})

describe('AST selection sync', () => {
  it('maps editor selections to AST nodes across multi-byte characters', async () => {
    const user = userEvent.setup()
    const unicodeSource = '"中"; boom;'
    // boom occupies bytes {7, 11} but UTF-16 positions {5, 9}.
    const unicodeAst = JSON.stringify({
      Program: {
        type: 'Program',
        span: { start: 0, end: 12 },
        body: [
          {
            type: 'ExpressionStatement',
            span: { start: 0, end: 6 },
            expression: {
              type: 'StringLiteral',
              span: { start: 0, end: 5 },
              value: '中',
            },
          },
          {
            type: 'ExpressionStatement',
            span: { start: 7, end: 12 },
            expression: {
              type: 'Identifier',
              span: { start: 7, end: 11 },
              name: 'boom',
            },
          },
        ],
      },
    })
    parseMock.mockImplementation((source: string) =>
      source === unicodeSource ? unicodeAst : defaultAstJson
    )
    renderApp()

    const sourceEditor = screen.getByLabelText('Source editor')
    await user.clear(sourceEditor)
    await user.type(sourceEditor, unicodeSource)
    await screen.findByText('Identifier')

    act(() => {
      sourceEditorHooks.onSelectionChange?.({ from: 5, to: 9 })
    })

    const identifierSummary = screen.getByText('Identifier').closest('summary')
    expect(identifierSummary).not.toBeNull()
    expect(identifierSummary).toHaveClass(
      'shadow-[inset_2px_0_0_var(--accent-9)]'
    )

    highlightRangeMock.mockClear()
    await user.click(identifierSummary!)
    expect(highlightRangeMock).toHaveBeenCalledWith(5, 9)
  })
})

describe('ARM64 view', () => {
  it('lowers only while the tab is active and renders the assembly', async () => {
    const user = userEvent.setup()
    renderApp()

    expect(buildArm64Mock).not.toHaveBeenCalled()

    await openArm64Tab(user)

    const sourceEditor =
      screen.getByLabelText<HTMLTextAreaElement>('Source editor')
    expect(buildArm64Mock).toHaveBeenCalledWith(sourceEditor.value)
    expect(
      screen.getByLabelText<HTMLTextAreaElement>('Output editor')
    ).toHaveValue('main:\n  mov x0, #7\n  ret')
    expect(screen.getByText(/aarch64-linux-gnu-gcc/)).toBeInTheDocument()

    const calls = buildArm64Mock.mock.calls.length
    await user.type(sourceEditor, 'x')
    await waitFor(() =>
      expect(buildArm64Mock.mock.calls.length).toBeGreaterThan(calls)
    )
    expect(buildArm64Mock).toHaveBeenLastCalledWith(sourceEditor.value)
  })

  it('starts the download help collapsed at the mobile breakpoint', async () => {
    const user = userEvent.setup()
    renderApp()

    await openArm64Tab(user)

    const disclosure = screen.getByRole('button', {
      name: 'Download .s and build help',
    })
    const help = document.getElementById('arm64-build-help')
    expect(help).not.toBeNull()
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
    expect(help).toHaveClass('max-[780px]:hidden')

    await user.click(disclosure)
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')
    expect(help).not.toHaveClass('max-[780px]:hidden')
    expect(help).toHaveClass('max-[780px]:flex')
  })

  it('collapses the reading guide until the summary is opened', async () => {
    const user = userEvent.setup()
    renderApp()

    await openArm64Tab(user)

    const summary = screen.getByText('How to read this assembly')
    const taggedValues = screen.getByText('Tagged values.')
    expect(taggedValues).not.toBeVisible()

    await user.click(summary)
    expect(taggedValues).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'backend design doc' })
    ).toHaveAttribute(
      'href',
      'https://github.com/gengjiawen/monkey-rust/blob/main/docs/arm64-asm-backend-design.md'
    )
    expect(taggedValues.closest('li')).toHaveTextContent(
      'builtins use (id << 3) | 0b101 (len is #0x5)'
    )
    expect(
      screen.getByText('Function frames.').closest('li')
    ).toHaveTextContent(
      'Ordinary calls enter them through rt_call; new uses rt_construct'
    )
  })

  it('renders lowering failures with a span jump button', async () => {
    const user = userEvent.setup()
    buildArm64Mock.mockImplementation(
      (): Arm64BuildEnvelope => ({
        status: 'error',
        stage: 'compile',
        message: "undefined variable 'missing'",
        span: { start: 0, end: 7 },
      })
    )
    renderApp()

    await user.click(screen.getByRole('radio', { name: 'ARM64' }))

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(
      "compile error: undefined variable 'missing'"
    )

    highlightRangeMock.mockClear()
    await user.click(
      screen.getByRole('button', { name: 'Show in editor (0–7)' })
    )
    expect(highlightRangeMock).toHaveBeenCalledWith(0, 7)
  })

  it('maps assembly cursor movements back to the source span', async () => {
    const user = userEvent.setup()
    renderApp()

    await openArm64Tab(user)

    highlightRangeMock.mockClear()
    act(() => {
      outputEditorHooks.onSelectionChange?.({ from: 7, to: 7 })
    })
    expect(highlightRangeMock).toHaveBeenCalledWith(0, 1)

    // The label line carries no span, so the source highlight clears.
    clearHighlightMock.mockClear()
    act(() => {
      outputEditorHooks.onSelectionChange?.({ from: 0, to: 0 })
    })
    expect(clearHighlightMock).toHaveBeenCalled()
  })

  it('maps source cursor movements to the lowered assembly lines', async () => {
    const user = userEvent.setup()
    renderApp()

    await openArm64Tab(user)

    highlightRangesMock.mockClear()
    act(() => {
      sourceEditorHooks.onSelectionChange?.({ from: 0, to: 0 })
    })
    // Both code lines share span {0,1}; being adjacent they merge into one
    // assembly range spanning `  mov x0, #7\n  ret`.
    expect(highlightRangesMock).toHaveBeenCalledWith([{ from: 6, to: 24 }])

    clearHighlightMock.mockClear()
    act(() => {
      sourceEditorHooks.onSelectionChange?.({ from: 99, to: 99 })
    })
    expect(clearHighlightMock).toHaveBeenCalled()
  })

  it('highlights every non-contiguous assembly range for a source span', async () => {
    const user = userEvent.setup()
    const lines = [
      { text: 'main:', kind: 'label' as const, span: null },
      {
        text: '  mov x0, #7',
        kind: 'code' as const,
        span: { start: 0, end: 1 },
      },
      { text: '  nop', kind: 'code' as const, span: null },
      {
        text: '  ret',
        kind: 'code' as const,
        span: { start: 0, end: 1 },
      },
    ]
    buildArm64Mock.mockImplementation(() => ({
      status: 'ok',
      lines,
      text: lines.map((line) => line.text).join('\n'),
    }))
    renderApp()

    await openArm64Tab(user)

    highlightRangesMock.mockClear()
    act(() => {
      sourceEditorHooks.onSelectionChange?.({ from: 0, to: 0 })
    })
    expect(highlightRangesMock).toHaveBeenCalledWith([
      { from: 6, to: 18 },
      { from: 25, to: 30 },
    ])
  })
})

describe('Minify view', () => {
  it('minifies only while active, reports UTF-8 bytes, and toggles mangling', async () => {
    const user = userEvent.setup()
    minifyMock.mockImplementation(() => ({ code: '中;' }))
    renderApp()

    expect(minifyMock).not.toHaveBeenCalled()
    await user.click(screen.getByRole('radio', { name: 'Minify' }))

    await waitFor(() => expect(minifyMock).toHaveBeenCalledTimes(1))
    expect(screen.getByLabelText('Minified byte statistics')).toHaveTextContent(
      '→ 4 UTF-8 bytes'
    )
    expect(screen.getByLabelText('Output editor')).toHaveValue('中;')

    await user.click(screen.getByRole('switch', { name: 'Mangle names' }))
    await waitFor(() =>
      expect(minifyMock).toHaveBeenLastCalledWith(
        expect.any(String),
        expect.objectContaining({ mangle: false })
      )
    )

    const calls = minifyMock.mock.calls.length
    await user.click(screen.getByRole('radio', { name: 'AST' }))
    await user.type(screen.getByLabelText('Source editor'), 'x')
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(minifyMock).toHaveBeenCalledTimes(calls)
  })

  it('keeps only the latest debounced source and displays errors', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(screen.getByRole('radio', { name: 'Minify' }))
    await waitFor(() => expect(minifyMock).toHaveBeenCalled())

    minifyMock.mockClear()
    const editor = screen.getByLabelText('Source editor')
    await user.clear(editor)
    await user.type(editor, '1 + 2')
    await waitFor(() => expect(minifyMock).toHaveBeenCalledTimes(1))
    expect(minifyMock).toHaveBeenLastCalledWith(
      '1 + 2',
      expect.objectContaining({ mangle: true })
    )

    minifyMock.mockImplementationOnce(() => {
      throw new SyntaxError('bad Monkey source')
    })
    await user.type(editor, ';')
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'bad Monkey source'
    )
  })
})
