/**
 * WP8 Phase C: the Telegram page's commands over a mocked `invoke`. Pins the
 * command and its camelCase arguments (Tauri's default), what each wrapper
 * hands back, and that a refusal reaches the page as an Error carrying the
 * app's own sentence.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))

import { telegramApi } from './telegram'

const SAVED = { configured: true, chat_id: '987654321', token_masked: '••••WXYZ' }

beforeEach(() => {
  mocks.invoke.mockReset()
})

describe('telegramApi', () => {
  it('reads the settings with telegram_get_settings', async () => {
    mocks.invoke.mockResolvedValueOnce(SAVED)
    await expect(telegramApi.getSettings()).resolves.toEqual(SAVED)
    expect(mocks.invoke).toHaveBeenCalledWith('telegram_get_settings', undefined)
  })

  it('saves the token and the chat id with telegram_save_settings', async () => {
    mocks.invoke.mockResolvedValueOnce(SAVED)
    await expect(
      telegramApi.saveSettings('123456789:AAFlowTestTokenNotARealOneWXYZ', '987654321')
    ).resolves.toEqual(SAVED)
    expect(mocks.invoke).toHaveBeenCalledWith('telegram_save_settings', {
      botToken: '123456789:AAFlowTestTokenNotARealOneWXYZ',
      chatId: '987654321',
    })
  })

  it('sends no token when the token field is blank, so the saved one is kept', async () => {
    mocks.invoke.mockResolvedValueOnce(SAVED)
    await telegramApi.saveSettings('   ', '-1001234567')
    expect(mocks.invoke).toHaveBeenCalledWith('telegram_save_settings', {
      botToken: null,
      chatId: '-1001234567',
    })
  })

  it('sends the test message with telegram_send_test', async () => {
    mocks.invoke.mockResolvedValueOnce('Test message sent. Check your Telegram chat.')
    await expect(telegramApi.sendTestMessage()).resolves.toBe(
      'Test message sent. Check your Telegram chat.'
    )
    expect(mocks.invoke).toHaveBeenCalledWith('telegram_send_test', undefined)
  })

  it("rejects with the app's sentence when a command refuses", async () => {
    mocks.invoke.mockRejectedValueOnce(
      'Telegram refused the message (400): Bad Request: chat not found'
    )
    await expect(telegramApi.sendTestMessage()).rejects.toThrow(
      'Telegram refused the message (400): Bad Request: chat not found'
    )
    mocks.invoke.mockRejectedValueOnce({ message: 'The Telegram settings could not be read.' })
    await expect(telegramApi.getSettings()).rejects.toThrow(
      'The Telegram settings could not be read.'
    )
  })
})
