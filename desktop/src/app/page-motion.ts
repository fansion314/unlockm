export function pageVariants(reducedMotion: boolean) {
  return {
    enter: (direction: 1 | -1) => ({ opacity: 0, x: reducedMotion ? 0 : direction * 24 }),
    center: { opacity: 1, x: 0 },
    exit: (direction: 1 | -1) => ({ opacity: 0, x: reducedMotion ? 0 : -direction * 24 }),
  };
}

/**
 * Keep route and shared-layout transitions deterministic. Motion defaults a
 * layout animation to a spring; that feels nice on a pointer device but is
 * unnecessarily animated for a reduced-motion preference.
 */
export function pageTransition(reducedMotion: boolean) {
  if (reducedMotion) {
    return {
      duration: 0.01,
      ease: "linear" as const,
      layout: { duration: 0.01, ease: "linear" as const, type: "tween" as const },
    };
  }
  const ease: [number, number, number, number] = [0.16, 1, 0.3, 1];
  return {
    duration: 0.24,
    ease,
    layout: { duration: 0.24, ease, type: "tween" as const },
  };
}
