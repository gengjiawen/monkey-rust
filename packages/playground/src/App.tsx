'use client'

import { Button, SegmentedControl, Select, Switch } from '@radix-ui/themes'
import { compile_with_debug, parse } from '@gengjiawen/monkey-wasm'
import { useAtom } from 'jotai'
import { atomWithStorage } from 'jotai/utils'
import debounce from 'lodash.debounce'
import type { Plugin } from 'prettier'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { arm64RangesForSourceOffset, spanForArm64Cursor } from './arm64'
import { buildArm64 } from './arm64Runner'
import { Arm64View, type Arm64BuildState } from './Arm64View'
import { AstTreeView } from './AstTreeView'
import { type BytecodeDebugView, spanForBytecodeCursor } from './bytecodeDebug'
import { Editor, type EditorHandle } from './Editor'
import { GcReportView, type GcPanelState } from './GcReportView'
import type { SourceSpan } from './gcReport'
import { runGc } from './gcRunner'
import { MinifyView, type MinifyState, utf8Bytes } from './MinifyView'
import { utf16OffsetToUtf8Byte, utf8ByteSpanToUtf16 } from './sourceSpan'
import {
  SnapshotView,
  type SnapshotBuildState,
  type SnapshotRunState,
} from './SnapshotView'
import { buildSnapshot, runSnapshot } from './snapshotRunner'

interface Snippet {
  label: string
  code: string
}

const snippets: Snippet[] = [
  {
    label: 'Intro',
    code: `
let a = 1 + 1;
print(a)
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

type OutputView = 'ast' | 'bytecode' | 'gc' | 'snapshot' | 'arm64' | 'minify'

const panelClass =
  'flex min-h-0 min-w-0 h-full flex-col overflow-hidden bg-(--color-background)'

const toolbarClass =
  'flex shrink-0 items-center gap-3 border-b border-(--gray-a5) bg-(--color-background) px-3 py-2'

const editorToolbarClass = `${toolbarClass} justify-between`

const outputToolbarClass = `${toolbarClass} flex-wrap`

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
  const [stripDebug, setStripDebug] = useState(false)
  const [snapshotBuild, setSnapshotBuild] = useState<SnapshotBuildState>({
    status: 'idle',
  })
  const [snapshotRun, setSnapshotRun] = useState<SnapshotRunState>({
    status: 'idle',
  })
  const [snapshotStale, setSnapshotStale] = useState(false)
  const snapshotRunRequestId = useRef(0)
  const [arm64Build, setArm64Build] = useState<Arm64BuildState>({
    status: 'idle',
  })
  const [minifyState, setMinifyState] = useState<MinifyState>({
    status: 'idle',
  })
  const [mangleNames, setMangleNames] = useState(true)
  const minifyRequestId = useRef(0)
  const minifyActive = useRef(false)
  const editorRef = useRef<EditorHandle>(null)
  const arm64EditorRef = useRef<EditorHandle>(null)
  const latestCode = useRef(code)

  useEffect(() => {
    latestCode.current = code
  }, [code])

  const astSelection = useMemo(
    () =>
      selection === null
        ? null
        : {
            from: utf16OffsetToUtf8Byte(code, selection.from),
            to: utf16OffsetToUtf8Byte(code, selection.to),
          },
    [code, selection]
  )

  // Keep the last build mounted and flag it stale instead of unmounting the
  // panel: unmounting would reset the output scroll position and drop keyboard
  // focus from the strip toggle on every rebuild.
  const invalidateSnapshot = useCallback(() => {
    snapshotRunRequestId.current += 1
    setSnapshotRun({ status: 'idle' })
    setSnapshotStale(true)
  }, [])

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

  const compileSnapshot = useCallback(
    (source: string, shouldStripDebug: boolean) => {
      try {
        setSnapshotBuild(buildSnapshot(source, shouldStripDebug))
      } catch (error) {
        setSnapshotBuild({
          status: 'invalid',
          message: getErrorMessage(error),
        })
      }
      setSnapshotStale(false)
    },
    []
  )

  const debouncedCompile = useMemo(
    () => debounce(compileCode, 200),
    [compileCode]
  )

  const debouncedSnapshotCompile = useMemo(
    () => debounce(compileSnapshot, 200),
    [compileSnapshot]
  )

  const compileArm64 = useCallback((source: string) => {
    try {
      setArm64Build(buildArm64(source))
    } catch (error) {
      setArm64Build({
        status: 'invalid',
        message: getErrorMessage(error),
      })
    }
  }, [])

  const debouncedArm64Compile = useMemo(
    () => debounce(compileArm64, 200),
    [compileArm64]
  )

  const minifyCode = useCallback(
    async (source: string, shouldMangle: boolean, requestId: number) => {
      try {
        const { minify } = await import('../../monkey-minifier/src/index')
        const result = minify(source, { mangle: shouldMangle })
        if (
          minifyRequestId.current !== requestId ||
          latestCode.current !== source ||
          !minifyActive.current
        ) {
          return
        }
        setMinifyState({
          status: 'ok',
          code: result.code,
          originalBytes: utf8Bytes(source),
          minifiedBytes: utf8Bytes(result.code),
        })
      } catch (error) {
        if (
          minifyRequestId.current === requestId &&
          latestCode.current === source &&
          minifyActive.current
        ) {
          setMinifyState({
            status: 'invalid',
            message: getErrorMessage(error),
          })
        }
      }
    },
    []
  )

  const debouncedMinify = useMemo(() => debounce(minifyCode, 200), [minifyCode])

  const editorOnChange = useCallback(
    (value: string) => {
      // CodeMirror reports a document change even when the replacement text is
      // identical (e.g. typing over a selection with the same character);
      // setCode would bail on the equal string, so invalidating here would
      // strand the snapshot as stale with no rebuild ever scheduled.
      if (value === code) {
        return
      }
      setCode(value)
      minifyRequestId.current += 1
      gcRequestId.current += 1
      setGcState({ status: 'idle' })
      invalidateSnapshot()
    },
    [code, invalidateSnapshot]
  )

  const handleOutputViewChange = useCallback((value: string) => {
    const nextView = value as OutputView
    if (nextView !== 'minify') {
      minifyActive.current = false
      minifyRequestId.current += 1
    }
    setOutputView(nextView)
  }, [])

  const handleMangleChange = useCallback((checked: boolean) => {
    minifyRequestId.current += 1
    setMangleNames(checked)
  }, [])

  const handleStripDebugChange = useCallback(
    (nextStripDebug: boolean) => {
      if (nextStripDebug === stripDebug) {
        return
      }
      invalidateSnapshot()
      setStripDebug(nextStripDebug)
    },
    [invalidateSnapshot, stripDebug]
  )

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
      if (latestCode.current !== code) {
        // The source was edited while prettier was loading; applying this
        // result would silently revert those edits.
        return
      }
      if (formatted === code) {
        return
      }
      setCode(formatted)
      minifyRequestId.current += 1
      setSelection(null)
      gcRequestId.current += 1
      setGcState({ status: 'idle' })
      invalidateSnapshot()
      compileCode(formatted)
    } catch (error) {
      const message = getErrorMessage(error)
      setAstData(null)
      setAstOutput(message)
      setCompilerOutput(message)
    } finally {
      setIsFormatting(false)
    }
  }, [code, compileCode, invalidateSnapshot])

  useEffect(() => {
    debouncedCompile(code)
  }, [code, debouncedCompile])

  useEffect(() => () => debouncedCompile.cancel(), [debouncedCompile])

  useEffect(() => {
    if (outputView !== 'snapshot') {
      debouncedSnapshotCompile.cancel()
      return
    }

    debouncedSnapshotCompile(code, stripDebug)
    return () => debouncedSnapshotCompile.cancel()
  }, [code, debouncedSnapshotCompile, outputView, stripDebug])

  useEffect(() => {
    if (outputView !== 'arm64') {
      debouncedArm64Compile.cancel()
      return
    }

    debouncedArm64Compile(code)
    return () => debouncedArm64Compile.cancel()
  }, [code, debouncedArm64Compile, outputView])

  useEffect(() => {
    minifyActive.current = outputView === 'minify'
    minifyRequestId.current += 1
    const requestId = minifyRequestId.current
    if (outputView !== 'minify') {
      debouncedMinify.cancel()
      return
    }

    setMinifyState({ status: 'idle' })
    debouncedMinify(code, mangleNames, requestId)
    return () => debouncedMinify.cancel()
  }, [code, debouncedMinify, mangleNames, outputView])

  useEffect(() => {
    const index = Math.min(Math.max(snippetIndex, 0), snippets.length - 1)
    if (index !== snippetIndex) {
      setSnippetIndex(index)
    }
    setSelection(null)
    gcRequestId.current += 1
    setGcState({ status: 'idle' })
    invalidateSnapshot()
    minifyRequestId.current += 1
    setCode(snippets[index].code)
  }, [invalidateSnapshot, snippetIndex, setSnippetIndex])

  useEffect(
    () => () => {
      gcRequestId.current += 1
      minifyActive.current = false
      minifyRequestId.current += 1
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

  const runSnapshotBytes = useCallback(async () => {
    if (snapshotBuild.status !== 'ok' || snapshotStale) {
      return
    }
    const requestId = snapshotRunRequestId.current + 1
    snapshotRunRequestId.current = requestId
    setSnapshotRun({ status: 'running' })

    try {
      const result = await runSnapshot(snapshotBuild.bytes)
      if (snapshotRunRequestId.current === requestId) {
        setSnapshotRun(result)
      }
    } catch (error) {
      if (snapshotRunRequestId.current === requestId) {
        setSnapshotRun({
          status: 'invalid',
          message: getErrorMessage(error),
        })
      }
    }
  }, [snapshotBuild, snapshotStale])

  const highlightSourceSpan = useCallback(
    (span: SourceSpan) => {
      const converted = utf8ByteSpanToUtf16(code, span)
      editorRef.current?.highlightRange(converted.start, converted.end)
    },
    [code]
  )

  const handleNodeSelect = useCallback(
    (start: number, end: number) => {
      highlightSourceSpan({ start, end })
    },
    [highlightSourceSpan]
  )

  const handleErrorSpanSelect = highlightSourceSpan

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

      highlightSourceSpan(span)
    },
    [bytecodeDebugView, highlightSourceSpan]
  )

  const handleArm64Selection = useCallback(
    (selection: { from: number; to: number }) => {
      if (arm64Build.status !== 'ok') {
        editorRef.current?.clearHighlight()
        return
      }

      const span = spanForArm64Cursor(arm64Build, selection.from)
      if (span == null) {
        editorRef.current?.clearHighlight()
        return
      }

      highlightSourceSpan(span)
    },
    [arm64Build, highlightSourceSpan]
  )

  // Source → assembly half of the arm64 linkage: light up the lines lowered
  // from the narrowest span around the source cursor.
  useEffect(() => {
    if (outputView !== 'arm64' || arm64Build.status !== 'ok') {
      return
    }

    if (selection === null) {
      arm64EditorRef.current?.clearHighlight()
      return
    }

    const byteOffset = utf16OffsetToUtf8Byte(code, selection.from)
    const ranges = arm64RangesForSourceOffset(arm64Build, byteOffset)
    if (ranges.length === 0) {
      arm64EditorRef.current?.clearHighlight()
      return
    }

    arm64EditorRef.current?.highlightRanges(ranges)
  }, [arm64Build, code, outputView, selection])

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
    highlightSourceSpan(span)
    return () => {
      editor?.clearHighlight()
    }
  }, [gcState, highlightSourceSpan, outputView])

  useEffect(() => {
    if (
      outputView !== 'snapshot' ||
      snapshotRun.status !== 'error' ||
      snapshotRun.span === null
    ) {
      return
    }
    const { span } = snapshotRun
    const editor = editorRef.current
    highlightSourceSpan(span)
    return () => {
      editor?.clearHighlight()
    }
  }, [highlightSourceSpan, snapshotRun, outputView])

  return (
    <div className="grid min-h-0 flex-1 grid-cols-2 overflow-hidden max-[780px]:grid-cols-1 max-[780px]:grid-rows-2">
      <div
        className={`${panelClass} border-r border-(--gray-a5) max-[780px]:border-r-0 max-[780px]:border-b`}
      >
        <div className={editorToolbarClass}>
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
        <div className={outputToolbarClass}>
          <SegmentedControl.Root
            className="max-w-full min-w-0!"
            size="2"
            value={outputView}
            onValueChange={handleOutputViewChange}
          >
            <SegmentedControl.Item value="ast">AST</SegmentedControl.Item>
            <SegmentedControl.Item value="bytecode">
              Bytecode
            </SegmentedControl.Item>
            <SegmentedControl.Item value="gc">GC</SegmentedControl.Item>
            <SegmentedControl.Item value="snapshot">
              Snapshot
            </SegmentedControl.Item>
            <SegmentedControl.Item value="arm64">ARM64</SegmentedControl.Item>
            <SegmentedControl.Item value="minify">Minify</SegmentedControl.Item>
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
          {outputView === 'snapshot' ? (
            <Button
              size="2"
              onClick={runSnapshotBytes}
              disabled={snapshotBuild.status !== 'ok' || snapshotStale}
              loading={snapshotRun.status === 'running'}
            >
              Run snapshot
            </Button>
          ) : null}
          {outputView === 'minify' ? (
            <label
              htmlFor="mangle-names"
              className="flex items-center gap-2 text-xs text-(--gray-11)"
            >
              <Switch
                id="mangle-names"
                size="1"
                checked={mangleNames}
                onCheckedChange={handleMangleChange}
              />
              Mangle names
            </label>
          ) : null}
        </div>
        <div className={editorFrameClass}>
          {outputView === 'ast' && astData !== null ? (
            <div className="min-h-0 flex-1 overflow-auto bg-(--color-background) px-2.5 pt-2 pb-4">
              <AstTreeView
                data={astData}
                selection={astSelection}
                onNodeSelect={handleNodeSelect}
              />
            </div>
          ) : null}
          {outputView === 'gc' ? (
            <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) bg-[image:radial-gradient(circle_at_top_right,var(--accent-a3),transparent_34%)] p-4.5">
              <GcReportView
                state={gcState}
                onErrorSpanSelect={handleErrorSpanSelect}
              />
            </div>
          ) : null}
          {outputView === 'snapshot' ? (
            <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) p-4.5">
              <SnapshotView
                build={snapshotBuild}
                run={snapshotRun}
                stale={snapshotStale}
                stripDebug={stripDebug}
                onStripDebugChange={handleStripDebugChange}
                onErrorSpanSelect={handleErrorSpanSelect}
              />
            </div>
          ) : null}
          {outputView === 'arm64' ? (
            <Arm64View
              build={arm64Build}
              editorRef={arm64EditorRef}
              onSelectionChange={handleArm64Selection}
              onErrorSpanSelect={handleErrorSpanSelect}
            />
          ) : null}
          {outputView === 'minify' ? <MinifyView state={minifyState} /> : null}
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
