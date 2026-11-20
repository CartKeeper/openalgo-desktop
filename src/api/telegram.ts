/**
 * The Telegram page's commands (WP8 Phase C): the bot token and chat id that
 * Flow's Telegram Alert nodes send with. The app stores both encrypted and
 * never hands the token back: the page reads it masked (its last four
 * characters). Replaces the web-version stubs this file held, which nothing
 * imported.
 */

import { invoke } from '@tauri-apps/api/core'

/** The settings as the page shows them. */
export interface TelegramSettings {
  /** Both the bot token and the chat id are saved. */
  configured: boolean
  chat_id: string | null
  /** The saved token as four dots and its last four characters. */
  token_masked: string | null
}

/** A command's refusal (the app's own sentence) as an Error. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    if (typeof error === 'string') throw new Error(error)
    const message = (error as { message?: unknown } | null)?.message
    throw new Error(typeof message === 'string' ? message : String(error))
  }
}

export const telegramApi = {
  getSettings: () => call<TelegramSettings>('telegram_get_settings'),

  /** Save the chat id, and the token when one is typed (blank keeps the saved one). */
  saveSettings: (botToken: string, chatId: string) =>
    call<TelegramSettings>('telegram_save_settings', {
      botToken: botToken.trim() === '' ? null : botToken,
      chatId,
    }),

  /** Send a test message to the saved chat; the app's confirmation sentence. */
  sendTestMessage: () => call<string>('telegram_send_test'),
}

export default telegramApi
