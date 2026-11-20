/**
 * Plain-English explanations of order types, validity options, and order
 * parameters, written for users new to trading. Single source of truth —
 * surfaced in the Place Order dialog and as tooltips wherever an order type
 * is displayed (e.g. the Order Book's Type column).
 */
export const ORDER_GLOSSARY = {
  orderTypes: {
    MARKET: {
      label: 'Market Order',
      desc: 'Executes immediately at the best available price. You are guaranteed a fill but not a specific price. Best for liquid stocks where you want in or out now.',
    },
    LIMIT: {
      label: 'Limit Order',
      desc: 'Executes only at your specified price or better. Buy limits fill at or below your price; sell limits fill at or above. May not fill if the market never reaches your price.',
    },
    SL: {
      label: 'Stop Loss Limit',
      desc: 'A two-price order: when the market hits your trigger price, a limit order is placed at your limit price. Protects against losses but the limit order may not fill if the market moves too fast past your limit.',
    },
    'SL-M': {
      label: 'Stop Loss Market',
      desc: 'When the market hits your trigger price, a market order is placed. Guarantees execution once triggered but the fill price may differ from the trigger in fast-moving markets.',
    },
    TRAILING_STOP: {
      label: 'Trailing Stop',
      desc: 'A dynamic stop that follows the market price by a fixed dollar amount or percentage. As the price rises, the stop rises with it. If the price drops by the trail amount, a market order triggers. Locks in profits while letting winners run.',
    },
  },
  validity: {
    DAY: 'Expires at market close today if not filled.',
    GTC: 'Good Till Cancelled — stays active until filled or you cancel it (up to 90 days).',
    IOC: 'Immediate or Cancel — fills what it can right now, cancels the rest.',
    FOK: 'Fill or Kill — the entire quantity must fill immediately or the whole order is cancelled.',
  },
  parameters: {
    extendedHours:
      'Execute during pre-market (4–9:30 AM ET) and after-hours (4–8 PM ET). Only limit orders are allowed in extended hours.',
    trailAmount: 'The dollar amount the stop trails behind the highest price reached.',
    trailPercent: 'The percentage the stop trails behind the highest price reached.',
    triggerPrice: 'The price that activates a stop loss order. Once hit, your order becomes live.',
    limitPrice:
      'The maximum price you will pay (buy) or the minimum you will accept (sell). The order only fills at this price or better.',
  },
} as const

export type GlossaryOrderType = keyof typeof ORDER_GLOSSARY.orderTypes
