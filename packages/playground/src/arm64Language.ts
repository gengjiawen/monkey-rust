/**
 * CodeMirror support for the assembly pane: a tokenizer for the exact dialect
 * asm/emitter.rs produces (ELF flavor), theme-aware highlighting through the
 * Radix palette, and hover documentation backed by arm64Docs.ts.
 */

import {
  HighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
  type StreamParser,
} from '@codemirror/language'
import type { Extension } from '@codemirror/state'
import { EditorView, hoverTooltip } from '@codemirror/view'
import { tags } from '@lezer/highlight'

import { ARM64_MNEMONICS, arm64TokenAt, arm64TokenDoc } from './arm64Docs'

const registerPattern = /^(?:x(?:[12]\d|30|\d)|w(?:[12]\d|30|\d)|sp)\b/
const numberPattern = /^#?-?(?:0x[0-9a-fA-F]+|\d+)\b/
const wordPattern = /^[A-Za-z_]\w*/

/**
 * Line shapes are fixed by the emitter, so classification never needs state:
 * labels sit alone at column 0, `.L*` is always a local label, a bare word is
 * a mnemonic exactly when it is in the mnemonic set.
 */
export const arm64StreamParser: StreamParser<unknown> = {
  name: 'monkey-arm64',
  token(stream) {
    if (stream.eatSpace()) {
      return null
    }
    if (stream.match('//')) {
      stream.skipToEnd()
      return 'comment'
    }
    if (stream.match(':lo12:')) {
      return 'reloc'
    }
    if (stream.match(/^\.L[\w.]*/)) {
      return 'label'
    }
    if (stream.match(/^\.[a-z]\w*/)) {
      return 'directive'
    }
    if (stream.match(registerPattern)) {
      return 'register'
    }
    const word = stream.match(wordPattern)
    if (Array.isArray(word)) {
      const text = word[0]
      if (ARM64_MNEMONICS.has(text)) {
        return 'mnemonic'
      }
      if (text === 'lsl') {
        return 'shift'
      }
      if (text.startsWith('rt_')) {
        return 'runtime'
      }
      if (text === 'main' || text === 'g_globals') {
        return 'label'
      }
      return null
    }
    if (stream.match(numberPattern)) {
      return 'number'
    }
    stream.next()
    return null
  },
  tokenTable: {
    mnemonic: tags.keyword,
    register: tags.variableName,
    number: tags.number,
    comment: tags.lineComment,
    directive: tags.processingInstruction,
    label: tags.labelName,
    runtime: tags.function(tags.variableName),
    reloc: tags.meta,
    shift: tags.operatorKeyword,
  },
}

// Step-11 Radix colors are the accessible low-contrast text steps, so every
// token color adapts to light/dark with the rest of the playground theme.
const arm64HighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: 'var(--accent-11)', fontWeight: '500' },
  { tag: tags.variableName, color: 'var(--teal-11)' },
  { tag: tags.number, color: 'var(--amber-11)' },
  { tag: tags.labelName, color: 'var(--violet-11)' },
  { tag: tags.function(tags.variableName), color: 'var(--crimson-11)' },
  { tag: tags.processingInstruction, color: 'var(--gray-10)' },
  { tag: tags.lineComment, color: 'var(--gray-10)', fontStyle: 'italic' },
  { tag: tags.operatorKeyword, color: 'var(--accent-11)' },
  { tag: tags.meta, color: 'var(--gray-10)' },
])

const arm64DocTheme = EditorView.baseTheme({
  // The editor runs with theme="none", so it always carries CodeMirror's
  // light scope class and `&light .cm-tooltip {background: #f5f5f5}` outranks
  // a bare .cm-tooltip rule — pinning the panel light in dark mode. The
  // compound hover-host selector wins on specificity, and --gray-* vars make
  // it track the Radix light/dark theme.
  '.cm-tooltip.cm-tooltip-hover': {
    backgroundColor: 'var(--gray-2)',
    border: '1px solid var(--gray-a6)',
    borderRadius: 'var(--radius-3)',
    boxShadow: 'var(--shadow-3)',
  },
  '.cm-arm64-doc': {
    padding: '6px 10px',
    maxWidth: '26rem',
    fontSize: '12px',
    lineHeight: '1.5',
    color: 'var(--gray-12)',
  },
  '.cm-arm64-doc-title': {
    fontFamily: 'var(--code-font-family, monospace)',
    fontWeight: '600',
    marginBottom: '2px',
  },
  '.cm-arm64-doc-detail': {
    color: 'var(--gray-11)',
  },
})

const arm64HoverDocs = hoverTooltip((view, pos) => {
  const line = view.state.doc.lineAt(pos)
  const found = arm64TokenAt(line.text, pos - line.from)
  if (found === null) {
    return null
  }
  const doc = arm64TokenDoc(found.text)
  if (doc === null) {
    return null
  }
  return {
    pos: line.from + found.from,
    end: line.from + found.to,
    above: true,
    create() {
      const dom = document.createElement('div')
      dom.className = 'cm-arm64-doc'
      const title = document.createElement('div')
      title.className = 'cm-arm64-doc-title'
      title.textContent = doc.title
      const detail = document.createElement('div')
      detail.className = 'cm-arm64-doc-detail'
      detail.textContent = doc.detail
      dom.append(title, detail)
      return { dom }
    },
  }
})

/** Everything the assembly pane needs on top of the shared Editor setup. */
export const arm64EditorExtensions: Extension[] = [
  StreamLanguage.define(arm64StreamParser),
  syntaxHighlighting(arm64HighlightStyle),
  arm64HoverDocs,
  arm64DocTheme,
]
