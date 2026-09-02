import { parse_lossless as wasmParse } from '@gengjiawen/monkey-wasm';
import type { MonkeyComment, Program, Span } from './types';

function extractLineComments(text: string): MonkeyComment[] {
  const comments: MonkeyComment[] = [];
  let index = 0;
  let inString = false;

  while (index < text.length) {
    const char = text[index];
    const nextChar = text[index + 1];

    if (inString) {
      if (char === '\\' && nextChar !== undefined) {
        index += 2;
        continue;
      }

      if (char === '"') {
        inString = false;
      }

      index += 1;
      continue;
    }

    if (char === '"') {
      inString = true;
      index += 1;
      continue;
    }

    if (char === '/' && nextChar === '/') {
      const start = index;
      index += 2;
      const valueStart = index;

      while (index < text.length && text[index] !== '\n' && text[index] !== '\r') {
        index += 1;
      }

      const end = index;
      const span: Span = { start, end };

      comments.push({
        type: 'CommentLine',
        value: text.slice(valueStart, end),
        start,
        end,
        span,
      });

      continue;
    }

    index += 1;
  }

  return comments;
}

/**
 * Map every UTF-8 byte offset in `text` to the index of the UTF-16 code unit it
 * falls in, or `null` when the source is pure ASCII and the two coincide.
 *
 * The Rust parser reports spans as byte offsets. Prettier indexes the source as
 * a JavaScript string, and so does `extractLineComments`, so a single multi-byte
 * character ahead of a node shifts its span past every comment that follows it
 * and Prettier rejects the tree with "Comment location overlaps with node
 * location".
 */
function buildByteToUnitTable(text: string): number[] | null {
  const table: number[] = [];
  let ascii = true;

  for (let index = 0; index < text.length; index += 1) {
    const codePoint = text.codePointAt(index) as number;
    const width =
      codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;

    if (width > 1) {
      ascii = false;
    }

    // Every byte of a character maps back to where that character starts;
    // spans only ever land on character boundaries.
    for (let byte = 0; byte < width; byte += 1) {
      table.push(index);
    }

    if (codePoint > 0xffff) {
      // A surrogate pair is one code point but two code units.
      index += 1;
    }
  }

  // One past the end, so a span that ends at EOF still resolves.
  table.push(text.length);

  return ascii ? null : table;
}

/** Rewrite every `span` in the tree from byte offsets to code-unit indices. */
function remapSpans(value: unknown, table: number[]): void {
  if (Array.isArray(value)) {
    for (const item of value) {
      remapSpans(item, table);
    }
    return;
  }

  if (!value || typeof value !== 'object') {
    return;
  }

  const node = value as Record<string, unknown>;
  const span = node.span as Span | undefined;

  if (span && typeof span.start === 'number' && typeof span.end === 'number') {
    span.start = table[span.start] ?? table[table.length - 1];
    span.end = table[span.end] ?? table[table.length - 1];
  }

  for (const key of Object.keys(node)) {
    if (key !== 'span') {
      remapSpans(node[key], table);
    }
  }
}

export function parse(text: string, options: any): Program {
  try {
    // The lossless entry keeps integer literals as their source text; the plain
    // `parse` export has already rounded them through a JavaScript number, which
    // rewrites anything past 2^53 into a different literal.
    const astJson = wasmParse(text);
    const ast = JSON.parse(astJson);

    // The WASM parse returns a Node enum wrapper, extract the Program
    const program = (ast.Program ?? ast) as Program;

    const table = buildByteToUnitTable(text);
    if (table) {
      remapSpans(program, table);
    }

    const comments = extractLineComments(text);
    if (comments.length > 0) {
      program.comments = comments;
    }

    return program;
  } catch (error) {
    if (error instanceof Error) {
      throw new SyntaxError(`Monkey parse error: ${error.message}`);
    }
    throw error;
  }
}

export function locStart(node: any): number {
  return node.span?.start ?? node.start ?? node.loc?.start?.offset ?? 0;
}

export function locEnd(node: any): number {
  return node.span?.end ?? node.end ?? node.loc?.end?.offset ?? 0;
}
