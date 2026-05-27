/*
 * @Author: fofo
 * @Date: 2026-05-26 15:51:07
 * @LastEditTime: 2026-05-26 16:17:33
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src/App.tsx
 */
import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  Button,
  Form,
  Input,
  Modal,
  Select,
  Spin,
  Table,
  Tag,
  Typography,
} from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { FeedbackModal, type FeedbackModalType } from './components/FeedbackModal'
import {
  RuntimeDetailDrawer,
  type RuntimeDetail,
  type RuntimeVersion,
} from './components/RuntimeDetailDrawer'
import { VersionManagerModal } from './components/VersionManagerModal'
import './App.css'

type InstallerOption = { id: string; label: string; description: string }

type SystemRuntime = {
  language: string
  version: string
  path: string
  source: string
  using_fopanel: boolean
}

const LANGUAGE_ORDER = ['python', 'node', 'java', 'bun', 'deno', 'go', 'rust', 'php']

function App() {
  const [loading, setLoading] = useState(false)
  const [runtimes, setRuntimes] = useState<RuntimeVersion[]>([])
  const [systemRuntimes, setSystemRuntimes] = useState<Record<string, SystemRuntime>>({})
  const [busy, setBusy] = useState(0)

  const [feedbackOpen, setFeedbackOpen] = useState(false)
  const [feedbackType, setFeedbackType] = useState<FeedbackModalType>('info')
  const [feedbackTitle, setFeedbackTitle] = useState('')
  const [feedbackContent, setFeedbackContent] = useState('')

  const [manualOpen, setManualOpen] = useState(false)
  const [manualSubmitting, setManualSubmitting] = useState(false)
  const [manualForm] = Form.useForm()

  const [versionManagerOpen, setVersionManagerOpen] = useState(false)

  const [installOpen, setInstallOpen] = useState(false)
  const [installLoading, setInstallLoading] = useState(false)
  const [installSubmitting, setInstallSubmitting] = useState(false)
  const [installForm] = Form.useForm()
  const [installerOptions, setInstallerOptions] = useState<
    { id: string; label: string; description: string }[]
  >([])

  const [detailOpen, setDetailOpen] = useState(false)
  const [detailLoading, setDetailLoading] = useState(false)
  const [detail, setDetail] = useState<RuntimeDetail | null>(null)

  const [switchOpen, setSwitchOpen] = useState(false)
  const [switchLang, setSwitchLang] = useState<string>('')

  const openFeedback = useCallback((type: FeedbackModalType, title: string, content: string) => {
    setFeedbackType(type)
    setFeedbackTitle(title)
    setFeedbackContent(content)
    setFeedbackOpen(true)
  }, [])

  const startBusy = useCallback(() => setBusy((x) => x + 1), [])
  const endBusy = useCallback(() => setBusy((x) => (x > 0 ? x - 1 : 0)), [])

  const refreshSystemRuntimes = useCallback(async () => {
    try {
      const list = await invoke<SystemRuntime[]>('get_system_runtimes')
      const map: Record<string, SystemRuntime> = {}
      for (const r of list) map[r.language] = r
      setSystemRuntimes(map)
    } catch {
      setSystemRuntimes({})
    }
  }, [])

  const refreshRuntimes = useCallback(async () => {
    setLoading(true)
    try {
      const list = await invoke<RuntimeVersion[]>('scan_runtimes')
      setRuntimes(list)
    } catch (e) {
      setRuntimes([])
      openFeedback('error', '扫描失败', String(e))
    } finally {
      setLoading(false)
    }
  }, [openFeedback])

  const loadInstallers = useCallback(
    async (language: string) => {
      setInstallLoading(true)
      try {
        const list = await invoke<InstallerOption[]>('list_installers', { language })
        setInstallerOptions(list)
        const current = installForm.getFieldValue('installer')
        if (!current && list.length > 0) {
          installForm.setFieldsValue({ installer: list[0].id })
        }
      } catch (e) {
        setInstallerOptions([])
        openFeedback('error', '加载安装器失败', String(e))
      } finally {
        setInstallLoading(false)
      }
    },
    [installForm, openFeedback],
  )

  const handleRemove = useCallback(
    async (runtime: RuntimeVersion) => {
      if (runtime.source !== 'manual') return
      Modal.confirm({
        title: '删除运行时',
        content: '确认删除该运行时？删除后将不会在列表中显示。',
        okText: '确定',
        cancelText: '取消',
        async onOk() {
          startBusy()
          try {
            await invoke('remove_runtime', { runtime })
            await refreshRuntimes()
            openFeedback('success', '删除成功', '')
          } catch (e) {
            openFeedback('error', '删除失败', String(e))
          } finally {
            endBusy()
          }
        },
      })
    },
    [endBusy, openFeedback, refreshRuntimes, startBusy],
  )

  const canUninstall = useCallback((runtime: RuntimeVersion) => {
    if (runtime.language === 'node' && (runtime.source === 'fnm' || runtime.source === 'nvm')) return true
    if (runtime.language === 'python' && runtime.source === 'pyenv') return true
    if (runtime.language === 'rust' && runtime.source === 'rustup') return true
    if (runtime.language === 'go' && runtime.source === 'goenv') return true
    if (runtime.language === 'php' && runtime.source === 'phpenv') return true
    if (runtime.language === 'java' && (runtime.source === 'homebrew' || runtime.source === 'winget' || runtime.source === 'sdkman'))
      return true
    return false
  }, [])

  const handleUninstall = useCallback(
    async (runtime: RuntimeVersion) => {
      Modal.confirm({
        title: '卸载版本',
        content:
          `将通过 ${runtime.source} 卸载 ${displayName(runtime.language)} ${runtime.version}。\n` +
          '该操作会从系统中删除对应版本，可能影响其他项目使用，请谨慎操作。',
        okText: '确认卸载',
        okButtonProps: { danger: true },
        cancelText: '取消',
        async onOk() {
          startBusy()
          try {
            const output = await invoke<string>('uninstall_runtime', { runtime })
            await refreshRuntimes()
            openFeedback('success', '卸载完成', output || '完成')
          } catch (e) {
            openFeedback('error', '卸载失败', String(e))
          } finally {
            endBusy()
          }
        },
      })
    },
    [endBusy, openFeedback, refreshRuntimes, startBusy],
  )

  const handleCheckUpgrade = useCallback(
    async (runtime: RuntimeVersion) => {
      startBusy()
      try {
        const out = await invoke<string>('check_runtime_upgrade', { runtime })
        openFeedback('info', '检查更新结果', out || '无输出')
      } catch (e) {
        openFeedback('error', '检查更新失败', String(e))
      } finally {
        endBusy()
      }
    },
    [endBusy, openFeedback, startBusy],
  )

  const openDetail = useCallback(
    async (runtime: RuntimeVersion) => {
      startBusy()
      setDetailOpen(true)
      setDetailLoading(true)
      setDetail(null)
      try {
        const d = await invoke<RuntimeDetail>('get_runtime_detail', { runtime })
        setDetail(d)
      } catch (e) {
        openFeedback('error', '加载详情失败', String(e))
        setDetailOpen(false)
      } finally {
        setDetailLoading(false)
        endBusy()
      }
    },
    [endBusy, openFeedback, startBusy],
  )

  const languages = useMemo(() => {
    const set = new Set<string>()
    for (const r of runtimes) set.add(r.language)
    const list = [...set]
    list.sort((a, b) => {
      const ia = LANGUAGE_ORDER.indexOf(a)
      const ib = LANGUAGE_ORDER.indexOf(b)
      if (ia !== -1 || ib !== -1) {
        return (ia === -1 ? 999 : ia) - (ib === -1 ? 999 : ib)
      }
      return a.localeCompare(b)
    })
    return list
  }, [runtimes])

  const byLanguage = useMemo(() => {
    const map = new Map<string, RuntimeVersion[]>()
    for (const r of runtimes) {
      const list = map.get(r.language) ?? []
      list.push(r)
      map.set(r.language, list)
    }
    for (const [k, list] of map.entries()) {
      list.sort((a, b) => a.version.localeCompare(b.version))
      map.set(k, list)
    }
    return map
  }, [runtimes])

  const activeByLanguage = useMemo(() => {
    const map = new Map<string, RuntimeVersion>()
    for (const lang of languages) {
      const list = byLanguage.get(lang) ?? []
      const active = list.find((x) => x.active) ?? list[0]
      if (active) map.set(lang, active)
    }
    return map
  }, [byLanguage, languages])

  function displayName(lang: string) {
    if (lang === 'node') return 'Node.js'
    if (lang === 'java') return 'Java'
    if (lang === 'bun') return 'Bun'
    if (lang === 'deno') return 'Deno'
    if (lang === 'rust') return 'Rust'
    if (lang === 'go') return 'Go'
    if (lang === 'php') return 'PHP'
    if (lang === 'python') return 'Python'
    return lang.toUpperCase()
  }

  function sourceLabel(source: string) {
    if (source === 'fnm') return 'fnm'
    if (source === 'nvm') return 'nvm'
    if (source === 'pyenv') return 'pyenv'
    if (source === 'rustup') return 'rustup'
    if (source === 'goenv') return 'goenv'
    if (source === 'phpenv') return 'phpenv'
    if (source === 'sdkman') return 'SDKMAN'
    if (source === 'homebrew') return 'Homebrew'
    if (source === 'winget') return 'winget'
    if (source === 'framework') return '系统 Framework'
    if (source === 'standalone') return '独立安装'
    if (source === 'volta') return 'Volta'
    if (source === 'asdf') return 'asdf'
    if (source === 'manual') return '手动'
    if (source === 'path') return 'PATH'
    if (source.startsWith('jvm-')) return `JVM(${source.slice(4)})`
    if (source === 'jvm') return 'JVM'
    return source
  }

  useEffect(() => {
    const t = window.setTimeout(() => {
      refreshRuntimes()
      refreshSystemRuntimes()
    }, 0)
    return () => window.clearTimeout(t)
  }, [refreshRuntimes, refreshSystemRuntimes])

  const submitManual = useCallback(async () => {
    const values = await manualForm.validateFields()
    setManualSubmitting(true)
    try {
      await invoke('add_manual_runtime', {
        runtime: {
          language: values.language,
          version: values.version,
          path: values.path,
        },
      })
      setManualOpen(false)
      manualForm.resetFields()
      await refreshRuntimes()
      openFeedback('success', '添加成功', '已写入手动运行时配置')
    } catch (e) {
      openFeedback('error', '添加失败', String(e))
    } finally {
      setManualSubmitting(false)
    }
  }, [manualForm, openFeedback, refreshRuntimes])

  const submitInstall = useCallback(async () => {
    const values = await installForm.validateFields()
    setInstallSubmitting(true)
    try {
      const output = await invoke<string>('install_runtime', {
        language: values.language,
        installer: values.installer,
        version: values.version,
      })
      setInstallOpen(false)
      installForm.resetFields()
      setInstallerOptions([])
      await refreshRuntimes()
      openFeedback('success', '安装完成', output || '完成')
    } catch (e) {
      openFeedback('error', '安装失败', String(e))
    } finally {
      setInstallSubmitting(false)
    }
  }, [installForm, openFeedback, refreshRuntimes])

  return (
    <div className="page">
      <Spin fullscreen spinning={busy > 0 || loading || installLoading || installSubmitting || manualSubmitting} />
      <div className="container">
        <header className="dashboard">
          <div className="brand">
            <h1>FoPanel</h1>
            <p>一体化开发环境工作台</p>
          </div>
          <div className="stats">
            <div className="stat-item">
              <div className="stat-val">{languages.length}</div>
              <div className="stat-label">已托管语言</div>
            </div>
            <button
              className="add-btn"
              type="button"
              onClick={() => {
                setVersionManagerOpen(true)
              }}
            >
              版本管理
            </button>
            <button
              className="add-btn primary"
              type="button"
              onClick={() => {
                setInstallOpen(true)
                installForm.setFieldsValue({ language: 'node', version: '' })
                loadInstallers('node')
              }}
            >
              + 新增环境
            </button>
            <button className="add-btn" type="button" onClick={() => setManualOpen(true)}>
              手动添加
            </button>
            <button
              className="add-btn"
              type="button"
              onClick={async () => {
                await refreshRuntimes()
                await refreshSystemRuntimes()
              }}
              disabled={loading}
            >
              <ReloadOutlined /> 刷新
            </button>
          </div>
        </header>

        <div className="grid">
          {languages.map((lang) => {
            const rt = activeByLanguage.get(lang)
            const list = byLanguage.get(lang) ?? []
            const isActive = !!rt?.active
            const version = rt?.version ? `v${rt.version}` : '-'
            const subtitle = isActive ? `${version} (Current)` : version
            const sources = [...new Set(list.map((x) => x.source))].map(sourceLabel).join(' / ')
            const sys = systemRuntimes[lang]
            const sysText = sys?.version
              ? `终端：${lang === 'python' ? sys.version : `v${sys.version}`} · ${sourceLabel(sys.source)}`
              : '终端：未检测到'

            return (
              <div
                key={lang}
                className={`card ${lang} ${isActive ? 'active' : ''}`}
                onClick={() => (rt ? openDetail(rt) : null)}
                role="button"
                tabIndex={0}
              >
                <div className="header-row">
                  <h2>{displayName(lang)}</h2>
                  <span className="status-pill">{isActive ? '本机 · 当前' : '本机 · 可用'}</span>
                </div>
                <div className="version-tag">
                  {subtitle} · {list.length} 个版本 · 来源：{sources || '-'} · {sysText}
                </div>
                <div className="actions">
                  <button
                    type="button"
                    className="btn-switch"
                    onClick={(e) => {
                      e.stopPropagation()
                      setSwitchLang(lang)
                      setSwitchOpen(true)
                    }}
                  >
                    版本列表
                  </button>
                  <button
                    type="button"
                    className="btn-manage"
                    onClick={(e) => {
                      e.stopPropagation()
                      if (rt) openDetail(rt)
                    }}
                  >
                    依赖/详情
                  </button>
                  {rt && rt.source === 'manual' ? (
                    <button
                      type="button"
                      className="btn-danger"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleRemove(rt)
                      }}
                    >
                      删除
                    </button>
                  ) : null}
                </div>
              </div>
            )
          })}
        </div>
      </div>

      <RuntimeDetailDrawer
        open={detailOpen}
        loading={detailLoading}
        detail={detail}
        onClose={() => setDetailOpen(false)}
        onCheckUpgrade={(rt) => handleCheckUpgrade(rt)}
      />

      <VersionManagerModal
        open={versionManagerOpen}
        onClose={() => setVersionManagerOpen(false)}
        onInstalled={async () => {
          await refreshRuntimes()
          await refreshSystemRuntimes()
        }}
      />

      <Modal
        open={switchOpen}
        title={`版本列表 · ${displayName(switchLang)}`}
        onCancel={() => setSwitchOpen(false)}
        footer={null}
        width={980}
      >
        <Table
          size="small"
          rowKey={(r) => `${r.language}-${r.source}-${r.path}-${r.version}`}
          pagination={{ pageSize: 8 }}
          dataSource={switchLang ? byLanguage.get(switchLang) ?? [] : []}
          onRow={(rt) => ({
            onClick: () => openDetail(rt),
          })}
          columns={[
            {
              title: '',
              width: 90,
              render: (_, rt) =>
                rt.active ? <Tag color="green">当前</Tag> : <Tag color="default">可用</Tag>,
            },
            { title: '版本', dataIndex: 'version', width: 160 },
            { title: '来源', dataIndex: 'source', width: 160, render: (v: string) => sourceLabel(v) },
            {
              title: '路径',
              dataIndex: 'path',
              render: (v: string) => (
                <Typography.Text className="mono" ellipsis={{ tooltip: v }} style={{ maxWidth: 460 }} copyable>
                  {v || '-'}
                </Typography.Text>
              ),
            },
            {
              title: '',
              width: 240,
              render: (_, rt) => {
                return (
                  <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
                    <Button
                      type="link"
                      onClick={(e) => {
                        e.stopPropagation()
                        openDetail(rt)
                      }}
                    >
                      详情
                    </Button>
                    <Button
                      type="link"
                      danger
                      style={{ display: rt.source === 'manual' ? undefined : 'none' }}
                      onClick={(e) => {
                        e.stopPropagation()
                        handleRemove(rt)
                      }}
                    >
                      删除
                    </Button>
                    {rt.source !== 'manual' && canUninstall(rt) ? (
                      <Button
                        type="link"
                        danger
                        onClick={(e) => {
                          e.stopPropagation()
                          handleUninstall(rt)
                        }}
                      >
                        卸载
                      </Button>
                    ) : null}
                  </div>
                )
              },
            },
          ]}
        />
      </Modal>

      <Modal
        open={installOpen}
        title="新增语言版本"
        onCancel={() => setInstallOpen(false)}
        onOk={submitInstall}
        okText="开始安装"
        cancelText="取消"
        confirmLoading={installSubmitting}
      >
        <Form
          form={installForm}
          layout="vertical"
          initialValues={{ language: 'node', version: '' }}
          onValuesChange={(changed) => {
            if (changed.language) {
              installForm.setFieldsValue({ installer: undefined })
              loadInstallers(changed.language)
            }
          }}
        >
          <Form.Item name="language" label="语言" rules={[{ required: true, message: '请选择语言' }]}>
            <Select
              options={[
                { value: 'node', label: 'node' },
                { value: 'python', label: 'python' },
                { value: 'java', label: 'java' },
                { value: 'bun', label: 'bun' },
                { value: 'deno', label: 'deno' },
                { value: 'go', label: 'go' },
                { value: 'rust', label: 'rust' },
                { value: 'php', label: 'php' },
              ]}
            />
          </Form.Item>

          <Form.Item
            name="installer"
            label="安装器"
            rules={[{ required: true, message: '当前环境未检测到可用安装器' }]}
          >
            <Select
              loading={installLoading}
              options={installerOptions.map((x) => ({ value: x.id, label: x.label }))}
              placeholder={installLoading ? '加载中...' : '请选择安装器'}
            />
          </Form.Item>

          {installerOptions.length > 0 ? (
            <Typography.Text type="secondary">
              {installerOptions.find((x) => x.id === installForm.getFieldValue('installer'))
                ?.description || ''}
            </Typography.Text>
          ) : null}

          <Form.Item name="version" label="版本" rules={[{ required: true, message: '请输入版本' }]}>
            <Input placeholder="例如：22.18.0 / 3.12.0 / stable / latest / 8.4" />
          </Form.Item>

          <Typography.Text type="secondary">
            没有可用安装器时，使用“手动添加”直接登记本机已有的可执行文件路径。
          </Typography.Text>
        </Form>
      </Modal>

      <Modal
        open={manualOpen}
        title="手动添加运行时"
        onCancel={() => setManualOpen(false)}
        onOk={submitManual}
        okText="添加"
        cancelText="取消"
        confirmLoading={manualSubmitting}
      >
        <Form form={manualForm} layout="vertical" initialValues={{ language: 'python' }}>
          <Form.Item name="language" label="语言" rules={[{ required: true, message: '请选择语言' }]}>
            <Select
              options={[
                { value: 'python', label: 'python' },
                { value: 'node', label: 'node' },
                { value: 'java', label: 'java' },
                { value: 'bun', label: 'bun' },
                { value: 'deno', label: 'deno' },
                { value: 'go', label: 'go' },
                { value: 'rust', label: 'rust' },
                { value: 'php', label: 'php' },
              ]}
            />
          </Form.Item>
          <Form.Item name="version" label="版本" rules={[{ required: true, message: '请输入版本' }]}>
            <Input placeholder="例如：3.12.0 / 22.18.0" />
          </Form.Item>
          <Form.Item name="path" label="可执行文件路径" rules={[{ required: true, message: '请输入路径' }]}>
            <Input placeholder="例如：/usr/local/bin/python3" />
          </Form.Item>
        </Form>
      </Modal>

      <FeedbackModal
        open={feedbackOpen}
        type={feedbackType}
        title={feedbackTitle}
        content={feedbackContent}
        onClose={() => setFeedbackOpen(false)}
      />
    </div>
  )
}

export default App
