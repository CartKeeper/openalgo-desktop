/**
 * Inline editor for the text-family drawing tools.
 *
 * `lightweight-charts-drawing` places a text drawing and then asks the host
 * (`textEdit` event) to open an editor at the given pane point and write the
 * text back. This is that editor: Enter or blur commits, Escape cancels. An
 * empty commit removes the drawing (handled by the hook).
 */
import { useEffect, useRef, useState } from 'react'
import { Input } from '@/components/ui/input'
import type { DrawingTextEdit } from '@/hooks/useChartDrawings'

export interface DrawingTextEditorProps {
  edit: DrawingTextEdit | null
  onCommit: (text: string) => void
  onCancel: () => void
}

export function DrawingTextEditor({ edit, onCommit, onCancel }: DrawingTextEditorProps) {
  const [text, setText] = useState(edit?.text ?? '')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    setText(edit?.text ?? '')
    if (edit) {
      const frame = requestAnimationFrame(() => {
        inputRef.current?.focus()
        inputRef.current?.select()
      })
      return () => cancelAnimationFrame(frame)
    }
  }, [edit])

  if (!edit) return null

  return (
    <div
      className="absolute z-10"
      style={{ left: Math.max(0, edit.x), top: Math.max(0, edit.y) }}
      data-drawing-text-editor={edit.id}
    >
      <Input
        ref={inputRef}
        aria-label="Drawing text"
        placeholder="Add text"
        className="h-8 w-48 bg-popover text-sm shadow-md"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            onCommit(text)
          } else if (e.key === 'Escape') {
            e.preventDefault()
            e.stopPropagation()
            onCancel()
          }
        }}
        onBlur={() => onCommit(text)}
      />
    </div>
  )
}
