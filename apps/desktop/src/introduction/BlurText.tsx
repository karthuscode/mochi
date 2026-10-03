// Adapted from React Bits BlurText (David Haz). See THIRD_PARTY_NOTICES.md.
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { useMediaPreference } from '../background/preferences';

export function BlurText({
  text,
  delay = 150,
  animateBy = 'words',
  direction = 'top',
  className = '',
  active = true,
  onAnimationComplete,
}: {
  text: string;
  delay?: number;
  animateBy?: 'words' | 'letters';
  direction?: 'top' | 'bottom';
  className?: string;
  active?: boolean;
  onAnimationComplete?: () => void;
}) {
  const reduced = useMediaPreference('(prefers-reduced-motion: reduce)');
  const forced = useMediaPreference('(forced-colors: active)');
  const ref = useRef<HTMLSpanElement>(null);
  const [inView, setInView] = useState(
    () => typeof IntersectionObserver === 'undefined',
  );
  useEffect(() => {
    const element = ref.current;
    if (!element || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setInView(true);
          observer.disconnect();
        }
      },
      { threshold: 0.1 },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const segments = animateBy === 'words' ? text.split(' ') : Array.from(text);
  const animate = active && !reduced && !forced;
  return (
    <span ref={ref} className={`blur-text ${className}`}>
      <span className="sr-only">{text}</span>
      <span className="blur-text-segments" aria-hidden="true">
        {segments.map((segment, index) =>
          animate ? (
            <motion.span
              key={index}
              className="blur-text-segment"
              initial={{
                filter: 'blur(10px)',
                opacity: 0,
                y: direction === 'top' ? -35 : 35,
              }}
              animate={
                inView
                  ? {
                      filter: ['blur(10px)', 'blur(5px)', 'blur(0px)'],
                      opacity: [0, 0.5, 1],
                      y: [
                        direction === 'top' ? -35 : 35,
                        direction === 'top' ? 5 : -5,
                        0,
                      ],
                    }
                  : false
              }
              transition={{
                duration: 0.7,
                times: [0, 0.5, 1],
                delay: (index * delay) / 1000,
                ease: 'easeOut',
              }}
              {...(index === segments.length - 1 && onAnimationComplete
                ? { onAnimationComplete }
                : {})}
            >
              {segment === ' ' ? '\u00a0' : segment}
              {animateBy === 'words' && index < segments.length - 1
                ? '\u00a0'
                : ''}
            </motion.span>
          ) : (
            <span key={index} className="blur-text-segment">
              {segment === ' ' ? '\u00a0' : segment}
              {animateBy === 'words' && index < segments.length - 1
                ? '\u00a0'
                : ''}
            </span>
          ),
        )}
      </span>
    </span>
  );
}
