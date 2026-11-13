/**
 * The Telegram page's help panel (the app's shared HelpButton), WP8 Phase C.
 * What it says about Telegram was checked against Telegram's own pages on
 * 2026-11-12: BotFather's /newbot gives the token; the Bot API's
 * getUpdates lists the messages a bot received, each with its chat's id;
 * sendMessage takes a chat's number or a channel's @name.
 */

import type { HelpItem } from '@/components/HelpButton'

export const TELEGRAM_HELP: HelpItem[] = [
  {
    title: 'What this page sets up',
    description:
      'The bot that Flow\'s Telegram Alert nodes send with, and the chat they send to. Until both are saved, a Telegram Alert node stops with "Telegram is not set up" and its branch of the run goes no further.',
  },
  {
    title: 'Bot token',
    description:
      'In Telegram, message @BotFather and send /newbot; it asks for a name and gives you a token like 123456789:AAH.... Paste it here and choose Save. The app stores it encrypted on this computer and shows only its last four characters afterwards. To replace it, paste a new one; leave the field empty to keep the saved one.',
  },
  {
    title: 'Chat ID',
    description:
      'Open a chat with your bot in Telegram and press Start (or send it any message). Then open https://api.telegram.org/bot<your token>/getUpdates in a browser: your chat id is the number after "chat":{"id":. A group\'s id starts with -; a channel can be its @name.',
  },
  {
    title: 'Send test message',
    description:
      'Sends a short message to the saved chat, so you can see the bot reach you. If Telegram refuses it, Telegram\'s own reason is shown, for example "Unauthorized" (check the bot token) or "chat not found" (check the chat id, and that the chat with the bot was started).',
  },
  {
    title: 'Alerts from Flow',
    description:
      "A Telegram Alert node sends its message, with any {{variables}} filled in, and the time in Eastern time (for example 3:45:12 PM ET). If the send fails, that branch stops and the reason is in the run's Execution Log.",
  },
]
