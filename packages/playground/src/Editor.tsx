'use client'

import { vim } from '@replit/codemirror-vim'
import CodeMirror, { type ReactCodeMirrorProps } from '@uiw/react-codemirror'
import { useMemo } from 'react'

interface EditorProps {
  extra?: ReactCodeMirrorProps
  code?: string
  onChange?: (code: string) => void
  vimMode?: boolean
}

export function Editor({
  extra,
  code = '',
  onChange,
  vimMode = true,
}: EditorProps) {
  const extensions = useMemo(() => (vimMode ? [vim()] : []), [vimMode])

  return (
    <CodeMirror
      {...extra}
      value={code}
      height="100%"
      extensions={extensions}
      onChange={onChange}
    />
  )
}
