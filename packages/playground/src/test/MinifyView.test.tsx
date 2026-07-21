import { Theme } from '@radix-ui/themes'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { MinifyView, utf8Bytes } from '../MinifyView'

afterEach(cleanup)

describe('MinifyView', () => {
  it('counts UTF-8 bytes rather than UTF-16 code units', () => {
    expect(utf8Bytes('中;')).toBe(4)
  })

  it('renders byte savings and parser failures', () => {
    const { rerender } = render(
      <Theme>
        <MinifyView
          state={{
            status: 'ok',
            code: '中;',
            originalBytes: 8,
            minifiedBytes: 4,
          }}
        />
      </Theme>
    )
    expect(screen.getByLabelText('Minified byte statistics')).toHaveTextContent(
      '8 → 4 UTF-8 bytes · saved 50.0%'
    )

    rerender(
      <Theme>
        <MinifyView state={{ status: 'invalid', message: 'parse failed' }} />
      </Theme>
    )
    expect(screen.getByRole('alert')).toHaveTextContent('parse failed')
  })
})
