import '@radix-ui/themes/styles.css'
import './globals.css'

import { Theme } from '@radix-ui/themes'
import type { Metadata } from 'next'
import type { ReactNode } from 'react'

import { Header } from '@/components/Header'

export const metadata: Metadata = {
  title: 'Monkey Playground',
  description: 'Compile and format Monkey language snippets in the browser.',
}

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <Theme accentColor="blue" grayColor="slate" radius="small">
          <div className="playground-root">
            <Header />
            {children}
          </div>
        </Theme>
      </body>
    </html>
  )
}
