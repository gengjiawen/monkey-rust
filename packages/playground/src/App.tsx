'use client'

import { Box, Button, Flex, SegmentedControl, Tabs } from '@radix-ui/themes'
import { compile } from '@gengjiawen/monkey-wasm'
import debounce from 'lodash.debounce'
import type { Plugin } from 'prettier'
import { useCallback, useEffect, useMemo, useState } from 'react'

import { Editor } from './Editor'

const initialCode = `
1 + 1;
if (true) { 10 }; 3333;
let a = [1, 2, 3];
`.trimStart()

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function App() {
  const [code, setCode] = useState(initialCode)
  const [compilerOutput, setCompilerOutput] = useState('')
  const [vimMode, setVimMode] = useState(true)

  const compileCode = useCallback((source: string) => {
    try {
      setCompilerOutput(compile(source))
    } catch (error) {
      setCompilerOutput(getErrorMessage(error))
    }
  }, [])

  const debouncedCompile = useMemo(() => debounce(compileCode, 200), [compileCode])

  const editorOnChange = useCallback(
    (value: string) => {
      setCode(value)
      debouncedCompile(value)
    },
    [debouncedCompile],
  )

  const formatCode = useCallback(async () => {
    try {
      const prettier = await import('prettier/standalone')
      const monkeyPlugin = await import('../../prettier-plugin-monkey/src/index')
      const formatted = await prettier.format(code, {
        parser: 'monkey',
        plugins: [monkeyPlugin.default as unknown as Plugin],
      })
      setCode(formatted)
      compileCode(formatted)
    } catch (error) {
      setCompilerOutput(getErrorMessage(error))
    }
  }, [code, compileCode])

  useEffect(() => {
    compileCode(code)
  }, [code, compileCode])

  useEffect(() => () => debouncedCompile.cancel(), [debouncedCompile])

  return (
    <main className="playground-shell">
      <section className="editor-column">
        <Flex className="toolbar" align="center" justify="between" gap="3">
          <Button size="2" onClick={formatCode}>
            Format
          </Button>
          <SegmentedControl.Root
            size="2"
            value={vimMode ? 'vim' : 'plain'}
            onValueChange={(value) => setVimMode(value === 'vim')}
          >
            <SegmentedControl.Item value="vim">Vim</SegmentedControl.Item>
            <SegmentedControl.Item value="plain">Plain</SegmentedControl.Item>
          </SegmentedControl.Root>
        </Flex>
        <Box className="editor-frame">
          <Editor code={code} onChange={editorOnChange} vimMode={vimMode} />
        </Box>
      </section>

      <section className="output-column">
        <Tabs.Root defaultValue="bytecode" className="output-tabs">
          <Tabs.List className="tabs-list">
            <Tabs.Trigger value="bytecode">Bytecode</Tabs.Trigger>
          </Tabs.List>
          <Tabs.Content value="bytecode" className="output-content">
            <Editor
              code={compilerOutput}
              extra={{ readOnly: true, editable: false }}
              vimMode={false}
            />
          </Tabs.Content>
        </Tabs.Root>
      </section>
    </main>
  )
}

export default App
