import React from 'react';
import CodeMirror, { ReactCodeMirrorProps } from '@uiw/react-codemirror';
import { vim } from '@replit/codemirror-vim';

interface EditorProps {
  extra?: ReactCodeMirrorProps,
  code?: string,
  onChange?: (code: string) => void;
}

export function Editor(editorProps? : EditorProps) {
  return (
    <CodeMirror
      {...editorProps?.extra}
      value= { editorProps?.code}
      height="100%"
      extensions={[vim()]}
      onChange={editorProps?.onChange}
    />
  );
}
