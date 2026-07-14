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
    <html lang="en" suppressHydrationWarning className="h-full">
      <body className="m-0 h-full">
        <PlaygroundTheme>
          <div className="flex h-dvh flex-col overflow-hidden">
            <Header />
            {children}
          </div>
        </PlaygroundTheme>
      </body>
    </html>
  )
}
