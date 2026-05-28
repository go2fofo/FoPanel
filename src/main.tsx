/*
 * @Author: fofo
 * @Date: 2026-05-26 15:51:07
 * @LastEditTime: 2026-05-28 14:58:55
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src/main.tsx
 */
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { ConfigProvider, theme } from 'antd'
import zhCN from 'antd/locale/zh_CN'
import 'antd/dist/reset.css'
import './index.css'
import App from './App.tsx'
import { setWasmUrl } from '@lottiefiles/dotlottie-react'

setWasmUrl('/lottie/dotlottie-player.wasm')

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm: theme.darkAlgorithm,
        token: {
          colorPrimary: '#55c7c2',
          colorInfo: '#5a8cff',
          colorSuccess: '#46c4a8',
          colorWarning: '#d3a23a',
          colorError: '#ef4444',
          borderRadius: 12,
          colorBgBase: '#050505',
          colorBgContainer: '#0c1020',
          colorBgElevated: '#0f172a',
          colorTextBase: 'rgba(255, 255, 255, 0.88)',
        },
      }}
    >
      <App />
    </ConfigProvider>
  </StrictMode>,
)
