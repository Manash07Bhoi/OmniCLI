import * as React from "react"
import { X } from "lucide-react"

export interface ToastProps extends React.HTMLAttributes<HTMLDivElement> {
  open?: boolean
  onOpenChange?: (open: boolean) => void
}
export type ToastActionElement = React.ReactElement

export function Toaster() {
  return null
}

export { X as Cross2Icon }
