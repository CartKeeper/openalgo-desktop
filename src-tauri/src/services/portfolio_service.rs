//! Portfolio service — P&L calculation and price enrichment

use crate::providers::types::PortfolioPosition;
use crate::providers::yahoo::YahooClient;
use reqwest::Client;

/// Enrich portfolio positions with current prices and P&L from Yahoo Finance
pub async fn enrich_positions_with_prices(
    http_client: &Client,
    positions: &mut [PortfolioPosition],
) -> Result<(), crate::error::AppError> {
    if positions.is_empty() {
        return Ok(());
    }

    // Collect unique symbols for batch quote
    let symbols: Vec<&str> = positions
        .iter()
        .map(|p| p.symbol.as_str())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let client = YahooClient::new(http_client.clone());

    // Yahoo Finance supports batch quotes — fetch all at once
    match client.get_quotes(&symbols).await {
        Ok(quotes) => {
            // Build price lookup
            let price_map: std::collections::HashMap<&str, f64> = quotes
                .iter()
                .map(|q| (q.symbol.as_str(), q.price))
                .collect();

            // Enrich each position
            for pos in positions.iter_mut() {
                if let Some(&price) = price_map.get(pos.symbol.as_str()) {
                    pos.current_price = Some(price);
                    let cost = pos.avg_price * pos.quantity;
                    let value = price * pos.quantity;
                    let pnl = if pos.position_type == "short" {
                        cost - value
                    } else {
                        value - cost
                    };
                    pos.pnl = Some(pnl);
                    if cost.abs() > 0.0001 {
                        pos.pnl_percent = Some((pnl / cost) * 100.0);
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to fetch prices for portfolio enrichment: {}", e);
            // Non-fatal — positions are returned without live prices
        }
    }

    Ok(())
}
