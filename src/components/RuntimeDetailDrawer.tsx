import { Button, Descriptions, Divider, Drawer, Space, Table, Tag, Typography } from 'antd'
import { LottiePlayer } from './LottiePlayer'

export type RuntimeVersion = {
  language: string
  version: string
  path: string
  active: boolean
  source: string
}

export type RuntimePackage = {
  name: string
  version: string
}

export type RuntimeDetail = {
  runtime: RuntimeVersion
  info: Record<string, string>
  packages: RuntimePackage[]
}

function sourceLabel(source: string) {
  if (source === 'fnm') return 'fnm'
  if (source === 'nvm') return 'nvm'
  if (source === 'pyenv') return 'pyenv'
  if (source === 'rustup') return 'rustup'
  if (source === 'homebrew') return 'Homebrew'
  if (source === 'framework') return '系统 Framework'
  if (source === 'standalone') return '独立安装'
  if (source === 'volta') return 'Volta'
  if (source === 'asdf') return 'asdf'
  if (source === 'manual') return '手动'
  if (source === 'path') return 'PATH'
  return source
}

type Props = {
  open: boolean
  loading: boolean
  detail: RuntimeDetail | null
  onClose: () => void
  onCheckUpgrade?: (runtime: RuntimeVersion) => void
}

export function RuntimeDetailDrawer({ open, loading, detail, onClose, onCheckUpgrade }: Props) {
  const runtime = detail?.runtime
  const title = runtime
    ? `${runtime.language.toUpperCase()} ${runtime.version}`
    : '运行时详情'
  const canCheckUpgrade = runtime?.language === 'deno' || runtime?.language === 'bun'

  return (
    <Drawer open={open} onClose={onClose} title={title} width={720}>
      {runtime ? (
        <Space direction="vertical" style={{ width: '100%' }} size={16}>
          <Space wrap>
            <Tag color={runtime.active ? 'green' : 'default'}>
              {runtime.active ? '当前' : '可用'}
            </Tag>
            <Tag color="blue">{sourceLabel(runtime.source)}</Tag>
            <Typography.Text
              className="mono"
              ellipsis={{ tooltip: runtime.path }}
              style={{ maxWidth: 520 }}
              copyable={{ text: runtime.path }}
            >
              {runtime.path}
            </Typography.Text>
          </Space>

          <Space wrap>
            {onCheckUpgrade && canCheckUpgrade ? (
              <Button onClick={() => onCheckUpgrade(runtime)}>检查更新</Button>
            ) : null}
          </Space>

          <Divider style={{ margin: '8px 0' }} />

          <Descriptions column={1} size="small" bordered>
            {Object.entries(detail.info).map(([k, v]) => (
              <Descriptions.Item key={k} label={k}>
                <Typography.Text className={k === 'executable' ? 'mono' : undefined}>
                  {v || '-'}
                </Typography.Text>
              </Descriptions.Item>
            ))}
          </Descriptions>

          <Divider style={{ margin: '8px 0' }} />

          <Typography.Title level={5} style={{ margin: 0 }}>
            依赖/扩展
          </Typography.Title>
          <Table
            size="small"
            rowKey={(r) => `${r.name}@${r.version}`}
            pagination={{ pageSize: 10 }}
            columns={[
              { title: '名称', dataIndex: 'name' },
              { title: '版本', dataIndex: 'version', width: 160 },
            ]}
            dataSource={detail.packages}
            loading={
              loading
                ? {
                    spinning: true,
                    indicator: <LottiePlayer src="/lottie/loading-dots.json" autoplay loop size={64} />,
                  }
                : false
            }
          />
        </Space>
      ) : (
        <div style={{ display: 'grid', placeItems: 'center', gap: 12, padding: '24px 0' }}>
          {loading ? (
            <LottiePlayer src="/lottie/loading-dots.json" autoplay loop size={120} />
          ) : (
            <LottiePlayer src="/lottie/empty-box.json" autoplay loop size={160} />
          )}
          <Typography.Text type="secondary">
            {loading ? '加载中...' : '未选择运行时'}
          </Typography.Text>
        </div>
      )}
    </Drawer>
  )
}
