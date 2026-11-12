/**
 * Desktop-only: the webhook URLs a trader copies from Flow.
 *
 * Upstream's desktop moved every URL onto its built-in ngrok tunnel's public
 * address while the tunnel was online. This app has no built-in tunnel: the
 * server already answers the webhook settings on the public base URL (the
 * webhook settings' Ngrok/Public URL when one is set, else this computer's
 * own address, which components/desktop/FlowWebhookReachability then flags),
 * so there is nothing to move here. Kept out of api/flow.ts so that file
 * stays the web's. Adapted in WP8 Phase B (docs/flow/PORT.md).
 */

import { getWebhookInfo, type WebhookInfo } from './flow'

/** A workflow's webhook settings as the trader should copy them. */
export async function getWebhookInfoForTrader(id: number): Promise<WebhookInfo> {
  return getWebhookInfo(id)
}
