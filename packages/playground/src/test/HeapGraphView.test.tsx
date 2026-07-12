import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type {
  GcCollectionReport,
  ObjectDecision,
  ValueKindCounts,
} from '../gcReport'

const { initializeMock, renderMock, useThemeMock } = vi.hoisted(() => ({
  initializeMock: vi.fn(),
  renderMock: vi.fn(),
  useThemeMock: vi.fn(),
}))

vi.mock('mermaid', () => ({
  default: { initialize: initializeMock, render: renderMock },
}))

vi.mock('next-themes', () => ({
  useTheme: useThemeMock,
}))

import { HeapGraphView } from '../HeapGraphView'

const counts = (): ValueKindCounts => ({
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
})

function cycleReport({
  omittedEdgeDetails = 0,
}: { omittedEdgeDetails?: number } = {}): GcCollectionReport {
  const decisions: ObjectDecision[] = [20, 21].map((objectId) => ({
    objectId,
    refCountBefore: 1,
    heapIncomingEdges: 1,
    trialRefCount: 0,
    decision: 'candidate',
    final: 'freed',
  }))
  const objects = [20, 21].map((id) => ({
    id,
    kind: 'instance' as const,
    label: `Instance(Node)#${id}`,
  }))
  const snapshot = { objectCount: 2, trackedBytes: 0, byValueKind: counts() }
  return {
    before: snapshot,
    after: snapshot,
    objects,
    globalRoots: [],
    omittedGlobalRoots: 0,
    phases: {
      trialDeletion: {
        edgesVisited: 2 + omittedEdgeDetails,
        candidates: 2,
        objectDecisions: decisions,
        visitedEdges: [
          {
            fromId: 20,
            toId: 21,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 21,
            toId: 20,
            relation: { kind: 'instanceField', name: 'next' },
          },
        ],
        omittedObjectDecisions: 0,
        omittedEdgeDetails,
      },
      scan: {
        restored: 0,
        garbageCandidates: 2,
        restoredObjects: [],
        garbageCandidateObjects: objects,
        restorationWitnesses: [],
        omittedWitnesses: 0,
      },
      freeCycles: { freed: 2 },
    },
    collectedByValueKind: counts(),
  }
}

function grantFullscreenSupport() {
  Object.defineProperty(document, 'fullscreenEnabled', {
    configurable: true,
    value: true,
  })
}

function setFullscreenElement(element: Element | null) {
  Object.defineProperty(document, 'fullscreenElement', {
    configurable: true,
    get: () => element,
  })
  fireEvent(document, new Event('fullscreenchange'))
}

describe('HeapGraphView', () => {
  afterEach(() => {
    cleanup()
    Reflect.deleteProperty(document, 'fullscreenEnabled')
    Reflect.deleteProperty(document, 'fullscreenElement')
    Reflect.deleteProperty(HTMLElement.prototype, 'requestFullscreen')
    Reflect.deleteProperty(document, 'exitFullscreen')
  })

  beforeEach(() => {
    initializeMock.mockReset()
    renderMock.mockReset()
    useThemeMock.mockReset()
    useThemeMock.mockReturnValue({ resolvedTheme: 'light' })
  })

  it('renders the mermaid graph for a drawable report', async () => {
    renderMock.mockResolvedValue({ svg: '<svg><title>mock graph</title></svg>' })
    const { container } = render(<HeapGraphView report={cycleReport()} />)

    expect(
      screen.getByRole('heading', { name: 'Heap topology' })
    ).toBeInTheDocument()
    const graphKeyItems = screen.getAllByRole('listitem')
    expect(graphKeyItems).toHaveLength(2)
    expect(graphKeyItems[0]).toHaveTextContent(
      'Solid arrows show heap-to-heap references at collection start.'
    )
    expect(graphKeyItems[1]).toHaveTextContent(
      "Dotted arrows from External refs mark each trial survivor's remaining non-heap references (×N is its trial RC)."
    )
    await waitFor(() => {
      expect(container.querySelector('.gc-graph-canvas svg')).not.toBeNull()
    })
    expect(initializeMock).toHaveBeenCalledWith(
      expect.objectContaining({ theme: 'default', securityLevel: 'strict' })
    )
    // 'inherit' resolves to different fonts in mermaid's measurement
    // container (a child of <body>) and in this card, which clips labels.
    expect(initializeMock.mock.calls[0][0].fontFamily).not.toBe('inherit')
    const [id, source] = renderMock.mock.calls[0] as [string, string]
    expect(id).toMatch(/^gc-heap-graph-\d+$/)
    expect(source).toContain('flowchart LR')
    expect(source).toContain('o20 -- "fields[#quot;next#quot;]" --> o21')
  })

  it('follows the dark theme', async () => {
    useThemeMock.mockReturnValue({ resolvedTheme: 'dark' })
    renderMock.mockResolvedValue({ svg: '<svg></svg>' })
    const { container } = render(<HeapGraphView report={cycleReport()} />)

    await waitFor(() => {
      expect(container.querySelector('.gc-graph-canvas svg')).not.toBeNull()
    })
    expect(initializeMock).toHaveBeenCalledWith(
      expect.objectContaining({ theme: 'dark' })
    )
  })

  it('explains why a truncated report has no graph', () => {
    render(<HeapGraphView report={cycleReport({ omittedEdgeDetails: 1 })} />)

    expect(
      screen.getByText(/truncated edge or decision details/)
    ).toBeInTheDocument()
    expect(renderMock).not.toHaveBeenCalled()
  })

  it('hides the full screen button when the browser does not support it', async () => {
    renderMock.mockResolvedValue({ svg: '<svg></svg>' })
    const { container } = render(<HeapGraphView report={cycleReport()} />)

    await waitFor(() => {
      expect(container.querySelector('.gc-graph-canvas svg')).not.toBeNull()
    })
    expect(screen.queryByRole('button', { name: /full screen/i })).toBeNull()
  })

  it('toggles full screen through the browser fullscreen API', async () => {
    grantFullscreenSupport()
    const requestFullscreenMock = vi.fn().mockResolvedValue(undefined)
    HTMLElement.prototype.requestFullscreen = requestFullscreenMock
    document.exitFullscreen = vi.fn().mockResolvedValue(undefined)
    renderMock.mockResolvedValue({ svg: '<svg></svg>' })
    const { container } = render(<HeapGraphView report={cycleReport()} />)
    const card = container.querySelector('.gc-graph-card')

    fireEvent.click(await screen.findByRole('button', { name: 'Full screen' }))
    expect(requestFullscreenMock).toHaveBeenCalledTimes(1)
    expect(requestFullscreenMock.mock.contexts[0]).toBe(card)

    setFullscreenElement(card)
    fireEvent.click(
      await screen.findByRole('button', { name: 'Exit full screen' })
    )
    expect(document.exitFullscreen).toHaveBeenCalledTimes(1)

    setFullscreenElement(null)
    expect(
      await screen.findByRole('button', { name: 'Full screen' })
    ).toBeInTheDocument()
  })

  it('reports a mermaid rendering failure', async () => {
    renderMock.mockRejectedValue(new Error('boom'))
    render(<HeapGraphView report={cycleReport()} />)

    expect(
      await screen.findByText(/The graph could not be rendered: boom/)
    ).toBeInTheDocument()
  })
})
