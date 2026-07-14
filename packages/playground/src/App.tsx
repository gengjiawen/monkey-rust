'use client'

import { Button, SegmentedControl, Select } from '@radix-ui/themes'
import { compile_with_debug, parse } from '@gengjiawen/monkey-wasm'
import { useAtom } from 'jotai'
import { atomWithStorage } from 'jotai/utils'
import debounce from 'lodash.debounce'
import type { Plugin } from 'prettier'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { AstTreeView } from './AstTreeView'
import { type BytecodeDebugView, spanForBytecodeCursor } from './bytecodeDebug'
import { Editor, type EditorHandle } from './Editor'
import { GcReportView, type GcPanelState } from './GcReportView'
import type { SourceSpan } from './gcReport'
import { runGc } from './gcRunner'

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
  {
    label: 'Class cycle (GC)',
    code: `
class Node {
  constructor(value) {
    this.value = value;
  }

  connect(other) {
    this.next = other;
  }
}

let makeCycle = fn() {
  let a = new Node("a");
  let b = new Node("b");
  a.connect(b);
  b.connect(a);
};

makeCycle();
`.trimStart(),
  },
]

type OutputView = 'ast' | 'bytecode' | 'gc'

const panelClass =
  'flex min-h-0 min-w-0 h-full flex-col overflow-hidden bg-(--color-background)'

const toolbarClass =
  'flex shrink-0 items-center justify-between gap-3 border-b border-(--gray-a5) bg-(--color-background) px-3 py-2'

const editorFrameClass =
  'flex min-h-0 flex-1 flex-col overflow-hidden bg-(--color-background)'

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
  const [bytecodeDebugView, setBytecodeDebugView] =
    useState<BytecodeDebugView | null>(null)
  const [vimMode, setVimMode] = useState(true)
  const [isFormatting, setIsFormatting] = useState(false)
  const [gcState, setGcState] = useState<GcPanelState>({ status: 'idle' })
  const gcRequestId = useRef(0)
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
      const debugJson = compile_with_debug(source)
      const view = JSON.parse(debugJson) as BytecodeDebugView
      setBytecodeDebugView(view)
      setCompilerOutput(view.detail)
    } catch (error) {
      setBytecodeDebugView(null)
      setCompilerOutput(getErrorMessage(error))
    }
  }, [])

  const debouncedCompile = useMemo(
    () => debounce(compileCode, 200),
    [compileCode]
  )

  const editorOnChange = useCallback((value: string) => {
    setCode(value)
    gcRequestId.current += 1
    setGcState({ status: 'idle' })
  }, [])

  const formatCode = useCallback(async () => {
    setIsFormatting(true)
    try {
      const prettier = await import('prettier/standalone')
      const monkeyPlugin = await import(
        '../../prettier-plugin-monkey/src/index'
      )
      const formatted = await prettier.format(code, {
        parser: 'monkey',
        plugins: [monkeyPlugin.default as unknown as Plugin],
      })
      setCode(formatted)
      setSelection(null)
      gcRequestId.current += 1
      setGcState({ status: 'idle' })
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
    debouncedCompile(code)
  }, [code, debouncedCompile])

  useEffect(() => () => debouncedCompile.cancel(), [debouncedCompile])

  useEffect(() => {
    const index = Math.min(Math.max(snippetIndex, 0), snippets.length - 1)
    if (index !== snippetIndex) {
      setSnippetIndex(index)
    }
    setSelection(null)
    gcRequestId.current += 1
    setGcState({ status: 'idle' })
    setCode(snippets[index].code)
  }, [snippetIndex, setSnippetIndex])

  useEffect(
    () => () => {
      gcRequestId.current += 1
    },
    []
  )

  const runGarbageCollector = useCallback(async () => {
    const requestId = gcRequestId.current + 1
    gcRequestId.current = requestId
    setGcState({ status: 'running' })

    try {
      const result = await runGc(code)
      if (gcRequestId.current === requestId) {
        setGcState(result)
      }
    } catch (error) {
      if (gcRequestId.current === requestId) {
        setGcState({
          status: 'invalid',
          message: getErrorMessage(error),
        })
      }
    }
  }, [code])

  const handleNodeSelect = useCallback((start: number, end: number) => {
    editorRef.current?.highlightRange(start, end)
  }, [])

  const handleGcErrorSpanSelect = useCallback((span: SourceSpan) => {
    editorRef.current?.highlightRange(span.start, span.end)
  }, [])

  const handleBytecodeSelection = useCallback(
    (selection: { from: number; to: number }) => {
      if (bytecodeDebugView == null) {
        editorRef.current?.clearHighlight()
        return
      }

      const span = spanForBytecodeCursor(bytecodeDebugView, selection.from)
      if (span == null) {
        editorRef.current?.clearHighlight()
        return
      }

      editorRef.current?.highlightRange(span.start, span.end)
    },
    [bytecodeDebugView]
  )

  useEffect(() => {
    if (outputView !== 'bytecode') {
      editorRef.current?.clearHighlight()
    }
  }, [outputView])

  useEffect(() => {
    if (
      outputView !== 'gc' ||
      gcState.status !== 'error' ||
      gcState.span === null
    ) {
      return
    }
    const { span } = gcState
    const editor = editorRef.current
    editor?.highlightRange(span.start, span.end)
    return () => {
      editor?.clearHighlight()
    }
  }, [gcState, outputView])

  return (
    <div className="grid min-h-0 flex-1 grid-cols-2 overflow-hidden max-[780px]:grid-cols-1 max-[780px]:grid-rows-2">
      <div
        className={`${panelClass} border-r border-(--gray-a5) max-[780px]:border-r-0 max-[780px]:border-b`}
      >
        <div className={toolbarClass}>
          <div className="flex items-center gap-3">
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
          </div>
          <SegmentedControl.Root
            size="2"
            value={vimMode ? 'vim' : 'plain'}
            onValueChange={(value) => setVimMode(value === 'vim')}
          >
            <SegmentedControl.Item value="vim">Vim</SegmentedControl.Item>
            <SegmentedControl.Item value="plain">Plain</SegmentedControl.Item>
          </SegmentedControl.Root>
        </div>
        <div className={editorFrameClass}>
          <Editor
            ref={editorRef}
            code={code}
            onChange={editorOnChange}
            onSelectionChange={setSelection}
            vimMode={vimMode}
            fill
          />
        </div>
      </div>

      <div className={panelClass}>
        <div className={toolbarClass}>
          <SegmentedControl.Root
            size="2"
            value={outputView}
            onValueChange={(value) => setOutputView(value as OutputView)}
          >
            <SegmentedControl.Item value="ast">AST</SegmentedControl.Item>
            <SegmentedControl.Item value="bytecode">
              Bytecode
            </SegmentedControl.Item>
            <SegmentedControl.Item value="gc">GC</SegmentedControl.Item>
          </SegmentedControl.Root>
          {outputView === 'gc' ? (
            <Button
              size="2"
              onClick={runGarbageCollector}
              loading={gcState.status === 'running'}
            >
              Run GC
            </Button>
          ) : null}
        </div>
        <div className={editorFrameClass}>
          {outputView === 'ast' && astData !== null ? (
            <div className="min-h-0 flex-1 overflow-auto bg-(--color-background) px-2.5 pt-2 pb-4">
              <AstTreeView
                data={astData}
                selection={selection}
                onNodeSelect={handleNodeSelect}
              />
            </div>
          ) : null}
          {outputView === 'gc' ? (
            <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) bg-[image:radial-gradient(circle_at_top_right,var(--accent-a3),transparent_34%)] p-4.5">
              <GcReportView
                state={gcState}
                onErrorSpanSelect={handleGcErrorSpanSelect}
              />
            </div>
          ) : null}
          {outputView === 'bytecode' ||
          (outputView === 'ast' && astData === null) ? (
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
              <Editor
                code={outputView === 'ast' ? astOutput : compilerOutput}
                extra={{ readOnly: true, editable: false }}
                onSelectionChange={
                  outputView === 'bytecode'
                    ? handleBytecodeSelection
                    : undefined
                }
                vimMode={false}
                fill
              />
            </div>
          ) : null}
        </div>
      </div>
    </div>
  )
}

export default App
