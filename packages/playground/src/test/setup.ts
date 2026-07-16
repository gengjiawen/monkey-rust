import '@testing-library/jest-dom/vitest'

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

Object.defineProperty(globalThis, 'ResizeObserver', {
  configurable: true,
  value: ResizeObserverMock,
})

Object.defineProperty(window, 'matchMedia', {
  configurable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => false,
  }),
})

// jsdom implements neither element scrolling nor the Pointer Events capture
// API, both of which Radix Select and the AST tree's selection sync call into.
if (!window.HTMLElement.prototype.scrollIntoView) {
  window.HTMLElement.prototype.scrollIntoView = () => {}
}
if (!window.Element.prototype.hasPointerCapture) {
  window.Element.prototype.hasPointerCapture = () => false
}
if (!window.Element.prototype.setPointerCapture) {
  window.Element.prototype.setPointerCapture = () => {}
}
if (!window.Element.prototype.releasePointerCapture) {
  window.Element.prototype.releasePointerCapture = () => {}
}
