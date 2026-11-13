// pages/telegram/TelegramIndex.tsx
// The Telegram page (WP8 Phase C): the bot token and chat id that Flow's
// Telegram Alert nodes send with, and a test message. The web version's bot
// status, users, analytics and broadcast sections were removed: they read
// a web server this desktop app does not have, so the page never got past
// "Failed to load Telegram data" (docs/flow/PORT.md).

import { AlertCircle, ArrowLeft, Loader2, MessageSquare, Send } from 'lucide-react'
import { type ReactNode, useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { type TelegramSettings, telegramApi } from '@/api/telegram'
import { HelpButton } from '@/components/HelpButton'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'
import { TELEGRAM_HELP } from './telegramHelp'

/** CLAUDE.md's medium button: 40px high, 16px padding, at least 80px wide. */
const BUTTON_40 =
  'relative h-10 min-w-20 px-4 transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2 disabled:cursor-not-allowed'

type Load =
  | { state: 'loading' }
  | { state: 'error'; message: string }
  | { state: 'ready'; settings: TelegramSettings }

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/** A button's label, kept in place (so the button keeps its width) under a spinner while busy. */
function Busy({ busy, children }: { busy: boolean; children: ReactNode }) {
  return (
    <>
      <span className={cn('inline-flex items-center gap-2', busy && 'invisible')}>{children}</span>
      {busy && <Loader2 className="absolute h-4 w-4 animate-spin" aria-hidden="true" />}
    </>
  )
}

/** The form's shape while the settings load. */
function SettingsSkeleton() {
  return (
    <div className="space-y-4" aria-busy="true">
      <Skeleton className="h-5 w-72" />
      <div className="space-y-1.5">
        <Skeleton className="h-5 w-20" />
        <Skeleton className="h-10 w-full" />
      </div>
      <div className="space-y-1.5">
        <Skeleton className="h-5 w-16" />
        <Skeleton className="h-10 w-full" />
      </div>
      <div className="flex gap-2">
        <Skeleton className="h-10 w-20" />
        <Skeleton className="h-10 w-44" />
      </div>
    </div>
  )
}

export default function TelegramIndex() {
  const [load, setLoad] = useState<Load>({ state: 'loading' })
  const [token, setToken] = useState('')
  const [chatId, setChatId] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [isSending, setIsSending] = useState(false)

  const read = useCallback(async () => {
    setLoad({ state: 'loading' })
    try {
      const settings = await telegramApi.getSettings()
      setChatId(settings.chat_id ?? '')
      setLoad({ state: 'ready', settings })
    } catch (error) {
      setLoad({ state: 'error', message: messageOf(error) })
    }
  }, [])

  useEffect(() => {
    read()
  }, [read])

  const save = async () => {
    setIsSaving(true)
    try {
      const settings = await telegramApi.saveSettings(token, chatId)
      setToken('')
      setChatId(settings.chat_id ?? '')
      setLoad({ state: 'ready', settings })
      toast.success('Telegram settings saved')
    } catch (error) {
      toast.error(messageOf(error))
    } finally {
      setIsSaving(false)
    }
  }

  const sendTest = async () => {
    setIsSending(true)
    try {
      toast.success(await telegramApi.sendTestMessage())
    } catch (error) {
      toast.error(messageOf(error))
    } finally {
      setIsSending(false)
    }
  }

  return (
    <div className="py-6 space-y-6">
      <HelpButton items={TELEGRAM_HELP} pageTitle="Telegram Bot" />

      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Link
            to="/dashboard"
            className="text-muted-foreground hover:text-foreground"
            aria-label="Back to the dashboard"
          >
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <MessageSquare className="h-6 w-6" />
            Telegram Bot
          </h1>
        </div>
        <p className="text-muted-foreground mt-1">
          Manage your Telegram bot for trade notifications
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Bot settings</CardTitle>
          <CardDescription>
            The bot Flow&apos;s Telegram Alert nodes send with, and the chat they send to. Both are
            stored encrypted on this computer.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {load.state === 'loading' && <SettingsSkeleton />}

          {load.state === 'error' && (
            <div
              role="alert"
              className="flex items-center justify-between gap-4 rounded-md border border-destructive p-4"
            >
              <p className="flex items-center gap-2 text-sm text-destructive">
                <AlertCircle className="h-4 w-4 shrink-0" aria-hidden="true" />
                {load.message}
              </p>
              <Button variant="outline" className={BUTTON_40} onClick={read}>
                Retry
              </Button>
            </div>
          )}

          {load.state === 'ready' && (
            <form
              className="space-y-4"
              onSubmit={(e) => {
                e.preventDefault()
                save()
              }}
            >
              <p
                className={cn(
                  'text-sm',
                  load.settings.configured ? 'text-foreground' : 'text-muted-foreground'
                )}
              >
                {load.settings.configured
                  ? `Set up. Alerts go to chat ${load.settings.chat_id}.`
                  : 'Not set up. Until it is, a Telegram Alert node stops with "Telegram is not set up".'}
              </p>
              <div className="space-y-1.5">
                <Label htmlFor="telegram-bot-token">Bot token</Label>
                <Input
                  id="telegram-bot-token"
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  className="h-10"
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  placeholder={
                    load.settings.token_masked
                      ? `Saved (${load.settings.token_masked}). Paste a new token to replace it.`
                      : '123456789:AAH...'
                  }
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="telegram-chat-id">Chat ID</Label>
                <Input
                  id="telegram-chat-id"
                  autoComplete="off"
                  spellCheck={false}
                  className="h-10"
                  value={chatId}
                  onChange={(e) => setChatId(e.target.value)}
                  placeholder="987654321"
                />
              </div>
              <div className="flex flex-wrap gap-2">
                <Button type="submit" className={BUTTON_40} disabled={isSaving}>
                  <Busy busy={isSaving}>Save</Busy>
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  className={BUTTON_40}
                  disabled={!load.settings.configured || isSending}
                  onClick={sendTest}
                >
                  <Busy busy={isSending}>
                    <Send className="h-4 w-4" aria-hidden="true" />
                    Send test message
                  </Busy>
                </Button>
              </div>
            </form>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
