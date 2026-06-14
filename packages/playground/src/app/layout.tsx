import '@radix-ui/themes/styles.css'
import './globals.css'

import type { Metadata } from 'next'
import type { ReactNode } from 'react'

import { Header } from '@/components/Header'
import { PlaygroundTheme } from '@/components/PlaygroundTheme'

export const metadata: Metadata = {
  title: 'Monkey Playground',
  description: 'Compile and format Monkey language snippets in the browser.',
}

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <PlaygroundTheme>
          <div className="playground-root">
            <Header />
            {children}
          </div>
        </PlaygroundTheme>
      </body>
    </html>
  )
}
