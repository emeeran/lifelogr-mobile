// Reminder scheduling. The DB is the source of truth; notifications are re-derived from it
// on every app start and after every change (the PRD's sync_reminders() contract).
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
  Schedule,
  cancel,
  pending,
} from "@tauri-apps/plugin-notification";
import { api, isAndroid, isTauri, type Reminder, type Repeat } from "./api";

export function advance(iso: string, repeat: Repeat, now = new Date()): string | null {
  if (repeat === "none") return null;
  const d = new Date(iso);
  // Step forward until strictly in the future (catch-up after the app was closed for days).
  let guard = 0;
  while (d <= now && guard++ < 10_000) {
    if (repeat === "daily") d.setDate(d.getDate() + 1);
    else if (repeat === "weekly") d.setDate(d.getDate() + 7);
    else d.setMonth(d.getMonth() + 1);
  }
  return d.toISOString();
}

/** Stable 31-bit notification id from the reminder's uuid. */
function notifId(id: string): number {
  let h = 0;
  for (const c of id) h = (Math.imul(h, 31) + c.charCodeAt(0)) | 0;
  return Math.abs(h) || 1;
}

async function ensurePermission(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  } catch {
    return false;
  }
}

/**
 * Fire anything that is due, then advance or disable it.
 * On Android the OS already delivered scheduled notifications (even with the app killed),
 * so here we only advance the schedule; on desktop we deliver them ourselves.
 */
export async function tick(): Promise<void> {
  const list = await api.listReminders();
  const now = new Date();
  const desktop = !isAndroid();
  let granted: boolean | null = null;
  for (const r of list) {
    if (!r.enabled || new Date(r.next_at) > now) continue;
    if (desktop) {
      granted ??= await ensurePermission();
      if (granted) sendNotification({ title: r.title, body: r.body || "LifeLogr reminder" });
    }
    const next = advance(r.next_at, r.repeat, now);
    await api.saveReminder({
      ...r,
      last_fired_at: now.toISOString(),
      next_at: next ?? r.next_at,
      enabled: next !== null,
    });
  }
  if (!desktop) await scheduleOs();
}

/** Android/iOS: hand future reminders to the OS so they fire with the app closed. */
export async function scheduleOs(): Promise<void> {
  if (!isAndroid() || !(await ensurePermission())) return;
  try {
    const p = await pending();
    if (p.length) await cancel(p.map((n) => n.id));
    for (const r of await api.listReminders()) {
      if (!r.enabled || !r.id) continue;
      sendNotification({
        id: notifId(r.id),
        title: r.title,
        body: r.body || "LifeLogr reminder",
        schedule: Schedule.at(new Date(r.next_at), false, true),
      });
    }
  } catch (e) {
    console.warn("Could not schedule notifications", e);
  }
}

let timer: number | undefined;
export function startReminderLoop(): void {
  const run = () => tick().catch((e) => console.warn("reminder tick failed", e));
  run(); // catch-up on launch
  clearInterval(timer);
  timer = window.setInterval(run, 30_000);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") run();
  });
}

export async function afterReminderChange(): Promise<void> {
  await scheduleOs();
}

export type { Reminder };
