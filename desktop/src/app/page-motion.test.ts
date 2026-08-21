import { expect, test } from "vite-plus/test";
import { pageTransition, pageVariants } from "./page-motion";

test("page travel is directional and reduced motion removes horizontal travel", () => {
  expect(pageVariants(false).enter(1).x).toBe(24);
  expect(pageVariants(false).exit(1).x).toBe(-24);
  expect(pageVariants(false).enter(-1).x).toBe(-24);
  expect(pageVariants(false).exit(-1).x).toBe(24);
  expect(pageVariants(true).enter(1).x).toBe(0);
  expect(pageVariants(true).exit(-1).x).toBe(0);
});

test("reduced motion uses an effectively instant tween for page and shared layouts", () => {
  const transition = pageTransition(true);
  expect(transition.duration).toBe(0.01);
  expect(transition.layout).toMatchObject({ duration: 0.01, type: "tween" });
  expect(pageTransition(false).layout).toMatchObject({ duration: 0.24, type: "tween" });
});
