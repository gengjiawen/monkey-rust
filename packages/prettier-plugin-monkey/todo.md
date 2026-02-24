# prettier-plugin-monkey: Known Issues & TODO

This document lists the current implementation deficiencies and improvement tasks.

---

## Critical Issues (P0)

### 1. Comments Are Lost During Formatting

**Location**: `src/printer.ts`

The current implementation completely ignores comments. The Printer does not implement:
- `printComment`
- `canAttachComment`
- `isBlockComment`
- `handleComments`

**Impact**: All comments in the source code will be lost after formatting, which is unacceptable for any real-world usage.

**TODO**:
- [ ] Implement comment attachment in parser
- [ ] Implement `printComment` in printer
- [ ] Implement `canAttachComment` to determine which nodes can have comments
- [ ] Add tests for comment preservation

---

### 2. Type Definitions Don't Match Actual AST

**Location**: `src/types.ts`, `src/printer.ts`

The TypeScript types defined in `types.ts` don't match the actual AST structure returned by the WASM parser.

**Evidence** (`printer.ts:106`):
```typescript
const identifierName = (node.identifier.kind as any).value?.name || '';
```

The use of `as any` indicates a type mismatch.

**TODO**:
- [ ] Debug and log actual AST structure from WASM parser
- [ ] Update `types.ts` to match actual AST structure
- [ ] Remove all `as any` type assertions in printer.ts

---

## High Priority Issues (P1)

### 3. Inconsistent Node Type Names

**Location**: `src/printer.ts`, `src/types.ts`

Switch cases in `printer.ts`:
```typescript
case 'IF':              // Actual AST node type
case 'Index':           // Actual AST node type
```

Type definitions in `types.ts`:
```typescript
type: 'IfExpression'    // Type definition
type: 'IndexExpression' // Type definition
```

**TODO**:
- [ ] Align type definitions with actual runtime node types
- [ ] Or update printer to use correct type names

---

### 4. Using `require` Instead of ES Import

**Location**: `src/printer.ts:25-33`

```typescript
const {
  group,
  indent,
  // ...
} = require('prettier').doc.builders;
```

Using `require` in an ESM module may cause compatibility issues.

**TODO**:
- [ ] Change to ES import:
  ```typescript
  import { doc } from 'prettier';
  const { group, indent, ... } = doc.builders;
  ```

---

### 5. Poor Error Location Information

**Location**: `src/parser.ts`

```typescript
throw new SyntaxError(`Monkey parse error: ${error.message}`);
```

No line number or column information is provided, making it difficult for users to locate syntax errors.

**TODO**:
- [ ] Extract position information from WASM parser errors
- [ ] Include line and column in error messages
- [ ] Consider implementing `locStart`/`locEnd` for error nodes

---

## Medium Priority Issues (P2)

### 6. Unnecessary Parentheses in Index Expressions

**Location**: `src/printer.ts:257-270`

```typescript
function printIndexExpression(...): Doc {
  return group([
    '(',
    path.call(print, 'object'),
    '[',
    path.call(print, 'index'),
    ']',
    ')',
  ]);
}
```

Always wrapping index expressions in parentheses is unnecessary and produces verbose output.

**Example**:
- Input: `arr[0]`
- Current output: `(arr[0])`
- Expected output: `arr[0]`

**TODO**:
- [ ] Only add parentheses when necessary (e.g., when object is a complex expression)
- [ ] Implement precedence-aware parenthesization

---

### 7. Incomplete Language Definition

**Location**: `src/language.ts`

```typescript
extensions: ['.monkey'],
```

But `README.md` claims:
```
Supported File Extensions: .monkey, .mk
```

**TODO**:
- [ ] Add `.mk` extension to language definition
- [ ] Or update README to reflect actual supported extensions

---

### 8. Missing ExpressionStatement Handling

**Location**: `src/printer.ts`

`types.ts` defines `ExpressionStatement`, but there's no corresponding case in the printer's switch statement.

**TODO**:
- [ ] Add explicit handling for ExpressionStatement
- [ ] Verify all statement types are handled

---

## Low Priority Issues (P3)

### 9. Console.warn in Production Code

**Location**: `src/printer.ts:76-77`

```typescript
default:
  console.warn('Unknown node type:', node);
```

Should use more graceful error handling in production.

**TODO**:
- [ ] Remove console.warn or conditionally enable based on environment
- [ ] Consider throwing an error for unknown node types during development
- [ ] Silently return empty string in production

---

### 10. Empty Options Definition

**Location**: `src/options.ts`

```typescript
export const options: Record<string, SupportOption> = {
  // We can add Monkey-specific options here in the future
};
```

No language-specific options are defined.

**TODO**:
- [ ] Consider adding Monkey-specific formatting options:
  - `monkeyExplicitSemicolons`: Whether to add semicolons
  - `monkeyBraceStyle`: Brace placement style
  - etc.

---

### 11. Missing Preprocessing

**Location**: `src/parser.ts`

No `preprocess` function is implemented to handle:
- BOM characters
- Different line endings (CRLF vs LF)
- Trailing whitespace

**TODO**:
- [ ] Implement `preprocess` function in parser
- [ ] Normalize line endings
- [ ] Strip BOM if present

---

### 12. Idempotency Edge Cases

Some complex cases (nested parentheses, operator precedence) may produce different results when formatted multiple times.

**TODO**:
- [ ] Add more comprehensive idempotency tests
- [ ] Test with deeply nested expressions
- [ ] Test with all operator combinations

---

## Summary Table

| Priority | Issue | Impact | Effort |
|----------|-------|--------|--------|
| P0 | Comments lost | User code destroyed | High |
| P0 | Type mismatch | Type safety lost | Medium |
| P1 | require vs import | ESM compatibility | Low |
| P1 | Error location | User experience | Medium |
| P2 | Extra parentheses | Code aesthetics | Medium |
| P2 | Missing .mk extension | Feature incomplete | Low |
| P3 | console.warn | Production noise | Low |
| P3 | No preprocessing | Edge cases | Low |

---

## References

- [Prettier Plugin API](https://prettier.io/docs/en/plugins.html)
- [Prettier Comment Handling](https://prettier.io/docs/en/plugins.html#handling-comments-in-a-printer)
