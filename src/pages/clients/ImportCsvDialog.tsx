import { invoke } from '@tauri-apps/api/core'
import { Loader2, Upload } from 'lucide-react'
import { useRef, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { ImportBatch } from '@/types/clients'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

interface ImportCsvDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  clientId: number
  onSuccess: () => void
}

interface CsvPreview {
  headers: string[]
  rows: string[][]
  totalRows: number
}

export default function ImportCsvDialog({
  open,
  onOpenChange,
  clientId,
  onSuccess,
}: ImportCsvDialogProps) {
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [csvContent, setCsvContent] = useState<string | null>(null)
  const [filename, setFilename] = useState('')
  const [preview, setPreview] = useState<CsvPreview | null>(null)
  const [isImporting, setIsImporting] = useState(false)

  const reset = () => {
    setCsvContent(null)
    setFilename('')
    setPreview(null)
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    const text = await file.text()
    setCsvContent(text)
    setFilename(file.name)

    // Build preview
    const lines = text.split('\n').filter((l) => l.trim())
    if (lines.length > 0) {
      const headers = lines[0].split(',').map((h) => h.trim().replace(/"/g, ''))
      const rows = lines.slice(1, 6).map((line) =>
        line.split(',').map((c) => c.trim().replace(/"/g, ''))
      )
      setPreview({ headers, rows, totalRows: lines.length - 1 })
    }
  }

  const handleImport = async () => {
    if (!csvContent) return

    setIsImporting(true)
    try {
      const batch = await invoke<ImportBatch>('import_client_trades_csv', {
        clientId,
        csvContent,
        filename,
      })
      toast.success(`Imported ${batch.row_count} trades from ${filename}`)
      reset()
      onSuccess()
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Import failed: ${msg}`)
    } finally {
      setIsImporting(false)
    }
  }

  const handleClose = (open: boolean) => {
    if (!open) reset()
    onOpenChange(open)
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import Trades from CSV</DialogTitle>
          <DialogDescription>
            Upload a brokerage CSV export. The importer detects columns automatically.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* File Picker */}
          <div className="flex items-center gap-4">
            <Button
              variant="outline"
              onClick={() => fileInputRef.current?.click()}
              disabled={isImporting}
            >
              <Upload className="h-4 w-4 mr-2" />
              Choose File
            </Button>
            <span className="text-sm text-muted-foreground">
              {filename || 'No file selected'}
            </span>
            <input
              ref={fileInputRef}
              type="file"
              accept=".csv"
              className="hidden"
              onChange={handleFileSelect}
            />
          </div>

          {/* Column Detection Info */}
          {preview && (
            <div className="rounded-lg border bg-muted/50 p-3 text-sm">
              <p className="font-medium mb-1">
                Detected {preview.headers.length} columns, {preview.totalRows} data rows
              </p>
              <p className="text-muted-foreground text-xs">
                Headers: {preview.headers.join(', ')}
              </p>
            </div>
          )}

          {/* Preview Table */}
          {preview && preview.rows.length > 0 && (
            <div className="overflow-x-auto rounded-lg border">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b bg-muted/50">
                    {preview.headers.map((h, i) => (
                      <th
                        key={i}
                        className="h-8 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground whitespace-nowrap"
                      >
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {preview.rows.map((row, ri) => (
                    <tr key={ri} className="border-b last:border-b-0">
                      {row.map((cell, ci) => (
                        <td key={ci} className="h-8 px-3 whitespace-nowrap text-xs">
                          {cell}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
              {preview.totalRows > 5 && (
                <p className="px-3 py-2 text-xs text-muted-foreground">
                  Showing 5 of {preview.totalRows} rows
                </p>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleClose(false)} disabled={isImporting}>
            Cancel
          </Button>
          <Button onClick={handleImport} disabled={!csvContent || isImporting}>
            {isImporting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Import {preview ? `${preview.totalRows} Trades` : ''}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
