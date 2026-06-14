'use client'

import { Box, Button, Flex, SegmentedControl, Select } from '@radix-ui/themes'
import { compile_detail, parse } from '@gengjiawen/monkey-wasm'
import { useAtom } from 'jotai'
import { atomWithStorage } from 'jotai/utils'
import debounce from 'lodash.debounce'
import type { Plugin } from 'prettier'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { AstTreeView } from './AstTreeView'
import { Editor, type EditorHandle } from './Editor'

interface Snippet {
  label: string
  code: string
}

const snippets: Snippet[] = [
  {
    label: 'Intro',
    code: `
1 + 1;
if (true) { 10 }; 3333;
let a = [1, 2, 3];
`.trimStart(),
  },
  {
    label: 'Functions',
    code: `
let add = fn(a, b) { a + b };
let multiply = fn(a, b) { a * b };
add(2, multiply(3, 4));
`.trimStart(),
  },
  {
    label: 'Closure',
    code: `
let makeAdder = fn(x) { fn(y) { x + y } };
let addTwo = makeAdder(2);
addTwo(5);
`.trimStart(),
  },
  {
    label: 'Fibonacci',
    code: `
let fibonacci = fn(n) {
  if (n == 0) { 0 } else {
    if (n == 1) { return 1 } else {
      fibonacci(n - 1) + fibonacci(n - 2);
    }
  }
};
fibonacci(10);
`.trimStart(),
  },
  {
    label: 'Hash map',
    code: `
let person = {"name": "Anna", "age": 24};
let people = [
  {"name": "Anna", "age": 24},
  {"name": "Bob", "age": 99}
];
people[0]["name"];
`.trimStart(),
  },
]

type OutputView = 'ast' | 'bytecode'

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

const snippetIndexAtom = atomWithStorage('monkey-playground-snippet', 0)

function App() {
  const [snippetIndex, setSnippetIndex] = useAtom(snippetIndexAtom)
  const [code, setCode] = useState(snippets[snippetIndex].code)
  const [outputView, setOutputView] = useState<OutputView>('ast')
  const [astOutput, setAstOutput] = useState('')
  const [astData, setAstData] = useState<unknown | null>(null)
  const [selection, setSelection] = useState<{
    from: number
    to: number
  } | null>(null)
  const [compilerOutput, setCompilerOutput] = useState('')
  const [vimMode, setVimMode] = useState(true)
  const [isFormatting, setIsFormatting] = useState(false)
  const editorRef = useRef<EditorHandle>(null)

  const compileCode = useCallback((source: string) => {
    try {
      const astJson = parse(source)
      const ast = JSON.parse(astJson) as unknown
      setAstData(ast)
      setAstOutput(JSON.stringify(ast, null, 2))
    } catch (error) {
      const message = getErrorMessage(error)
      setAstData(null)
      setAstOutput(message)
    }

    try {
      setCompilerOutput(compile_detail(source))
    } catch (error) {
      setCompilerOutput(getErrorMessage(error))
    }
  }, [])

  const debouncedCompile = useMemo(
    () => debounce(compileCode, 200),
    [compileCode],
  )

  const editorOnChange = useCallback(
    (value: string) => {
      setCode(value)
      debouncedCompile(value)
    },
    [debouncedCompile],
  )

  const formatCode = useCallback(async () => {
    setIsFormatting(true)
    try {
      const prettier = await import('prettier/standalone')
      const monkeyPlugin =
        await import('../../prettier-plugin-monkey/src/index')
      const formatted = await prettier.format(code, {
        parser: 'monkey',
        plugins: [monkeyPlugin.default as unknown as Plugin],
      })
      setCode(formatted)
      setSelection(null)
      compileCode(formatted)
    } catch (error) {
      const message = getErrorMessage(error)
      setAstData(null)
      setAstOutput(message)
      setCompilerOutput(message)
    } finally {
      setIsFormatting(false)
    }
  }, [code, compileCode])

  useEffect(() => {
    compileCode(code)
  }, [code, compileCode])

  useEffect(() => () => debouncedCompile.cancel(), [debouncedCompile])

  useEffect(() => {
    const index = Math.min(Math.max(snippetIndex, 0), snippets.length - 1)
    if (index !== snippetIndex) {
      setSnippetIndex(index)
    }
    setSelection(null)
    setCode(snippets[index].code)
  }, [snippetIndex, setSnippetIndex])

  const handleNodeSelect = useCallback((start: number, end: number) => {
    editorRef.current?.highlightRange(start, end)
  }, [])

  return (
    <Flex className="playground-shell">
      <Flex direction="column" className="panel editor-column">
        <Flex
          className="toolbar"
          align="center"
          justify="between"
          gap="3"
          px="3"
          py="2"
        >
          <Flex align="center" gap="3">
            <Button size="2" onClick={formatCode} loading={isFormatting}>
              Format
            </Button>
            <Select.Root
              size="2"
              value={String(snippetIndex)}
              onValueChange={(value) => setSnippetIndex(Number(value))}
            >
              <Select.Trigger />
              <Select.Content>
                {snippets.map((snippet, index) => (
                  <Select.Item key={snippet.label} value={String(index)}>
                    {snippet.label}
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Root>
          </Flex>
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
          <Editor
            ref={editorRef}
            code={code}
            onChange={editorOnChange}
            onSelectionChange={setSelection}
            vimMode={vimMode}
            fill
          />
        </Box>
      </Flex>

      <Flex direction="column" className="panel output-column">
        <Flex className="toolbar" align="center" px="3" py="2">
          <SegmentedControl.Root
            size="2"
            value={outputView}
            onValueChange={(value) => setOutputView(value as OutputView)}
          >
            <SegmentedControl.Item value="ast">AST</SegmentedControl.Item>
            <SegmentedControl.Item value="bytecode">
              Bytecode
            </SegmentedControl.Item>
          </SegmentedControl.Root>
        </Flex>
        <Box className="editor-frame">
          {astData !== null ? (
            <Box className="ast-frame" hidden={outputView !== 'ast'}>
              <AstTreeView
                data={astData}
                selection={selection}
                onNodeSelect={handleNodeSelect}
              />
            </Box>
          ) : null}
          <Box hidden={outputView === 'ast' && astData !== null} className="output-editor">
            <Editor
              code={outputView === 'ast' ? astOutput : compilerOutput}
              extra={{ readOnly: true, editable: false }}
              vimMode={false}
              fill
            />
          </Box>
        </Box>
      </Flex>
    </Flex>
  )
}

export default App
