// Cloudflare Pages Functions middleware — gates the entire site behind a single
// shared passphrase.
//
// This runs server-side at Cloudflare's edge before ANY static asset (the
// index, the wasm, the JS loader, the sprites) is served, so unauthenticated
// requests never receive the game files at all. That is the key difference from
// a browser-side gate: a scraper hitting the wasm URL directly gets the login
// page, not the bundle.
//
// Configuration (set as environment variables on the Cloudflare Pages project,
// NOT in this repo):
//   * SITE_PASSPHRASE — the shared passphrase users must enter (required).
//   * COOKIE_SECRET   — random string used to sign the auth cookie so it can't
//                       be forged (recommended; falls back to the passphrase).

const COOKIE_NAME = "site_auth";
const COOKIE_MAX_AGE = 60 * 60 * 24 * 7; // 7 days
const LOGIN_PATH = "/__login";

/**
 * Deterministic, unforgeable cookie value derived from the configured secrets.
 * A client can only present this value after submitting the correct passphrase,
 * and cannot compute it without knowing COOKIE_SECRET (or the passphrase).
 */
async function expectedToken(env) {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    enc.encode(env.COOKIE_SECRET || env.SITE_PASSPHRASE),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", key, enc.encode("authorized-v1"));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Constant-time-ish string comparison to avoid leaking length/contents via timing. */
function safeEqual(a, b) {
  if (typeof a !== "string" || typeof b !== "string" || a.length !== b.length) {
    return false;
  }
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

function loginResponse(error, status = 401) {
  const message = error
    ? `<p class="error">${error}</p>`
    : "";
  const html = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="robots" content="noindex, nofollow" />
    <title>Bevy 2D RPG — locked</title>
    <style>
      :root { color-scheme: dark; }
      body {
        margin: 0; min-height: 100vh; display: flex; align-items: center;
        justify-content: center; background: #111; color: #e6e6e6;
        font-family: system-ui, sans-serif;
      }
      form {
        display: flex; flex-direction: column; gap: 0.75rem; width: min(20rem, 90vw);
        padding: 2rem; background: #1c1c1c; border: 1px solid #2c2c2c; border-radius: 0.75rem;
      }
      h1 { font-size: 1.15rem; margin: 0 0 0.25rem; }
      input {
        padding: 0.6rem 0.7rem; font-size: 1rem; border-radius: 0.5rem;
        border: 1px solid #3a3a3a; background: #0e0e0e; color: #fff;
      }
      button {
        padding: 0.6rem; font-size: 1rem; border: 0; border-radius: 0.5rem;
        background: #d8b62a; color: #111; font-weight: 600; cursor: pointer;
      }
      .error { color: #ff8080; margin: 0; font-size: 0.9rem; }
    </style>
  </head>
  <body>
    <form method="POST" action="${LOGIN_PATH}">
      <h1>Bevy 2D RPG</h1>
      <label for="passphrase">Enter the passphrase to play.</label>
      <input id="passphrase" name="passphrase" type="password" autofocus autocomplete="current-password" />
      ${message}
      <button type="submit">Unlock</button>
    </form>
  </body>
</html>`;
  return new Response(html, {
    status,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Robots-Tag": "noindex, nofollow",
    },
  });
}

export async function onRequest(context) {
  const { request, env, next } = context;

  // Fail closed if the gate is misconfigured — never serve the game ungated.
  if (!env.SITE_PASSPHRASE) {
    return new Response(
      "Site passphrase is not configured. Set SITE_PASSPHRASE on the Cloudflare Pages project.",
      { status: 503, headers: { "Cache-Control": "no-store" } },
    );
  }

  const token = await expectedToken(env);
  const url = new URL(request.url);

  // Handle passphrase submissions.
  if (request.method === "POST" && url.pathname === LOGIN_PATH) {
    const form = await request.formData();
    const supplied = form.get("passphrase") || "";
    if (safeEqual(String(supplied), env.SITE_PASSPHRASE)) {
      return new Response(null, {
        status: 303,
        headers: {
          Location: "/",
          "Set-Cookie": `${COOKIE_NAME}=${token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=${COOKIE_MAX_AGE}`,
        },
      });
    }
    return loginResponse("Incorrect passphrase.");
  }

  // Already authenticated? Serve the requested asset.
  const cookies = request.headers.get("Cookie") || "";
  const match = cookies.match(new RegExp(`(?:^|;\\s*)${COOKIE_NAME}=([^;]+)`));
  if (match && safeEqual(match[1], token)) {
    return next();
  }

  // Otherwise show the lock screen (and never index it).
  return loginResponse(null);
}
