'use client'

import { useTheme } from 'next-themes'
import { useEffect, useState } from 'react'

const toggleClass =
  'group m-0 inline-flex cursor-pointer items-center justify-center rounded-full border-0 bg-transparent p-0 text-(--gray-11) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--accent-8) disabled:cursor-default disabled:opacity-55'

const trackClass =
  'relative grid h-7 w-13 grid-cols-2 items-center rounded-full border border-(--gray-a6) bg-(--gray-a3) p-0.5 shadow-[inset_0_1px_1px_var(--gray-a2)] transition-[background,border-color] duration-[160ms] ease-[ease] group-enabled:group-hover:border-(--gray-a8) group-enabled:group-hover:bg-(--gray-a4)'

const iconClass =
  'relative z-1 inline-flex h-full w-full items-center justify-center transition-[color,opacity] duration-[160ms] ease-[ease]'

const thumbClass =
  'pointer-events-none absolute top-0.5 left-0.5 h-5.5 w-5.5 rounded-full border border-(--gray-a5) bg-(--color-panel-solid) shadow-[0_1px_2px_var(--gray-a4),0_0_0_1px_var(--gray-a2)] transition-transform duration-[180ms] ease-[cubic-bezier(0.22,1,0.36,1)]'

function SunIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="4" fill="currentColor" />
      <path
        d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  )
}

function MoonIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M21 14.5A8.5 8.5 0 0 1 9.5 3 7 7 0 1 0 21 14.5Z"
        fill="currentColor"
      />
    </svg>
  )
}

export function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme()
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  if (!mounted) {
    return (
      <button
        type="button"
        className={toggleClass}
        disabled
        aria-label="Toggle color theme"
      >
        <span className={trackClass} aria-hidden="true">
          <span className={`${iconClass} text-(--gray-9)`}>
            <SunIcon />
          </span>
          <span className={`${iconClass} text-(--gray-9)`}>
            <MoonIcon />
          </span>
          <span className={thumbClass} />
        </span>
      </button>
    )
  }

  const isDark = resolvedTheme === 'dark'

  return (
    <button
      type="button"
      className={toggleClass}
      aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      aria-pressed={isDark}
      onClick={() => setTheme(isDark ? 'light' : 'dark')}
    >
      <span className={trackClass} aria-hidden="true">
        <span
          className={`${iconClass} ${
            isDark ? 'text-(--gray-9) opacity-55' : 'text-(--amber-9)'
          }`}
        >
          <SunIcon />
        </span>
        <span
          className={`${iconClass} ${
            isDark ? 'text-(--blue-9)' : 'text-(--gray-9) opacity-55'
          }`}
        >
          <MoonIcon />
        </span>
        <span className={`${thumbClass} ${isDark ? 'translate-x-6' : ''}`} />
      </span>
    </button>
  )
}
