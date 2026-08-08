#!/usr/bin/env bun

import { createSemathLspServer } from "./src/index.ts";

const server = await createSemathLspServer((message) => {
  const body = JSON.stringify(message);
  process.stdout.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
});

let buffered = Buffer.alloc(0);
process.stdin.on("data", async (chunk) => {
  buffered = Buffer.concat([buffered, chunk]);
  while (true) {
    const headerEnd = buffered.indexOf("\r\n\r\n");
    if (headerEnd < 0) return;
    const header = buffered.subarray(0, headerEnd).toString("ascii");
    const match = /(?:^|\r\n)Content-Length:\s*(\d+)/i.exec(header);
    if (!match) throw new Error("missing Content-Length header");
    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    if (buffered.length < bodyStart + length) return;
    const body = buffered.subarray(bodyStart, bodyStart + length);
    buffered = buffered.subarray(bodyStart + length);
    await server.handle(JSON.parse(body.toString("utf8")));
  }
});

process.once("exit", () => server.dispose());
