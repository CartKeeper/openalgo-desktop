import { invoke } from '@tauri-apps/api/core'
import { ArrowLeft, Check, Globe, Key, Loader2, Trash2, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

interface ProviderConfig {
  id: string
  name: string
  description: string
  required: boolean
  configured: boolean
}

export default function GenericSetup() {
  const navigate = useNavigate()
  const location = useLocation()
  const isSettingsMode = location.pathname === '/providers'
  const [providers, setProviders] = useState<ProviderConfig[]>([
    {
      id: 'anthropic',
      name: 'Anthropic (Claude)',
      description: 'AI Research Copilot — Claude-powered financial research assistant',
      required: false,
      configured: false,
    },
    {
      id: 'fmp',
      name: 'Financial Modeling Prep',
      description: 'Fundamentals, news, screener, analyst data (250 req/day free)',
      required: false,
      configured: false,
    },
    {
      id: 'fred',
      name: 'FRED (Federal Reserve)',
      description: 'Economic indicators, interest rates, calendar (120 req/min free)',
      required: false,
      configured: false,
    },
  ])
  const [editingProvider, setEditingProvider] = useState<string | null>(null)
  const [apiKeyInput, setApiKeyInput] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    loadProviderStatus()
  }, [])

  const loadProviderStatus = async () => {
    try {
      const configured = await invoke<string[]>('get_configured_providers')
      setProviders((prev) =>
        prev.map((p) => ({
          ...p,
          configured: configured.includes(p.id),
        }))
      )
    } catch (err) {
      console.error('Failed to load provider status:', err)
    } finally {
      setIsLoading(false)
    }
  }

  const handleSaveKey = async (providerId: string) => {
    if (!apiKeyInput.trim()) {
      toast.error('API key cannot be empty')
      return
    }

    setIsSaving(true)
    try {
      await invoke('save_provider_api_key', {
        provider: providerId,
        apiKey: apiKeyInput.trim(),
      })
      toast.success(`${providers.find((p) => p.id === providerId)?.name} API key saved`)
      setEditingProvider(null)
      setApiKeyInput('')
      await loadProviderStatus()
    } catch (err) {
      console.error('Failed to save API key:', err)
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Failed to save: ${msg}`)
    } finally {
      setIsSaving(false)
    }
  }

  const handleDeleteKey = async (providerId: string) => {
    try {
      await invoke('delete_provider_api_key', { provider: providerId })
      toast.success('API key removed')
      await loadProviderStatus()
    } catch (err) {
      console.error('Failed to delete API key:', err)
    }
  }

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin" />
      </div>
    )
  }

  if (isSettingsMode) {
    return (
      <div className="space-y-6">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <h1 className="text-2xl font-bold">Data Providers</h1>
          </div>
          <p className="text-sm text-muted-foreground">
            Configure API keys for market data, research, and AI features.
          </p>
        </div>

        <Card>
          <CardContent className="space-y-4 pt-6">
            {/* Yahoo Finance - always available */}
            <div className="flex items-center justify-between p-4 rounded-lg border bg-muted/50">
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-sm">Yahoo Finance</span>
                  <Check className="h-4 w-4 text-green-500" />
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  Real-time quotes, historical prices, global symbol search (no key required)
                </p>
              </div>
            </div>

            {/* Configurable providers */}
            {providers.map((provider) => (
              <div
                key={provider.id}
                className="flex items-center justify-between p-4 rounded-lg border"
              >
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-sm">{provider.name}</span>
                    {provider.configured ? (
                      <Check className="h-4 w-4 text-green-500" />
                    ) : (
                      <X className="h-4 w-4 text-muted-foreground" />
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">{provider.description}</p>
                </div>

                <div className="flex items-center gap-2 ml-4">
                  {editingProvider === provider.id ? (
                    <div className="flex items-center gap-2">
                      <Input
                        type="password"
                        placeholder="Enter API key"
                        value={apiKeyInput}
                        onChange={(e) => setApiKeyInput(e.target.value)}
                        className="w-48 h-8 text-sm"
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleSaveKey(provider.id)
                          if (e.key === 'Escape') {
                            setEditingProvider(null)
                            setApiKeyInput('')
                          }
                        }}
                        autoFocus
                      />
                      <Button
                        size="sm"
                        onClick={() => handleSaveKey(provider.id)}
                        disabled={isSaving}
                      >
                        {isSaving ? <Loader2 className="h-3 w-3 animate-spin" /> : 'Save'}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setEditingProvider(null)
                          setApiKeyInput('')
                        }}
                      >
                        <X className="h-3 w-3" />
                      </Button>
                    </div>
                  ) : provider.configured ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-destructive hover:text-destructive"
                      onClick={() => handleDeleteKey(provider.id)}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        setEditingProvider(provider.id)
                        setApiKeyInput('')
                      }}
                    >
                      <Key className="h-3 w-3 mr-1" />
                      Add Key
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    )
  }

  return (
    <div className="min-h-screen flex items-center justify-center py-8 px-4">
      <div className="w-full max-w-2xl">
        <Card className="shadow-xl">
          <CardHeader className="text-center">
            <div className="flex justify-center mb-4">
              <Globe className="h-12 w-12 text-primary" />
            </div>
            <CardTitle className="text-2xl">Research Only Mode Setup</CardTitle>
            <CardDescription>
              Configure data provider API keys for market research and analysis.
              Yahoo Finance works without an API key.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* Yahoo Finance - always available */}
            <div className="flex items-center justify-between p-4 rounded-lg border bg-muted/50">
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-sm">Yahoo Finance</span>
                  <Check className="h-4 w-4 text-green-500" />
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  Real-time quotes, historical prices, global symbol search (no key required)
                </p>
              </div>
            </div>

            {/* Configurable providers */}
            {providers.map((provider) => (
              <div
                key={provider.id}
                className="flex items-center justify-between p-4 rounded-lg border"
              >
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-sm">{provider.name}</span>
                    {provider.configured ? (
                      <Check className="h-4 w-4 text-green-500" />
                    ) : (
                      <X className="h-4 w-4 text-muted-foreground" />
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">{provider.description}</p>
                </div>

                <div className="flex items-center gap-2 ml-4">
                  {editingProvider === provider.id ? (
                    <div className="flex items-center gap-2">
                      <Input
                        type="password"
                        placeholder="Enter API key"
                        value={apiKeyInput}
                        onChange={(e) => setApiKeyInput(e.target.value)}
                        className="w-48 h-8 text-sm"
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleSaveKey(provider.id)
                          if (e.key === 'Escape') {
                            setEditingProvider(null)
                            setApiKeyInput('')
                          }
                        }}
                        autoFocus
                      />
                      <Button
                        size="sm"
                        onClick={() => handleSaveKey(provider.id)}
                        disabled={isSaving}
                      >
                        {isSaving ? <Loader2 className="h-3 w-3 animate-spin" /> : 'Save'}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setEditingProvider(null)
                          setApiKeyInput('')
                        }}
                      >
                        <X className="h-3 w-3" />
                      </Button>
                    </div>
                  ) : provider.configured ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-destructive hover:text-destructive"
                      onClick={() => handleDeleteKey(provider.id)}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        setEditingProvider(provider.id)
                        setApiKeyInput('')
                      }}
                    >
                      <Key className="h-3 w-3 mr-1" />
                      Add Key
                    </Button>
                  )}
                </div>
              </div>
            ))}

            <div className="pt-4">
              <Button className="w-full" onClick={() => navigate('/dashboard')}>
                Continue to Dashboard
              </Button>
              <p className="text-xs text-muted-foreground text-center mt-2">
                You can configure API keys later in Settings.
              </p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
