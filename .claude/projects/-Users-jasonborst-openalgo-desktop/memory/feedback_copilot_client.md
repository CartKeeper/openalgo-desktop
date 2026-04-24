---
name: Copilot client context
description: AI copilot should only look up client data when the user explicitly mentions a client
type: feedback
---

The copilot should NOT anticipate or proactively look up client data unless the user explicitly mentions a client. Don't inject client context into prompts or auto-search clients.

**Why:** User wants the copilot to be a general research tool by default, not a client-focused one. Client lookups should only happen when the user asks about a specific client.

**How to apply:** When building copilot tools and system prompts, provide the tools for client/account lookup but do NOT instruct the AI to proactively use them. The AI should only call client/broker tools when the user's message explicitly references a client or their account.
