import { useState } from 'react'
import { AlertCircle, Eye, EyeOff } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import type {
  ConfigJson,
  ConfigSchemaField,
  ConfigSchemaGroup,
  ConfigSchemaResponse,
} from '@/types/api'

interface ConfigFormProps {
  schema: ConfigSchemaResponse
  /** 当前编辑中的 Config 对象 */
  value: ConfigJson
  /** 字段路径 → 新值；form 内部直接 mutate value 并通过此回调上抛 */
  onChange: (next: ConfigJson) => void
}

/** 点号路径读取嵌套值 */
function getByPath(obj: ConfigJson, path: string): unknown {
  return path.split('.').reduce<unknown>((acc, key) => {
    if (acc && typeof acc === 'object' && key in (acc as Record<string, unknown>)) {
      return (acc as Record<string, unknown>)[key]
    }
    return undefined
  }, obj)
}

/** 点号路径写入嵌套值（克隆 + 不可变写入） */
function setByPath(obj: ConfigJson, path: string, value: unknown): ConfigJson {
  const keys = path.split('.')
  const result = { ...obj }
  let cursor: Record<string, unknown> = result as Record<string, unknown>
  for (let i = 0; i < keys.length - 1; i++) {
    const k = keys[i]
    const existing = cursor[k]
    const cloned =
      existing && typeof existing === 'object' && !Array.isArray(existing)
        ? { ...(existing as Record<string, unknown>) }
        : {}
    cursor[k] = cloned
    cursor = cloned as Record<string, unknown>
  }
  cursor[keys[keys.length - 1]] = value
  return result
}

export function ConfigForm({ schema, value, onChange }: ConfigFormProps) {
  return (
    <div className="space-y-4 max-h-[480px] overflow-y-auto pr-2">
      {schema.groups.map((group) => (
        <GroupCard key={group.id} group={group} value={value} onChange={onChange} />
      ))}
    </div>
  )
}

function GroupCard({
  group,
  value,
  onChange,
}: {
  group: ConfigSchemaGroup
  value: ConfigJson
  onChange: (next: ConfigJson) => void
}) {
  return (
    <div className="border rounded-md">
      <div className="flex items-center justify-between px-3 py-2 border-b bg-muted/40">
        <div>
          <div className="font-medium text-sm flex items-center gap-2">
            {group.label}
            {group.needsRestart && (
              <Badge variant="outline" className="text-amber-600 text-xs">⚠ 需重启</Badge>
            )}
            {group.sensitive && (
              <Badge variant="destructive" className="text-xs">🔒 敏感</Badge>
            )}
            {!group.needsRestart && !group.sensitive && (
              <Badge variant="secondary" className="text-xs">🟢 热生效</Badge>
            )}
          </div>
          {group.description && (
            <div className="text-xs text-muted-foreground mt-0.5">{group.description}</div>
          )}
        </div>
      </div>
      <div className="p-3 space-y-3">
        {group.fields.map((f) => (
          <FieldRow
            key={f.key}
            field={f}
            value={getByPath(value, f.key)}
            onChange={(v) => onChange(setByPath(value, f.key, v))}
          />
        ))}
      </div>
    </div>
  )
}

function FieldRow({
  field,
  value,
  onChange,
}: {
  field: ConfigSchemaField
  value: unknown
  onChange: (v: unknown) => void
}) {
  const [showSensitive, setShowSensitive] = useState(false)

  return (
    <div className="grid grid-cols-12 gap-2 items-start">
      {/* 标签列 */}
      <div className="col-span-4">
        <div className="text-sm font-medium flex items-center gap-1">
          {field.label}
          {field.needsRestart && (
            <span className="text-amber-600 text-xs" title="需重启">⚠</span>
          )}
          {field.sensitive && (
            <span className="text-red-600 text-xs" title="敏感字段">🔒</span>
          )}
        </div>
        <div className="font-mono text-[10px] text-muted-foreground">{field.key}</div>
      </div>

      {/* 输入列 */}
      <div className="col-span-8 space-y-1">
        <FieldInput
          field={field}
          value={value}
          onChange={onChange}
          revealed={showSensitive}
        />
        {field.sensitive && field.type === 'string' && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-6 px-1 text-xs"
            onClick={() => setShowSensitive((v) => !v)}
          >
            {showSensitive ? <EyeOff className="h-3 w-3 mr-0.5" /> : <Eye className="h-3 w-3 mr-0.5" />}
            {showSensitive ? '隐藏' : '显示'}
          </Button>
        )}
        {field.description && (
          <div className="text-xs text-muted-foreground">{field.description}</div>
        )}
        {field.warning && (
          <div className="text-xs flex items-start gap-1 text-amber-600">
            <AlertCircle className="h-3 w-3 mt-0.5 shrink-0" />
            {field.warning}
          </div>
        )}
      </div>
    </div>
  )
}

function FieldInput({
  field,
  value,
  onChange,
  revealed,
}: {
  field: ConfigSchemaField
  value: unknown
  onChange: (v: unknown) => void
  revealed: boolean
}) {
  if (field.type === 'boolean') {
    return (
      <div className="flex items-center gap-2">
        <Switch
          checked={Boolean(value)}
          onCheckedChange={(checked) => onChange(checked)}
        />
        <span className="text-xs text-muted-foreground">
          {Boolean(value) ? '已启用' : '已禁用'}
        </span>
      </div>
    )
  }

  if (field.type === 'enum') {
    return (
      <select
        className="w-full h-9 px-2 text-sm border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/40"
        value={typeof value === 'string' ? value : ''}
        onChange={(e) => onChange(e.target.value)}
      >
        {field.enumOptions?.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    )
  }

  if (field.type === 'number') {
    return (
      <Input
        type="number"
        min={field.min}
        max={field.max}
        placeholder={field.placeholder}
        value={typeof value === 'number' ? value : ''}
        onChange={(e) => {
          const raw = e.target.value
          if (raw === '') {
            onChange(field.nullable ? null : 0)
          } else {
            const n = Number(raw)
            onChange(Number.isNaN(n) ? 0 : n)
          }
        }}
        className="h-9"
      />
    )
  }

  // string
  const inputType = field.sensitive && !revealed ? 'password' : 'text'
  return (
    <Input
      type={inputType}
      placeholder={field.placeholder}
      value={typeof value === 'string' ? value : value == null ? '' : String(value)}
      onChange={(e) => {
        const v = e.target.value
        // nullable + 空串 → null
        if (field.nullable && v === '') {
          onChange(null)
        } else {
          onChange(v)
        }
      }}
      className="h-9 font-mono text-xs"
    />
  )
}
