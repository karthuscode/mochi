// Adapted from React Bits SpotlightCard (David Haz). See THIRD_PARTY_NOTICES.md.
import type { CSSProperties, ReactNode } from 'react';
import { useRef } from 'react';

export function SpotlightCard({
  children,
  className = '',
  spotlightColor = 'rgba(255, 112, 72, 0.16)',
}: {
  children: ReactNode;
  className?: string;
  spotlightColor?: string;
}) {
  const ref = useRef<HTMLElement>(null);
  return (
    <article
      ref={ref}
      className={`card-spotlight ${className}`}
      style={{ '--spotlight-color': spotlightColor } as CSSProperties}
      onPointerMove={(event) => {
        const card = ref.current;
        if (!card) return;
        const rect = card.getBoundingClientRect();
        card.style.setProperty('--mouse-x', `${event.clientX - rect.left}px`);
        card.style.setProperty('--mouse-y', `${event.clientY - rect.top}px`);
      }}
      onPointerLeave={() => {
        ref.current?.style.setProperty('--mouse-x', '50%');
        ref.current?.style.setProperty('--mouse-y', '50%');
      }}
    >
      {children}
    </article>
  );
}
