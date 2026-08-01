import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

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

import { DebuggerView } from '../DebuggerView'
import type { DebuggerRunEnvelope } from '../debuggerReport'
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

function twoHitEnvelope(): DebuggerRunEnvelope {
  return okEnvelope(
    [
      hit(1, {
        span: { start: 30, end: 39 },
        frames: [
          frame({ currentSpan: { start: 0, end: 5 } }),
          frame({
            name: 'sum',
            currentSpan: { start: 60, end: 70 },
            callee: refValue(2, '[closure function]', 'closure'),
            locals: [slot('total', 0, inlineValue('3'))],
          }),
          frame({
            name: 'makePoint',
            currentSpan: { start: 30, end: 39 },
            callee: refValue(3, '[closure function]', 'closure'),
            locals: [
              slot('x', 0, inlineValue('3')),
              slot('p', 1, refValue(7, '[3, 2]')),
              slot('ghost', 2, null),
            ],
            captures: [{ name: 'b', index: 0, value: inlineValue('20') }],
            temporaries: [{ slot: 6, value: inlineValue('42') }],
          }),
        ],
        globals: [
          slot('ready', 0, inlineValue('null', 'null')),
          slot('faraway', 1, refValue(77, '[…]')),
        ],
        heap: emptyHeap({
          objects: [
            { id: 2, kind: 'closure', label: 'Closure(sum)', members: [] },
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
                { relation: { kind: 'arrayElement', index: 0 }, display: '3' },
              ],
            },
          ],
          omittedObjects: 2,
          omittedEdges: 1,
        }),
      }),
      hit(2, { span: { start: 80, end: 89 } }),
    ],
    { result: '3', stdout: 'before\n' }
  )
}

afterEach(cleanup)

beforeEach(() => {
  initializeMock.mockReset()
  renderMock.mockReset()
  renderMock.mockResolvedValue({ svg: '<svg></svg>' })
  useThemeMock.mockReset()
  useThemeMock.mockReturnValue({ resolvedTheme: 'light' })
})

describe('DebuggerView', () => {
  it('waits for an explicit run while idle', () => {
    render(<DebuggerView state={{ status: 'idle' }} />)

    expect(
      screen.getByText('Editing never executes the program automatically.')
    ).toBeInTheDocument()
  })

  it('renders frames innermost first with callee, slot, and capture state', () => {
    const onSpanSelect = vi.fn()
    render(
      <DebuggerView state={twoHitEnvelope()} onSpanSelect={onSpanSelect} />
    )

    expect(screen.getByText('Hit 1 of 2')).toBeInTheDocument()
    expect(onSpanSelect).toHaveBeenLastCalledWith({ start: 30, end: 39 })
    const stack = screen.getByLabelText('Call stack')
    expect(
      within(stack).getByText('Call stack (3 frames)')
    ).toBeInTheDocument()
    expect(within(stack).getByText(/top of stack/)).toBeInTheDocument()
    expect(within(stack).getByText(/stack base/)).toBeInTheDocument()
    const frameButtons = within(stack)
      .getAllByRole('button')
      .filter((button) => button.textContent?.includes('show in editor'))
    expect(frameButtons[0]).toHaveTextContent('makePoint')
    expect(frameButtons[0]).toHaveTextContent('frame 2')
    expect(frameButtons[0]).toHaveTextContent('current')
    expect(frameButtons[1]).toHaveTextContent('sum')
    expect(frameButtons[1]).toHaveTextContent('frame 1')
    expect(frameButtons[2]).toHaveTextContent('main')
    expect(frameButtons[2]).toHaveTextContent('frame 0')
    expect(frameButtons[1]).not.toHaveTextContent('current')

    expect(within(stack).getByText('<uninitialized>')).toBeInTheDocument()
    const globals = screen.getByRole('heading', { name: 'Globals' })
      .parentElement as HTMLElement
    expect(within(globals).getByText('null')).toBeInTheDocument()
    expect(within(stack).getByText('Captures (1)')).toBeInTheDocument()
    expect(within(stack).getByText('b')).toBeInTheDocument()
    expect(
      within(stack).getByText('Operand stack temporaries (1)')
    ).toBeInTheDocument()
    expect(screen.getByText('before')).toBeInTheDocument()
  })

  it('reports dropped hits and snapshot omissions', () => {
    const envelope = okEnvelope([hit(1, twoHitEnvelope().hits[0])], {
      droppedHits: 5,
    })
    render(<DebuggerView state={envelope} />)

    expect(
      screen.getByText(/Recording stopped after the first 1 hits/)
    ).toBeInTheDocument()
    expect(
      screen.getByText(/omitted 2 objects and 1 reference at record time/)
    ).toBeInTheDocument()
  })

  it('clicking a frame highlights where it is paused', () => {
    const onSpanSelect = vi.fn()
    render(
      <DebuggerView state={twoHitEnvelope()} onSpanSelect={onSpanSelect} />
    )

    fireEvent.click(screen.getByRole('button', { name: /sum/ }))
    expect(onSpanSelect).toHaveBeenCalledWith({ start: 60, end: 70 })
  })

  it('pages between hits, highlights the new span, and clears the pin', () => {
    const onSpanSelect = vi.fn()
    render(
      <DebuggerView state={twoHitEnvelope()} onSpanSelect={onSpanSelect} />
    )

    const chip = screen.getByRole('button', { name: 'ref #7' })
    fireEvent.click(chip)
    expect(chip).toHaveAttribute('aria-pressed', 'true')

    fireEvent.click(screen.getByRole('button', { name: 'Next hit' }))
    expect(onSpanSelect).toHaveBeenCalledWith({ start: 80, end: 89 })
    expect(screen.getByText('Hit 2 of 2')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Previous hit' }))
    expect(onSpanSelect).toHaveBeenCalledWith({ start: 30, end: 39 })
    expect(screen.getByRole('button', { name: 'ref #7' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
  })

  it('clears a stale span while rerunning and prefers a recorded hit over a later error', () => {
    const onSpanSelect = vi.fn()
    const { rerender } = render(
      <DebuggerView state={twoHitEnvelope()} onSpanSelect={onSpanSelect} />
    )
    expect(onSpanSelect).toHaveBeenLastCalledWith({ start: 30, end: 39 })

    rerender(
      <DebuggerView state={{ status: 'running' }} onSpanSelect={onSpanSelect} />
    )
    expect(onSpanSelect).toHaveBeenLastCalledWith(null)

    rerender(
      <DebuggerView
        state={errorEnvelope([hit(1)], { span: { start: 90, end: 99 } })}
        onSpanSelect={onSpanSelect}
      />
    )
    expect(onSpanSelect).toHaveBeenLastCalledWith({ start: 10, end: 19 })

    rerender(
      <DebuggerView
        state={errorEnvelope([], { span: { start: 90, end: 99 } })}
        onSpanSelect={onSpanSelect}
      />
    )
    expect(onSpanSelect).toHaveBeenLastCalledWith({ start: 90, end: 99 })
  })

  it('hovering a ref chip regenerates the graph with a highlight class', async () => {
    render(<DebuggerView state={twoHitEnvelope()} />)

    await waitFor(() => {
      expect(renderMock).toHaveBeenCalled()
    })
    expect(renderMock.mock.calls[0][1]).not.toContain('highlighted')

    fireEvent.mouseEnter(screen.getByRole('button', { name: 'ref #7' }))
    await waitFor(() => {
      const sources = renderMock.mock.calls.map((call) => call[1] as string)
      expect(
        sources.some((source) => source.includes('class o7 highlighted'))
      ).toBe(true)
    })
  })

  it('disables chips for objects the snapshot budget omitted', () => {
    render(<DebuggerView state={twoHitEnvelope()} />)

    const chip = screen.getByRole('button', { name: 'ref #77' })
    expect(chip).toBeDisabled()
    expect(chip).toHaveAttribute(
      'title',
      'This object was omitted by the snapshot budget, so it cannot be highlighted.'
    )
  })

  it('keeps recorded hits visible under a runtime error', () => {
    const onSpanSelect = vi.fn()
    render(
      <DebuggerView
        state={errorEnvelope([hit(1)], { stdout: 'partial\n' })}
        onSpanSelect={onSpanSelect}
      />
    )

    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent('runtime error · call')
    expect(alert).toHaveTextContent('calling non-function')
    expect(
      screen.getByText('The snapshots below were recorded before the failure.')
    ).toBeInTheDocument()
    expect(screen.getByText('Hit 1 of 1')).toBeInTheDocument()
    expect(screen.getByText('partial')).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: 'Show in editor (3–8)' })
    )
    expect(onSpanSelect).toHaveBeenCalledWith({ start: 3, end: 8 })
  })

  it('explains an ok run that never reached a debugger statement', () => {
    render(<DebuggerView state={okEnvelope([], { result: '42' })} />)

    expect(screen.getByText('No debugger hits')).toBeInTheDocument()
    expect(screen.getByText('42')).toBeInTheDocument()
  })

  it('surfaces invalid responses as an alert', () => {
    render(<DebuggerView state={{ status: 'invalid', message: 'bad JSON' }} />)

    expect(screen.getByRole('alert')).toHaveTextContent('bad JSON')
  })
})
