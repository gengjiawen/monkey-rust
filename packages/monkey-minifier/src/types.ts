// AST node definitions live in `@gengjiawen/monkey-ast-types`, shared with the
// linter, the prettier plugin and the type checker. Re-exported here so the
// rest of the minifier keeps importing from one place. The helpers are listed
// by name so bundlers see a static re-export instead of having to resolve the
// CommonJS star at runtime.
export type * from '@gengjiawen/monkey-ast-types'
export {
  identifierName,
  isTypeAnnotation,
  printTypeAnnotation,
  setIdentifierName,
  tokenType,
} from '@gengjiawen/monkey-ast-types'
