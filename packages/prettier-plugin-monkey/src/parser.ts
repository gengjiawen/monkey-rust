import { parse as wasmParse } from '@gengjiawen/monkey-wasm';
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

export function parse(text: string, options: any): Program {
  try {
    const astJson = wasmParse(text);
    const ast = JSON.parse(astJson);

    // The WASM parse returns a Node enum wrapper, extract the Program
    const program = (ast.Program ?? ast) as Program;
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
