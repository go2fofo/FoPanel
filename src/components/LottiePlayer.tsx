/*
 * @Author: fofo
 * @Date: 2026-05-28 14:19:52
 * @LastEditTime: 2026-05-28 14:50:15
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src/components/LottiePlayer.tsx
 */
import { DotLottieReact } from '@lottiefiles/dotlottie-react'
import type { ComponentProps, CSSProperties } from 'react'

type DotProps = ComponentProps<typeof DotLottieReact>

type Props = Omit<DotProps, 'src' | 'style'> & {
  src: string
  size?: number | { width: number; height: number }
  style?: CSSProperties
}

export function LottiePlayer({ src, size = 96, style, ...rest }: Props) {
  const width = typeof size === 'number' ? size : size.width
  const height = typeof size === 'number' ? size : size.height

  return (
    <DotLottieReact
      src={src}
      style={{ width, height, display: 'block', ...style }}
      {...rest}
    />
  )
}
