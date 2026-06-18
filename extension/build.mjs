// Build per-browser extension packages from the shared source in this folder.
//
//   node extension/build.mjs   →   extension/dist/chrome/   (and firefox/)
//
// The shared scripts/assets are identical across browsers; only the manifest
// differs (Chrome uses an MV3 service worker; Firefox uses an event-page
// background and needs a gecko id). Load the right folder per browser:
//   • Chrome/Edge:  chrome://extensions → Load unpacked → dist/chrome
//   • Firefox:      about:debugging → Load Temporary Add-on → dist/firefox
//                   (or: npx web-ext run -s extension/dist/firefox)

import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, "dist");

// Files shared verbatim by every browser package.
const SHARED = [
  "background.js",
  "popup.html",
  "popup.js",
  "popup.css",
  "icons",
];

const base = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));

function build(name, transform) {
  const out = join(dist, name);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  for (const file of SHARED) {
    cpSync(join(root, file), join(out, file), { recursive: true });
  }
  const manifest = transform(structuredClone(base));
  writeFileSync(
    join(out, "manifest.json"),
    JSON.stringify(manifest, null, 2) + "\n",
  );
  console.log(`built dist/${name} (v${manifest.version})`);
}

// Chrome / Edge / Brave: the source manifest, unchanged (MV3 service worker).
build("chrome", (m) => m);

// Firefox: event-page background (broad MV3 support) + required gecko id.
build("firefox", (m) => {
  m.background = { scripts: ["background.js"] };
  m.browser_specific_settings = {
    gecko: {
      id: "websave@venugopal.dev",
      // WebSave sends captures only to the user's own localhost app — it
      // collects no data, which Firefox/AMO now requires extensions to
      // declare explicitly. (Min version is left to Firefox to infer from
      // the manifest features rather than pinned.)
      data_collection_permissions: { required: ["none"] },
    },
  };
  return m;
});
