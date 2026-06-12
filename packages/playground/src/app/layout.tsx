import '@radix-ui/themes/styles.css'
import './globals.css'

import { Theme } from '@radix-ui/themes'
import type { Metadata } from 'next'
import type { ReactNode } from 'react'

export const metadata: Metadata = {
  title: 'Monkey Playground',
  description: 'Compile and format Monkey language snippets in the browser.',
}

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <Theme accentColor="blue" grayColor="slate" radius="small">
          {children}
        </Theme>
      </body>
    </html>
  )
}
