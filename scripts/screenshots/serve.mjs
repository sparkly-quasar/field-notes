// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Serve the built frontend (build/, SPA fallback) and forward /api/* to the dev
// portal, which serves no frontend of its own under test. See README.md here.
// usage: node serve.mjs <buildDir> <portalPort> <listenPort>
import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const [dir, portalPort, listenPort] = process.argv.slice(2);
const buildDir = path.resolve(dir);
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".wasm": "application/wasm", ".webmanifest": "application/manifest+json" };

http
  .createServer((req, res) => {
    if (req.url.startsWith("/api/")) {
      const p = http.request({ host: "127.0.0.1", port: portalPort, path: req.url, method: req.method, headers: req.headers }, (r) => {
        res.writeHead(r.statusCode, r.headers);
        r.pipe(res);
      });
      p.on("error", () => { res.writeHead(502); res.end(); });
      req.pipe(p);
      return;
    }
    const url = decodeURIComponent(req.url.split("?")[0]);
    let f = path.join(buildDir, url);
    if (!f.startsWith(path.resolve(buildDir)) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) f = path.join(buildDir, "index.html");
    res.writeHead(200, { "Content-Type": types[path.extname(f)] ?? "application/octet-stream" });
    fs.createReadStream(f).pipe(res);
  })
  .listen(Number(listenPort), "127.0.0.1");
