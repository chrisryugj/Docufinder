import type { CSSProperties } from 'react';

/** CSS custom property를 포함하는 스타일 타입 */
export type CSSPropertiesWithVars = CSSProperties & Record<`--${string}`, string>;
