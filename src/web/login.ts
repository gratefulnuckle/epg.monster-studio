export async function gateWebLogin(app: HTMLElement): Promise<void> {
  for (;;) {
    const s = await session();
    if (s?.ok && !s.mustChange) return;
    await new Promise<void>((resolve) => {
      paint(app, s, () => resolve());
    });
  }
}

type Sess = { ok: boolean; username?: string; mustChange?: boolean; error?: string };

async function session(): Promise<Sess | null> {
  try {
    const r = await fetch("/api/session", { credentials: "same-origin" });
    const j = (await r.json()) as Sess;
    if (!r.ok) return { ok: false };
    return j;
  } catch {
    return { ok: false, error: "Cannot reach studio-server." };
  }
}

function paint(app: HTMLElement, sess: Sess | null, done: () => void): void {
  const must = !!sess?.mustChange;
  app.innerHTML = `
    <div class="studio-login">
      <img src="/logo.png" alt="" />
      <h1>epg.monster studio</h1>
      <p class="page-sub">${must ? "Set a new admin password to continue." : "Sign in to the web studio."}</p>
      <form id="login-form" class="studio-login-form">
        ${
          must
            ? `
          <div class="field"><label>Current (temporary) password</label><input id="login-current" type="password" autocomplete="current-password" /></div>
          <div class="field"><label>New password</label><input id="login-next" type="password" autocomplete="new-password" /></div>
          <div class="field"><label>Confirm new password</label><input id="login-next2" type="password" autocomplete="new-password" /></div>
        `
            : `
          <div class="field"><label>Username</label><input id="login-user" value="admin" autocomplete="username" /></div>
          <div class="field"><label>Password</label><input id="login-pass" type="password" autocomplete="current-password" /></div>
        `
        }
        <p class="page-sub" id="login-err" hidden></p>
        <button class="accent" type="submit">${must ? "Save password" : "Sign in"}</button>
      </form>
      <p class="page-sub">Create a temporary password with <code>studio.ps1 --makepass</code> or <code>studio.sh --makepass</code>.</p>
    </div>
  `;
  const err = app.querySelector<HTMLElement>("#login-err")!;
  const form = app.querySelector<HTMLFormElement>("#login-form")!;
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    void (async () => {
      err.hidden = true;
      try {
        if (must) {
          const current = (app.querySelector("#login-current") as HTMLInputElement).value;
          const next = (app.querySelector("#login-next") as HTMLInputElement).value;
          const next2 = (app.querySelector("#login-next2") as HTMLInputElement).value;
          if (next !== next2) throw new Error("New passwords do not match.");
          const r = await fetch("/api/password", {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ current, next }),
          });
          const j = (await r.json()) as { error?: string };
          if (!r.ok) throw new Error(j.error || "Could not change password.");
        } else {
          const username = (app.querySelector("#login-user") as HTMLInputElement).value;
          const password = (app.querySelector("#login-pass") as HTMLInputElement).value;
          const r = await fetch("/api/login", {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ username, password }),
          });
          const j = (await r.json()) as { error?: string; mustChange?: boolean };
          if (!r.ok) throw new Error(j.error || "Sign-in failed.");
        }
        done();
      } catch (e) {
        err.hidden = false;
        err.textContent = String(e instanceof Error ? e.message : e);
      }
    })();
  });
}
