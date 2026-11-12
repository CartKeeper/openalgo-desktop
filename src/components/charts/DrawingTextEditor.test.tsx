// @vitest-environment jsdom
import '@/test/setup'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { DrawingTextEditor } from './DrawingTextEditor'

const edit = { id: 'd1', x: 40, y: 30, text: '' }

describe('DrawingTextEditor', () => {
  it('renders nothing without an edit request', () => {
    const { container } = render(
      <DrawingTextEditor edit={null} onCommit={vi.fn()} onCancel={vi.fn()} />
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('shows the input at the pane point with the current text', () => {
    render(
      <DrawingTextEditor edit={{ ...edit, text: 'Hello' }} onCommit={vi.fn()} onCancel={vi.fn()} />
    )
    const input = screen.getByLabelText('Drawing text')
    expect(input).toHaveValue('Hello')
    const box = input.parentElement as HTMLElement
    expect(box.style.left).toBe('40px')
    expect(box.style.top).toBe('30px')
  })

  it('Enter commits the typed text', () => {
    const onCommit = vi.fn()
    render(<DrawingTextEditor edit={edit} onCommit={onCommit} onCancel={vi.fn()} />)
    const input = screen.getByLabelText('Drawing text')
    fireEvent.change(input, { target: { value: 'Breakout' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onCommit).toHaveBeenCalledWith('Breakout')
  })

  it('Escape cancels without committing', () => {
    const onCommit = vi.fn()
    const onCancel = vi.fn()
    render(<DrawingTextEditor edit={edit} onCommit={onCommit} onCancel={onCancel} />)
    const input = screen.getByLabelText('Drawing text')
    fireEvent.change(input, { target: { value: 'nope' } })
    fireEvent.keyDown(input, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('blur commits', () => {
    const onCommit = vi.fn()
    render(<DrawingTextEditor edit={edit} onCommit={onCommit} onCancel={vi.fn()} />)
    const input = screen.getByLabelText('Drawing text')
    fireEvent.change(input, { target: { value: 'Support' } })
    fireEvent.blur(input)
    expect(onCommit).toHaveBeenCalledWith('Support')
  })
})
