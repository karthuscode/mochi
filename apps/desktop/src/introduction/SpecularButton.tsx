// Adapted from React Bits SpecularButton (David Haz). See THIRD_PARTY_NOTICES.md.
import { useEffect, useRef } from 'react';
import type { ButtonHTMLAttributes } from 'react';
import { useMediaPreference, useWindowActive } from '../background/preferences';
import { createSpecularEffect } from './specular-effect';

export function SpecularButton({
  children,
  active = true,
  dark,
  className = '',
  type = 'button',
  disabled,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean;
  dark: boolean;
}) {
  const button = useRef<HTMLButtonElement>(null);
  const effect = useRef<HTMLSpanElement>(null);
  const reduced = useMediaPreference('(prefers-reduced-motion: reduce)');
  const forced = useMediaPreference('(forced-colors: active)');
  const windowActive = useWindowActive();
  useEffect(() => {
    if (
      !active ||
      !windowActive ||
      reduced ||
      forced ||
      disabled ||
      !button.current ||
      !effect.current
    )
      return;
    return createSpecularEffect(button.current, effect.current, dark);
  }, [active, windowActive, reduced, forced, disabled, dark]);
  return (
    <button
      ref={button}
      type={type}
      disabled={disabled}
      {...props}
      className={`specular-button ${className}`}
    >
      <span ref={effect} className="specular-button__fx" aria-hidden="true" />
      <span className="specular-button__label">{children}</span>
    </button>
  );
}
