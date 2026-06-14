'use client'

import { Theme } from '@radix-ui/themes'
import { ThemeProvider as NextThemesProvider } from 'next-themes'
import type { ReactNode } from 'react'

export function PlaygroundTheme({ children }: { children: ReactNode }) {
  return (
    <NextThemesProvider attribute="class" defaultTheme="system" enableSystem>
      <Theme accentColor="blue" grayColor="slate" radius="small">
        {children}
      </Theme>
    </NextThemesProvider>
  )
}
