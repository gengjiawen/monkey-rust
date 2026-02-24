import type { Parser, Printer } from 'prettier';
import { parse, locStart, locEnd } from './parser';
import { print, canAttachComment, isBlockComment, printComment } from './printer';
import { languages } from './language';
import { options } from './options';

const monkeyParser: Parser = {
  parse,
  astFormat: 'monkey-ast',
  locStart,
  locEnd,
};

const monkeyPrinter: Printer = {
  print,
  canAttachComment,
  isBlockComment,
  printComment,
};

export const parsers = {
  monkey: monkeyParser,
};

export const printers = {
  'monkey-ast': monkeyPrinter,
};

export { languages, options };

// Default export for plugin
export default {
  languages,
  parsers,
  printers,
  options,
};
