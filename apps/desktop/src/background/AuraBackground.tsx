import { DotField } from './DotField';
import { useMediaPreference, useWindowActive } from './preferences';

export function AuraBackground({
  dark,
  animated,
}: {
  dark: boolean;
  animated: boolean;
}) {
  const reduced = useMediaPreference('(prefers-reduced-motion: reduce)');
  const forced = useMediaPreference('(forced-colors: active)');
  const active = useWindowActive();
  return (
    <div className="aura-background" aria-hidden="true">
      {!forced && (
        <DotField dark={dark} animated={animated && !reduced} active={active} />
      )}
    </div>
  );
}
