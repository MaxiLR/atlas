import { createCn } from "cn/config";

/**
 * Tailwind class merging that knows Atlas's theme: the type scale, so a size
 * such as `text-13` and a colour such as `text-body` on one element do not
 * cancel each other, and the one soft shadow, `shadow-float`.
 */
export const cn = createCn({
  extend: {
    theme: {
      text: ["12", "13", "15", "16", "20", "28"],
      shadow: ["float"],
    },
  },
});
