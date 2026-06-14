'use client'

import { useEffect, useMemo, useRef, type MouseEvent } from 'react'

interface AstTreeViewProps {
  data: unknown
  selection: { from: number; to: number } | null
  onNodeSelect?: (start: number, end: number) => void
}

interface SourceSpan {
  start: number
  end: number
}

interface NodeMatch {
  path: string[]
  depth: number
  span: number
}

interface VisitResult {
  best: NodeMatch | null
  start: number
  end: number
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !ArrayBuffer.isView(value)
}

function getSourceSpan(value: unknown): SourceSpan | null {
  if (!isRecord(value) || !isRecord(value.span)) {
    return null
  }

  const { start, end } = value.span
  if (typeof start !== 'number' || typeof end !== 'number') {
    return null
  }

  return { start, end }
}

function getNodeType(value: Record<string, unknown>): string | null {
  if (typeof value.type === 'string') {
    return value.type
  }

  if (isRecord(value.kind) && typeof value.kind.type === 'string') {
    return `Token(${value.kind.type})`
  }

  return null
}

function isSelectionWithinNode(
  selFrom: number,
  selTo: number,
  nodeStart: number,
  nodeEnd: number,
): boolean {
  if (nodeEnd <= nodeStart) {
    return selFrom === selTo && selFrom === nodeStart
  }
  if (selFrom === selTo) {
    return selFrom >= nodeStart && selFrom < nodeEnd
  }
  return selFrom >= nodeStart && selTo <= nodeEnd
}

function findNodePathForSelection(
  data: unknown,
  from: number,
  to: number,
): string[] | null {
  const selFrom = Math.min(from, to)
  const selTo = Math.max(from, to)

  const pickBetter = (
    current: NodeMatch | null,
    next: NodeMatch | null,
  ): NodeMatch | null => {
    if (!current) return next
    if (!next) return current
    if (next.span !== current.span)
      return next.span < current.span ? next : current
    if (next.depth !== current.depth)
      return next.depth > current.depth ? next : current
    return current
  }

  const visit = (value: unknown, path: string[]): VisitResult => {
    if (!isRecord(value)) {
      return {
        best: null,
        start: Number.POSITIVE_INFINITY,
        end: Number.NEGATIVE_INFINITY,
      }
    }

    if (Array.isArray(value)) {
      let best: NodeMatch | null = null
      let start = Number.POSITIVE_INFINITY
      let end = Number.NEGATIVE_INFINITY

      for (let index = 0; index < value.length; index++) {
        const child = visit(value[index], [...path, String(index)])
        best = pickBetter(best, child.best)
        start = Math.min(start, child.start)
        end = Math.max(end, child.end)
      }

      return { best, start, end }
    }

    const sourceSpan = getSourceSpan(value)
    let best: NodeMatch | null = null
    let start = sourceSpan?.start ?? Number.POSITIVE_INFINITY
    let end = sourceSpan?.end ?? Number.NEGATIVE_INFINITY

    for (const key of Object.keys(value)) {
      const child = visit(value[key], [...path, key])
      best = pickBetter(best, child.best)
      start = Math.min(start, child.start)
      end = Math.max(end, child.end)
    }

    if (
      sourceSpan &&
      Number.isFinite(start) &&
      Number.isFinite(end) &&
      isSelectionWithinNode(selFrom, selTo, start, end)
    ) {
      best = pickBetter(best, {
        path,
        depth: path.length,
        span: Math.max(0, end - start),
      })
    }

    return { best, start, end }
  }

  return visit(data, []).best?.path ?? null
}

export function AstTreeView({
  data,
  selection,
  onNodeSelect,
}: AstTreeViewProps) {
  const activePath = useMemo(() => {
    if (selection == null) return null
    return findNodePathForSelection(data, selection.from, selection.to)
  }, [data, selection])

  return (
    <div className="ast-tree">
      <TreeNode
        value={data}
        depth={0}
        activePath={activePath}
        pathIndex={0}
        onNodeSelect={onNodeSelect}
      />
    </div>
  )
}

interface TreeNodeProps {
  label?: string
  value: unknown
  depth: number
  activePath: string[] | null
  pathIndex: number
  onNodeSelect?: (start: number, end: number) => void
}

function TreeNode({
  label,
  value,
  depth,
  activePath,
  pathIndex,
  onNodeSelect,
}: TreeNodeProps) {
  const isTarget = activePath !== null && pathIndex === activePath.length
  const isOnPath = activePath !== null && pathIndex < activePath.length
  const detailsRef = useRef<HTMLDetailsElement>(null)
  const summaryRef = useRef<HTMLElement>(null)

  useEffect(() => {
    if (isOnPath && detailsRef.current) {
      detailsRef.current.open = true
    }
  })

  useEffect(() => {
    if (isTarget && summaryRef.current) {
      summaryRef.current.scrollIntoView({ block: 'nearest' })
    }
  }, [isTarget])

  if (value === null || value === undefined) {
    return (
      <div className="tree-leaf">
        {label && <span className="tree-key">{label}: </span>}
        <span className="tree-null">null</span>
      </div>
    )
  }

  if (typeof value === 'bigint') {
    return (
      <div className="tree-leaf">
        {label && <span className="tree-key">{label}: </span>}
        <span className="tree-number">{String(value)}n</span>
      </div>
    )
  }

  if (typeof value === 'boolean' || typeof value === 'number') {
    return (
      <div className="tree-leaf">
        {label && <span className="tree-key">{label}: </span>}
        <span className="tree-number">{String(value)}</span>
      </div>
    )
  }

  if (typeof value === 'string') {
    return (
      <div className="tree-leaf">
        {label && <span className="tree-key">{label}: </span>}
        <span className="tree-string">"{value}"</span>
      </div>
    )
  }

  if (value instanceof Uint8Array) {
    return (
      <div className="tree-leaf">
        {label && <span className="tree-key">{label}: </span>}
        <span className="tree-null">[bytes({value.length})]</span>
      </div>
    )
  }

  if (Array.isArray(value)) {
    if (value.length === 0) {
      return (
        <div className="tree-leaf">
          {label && <span className="tree-key">{label}: </span>}
          <span className="tree-null">[]</span>
        </div>
      )
    }

    const nextKey = activePath?.[pathIndex]

    return (
      <details ref={detailsRef} open={depth < 2}>
        <summary
          ref={summaryRef}
          className={`tree-summary ${isOnPath ? 'tree-on-path' : ''}`}
        >
          {label && <span className="tree-key">{label}: </span>}
          <span className="tree-bracket">[{value.length}]</span>
        </summary>
        <div className="tree-children">
          {value.map((item, index) => {
            const childOnPath = nextKey === String(index)
            return (
              <TreeNode
                key={index}
                label={String(index)}
                value={item}
                depth={depth + 1}
                activePath={childOnPath ? activePath : null}
                pathIndex={childOnPath ? pathIndex + 1 : 0}
                onNodeSelect={onNodeSelect}
              />
            )
          })}
        </div>
      </details>
    )
  }

  if (typeof value === 'object') {
    const obj = value as Record<string, unknown>
    const keys = Object.keys(obj)
    const sourceSpan = getSourceSpan(obj)
    const typeName = getNodeType(obj)

    const handleClick = (event: MouseEvent) => {
      if (!sourceSpan || !onNodeSelect) return
      event.stopPropagation()
      onNodeSelect(sourceSpan.start, sourceSpan.end)
    }

    const nextKey = activePath?.[pathIndex]
    const summaryClassName = [
      'tree-summary',
      sourceSpan ? 'tree-clickable' : '',
      isOnPath ? 'tree-on-path' : '',
      isTarget ? 'tree-active' : '',
    ]
      .filter(Boolean)
      .join(' ')

    return (
      <details ref={detailsRef} open={depth < 2}>
        <summary
          ref={summaryRef}
          className={summaryClassName}
          onClick={handleClick}
        >
          {label && <span className="tree-key">{label}: </span>}
          {typeName ? (
            <span className="tree-type">{typeName}</span>
          ) : (
            <span className="tree-bracket">{`{${keys.length}}`}</span>
          )}
        </summary>
        <div className="tree-children">
          {keys.map((key) => {
            const childOnPath = nextKey === key
            return (
              <TreeNode
                key={key}
                label={key}
                value={obj[key]}
                depth={depth + 1}
                activePath={childOnPath ? activePath : null}
                pathIndex={childOnPath ? pathIndex + 1 : 0}
                onNodeSelect={onNodeSelect}
              />
            )
          })}
        </div>
      </details>
    )
  }

  return (
    <div className="tree-leaf">
      {label && <span className="tree-key">{label}: </span>}
      <span>{String(value)}</span>
    </div>
  )
}
