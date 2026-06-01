import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'

/**
 * Save text content to a file via the Rust backend.
 *
 * The browser `<a download>` / `a.click()` pattern does not work inside Tauri's
 * macOS WKWebView (the `download` attribute is ignored and nothing happens), so
 * exports must go through the backend. The file is written to the user's
 * Downloads folder and a toast reports the saved location.
 *
 * @returns the absolute path written, or null on failure.
 */
export async function saveTextFile(filename: string, contents: string): Promise<string | null> {
  try {
    const path = await invoke<string>('save_export_file', { filename, contents })
    toast.success('Export saved', { description: path })
    return path
  } catch (e) {
    toast.error('Export failed', { description: String(e) })
    return null
  }
}
