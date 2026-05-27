import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button, Input, Modal, Select, Space, Table, Tag, Typography } from 'antd'

type RuntimeProfileItem = {
  language: string
  installer: string
  version: string
}

type RuntimeProfile = {
  id: string
  name: string
  items: RuntimeProfileItem[]
}

type InstallerStatus = {
  id: string
  installed: boolean
  hint: string
}

const LANGUAGE_OPTIONS = [
  { value: 'node', label: 'Node.js' },
  { value: 'python', label: 'Python' },
  { value: 'rust', label: 'Rust' },
  { value: 'java', label: 'Java' },
  { value: 'bun', label: 'Bun' },
  { value: 'deno', label: 'Deno' },
  { value: 'go', label: 'Go' },
  { value: 'php', label: 'PHP' },
]

function installerOptions(language: string) {
  if (language === 'node') return ['fnm', 'nvm']
  if (language === 'python') return ['pyenv']
  if (language === 'rust') return ['rustup']
  const isWin = navigator.userAgent.toLowerCase().includes('windows')
  if (language === 'java') return isWin ? ['winget'] : ['sdkman', 'homebrew']
  if (language === 'bun') return isWin ? ['winget'] : ['homebrew']
  if (language === 'deno') return isWin ? ['winget'] : ['homebrew']
  if (language === 'go') return isWin ? ['winget'] : ['goenv', 'homebrew']
  if (language === 'php') return isWin ? ['winget'] : ['phpenv', 'homebrew']
  return []
}

function newId() {
  try {
    return crypto.randomUUID()
  } catch {
    return String(Date.now())
  }
}

export function VersionManagerModal({
  open,
  onClose,
  onInstalled,
}: {
  open: boolean
  onClose: () => void
  onInstalled?: () => void
}) {
  const [profiles, setProfiles] = useState<RuntimeProfile[]>([])
  const [selectedId, setSelectedId] = useState<string>('')
  const [name, setName] = useState<string>('')
  const [items, setItems] = useState<RuntimeProfileItem[]>([])
  const [statuses, setStatuses] = useState<InstallerStatus[]>([])
  const [log, setLog] = useState<string>('')
  const [saving, setSaving] = useState(false)
  const [installing, setInstalling] = useState(false)

  const statusMap = useMemo(() => {
    const m: Record<string, InstallerStatus> = {}
    for (const s of statuses) m[s.id] = s
    return m
  }, [statuses])

  const loadAll = useCallback(async () => {
    const [p, s] = await Promise.all([
      invoke<RuntimeProfile[]>('list_runtime_profiles'),
      invoke<InstallerStatus[]>('get_installer_status'),
    ])
    setProfiles(p)
    setStatuses(s)
    const first = p[0]
    if (first) {
      setSelectedId(first.id)
      setName(first.name)
      setItems(first.items ?? [])
    }
  }, [])

  useEffect(() => {
    if (!open) return
    setLog('')
    loadAll()
  }, [loadAll, open])

  const selectProfile = useCallback(
    (id: string) => {
      setSelectedId(id)
      const p = profiles.find((x) => x.id === id)
      setName(p?.name ?? '')
      setItems(p?.items ?? [])
      setLog('')
    },
    [profiles],
  )

  const saveProfile = useCallback(async () => {
    if (!name.trim()) return
    setSaving(true)
    try {
      const profile: RuntimeProfile = {
        id: selectedId || newId(),
        name: name.trim(),
        items,
      }
      await invoke('upsert_runtime_profile', { profile })
      await loadAll()
      setSelectedId(profile.id)
      setLog('已保存配置文件。')
    } finally {
      setSaving(false)
    }
  }, [items, loadAll, name, selectedId])

  const createProfile = useCallback(() => {
    const id = newId()
    setSelectedId(id)
    setName('新配置')
    setItems([{ language: 'node', installer: 'fnm', version: '22.0.0' }])
    setLog('')
  }, [])

  const deleteProfile = useCallback(async () => {
    if (!selectedId) return
    if (selectedId === 'default') {
      setLog('默认配置不支持删除。')
      return
    }
    Modal.confirm({
      title: '删除配置文件',
      content: '确认删除该配置文件？',
      okText: '删除',
      okButtonProps: { danger: true },
      cancelText: '取消',
      async onOk() {
        await invoke('delete_runtime_profile', { id: selectedId })
        await loadAll()
        setLog('已删除配置文件。')
      },
    })
  }, [loadAll, selectedId])

  const addItem = useCallback(() => {
    setItems((old) => [...old, { language: 'node', installer: 'fnm', version: '' }])
  }, [])

  const removeItem = useCallback((idx: number) => {
    setItems((old) => old.filter((_, i) => i !== idx))
  }, [])

  const updateItem = useCallback((idx: number, patch: Partial<RuntimeProfileItem>) => {
    setItems((old) =>
      old.map((x, i) => {
        if (i !== idx) return x
        const next = { ...x, ...patch }
        if (patch.language) {
          const opts = installerOptions(patch.language)
          if (opts.length > 0 && !opts.includes(next.installer)) next.installer = opts[0]
        }
        return next
      }),
    )
  }, [])

  const installAll = useCallback(async () => {
    setInstalling(true)
    setLog('')
    try {
      const lines: string[] = []
      for (const it of items) {
        const st = statusMap[it.installer]
        if (!st?.installed) {
          const hint = st?.hint || (await invoke<string>('get_installer_bootstrap', { installer: it.installer }))
          lines.push(
            `✗ ${it.language} ${it.version}：安装器 ${it.installer} 未检测到。\n建议先执行：\n${hint}`,
          )
          break
        }
        try {
          const out = await invoke<string>('install_runtime', {
            language: it.language,
            installer: it.installer,
            version: it.version,
          })
          lines.push(`✓ ${it.language} ${it.version}（${it.installer}）\n${out || '完成'}`)
        } catch (e) {
          lines.push(`✗ ${it.language} ${it.version}（${it.installer}）\n${String(e)}`)
          break
        }
      }
      setLog(lines.join('\n\n'))
      onInstalled?.()
    } finally {
      setInstalling(false)
    }
  }, [items, onInstalled, statusMap])

  const clearItems = useCallback(() => {
    Modal.confirm({
      title: '一键清空',
      content: '确认清空当前配置文件的全部条目？',
      okText: '清空',
      okButtonProps: { danger: true },
      cancelText: '取消',
      onOk() {
        setItems([])
        setLog('已清空条目。')
      },
    })
  }, [])

  return (
    <Modal open={open} title="语言版本管理" onCancel={onClose} footer={null} width={1040}>
      <div style={{ display: 'grid', gap: 12 }}>
        <Space wrap style={{ justifyContent: 'space-between' }}>
          <Space wrap>
            <Select
              style={{ width: 260 }}
              value={selectedId || undefined}
              placeholder="选择配置文件"
              options={profiles.map((p) => ({ value: p.id, label: p.name }))}
              onChange={selectProfile}
            />
            <Input
              style={{ width: 260 }}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="配置文件名称"
            />
          </Space>
          <Space wrap>
            <Button onClick={createProfile}>新建</Button>
            <Button danger onClick={deleteProfile} disabled={!selectedId || selectedId === 'default'}>
              删除
            </Button>
            <Button type="primary" onClick={saveProfile} loading={saving} disabled={!name.trim()}>
              保存
            </Button>
            <Button type="primary" onClick={installAll} loading={installing} disabled={items.length === 0}>
              一键安装
            </Button>
            <Button danger onClick={clearItems} disabled={items.length === 0}>
              一键清空
            </Button>
          </Space>
        </Space>

        <Table
          size="small"
          rowKey={(_, idx) => String(idx)}
          pagination={false}
          dataSource={items}
          columns={[
            {
              title: '语言',
              dataIndex: 'language',
              width: 180,
              render: (v, _, idx) => (
                <Select
                  style={{ width: 160 }}
                  value={String(v)}
                  options={LANGUAGE_OPTIONS}
                  onChange={(val) => updateItem(idx, { language: val })}
                />
              ),
            },
            {
              title: '安装器',
              dataIndex: 'installer',
              width: 220,
              render: (v, record, idx) => {
                const opts = installerOptions(record.language).map((x) => ({ value: x, label: x }))
                const st = statusMap[String(v)]
                return (
                  <Space wrap>
                    <Select
                      style={{ width: 120 }}
                      value={String(v)}
                      options={opts}
                      onChange={(val) => updateItem(idx, { installer: val })}
                    />
                    {st?.installed ? (
                      <Tag color="green">已安装</Tag>
                    ) : (
                      <Tag color="red">未安装</Tag>
                    )}
                  </Space>
                )
              },
            },
            {
              title: '版本',
              dataIndex: 'version',
              width: 220,
              render: (v, _, idx) => (
                <Input
                  value={String(v)}
                  placeholder="例如：22.0.0 / 3.12.13 / stable"
                  onChange={(e) => updateItem(idx, { version: e.target.value })}
                />
              ),
            },
            {
              title: '安装建议（未安装时）',
              dataIndex: 'installer',
              render: (v) => {
                const st = statusMap[String(v)]
                if (st?.installed) return <Typography.Text type="secondary">-</Typography.Text>
                return (
                  <Typography.Text className="mono" copyable={{ text: st?.hint || '' }}>
                    {st?.hint || '-'}
                  </Typography.Text>
                )
              },
            },
            {
              title: '',
              width: 110,
              render: (_, __, idx) => (
                <Button danger type="link" onClick={() => removeItem(idx)}>
                  删除
                </Button>
              ),
            },
          ]}
        />

        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
          <Button onClick={addItem}>+ 添加条目</Button>
          <Button
            onClick={async () => {
              await loadAll()
              setLog('已刷新安装器状态。')
            }}
          >
            刷新安装器状态
          </Button>
        </div>

        <Typography.Paragraph
          className="mono"
          style={{ marginBottom: 0, whiteSpace: 'pre-wrap' }}
          copyable={log ? { text: log } : false}
        >
          {log || '提示：在干净环境下，先按“安装建议”安装对应安装器，再点击“一键安装”。'}
        </Typography.Paragraph>
      </div>
    </Modal>
  )
}
