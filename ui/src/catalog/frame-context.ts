import { createContext, useContext } from "react";

/**
 * Set by the section view around a catalog component it shows inside a
 * figure frame: the frame carries the number and the visible caption, so the
 * component renders its content only.
 */
export const CaptionInFrame = createContext(false);

/** Whether a frame around this component already shows its caption. */
export function useCaptionInFrame(): boolean {
  return useContext(CaptionInFrame);
}

/**
 * Set by the section view around a framed catalog component: the block's
 * own text description, which stands in for a visual that cannot be drawn.
 */
export const FrameDescription = createContext<string | null>(null);

/** The text description of the block this component draws, if it has one. */
export function useFrameDescription(): string | null {
  return useContext(FrameDescription);
}
