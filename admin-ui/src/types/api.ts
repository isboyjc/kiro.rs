// 凭据状态响应
export interface CredentialsStatusResponse {
  total: number
  available: number
  currentId: number
  credentials: CredentialStatusItem[]
}

// 单个凭据状态
export interface CredentialStatusItem {
  id: number
  priority: number
  disabled: boolean
  failureCount: number
  isCurrent: boolean
  expiresAt: string | null
  authMethod: string | null
  hasProfileArn: boolean
  email?: string
  refreshTokenHash?: string
  apiKeyHash?: string
  maskedApiKey?: string
  successCount: number
  lastUsedAt: string | null
  hasProxy: boolean
  proxyUrl?: string
  refreshFailureCount: number
  disabledReason?: string
  endpoint: string
}

// 余额响应
export interface BalanceResponse {
  id: number
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  remaining: number
  usagePercentage: number
  nextResetAt: number | null
}

// 成功响应
export interface SuccessResponse {
  success: boolean
  message: string
}

// 错误响应
export interface AdminErrorResponse {
  error: {
    type: string
    message: string
  }
}

// 请求类型
export interface SetDisabledRequest {
  disabled: boolean
}

export interface SetPriorityRequest {
  priority: number
}

// 添加凭据请求
export interface AddCredentialRequest {
  refreshToken?: string
  authMethod?: 'social' | 'idc' | 'api_key'
  clientId?: string
  clientSecret?: string
  priority?: number
  authRegion?: string
  apiRegion?: string
  machineId?: string
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
  kiroApiKey?: string
  endpoint?: string
}

// 添加凭据响应
export interface AddCredentialResponse {
  success: boolean
  message: string
  credentialId: number
  email?: string
}

// === 阶段 5.3a 批量导入 token.json 类型 ===

export interface TokenJsonItem {
  provider?: string
  refreshToken?: string
  clientId?: string
  clientSecret?: string
  authMethod?: string
  priority?: number
  region?: string
  apiRegion?: string
  machineId?: string
}

export interface ImportTokenJsonRequest {
  dryRun?: boolean
  items: TokenJsonItem | TokenJsonItem[]
}

export type ImportAction = 'added' | 'skipped' | 'invalid'

export interface ImportItemResult {
  index: number
  fingerprint: string
  action: ImportAction
  reason?: string
  credentialId?: number
}

export interface ImportSummary {
  parsed: number
  added: number
  skipped: number
  invalid: number
}

export interface ImportTokenJsonResponse {
  summary: ImportSummary
  items: ImportItemResult[]
}
