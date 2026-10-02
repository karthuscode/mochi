import type { ReactNode } from 'react';

interface AppFrameProps {
  readonly children: ReactNode;
  readonly className?: string;
}

export function AppFrame({ children, className = '' }: AppFrameProps) {
  return <main className={`app-frame ${className}`}>{children}</main>;
}
