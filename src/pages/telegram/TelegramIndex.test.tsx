// @vitest-environment jsdom
import '@/test/setup'
/**
 * WP8 Phase C: the Telegram page sets up the bot Flow's Telegram Alert nodes
 * send with. A "Bot settings" card: the bot token (a password field; the
 * saved token is only ever shown masked), the chat id, Save, and Send test
 * message (secondary, once both are saved). Loading shows a skeleton, a
 * failed read an inline error with Retry; the page has the Help button.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  sendTestMessage: vi.fn(),
}))
vi.mock('@/api/telegram', () => ({ telegramApi: api, default: api }))

const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))
vi.mock('sonner', () => ({ toast }))

import TelegramIndex from './TelegramIndex'

const NOT_SET_UP = { configured: false, chat_id: null, token_masked: null }
const SAVED = { configured: true, chat_id: '987654321', token_masked: '••••WXYZ' }
const TOKEN = '123456789:AAFlowTestTokenNotARealOneWXYZ'

function renderPage() {
  return render(
    <MemoryRouter initialEntries={['/telegram']}>
      <TelegramIndex />
    </MemoryRouter>
  )
}

beforeEach(() => {
  for (const f of [...Object.values(api), ...Object.values(toast)]) f.mockReset()
})

describe('the Telegram page', () => {
  it('shows a skeleton while loading, then the saved chat and only the masked token', async () => {
    let resolve: (v: typeof SAVED) => void = () => {}
    api.getSettings.mockReturnValueOnce(
      new Promise((r) => {
        resolve = r
      })
    )
    const { container } = renderPage()
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0)
    resolve(SAVED)
    expect(await screen.findByText('Bot settings')).toBeInTheDocument()
    const token = screen.getByLabelText('Bot token') as HTMLInputElement
    expect(token.type).toBe('password')
    expect(token.value).toBe('')
    expect(token.placeholder).toContain('••••WXYZ')
    expect((screen.getByLabelText('Chat ID') as HTMLInputElement).value).toBe('987654321')
    expect(screen.getByText(/Alerts go to chat 987654321/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Help' })).toBeInTheDocument()
  })

  it('saves the typed token and chat id, then clears the token field', async () => {
    api.getSettings.mockResolvedValueOnce(NOT_SET_UP)
    api.saveSettings.mockResolvedValueOnce(SAVED)
    renderPage()
    const user = userEvent.setup()
    await user.type(await screen.findByLabelText('Bot token'), TOKEN)
    await user.type(screen.getByLabelText('Chat ID'), '987654321')
    await user.click(screen.getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith(TOKEN, '987654321'))
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Telegram settings saved'))
    const token = screen.getByLabelText('Bot token') as HTMLInputElement
    expect(token.value).toBe('')
    expect(token.placeholder).toContain('••••WXYZ')
  })

  it("shows the app's refusal and keeps what was typed", async () => {
    api.getSettings.mockResolvedValueOnce(NOT_SET_UP)
    api.saveSettings.mockRejectedValueOnce(
      new Error("The chat id is a number (a group's starts with -) or a channel's @name.")
    )
    renderPage()
    const user = userEvent.setup()
    await user.type(await screen.findByLabelText('Bot token'), TOKEN)
    await user.type(screen.getByLabelText('Chat ID'), 'my chat')
    await user.click(screen.getByRole('button', { name: 'Save' }))
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        "The chat id is a number (a group's starts with -) or a channel's @name."
      )
    )
    expect((screen.getByLabelText('Bot token') as HTMLInputElement).value).toBe(TOKEN)
    expect((screen.getByLabelText('Chat ID') as HTMLInputElement).value).toBe('my chat')
  })

  it('offers Send test message only once both are saved, and reports what Telegram said', async () => {
    api.getSettings.mockResolvedValueOnce(NOT_SET_UP)
    const first = renderPage()
    expect(await screen.findByRole('button', { name: 'Send test message' })).toBeDisabled()
    expect(screen.getByText(/Not set up/)).toBeInTheDocument()
    first.unmount()

    api.getSettings.mockResolvedValueOnce(SAVED)
    api.sendTestMessage
      .mockResolvedValueOnce('Test message sent. Check your Telegram chat.')
      .mockRejectedValueOnce(
        new Error('Telegram refused the message (400): Bad Request: chat not found')
      )
    renderPage()
    const user = userEvent.setup()
    const send = await screen.findByRole('button', { name: 'Send test message' })
    expect(send).toBeEnabled()
    await user.click(send)
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith('Test message sent. Check your Telegram chat.')
    )
    await user.click(screen.getByRole('button', { name: 'Send test message' }))
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        'Telegram refused the message (400): Bad Request: chat not found'
      )
    )
  })

  it('shows a failed read inline with Retry, which reads again', async () => {
    api.getSettings
      .mockRejectedValueOnce(new Error('The Telegram settings could not be read.'))
      .mockResolvedValueOnce(SAVED)
    renderPage()
    expect(await screen.findByText('The Telegram settings could not be read.')).toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByText('Bot settings')).toBeInTheDocument()
    expect(api.getSettings).toHaveBeenCalledTimes(2)
  })
})
