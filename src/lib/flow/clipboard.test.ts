import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyLater } from './clipboard'

/**
 * WP8 Phase D (the review's finding): the Flow cards' Copy reads each
 * workflow's export before it has the text. WebKit (the app's webview on a
 * Mac) refuses a clipboard write made after such awaits, outside the click,
 * so the write is made at once, inside the click, with a ClipboardItem
 * whose text is still to come; where there is no ClipboardItem the text is
 * written when it is ready.
 */

const realClipboard = navigator.clipboard

afterEach(() => {
  Object.defineProperty(navigator, 'clipboard', { value: realClipboard, configurable: true })
  vi.unstubAllGlobals()
})

describe('copyLater', () => {
  it('writes at once, with the text to come, where ClipboardItem exists', async () => {
    const items: Array<Record<string, Promise<Blob>>> = []
    class FakeItem {
      constructor(readonly data: Record<string, Promise<Blob>>) {
        items.push(data)
      }
    }
    vi.stubGlobal('ClipboardItem', FakeItem)
    const write = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      value: { write, writeText: vi.fn() },
      configurable: true,
    })
    let resolve: (text: string) => void = () => {}
    const text = new Promise<string>((r) => {
      resolve = r
    })
    const done = copyLater(text)
    // Inside the click: the write is already made, the text not yet known.
    expect(write).toHaveBeenCalledTimes(1)
    resolve('[{"name": "Gap fill"}]')
    await done
    const blob = await items[0]['text/plain']
    expect(await blob.text()).toBe('[{"name": "Gap fill"}]')
  })

  it('writes the text when it is ready where there is no ClipboardItem', async () => {
    vi.stubGlobal('ClipboardItem', undefined)
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    })
    await copyLater(Promise.resolve('{"name": "Gap fill"}'))
    expect(writeText).toHaveBeenCalledWith('{"name": "Gap fill"}')
  })
})
