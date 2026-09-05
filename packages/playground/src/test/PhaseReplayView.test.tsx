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

import { PhaseReplayView } from '../PhaseReplayView'

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
  omittedWitnesses = 0,
}: {
  omittedEdgeDetails?: number
  omittedWitnesses?: number
} = {}): GcCollectionReport {
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
        omittedWitnesses,
      },
      freeCycles: { freed: 2 },
    },
    collectedByValueKind: counts(),
  }
}

describe('PhaseReplayView', () => {
  afterEach(() => {
    cleanup()
  })

  beforeEach(() => {
    initializeMock.mockReset()
    renderMock.mockReset()
    useThemeMock.mockReset()
    useThemeMock.mockReturnValue({ resolvedTheme: 'light' })
  })

  it('renders the first step and walks forward through the plan', async () => {
    renderMock.mockResolvedValue({ svg: '<svg><title>frame</title></svg>' })
    const { container } = render(<PhaseReplayView report={cycleReport()} />)

    expect(
      screen.getByRole('heading', { name: 'Phase replay' })
    ).toBeInTheDocument()
    expect(screen.getByText(/Deterministic replay/)).toBeInTheDocument()
    expect(screen.getByText('Step 1 of 8 · Start')).toBeInTheDocument()
    expect(screen.getByText(/Before the collector runs/)).toBeInTheDocument()
    await waitFor(() => {
      expect(container.querySelector('.gc-replay-canvas svg')).not.toBeNull()
    })
    expect(initializeMock).toHaveBeenCalledWith(
      expect.objectContaining({ securityLevel: 'strict', theme: 'default' })
    )
    const [id, firstSource] = renderMock.mock.calls[0] as [string, string]
    expect(id).toMatch(/^gc-phase-replay-\d+$/)
    expect(firstSource).toContain('flowchart LR')
    expect(firstSource).not.toContain(':::candidate')

    expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Next' }))

    expect(
      screen.getByText('Step 2 of 8 · Trial deletion')
    ).toBeInTheDocument()
    expect(screen.getByText(/RC 1 → 0/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled()
    await waitFor(() => {
      expect(renderMock).toHaveBeenCalledTimes(2)
    })
    const secondSource = renderMock.mock.calls[1][1] as string
    expect(secondSource).toContain(':::candidate')
    expect(secondSource).toContain('linkStyle 0 stroke-width:3.5px')
  })

  it('jumps to the final step through the slider', async () => {
    renderMock.mockResolvedValue({ svg: '<svg></svg>' })
    render(<PhaseReplayView report={cycleReport()} />)

    fireEvent.change(screen.getByRole('slider', { name: 'Replay step' }), {
      target: { value: '7' },
    })

    expect(screen.getByText('Step 8 of 8 · Done')).toBeInTheDocument()
    expect(screen.getByText(/Collection complete: 2 objects freed/)).toBeInTheDocument()
    expect(screen.getByText(/Freed 2/)).toBeInTheDocument()
    // At the end the play button offers to start over.
    expect(
      await screen.findByRole('button', { name: 'Replay' })
    ).toBeInTheDocument()
  })

  it('toggles between play and pause', async () => {
    renderMock.mockResolvedValue({ svg: '<svg></svg>' })
    render(<PhaseReplayView report={cycleReport()} />)

    fireEvent.click(screen.getByRole('button', { name: 'Play' }))
    expect(
      await screen.findByRole('button', { name: 'Pause' })
    ).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Pause' }))
    expect(
      await screen.findByRole('button', { name: 'Play' })
    ).toBeInTheDocument()
  })

  it('explains why a truncated report cannot be replayed', () => {
    render(<PhaseReplayView report={cycleReport({ omittedEdgeDetails: 1 })} />)

    expect(
      screen.getByText(/topology card above explains why/)
    ).toBeInTheDocument()
    expect(renderMock).not.toHaveBeenCalled()
  })

  it('explains why truncated witnesses block the replay', () => {
    render(<PhaseReplayView report={cycleReport({ omittedWitnesses: 1 })} />)

    expect(
      screen.getByText(/truncated restoration witnesses/)
    ).toBeInTheDocument()
    expect(renderMock).not.toHaveBeenCalled()
  })

  it('reports a mermaid rendering failure', async () => {
    renderMock.mockRejectedValue(new Error('boom'))
    render(<PhaseReplayView report={cycleReport()} />)

    expect(
      await screen.findByText(/The replay frame could not be rendered: boom/)
    ).toBeInTheDocument()
  })
})
