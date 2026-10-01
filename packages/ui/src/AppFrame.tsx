import type { ReactNode } from 'react';

interface AppFrameProps {
  readonly children: ReactNode;
}

export function AppFrame({ children }: AppFrameProps) {
  return <main className="app-frame">{children}</main>;
}
