/**
 * Put text on the clipboard that is not known yet when the button is
 * clicked (WP8 Phase D: the Flow cards' Copy reads each workflow's export
 * first). WebKit, the app's webview on a Mac, refuses a clipboard write made
 * after such awaits, outside the click; a ClipboardItem whose data is a
 * promise lets the write be made inside the click and the text follow.
 * Where there is no ClipboardItem, the text is written when it is ready.
 */
export async function copyLater(text: Promise<string>): Promise<void> {
  if (typeof ClipboardItem !== 'undefined' && navigator.clipboard?.write) {
    const blob = text.then((t) => new Blob([t], { type: 'text/plain' }))
    await navigator.clipboard.write([new ClipboardItem({ 'text/plain': blob })])
    return
  }
  await navigator.clipboard.writeText(await text)
}
