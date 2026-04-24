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
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { ImportBatch } from '@/types/clients'
import { ACCOUNT_TYPES } from '@/types/clients'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

/** Parse a single CSV line respecting quoted fields (handles commas inside quotes) */
function parseCsvLine(line: string): string[] {
  const fields: string[] = []
  let current = ''
  let inQuotes = false

  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (inQuotes) {
      if (ch === '"') {
        // Check for escaped quote ("")
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"'
          i++ // skip next quote
        } else {
          inQuotes = false
        }
      } else {
        current += ch
      }
    } else {
      if (ch === '"') {
        inQuotes = true
      } else if (ch === ',') {
        fields.push(current.trim())
        current = ''
      } else {
        current += ch
      }
    }
  }
  fields.push(current.trim())
  return fields
}

/** Headers that indicate a numeric/monetary column */
const NUMERIC_HEADERS = new Set([
  'quantity', 'qty', 'shares', 'volume',
  'price', 'rate', 'avg_price',
  'fees', 'fees & comm', 'commission', 'brokerage', 'charges',
  'amount', 'net_amount', 'total', 'value',
])

function isNumericHeader(header: string): boolean {
  return NUMERIC_HEADERS.has(header.toLowerCase())
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
  const [accountType, setAccountType] = useState<string>('none')

  const reset = () => {
    setCsvContent(null)
    setFilename('')
    setPreview(null)
    setAccountType('none')
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

    // Build preview using proper CSV parser
    const lines = text.split('\n').filter((l) => l.trim())
    if (lines.length > 0) {
      const headers = parseCsvLine(lines[0])
      const rows = lines.slice(1, 6).map((line) => parseCsvLine(line))
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
        accountType: accountType === 'none' ? null : accountType,
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
      <DialogContent className="sm:max-w-4xl h-[75vh] flex flex-col p-0">
        <DialogHeader className="px-6 pt-6 pb-3 border-b shrink-0">
          <DialogTitle>Import Trades from CSV</DialogTitle>
          <DialogDescription>
            Upload a brokerage CSV export. The importer detects columns automatically.
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 space-y-4">
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
            <span className="text-sm text-muted-foreground truncate max-w-[320px]">
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

          {/* Account Type */}
          {csvContent && (
            <div className="space-y-1.5">
              <Label htmlFor="import-account-type">Account Type</Label>
              <Select value={accountType} onValueChange={setAccountType}>
                <SelectTrigger id="import-account-type" className="w-64">
                  <SelectValue placeholder="Select account type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Not specified</SelectItem>
                  {ACCOUNT_TYPES.map((at) => (
                    <SelectItem key={at.value} value={at.value}>
                      {at.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-[10px] text-muted-foreground">
                Identifies the account these trades belong to (e.g., 401(k), IRA, Individual).
              </p>
            </div>
          )}

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
              <table className="w-full text-sm table-fixed">
                <thead>
                  <tr className="border-b bg-muted/50">
                    {preview.headers.map((h, i) => {
                      const isNum = isNumericHeader(h)
                      const isDesc = h.toLowerCase() === 'description'
                      return (
                        <th
                          key={i}
                          className={`h-10 px-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground whitespace-nowrap ${
                            isNum ? 'text-right' : 'text-left'
                          } ${isDesc ? 'w-50' : ''}`}
                        >
                          {h}
                        </th>
                      )
                    })}
                  </tr>
                </thead>
                <tbody>
                  {preview.rows.map((row, ri) => (
                    <tr key={ri} className="border-b last:border-b-0">
                      {preview.headers.map((h, ci) => {
                        const cell = row[ci] ?? ''
                        const isNum = isNumericHeader(h)
                        const isDesc = h.toLowerCase() === 'description'
                        const isEmpty = cell.trim() === ''

                        return (
                          <td
                            key={ci}
                            className={`h-12 px-3 text-xs ${
                              isNum ? 'text-right font-mono tabular-nums' : 'text-left'
                            } ${isDesc ? 'truncate max-w-50' : 'whitespace-nowrap'} ${
                              isEmpty ? 'text-muted-foreground' : ''
                            }`}
                          >
                            {isEmpty ? '—' : cell}
                          </td>
                        )
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
              {preview.totalRows > 5 && (
                <p className="px-3 py-2 text-xs text-muted-foreground border-t">
                  Showing 5 of {preview.totalRows} rows
                </p>
              )}
            </div>
          )}
        </div>

        <DialogFooter className="px-6 py-3 border-t shrink-0">
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
