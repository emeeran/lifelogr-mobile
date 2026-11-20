import { onBeforeUnmount, onMounted, ref } from "vue";

/**
 * Debounced write-through autosave. Flushes immediately when the app is hidden or the
 * editor closes, so a thought typed right before the app is killed is not lost.
 */
export function useAutosave(save: () => Promise<void>, delay = 600) {
  const status = ref<"idle" | "pending" | "saving" | "saved" | "error">("idle");
  const error = ref("");
  let t: number | undefined;
  let running: Promise<void> | null = null;
  let again = false;

  async function flush(): Promise<void> {
    clearTimeout(t);
    if (status.value !== "pending" && !again) return running ?? undefined;
    if (running) {
      again = true;
      return running;
    }
    status.value = "saving";
    again = false;
    running = save()
      .then(() => {
        status.value = "saved";
      })
      .catch((e) => {
        status.value = "error";
        error.value = String(e);
      })
      .finally(() => {
        running = null;
      });
    await running;
    if (again) {
      status.value = "pending";
      await flush();
    }
  }

  function touch() {
    if (status.value === "saving") again = true;
    else status.value = "pending";
    clearTimeout(t);
    t = window.setTimeout(flush, delay);
  }

  const onHide = () => document.visibilityState === "hidden" && flush();
  onMounted(() => document.addEventListener("visibilitychange", onHide));
  onBeforeUnmount(() => {
    document.removeEventListener("visibilitychange", onHide);
    flush();
  });

  return { status, error, touch, flush };
}
