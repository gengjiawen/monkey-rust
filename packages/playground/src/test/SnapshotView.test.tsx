import { Theme } from '@radix-ui/themes'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { SnapshotLayout } from '../snapshot'
import {
  SnapshotView,
  type SnapshotBuildState,
  type SnapshotRunState,
} from '../SnapshotView'

function layout(hasDebugInfo = true): SnapshotLayout {
  const regions: SnapshotLayout['regions'] = [
    {
      offset: 0,
      length: 4,
      section: 'header',
      label: 'magic',
      detail: 'file signature "MBC\\0"',
    },
    {
      offset: 4,
      length: 1,
      section: 'header',
      label: 'version',
      detail: 'container format version 1',
    },
    {
      offset: 5,
      length: 4,
      section: 'header',
      label: 'abi fingerprint',
      detail: '0x0000002a — FNV-1a over the opcode and builtin tables',
    },
    {
      offset: 9,
      length: 1,
      section: 'header',
      label: 'flags',
      detail: hasDebugInfo
        ? '0b00000001 — debug info present'
        : '0b00000000 — debug info stripped',
    },
    {
      offset: 10,
      length: 1,
      section: 'main',
      label: 'main length',
      detail: '1 bytes of main instructions follow (ULEB128)',
    },
    {
      offset: 11,
      length: 1,
      section: 'main',
      label: 'OpPop',
      detail: 'main pc 0000',
    },
  ]
  if (hasDebugInfo) {
    regions.push({
      offset: 12,
      length: 1,
      section: 'debug',
      label: 'main span count',
      detail: '0 pc→span entries (ULEB128)',
    })
  }
  return {
    byteLength: hasDebugInfo ? 13 : 12,
    formatVersion: 1,
    abiFingerprint: '0x0000002a',
    hasDebugInfo,
    regions,
  }
}

function okBuild(hasDebugInfo = true): SnapshotBuildState {
  const built = layout(hasDebugInfo)
  return {
    status: 'ok',
    bytes: new Uint8Array([
      0x4d, 0x42, 0x43, 0x00, 0x01, 0x2a, 0x00, 0x00, 0x00,
      hasDebugInfo ? 0x01 : 0x00, 0x01, 0x02,
      ...(hasDebugInfo ? [0x00] : []),
    ]),
    layout: built,
  }
}

function renderView({
  build = okBuild(),
  run = { status: 'idle' } as SnapshotRunState,
  stripDebug = false,
  onStripDebugChange = vi.fn(),
  onErrorSpanSelect = vi.fn(),
} = {}) {
  render(
    <Theme>
      <SnapshotView
        build={build}
        run={run}
        stripDebug={stripDebug}
        onStripDebugChange={onStripDebugChange}
        onErrorSpanSelect={onErrorSpanSelect}
      />
    </Theme>
  )
  return { onStripDebugChange, onErrorSpanSelect }
}

describe('SnapshotView', () => {
  afterEach(cleanup)

  it('renders the summary facts and the annotated hexdump', () => {
    renderView()

    expect(screen.getByLabelText('Snapshot size')).toHaveTextContent(
      '13 bytes'
    )
    expect(screen.getByLabelText('Snapshot format version')).toHaveTextContent(
      '1'
    )
    expect(
      screen.getByLabelText('Snapshot ABI fingerprint')
    ).toHaveTextContent('0x0000002a')
    expect(screen.getByLabelText('Snapshot debug info')).toHaveTextContent(
      'included'
    )

    expect(
      screen.getByRole('heading', { name: 'Header' })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Main program' })
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Debug info' })
    ).toBeInTheDocument()
    expect(screen.getByText('magic')).toBeInTheDocument()
    expect(screen.getByText('4d 42 43 00')).toBeInTheDocument()
    expect(screen.getByText('OpPop')).toBeInTheDocument()
    expect(screen.getByText('main pc 0000')).toBeInTheDocument()
  })

  it('reports the strip toggle and downloads the bytes', async () => {
    const user = userEvent.setup()
    const createObjectURL = vi.fn(() => 'blob:snapshot')
    const revokeObjectURL = vi.fn()
    vi.stubGlobal('URL', {
      ...URL,
      createObjectURL,
      revokeObjectURL,
    })
    const anchorClick = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => {})

    try {
      const { onStripDebugChange } = renderView()

      await user.click(screen.getByRole('radio', { name: 'Stripped' }))
      expect(onStripDebugChange).toHaveBeenCalledWith(true)

      await user.click(screen.getByRole('button', { name: 'Download .mbc' }))
      expect(createObjectURL).toHaveBeenCalledTimes(1)
      expect(anchorClick).toHaveBeenCalledTimes(1)
      expect(revokeObjectURL).toHaveBeenCalledWith('blob:snapshot')
    } finally {
      anchorClick.mockRestore()
      vi.unstubAllGlobals()
    }
  })

  it('shows run results and lets runtime errors jump to their span', async () => {
    const user = userEvent.setup()
    const { onErrorSpanSelect } = renderView({
      run: {
        status: 'error',
        stage: 'runtime',
        message: 'not a function: Integer',
        span: { start: 22, end: 36 },
      },
    })

    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent('runtime error')
    expect(alert).toHaveTextContent('not a function: Integer')

    await user.click(
      screen.getByRole('button', { name: 'Show in editor (22–36)' })
    )
    expect(onErrorSpanSelect).toHaveBeenCalledWith({ start: 22, end: 36 })
  })

  it('explains missing spans on stripped snapshots', () => {
    renderView({
      build: okBuild(false),
      stripDebug: true,
      run: {
        status: 'error',
        stage: 'runtime',
        message: 'not a function: Integer',
        span: null,
      },
    })

    expect(screen.getByLabelText('Snapshot debug info')).toHaveTextContent(
      'stripped'
    )
    expect(
      screen.getByText(/Stripped snapshots drop the pc→span table/)
    ).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: /Show in editor/ })
    ).not.toBeInTheDocument()
  })

  it('renders idle hints, run results, and build failures', () => {
    renderView({ run: { status: 'ok', result: '3' } })
    expect(screen.getByLabelText('Snapshot run result')).toHaveTextContent('3')
    cleanup()

    renderView()
    expect(
      screen.getByText(/executes the bytes above on the GC VM/)
    ).toBeInTheDocument()
    cleanup()

    renderView({
      build: { status: 'error', stage: 'parse', message: 'unexpected token' },
    })
    expect(screen.getByRole('alert')).toHaveTextContent(
      'parse error: unexpected token'
    )
  })
})
