import { describe, it, expect } from 'vitest'
import {
  chromeMajor,
  buildUserAgent,
  buildUaBrands,
  buildClientHints,
  mergeClientHintHeaders,
  isGoogleSignInHost,
  FIREFOX_UA,
} from './userAgent'

describe('chromeMajor', () => {
  it('extracts the major from a full Chromium version', () => {
    expect(chromeMajor('130.0.6723.59')).toBe('130')
  })
  it('accepts a bare major', () => {
    expect(chromeMajor('118')).toBe('118')
  })
  it('falls back to "0" on an empty string', () => {
    expect(chromeMajor('')).toBe('0')
  })
  it('falls back to "0" when the major is non-numeric', () => {
    expect(chromeMajor('abc.1.2')).toBe('0')
  })
})

describe('buildUserAgent', () => {
  it('produces a stock desktop-Chrome UA with the given version', () => {
    const ua = buildUserAgent('130.0.6723.59')
    expect(ua).toBe(
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.59 Safari/537.36',
    )
    expect(ua).not.toMatch(/electron/i)
  })
})

describe('buildUaBrands', () => {
  it('lists Chromium + Google Chrome at the major, plus the GREASE brand', () => {
    expect(buildUaBrands('130.0.6723.59')).toEqual([
      { brand: 'Chromium', version: '130' },
      { brand: 'Google Chrome', version: '130' },
      { brand: 'Not?A_Brand', version: '99' },
    ])
  })
  it('never advertises Electron', () => {
    expect(JSON.stringify(buildUaBrands('130'))).not.toMatch(/electron/i)
  })
})

describe('isGoogleSignInHost', () => {
  it('matches Google account sign-in hosts', () => {
    expect(isGoogleSignInHost('accounts.google.com')).toBe(true)
    expect(isGoogleSignInHost('accounts.youtube.com')).toBe(true)
  })
  it('rejects other Google + unrelated hosts', () => {
    expect(isGoogleSignInHost('www.google.com')).toBe(false)
    expect(isGoogleSignInHost('mail.google.com')).toBe(false)
    expect(isGoogleSignInHost('poe.ninja')).toBe(false)
    expect(isGoogleSignInHost('evil-accounts.google.com.attacker.com')).toBe(false)
  })
})

describe('FIREFOX_UA', () => {
  it('is a Firefox UA with no Chrome token', () => {
    expect(FIREFOX_UA).toMatch(/Firefox\/\d/)
    expect(FIREFOX_UA).not.toMatch(/Chrome/)
  })
})

describe('buildClientHints', () => {
  it('aligns the brand version with the Chromium major', () => {
    const hints = buildClientHints('130.0.6723.59')
    expect(hints['Sec-CH-UA']).toBe(
      '"Chromium";v="130", "Google Chrome";v="130", "Not?A_Brand";v="99"',
    )
    expect(hints['Sec-CH-UA-Mobile']).toBe('?0')
    expect(hints['Sec-CH-UA-Platform']).toBe('"Windows"')
  })
  it('never advertises Electron', () => {
    expect(buildClientHints('130.0.0.0')['Sec-CH-UA']).not.toMatch(/electron/i)
  })
})

describe('mergeClientHintHeaders', () => {
  const hints = buildClientHints('130.0.0.0')

  it('overrides an Electron-branded hint regardless of header casing', () => {
    const merged = mergeClientHintHeaders(
      { 'sec-ch-ua': '"Electron";v="33"', 'Sec-CH-UA-Platform': '"Linux"' },
      hints,
    )
    expect(merged['Sec-CH-UA']).toBe(hints['Sec-CH-UA'])
    expect(merged['Sec-CH-UA-Platform']).toBe('"Windows"')
    // the old lowercase variant must not survive alongside the canonical one
    expect(merged['sec-ch-ua']).toBeUndefined()
  })

  it('preserves unrelated headers', () => {
    const merged = mergeClientHintHeaders(
      { 'User-Agent': 'x', Accept: 'text/html' },
      hints,
    )
    expect(merged['User-Agent']).toBe('x')
    expect(merged.Accept).toBe('text/html')
    expect(merged['Sec-CH-UA-Mobile']).toBe('?0')
  })

  it('does not mutate the input object', () => {
    const input = { 'sec-ch-ua': '"Electron";v="33"' }
    mergeClientHintHeaders(input, hints)
    expect(input).toEqual({ 'sec-ch-ua': '"Electron";v="33"' })
  })
})
