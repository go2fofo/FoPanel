import { Modal, Typography } from 'antd'

export type FeedbackModalType = 'error' | 'success' | 'info'

type Props = {
  open: boolean
  type: FeedbackModalType
  title: string
  content: string
  onClose: () => void
}

export function FeedbackModal({ open, type, title, content, onClose }: Props) {
  const color =
    type === 'error' ? '#ef4444' : type === 'success' ? '#22c55e' : '#60a5fa'

  return (
    <Modal
      open={open}
      title={<span style={{ color }}>{title}</span>}
      onCancel={onClose}
      onOk={onClose}
      okText="确定"
      cancelButtonProps={{ style: { display: 'none' } }}
    >
      <Typography.Paragraph style={{ marginBottom: 0, whiteSpace: 'pre-wrap' }}>
        {content}
      </Typography.Paragraph>
    </Modal>
  )
}

