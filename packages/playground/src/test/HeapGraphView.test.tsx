import { cleanup, render, screen, waitFor } from '@testing-library/react'
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

describe('HeapGraphView', () => {
  afterEach(cleanup)

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
    await waitFor(() => {
      expect(container.querySelector('.gc-graph-canvas svg')).not.toBeNull()
    })
    expect(initializeMock).toHaveBeenCalledWith(
      expect.objectContaining({ theme: 'default', securityLevel: 'strict' })
    )
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

  it('reports a mermaid rendering failure', async () => {
    renderMock.mockRejectedValue(new Error('boom'))
    render(<HeapGraphView report={cycleReport()} />)

    expect(
      await screen.findByText(/The graph could not be rendered: boom/)
    ).toBeInTheDocument()
  })
})
