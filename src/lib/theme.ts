import { ref, watchEffect } from "vue";

export type Theme = "system" | "light" | "dark";
export type FontSize = "small" | "medium" | "large";

const read = <T extends string>(k: string, d: T): T => {
  try {
    return (localStorage.getItem(k) as T) || d;
  } catch {
    return d;
  }
};

export const theme = ref<Theme>(read("lifelogr.theme", "system"));
export const fontSize = ref<FontSize>(read("lifelogr.fontSize", "medium"));

watchEffect(() => {
  const root = document.documentElement;
  if (theme.value === "system") delete root.dataset.theme;
  else root.dataset.theme = theme.value;
  root.dataset.font = fontSize.value;
  try {
    localStorage.setItem("lifelogr.theme", theme.value);
    localStorage.setItem("lifelogr.fontSize", fontSize.value);
  } catch {
    /* storage unavailable: settings just won't persist */
  }
});
