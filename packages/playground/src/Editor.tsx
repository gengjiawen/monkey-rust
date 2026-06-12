'use client'

import { vim } from '@replit/codemirror-vim'
import CodeMirror, { type ReactCodeMirrorProps } from '@uiw/react-codemirror'
import { useMemo } from 'react'

interface EditorProps {
  extra?: ReactCodeMirrorProps
  code?: string
  onChange?: (code: string) => void
  vimMode?: boolean
  fill?: boolean
}

export function Editor({
  extra,
  code = '',
  onChange,
  vimMode = true,
  fill = false,
}: EditorProps) {
  const extensions = useMemo(() => (vimMode ? [vim()] : []), [vimMode])

  return (
    <CodeMirror
      {...extra}
      className={fill ? 'editor-fill' : undefined}
      value={code}
      height={fill ? undefined : '100%'}
      extensions={extensions}
      onChange={onChange}
    />
  )
}
