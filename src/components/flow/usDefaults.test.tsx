// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase B follow-up: a node saved without its exchange or its times (an
 * import, an old file) shows this app's US defaults, the ones the run uses
 * for a missing field (src-tauri flow::node_contracts): the config panel's
 * fallbacks and the node faces on the canvas agree with the run.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, within } from '@testing-library/react'
import { type Node, ReactFlow, ReactFlowProvider } from '@xyflow/react'
import { MemoryRouter } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useFlowWorkflowStore } from '@/stores/flowWorkflowStore'
import { nodeTypes } from './nodes'

vi.mock('@/api/flow', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/flow')>()),
  getIndexSymbolsLotSizes: vi.fn(async () => []),
}))

const { ConfigPanel } = await import('./panels/ConfigPanel')

function openPanel(type: string, data: Record<string, unknown>) {
  useFlowWorkflowStore.setState({
    nodes: [{ id: 'n1', type, position: { x: 0, y: 0 }, data }],
    selectedNodeId: 'n1',
  })
  render(
    <MemoryRouter>
      <QueryClientProvider client={new QueryClient()}>
        <ConfigPanel />
      </QueryClientProvider>
    </MemoryRouter>
  )
}

function labelled(label: string): HTMLElement {
  return screen.getByText(label, { selector: 'label' })
}

function fieldUnder(label: string): HTMLInputElement {
  const input = labelled(label).parentElement?.querySelector('input')
  if (!input) throw new Error(`no input under ${label}`)
  return input
}

function triggerUnder(label: string): HTMLElement {
  const button = labelled(label).parentElement?.querySelector('button')
  if (!button) throw new Error(`no control under ${label}`)
  return button
}

function face(type: string, data: Record<string, unknown>): HTMLElement {
  const node: Node = { id: `n-${type}`, type, position: { x: 0, y: 0 }, data }
  const { container } = render(
    <div style={{ width: 800, height: 600 }}>
      <ReactFlowProvider>
        <ReactFlow nodes={[node]} edges={[]} nodeTypes={nodeTypes} />
      </ReactFlowProvider>
    </div>
  )
  const el = container.querySelector<HTMLElement>(`.react-flow__node[data-id="${node.id}"]`)
  if (!el) throw new Error(`${type} not drawn`)
  return el
}

beforeEach(() => useFlowWorkflowStore.getState().resetWorkflow())

describe('the config panel', () => {
  it('shows a Start node saved without times at 09:35, inside 09:30 to 16:00, on the US calendar', () => {
    openPanel('start', { scheduleType: 'daily' })
    expect(fieldUnder('Time')).toHaveValue('09:35')
    expect(fieldUnder('Start')).toHaveValue('09:30')
    expect(fieldUnder('End')).toHaveValue('16:00')
    expect(within(triggerUnder('Calendar')).getByText('US')).toBeInTheDocument()
  })

  it('shows a Time Window saved without times as 09:30 to 16:00', () => {
    openPanel('timeWindow', {})
    expect(fieldUnder('Start Time')).toHaveValue('09:30')
    expect(fieldUnder('End Time')).toHaveValue('16:00')
  })

  // The run resolves a missing exchange to US and its product to CNC
  // (node_contracts::default_product_for_exchange), so the panel does too.
  it.each(['placeOrder', 'smartOrder', 'splitOrder', 'closePositions', 'openPosition'])(
    'shows %s saved without an exchange or a product as CNC',
    (type) => {
      openPanel(type, { symbol: 'AAPL' })
      expect(within(triggerUnder('Product')).getByText('CNC')).toBeInTheDocument()
    }
  )

  it.each([
    'priceAlert',
    'placeOrder',
    'smartOrder',
    'splitOrder',
    'closePositions',
    'getQuote',
    'getDepth',
    'openPosition',
    'history',
    'indicator',
    'priorPeriodOhlc',
    'barOffset',
    'multiQuotes',
    'subscribeLtp',
    'subscribeQuote',
    'subscribeDepth',
    'unsubscribe',
    'priceCondition',
    'positionCheck',
  ])('shows %s saved without an exchange on US', (type) => {
    openPanel(type, { symbol: 'AAPL' })
    expect(within(triggerUnder('Exchange')).getByText('US')).toBeInTheDocument()
  })
})

describe('the node faces', () => {
  it.each([
    'symbol',
    'subscribeQuote',
    'subscribeDepth',
    'subscribeLtp',
    'multiQuotes',
    'openPosition',
    'history',
    'placeOrder',
    'priceAlert',
    'priorPeriodOhlc',
    'smartOrder',
    'getDepth',
    'getQuote',
    'splitOrder',
  ])('%s saved without an exchange reads US', (type) => {
    const el = face(type, { symbol: 'AAPL' })
    expect(within(el).getByText('US')).toBeInTheDocument()
    expect(within(el).queryByText('NSE')).not.toBeInTheDocument()
  })

  it.each(['placeOrder', 'smartOrder', 'openPosition'])(
    '%s saved without an exchange or a product reads CNC, as the run sends',
    (type) => {
      const el = face(type, { symbol: 'AAPL' })
      expect(within(el).getByText('CNC')).toBeInTheDocument()
      expect(within(el).queryByText('MIS')).not.toBeInTheDocument()
    }
  )

  it('a Start node saved without a time reads 09:35', () => {
    const el = face('start', { scheduleType: 'daily' })
    expect(within(el).getByText('09:35')).toBeInTheDocument()
  })

  it('a Time Window saved without times reads 09:30 and 16:00', () => {
    const el = face('timeWindow', {})
    expect(within(el).getByText('09:30')).toBeInTheDocument()
    expect(within(el).getByText('16:00')).toBeInTheDocument()
  })
})
